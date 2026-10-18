use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    loadout::edition::Edition,
    loadout::host_settings::HostSettings,
    loadout::saved::{SavedPath, SavedWad},
    loadout::scribe,
    loadout::shelf,
    loadout::uac::Uac,
    loadout::wad::{Fault, Wad},
};

/// The profiles dir inside the config dir. The dot keeps it from ever matching a profile's
/// name (names can't contain one), which matters where the config and data dirs are the same.
pub const SUBDIRECTORY: &str = ".profiles";

/// A named, ordered list of wads (load order matters) plus optional per-profile overrides.
///
/// Stored as `<config>/.profiles/<name>.rite` (MessagePack); saves live in `<data>/<name>/saves`.
/// The name is also the file name, so it is restricted to `A-Z a-z 0-9 _`.
///
/// In memory every wad has its whole path; on disk the dirs are listed once and each wad names
/// its dir and file (see [`SavedProfile`]), so the file reads on any OS and a profile can later be
/// shared by dropping just the dirs.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(try_from = "SavedProfile", into = "SavedProfile")]
pub struct Profile {
    pub name: String,
    pub wads: Vec<Wad>,
    /// Overrides [`Config::base_wad`].
    pub base_wad: Option<Wad>,
    /// Overrides [`Config::uzdoom`].
    pub uzdoom: Option<OsString>,
    pub ini: Option<OsString>,
    /// What the host form starts with for this profile, saved from that form. Missing in older
    /// files, which count as none.
    pub host_defaults: Option<HostSettings>,
    /// Overrides [`Config::uzdoom_args`]: extra arguments for UZDoom, for this profile only.
    pub uzdoom_args: Option<String>,
    /// A dir the profile watches: wads put in it (at any depth) are added on their own, and wads
    /// whose files leave it are taken out (see [`Profile::look`]).
    pub watched: Option<OsString>,
    /// Wads in the watched dir that were taken out of the profile by hand, so they aren't added
    /// back. Forgotten once their files are gone.
    pub dismissed: Vec<OsString>,
}

/// What changed in a watched dir since the profile last looked.
#[derive(Debug, Default, PartialEq)]
pub struct Watching {
    /// The positions of the profile's wads whose files have left the dir, in order.
    pub gone: Vec<usize>,
    /// Wad files in the dir that the profile doesn't have yet, in path order.
    pub new: Vec<PathBuf>,
}

/// A profile as it is saved. The field order is the layout of edition 0, where this was a bare
/// array, so new fields only ever go at the end.
#[derive(Deserialize, Serialize)]
struct SavedProfile {
    name: String,
    wads: Vec<SavedEntry>,
    base_wad: Option<SavedEntry>,
    uzdoom: Option<SavedPath>,
    ini: Option<SavedPath>,
    /// Missing in files from before host defaults, which count as none.
    #[serde(default)]
    host_defaults: Option<HostSettings>,
    /// Every dir the wads are in, each listed once. Missing in files from before edition 2,
    /// whose wads all carry their whole path. Called `folders` before edition 4.
    #[serde(default, alias = "folders")]
    dirs: Vec<SavedPath>,
    /// Missing in files from before profiles had arguments of their own, which count as none.
    #[serde(default)]
    uzdoom_args: Option<String>,
    /// Missing in files from before profiles could watch a dir, which count as watching none.
    #[serde(default)]
    watched: Option<SavedPath>,
    #[serde(default)]
    dismissed: Vec<SavedPath>,
}

/// One wad in a saved profile.
///
/// Untagged, so files from before edition 2 still read: their wads have no `dir`, so they land
/// in [`SavedEntry::Loose`].
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum SavedEntry {
    /// `name` in the profile's dir number `dir` (called `folder` before edition 4): the usual case.
    Filed {
        #[serde(alias = "folder")]
        dir: u32,
        name: String,
        hash: String,
        size: u64,
        volatile: bool,
        modified: u64,
    },
    /// A wad saved with its whole path, for one whose file name isn't valid UTF-8 (or that has no
    /// dir to speak of), and for every wad saved before edition 2.
    Loose(SavedWad),
}

impl From<Profile> for SavedProfile {
    fn from(profile: Profile) -> Self {
        let mut dirs = Vec::new();
        let mut numbers: HashMap<PathBuf, u32> = HashMap::new();
        let mut entry = |wad: Wad| {
            let path = Path::new(&wad.path);
            match (
                path.parent(),
                path.file_name().and_then(|name| name.to_str()),
            ) {
                (Some(dir), Some(name)) if !dir.as_os_str().is_empty() => {
                    let dir = *numbers.entry(dir.to_path_buf()).or_insert_with(|| {
                        dirs.push(SavedPath::from(dir.as_os_str()));
                        dirs.len() as u32 - 1
                    });
                    SavedEntry::Filed {
                        dir,
                        name: name.to_string(),
                        hash: wad.hash,
                        size: wad.size,
                        volatile: wad.volatile,
                        modified: wad.modified,
                    }
                }
                _ => SavedEntry::Loose(wad.into()),
            }
        };
        let wads = profile.wads.into_iter().map(&mut entry).collect();
        let base_wad = profile.base_wad.map(&mut entry);
        Self {
            name: profile.name,
            wads,
            base_wad,
            uzdoom: profile.uzdoom.as_deref().map(SavedPath::from),
            ini: profile.ini.as_deref().map(SavedPath::from),
            host_defaults: profile.host_defaults,
            dirs,
            uzdoom_args: profile.uzdoom_args,
            watched: profile.watched.as_deref().map(SavedPath::from),
            dismissed: profile
                .dismissed
                .iter()
                .map(|path| SavedPath::from(path.as_os_str()))
                .collect(),
        }
    }
}

impl TryFrom<SavedProfile> for Profile {
    type Error = &'static str;

    /// Fails if a wad names a dir the profile doesn't list, which only a damaged file does.
    fn try_from(saved: SavedProfile) -> Result<Self, Self::Error> {
        let dirs: Vec<PathBuf> = saved
            .dirs
            .into_iter()
            .map(|dir| PathBuf::from(OsString::from(dir)))
            .collect();
        let wad = |entry: SavedEntry| -> Result<Wad, Self::Error> {
            match entry {
                SavedEntry::Filed {
                    dir,
                    name,
                    hash,
                    size,
                    volatile,
                    modified,
                } => {
                    let dir = dirs
                        .get(dir as usize)
                        .ok_or("a wad names a dir the profile doesn't list")?;
                    Ok(Wad {
                        path: dir.join(name).into_os_string(),
                        hash,
                        size,
                        volatile,
                        modified,
                    })
                }
                SavedEntry::Loose(saved) => Ok(Wad::from(saved)),
            }
        };
        Ok(Self {
            name: saved.name,
            wads: saved.wads.into_iter().map(wad).collect::<Result<_, _>>()?,
            base_wad: saved.base_wad.map(wad).transpose()?,
            uzdoom: saved.uzdoom.map(OsString::from),
            ini: saved.ini.map(OsString::from),
            host_defaults: saved.host_defaults,
            uzdoom_args: saved.uzdoom_args,
            watched: saved.watched.map(OsString::from),
            dismissed: saved.dismissed.into_iter().map(OsString::from).collect(),
        })
    }
}

/// `name` with a number on the end that no profile in `profiles` has yet: `brutal_2`, then
/// `brutal_3` (see [`crate::tag::tagged`]).
pub fn tagged_name(name: &str, profiles: &[Profile]) -> String {
    crate::tag::tagged(name, |tagged| profiles.iter().all(|p| p.name != tagged))
}

/// Which piece of a profile a [`Fault`] was found in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Part {
    /// The wad at this position in the list.
    Wad(usize),
    BaseWad,
    Uzdoom,
    Ini,
}

impl Edition for Profile {
    const UPGRADES: &[fn(Self) -> Self] = &[v0_to_v1, v1_to_v2, v2_to_v3, v3_to_v4];
}

/// Edition 1 only wrapped the same data in a named, versioned map, so nothing needs fixing.
fn v0_to_v1(profile: Profile) -> Profile {
    profile
}

/// Edition 2 saves wads as a dir table and text names, and paths as text, so the file reads on
/// any OS. Reading either shape into a [`Profile`] already did the work (see [`SavedEntry`]); the
/// new edition number is what keeps older builds from reading a file they can't understand.
fn v1_to_v2(profile: Profile) -> Profile {
    profile
}

/// Edition 3 fills in the size of every wad added before sizes were recorded (saved as 0), from
/// the file as it is now, so a change from here on is marked as one. Wads whose file is gone keep
/// 0. Nothing is hashed, so this stays quick.
fn v2_to_v3(profile: Profile) -> Profile {
    Profile {
        wads: profile
            .wads
            .into_iter()
            .map(Wad::with_size_backfilled)
            .collect(),
        base_wad: profile.base_wad.map(Wad::with_size_backfilled),
        ..profile
    }
}

/// Edition 4 calls the dir table `dirs` and each wad's `folder` its `dir`. The old names still read
/// (see [`SavedProfile`]); the new edition number keeps older builds from reading the file.
fn v3_to_v4(profile: Profile) -> Profile {
    profile
}

impl Profile {
    /// Points every wad of the profile (and its base wad) whose file was at `from` at `to`, where
    /// the wads screen moved it. Returns whether any was.
    pub fn follow(&mut self, from: &Path, to: &Path) -> bool {
        let mut followed = false;
        for wad in self.wads.iter_mut().chain(&mut self.base_wad) {
            followed |= wad.follow(from, to);
        }
        for dismissed in &mut self.dismissed {
            if Path::new(dismissed) == from {
                *dismissed = to.as_os_str().to_os_string();
                followed = true;
            }
        }
        followed
    }

    /// What changed in the watched dir: wads whose files left it, and wad files new to it that
    /// weren't taken out by hand. `None` if the profile watches no dir, if the dir isn't there
    /// (an unplugged drive changes nothing), or if nothing changed.
    pub fn look(&self) -> Option<Watching> {
        let dir = Path::new(self.watched.as_ref()?);
        if !dir.is_dir() {
            return None;
        }
        let gone = self
            .wads
            .iter()
            .enumerate()
            .filter(|(_, wad)| {
                let path = Path::new(&wad.path);
                path.starts_with(dir) && !path.is_file()
            })
            .map(|(i, _)| i)
            .collect();
        let known: HashSet<&Path> = self
            .wads
            .iter()
            .map(|wad| Path::new(&wad.path))
            .chain(self.dismissed.iter().map(Path::new))
            .collect();
        let new = shelf::scan(dir)
            .into_iter()
            .filter(|path| !known.contains(path.as_path()))
            .collect();
        let watching = Watching { gone, new };
        (watching != Watching::default()).then_some(watching)
    }

    /// Takes in what [`Profile::look`] found, with `new` the wads hashed from its new files: the
    /// gone ones are taken out, the new ones go at the end, and dismissed wads whose files are gone
    /// are forgotten.
    pub fn take_in(&mut self, watching: &Watching, new: Vec<Wad>) {
        for &gone in watching.gone.iter().rev() {
            self.wads.remove(gone);
        }
        self.wads.extend(new);
        self.dismissed.retain(|path| Path::new(path).is_file());
    }

    /// After an edit from `before`, remembers each wad in the watched dir that was taken out, so
    /// looking doesn't add it back, and forgets any that was put back in.
    pub fn note_dismissed(&mut self, before: &Profile) {
        let Some(dir) = self.watched.as_ref().map(PathBuf::from) else {
            return;
        };
        let kept: HashSet<&OsString> = self.wads.iter().map(|wad| &wad.path).collect();
        let taken_out: Vec<OsString> = before
            .wads
            .iter()
            .map(|wad| &wad.path)
            .filter(|path| !kept.contains(path) && Path::new(path).starts_with(&dir))
            .filter(|path| !self.dismissed.contains(path))
            .cloned()
            .collect();
        self.dismissed.retain(|path| !kept.contains(path));
        self.dismissed.extend(taken_out);
    }

    /// A profile with no overrides, as created when downloading from a host.
    pub fn new(name: String, wads: Vec<Wad>) -> Self {
        Self {
            name,
            wads,
            base_wad: None,
            uzdoom: None,
            ini: None,
            host_defaults: None,
            uzdoom_args: None,
            watched: None,
            dismissed: Vec::new(),
        }
    }

    /// The UZDoom path to use: this session's from `--uzdoom` if there is one, else the profile's
    /// own, else the global one.
    pub fn uzdoom<'a>(&'a self, global: &'a Uac) -> Option<&'a OsString> {
        global
            .session_uzdoom
            .as_ref()
            .or(self.uzdoom.as_ref())
            .or(global.uzdoom.as_ref())
    }

    /// Everything wrong with the files this profile needs to run, UZDoom first. The UZDoom path,
    /// base wad and ini are the effective ones, so the global settings count too. This only looks
    /// at the disk; nothing is hashed.
    pub fn faults(&self, global: &Uac) -> Vec<(Part, Fault)> {
        let uzdoom = self
            .uzdoom(global)
            .and_then(|path| Fault::of(Path::new(path), 0))
            .map(|fault| (Part::Uzdoom, fault));
        let base_wad = self
            .base_wad(global)
            .and_then(Wad::check)
            .map(|fault| (Part::BaseWad, fault));
        let ini = self
            .ini
            .as_ref()
            .and_then(|path| Fault::of(Path::new(path), 0))
            .map(|fault| (Part::Ini, fault));
        let wads = self
            .wads
            .iter()
            .enumerate()
            .filter_map(|(i, wad)| wad.check().map(|fault| (Part::Wad(i), fault)));
        uzdoom
            .into_iter()
            .chain(base_wad)
            .chain(ini)
            .chain(wads)
            .collect()
    }

    /// The extra arguments for UZDoom: the profile's own, falling back to the global ones.
    pub fn uzdoom_args<'a>(&'a self, global: &'a Uac) -> &'a str {
        self.uzdoom_args.as_deref().unwrap_or(&global.uzdoom_args)
    }

    /// The base wad to use: this session's from `--base-wad` if there is one, else the profile's
    /// own, else the global one.
    pub fn base_wad<'a>(&'a self, global: &'a Uac) -> Option<&'a Wad> {
        global
            .session_base_wad
            .as_ref()
            .or(self.base_wad.as_ref())
            .or(global.base_wad.as_ref())
    }

    /// Where the profile called `name` is kept.
    pub fn path(config_dir: &Path, name: &str) -> PathBuf {
        config_dir
            .join(SUBDIRECTORY)
            .join(format!("{name}.{}", scribe::EXTENSION))
    }

    /// Writes the profile file and makes sure its `saves` dir exists.
    pub fn write(&self, config_dir: &Path, data_dir: &Path) -> io::Result<()> {
        self.save(&Self::path(config_dir, &self.name))?;
        self.create_saves_dir(data_dir)
    }

    /// UZDoom's save games for this profile live here.
    pub fn create_saves_dir(&self, data_dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(data_dir.join(&self.name).join("saves"))
    }

    /// Does nothing for an empty name, which would otherwise resolve to the whole data directory.
    pub fn remove(&self, config_dir: &Path, data_dir: &Path) {
        if self.name.is_empty() {
            return;
        }
        let _ = std::fs::remove_dir_all(data_dir.join(&self.name));
        scribe::burn(&Self::path(config_dir, &self.name));
    }

    /// A copy of this profile called `name`, for pasting: the same wads and overrides, and its own
    /// ini copied into the new profile's data dir if it had one there, so changing one profile's
    /// settings in UZDoom doesn't change the other's. Saves are not copied.
    pub fn copied_as(&self, name: String, data_dir: &Path) -> io::Result<Self> {
        let mut copy = Self {
            name,
            ..self.clone()
        };
        let own_ini = self
            .ini
            .as_ref()
            .and_then(|ini| Path::new(ini).strip_prefix(data_dir.join(&self.name)).ok())
            .map(Path::to_path_buf);
        if let (Some(ini), Some(rest)) = (&self.ini, own_ini) {
            let copied = data_dir.join(&copy.name).join(rest);
            if let Some(dir) = copied.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::copy(ini, &copied)?;
            copy.ini = Some(copied.into());
        }
        Ok(copy)
    }

    /// After the data dir moved from `old_name` to this profile's name, an ini that lived inside
    /// it moved too, so its path has to follow.
    pub fn follow_rename(&mut self, data_dir: &Path, old_name: &str) {
        let moved = self
            .ini
            .as_ref()
            .and_then(|ini| Path::new(ini).strip_prefix(data_dir.join(old_name)).ok())
            .map(|rest| data_dir.join(&self.name).join(rest));
        if let Some(moved) = moved {
            self.ini = Some(moved.into());
        }
    }

    /// Moves the saves and deletes the old file; the caller then writes the profile under its new name.
    pub fn rename(&self, new_name: &str, config_dir: &Path, data_dir: &Path) {
        if self.name.is_empty() {
            return;
        }
        let _ = std::fs::rename(data_dir.join(&self.name), data_dir.join(new_name));
        scribe::burn(&Self::path(config_dir, &self.name));
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::loadout::scribe::tests::Altar;

    fn wad(altar: &Altar, name: &str) -> Wad {
        fs::write(altar.file(name), b"1234").unwrap();
        Wad::from_path(&altar.file(name)).unwrap()
    }

    #[test]
    fn a_healthy_profile_has_no_faults() {
        let altar = Altar::new("profile-healthy");
        fs::write(altar.file("uzdoom"), b"").unwrap();
        fs::write(altar.file("a.ini"), b"").unwrap();
        let profile = Profile {
            wads: vec![wad(&altar, "a.wad"), wad(&altar, "b.pk3")],
            base_wad: Some(wad(&altar, "base.wad")),
            uzdoom: Some(altar.file("uzdoom").into()),
            ini: Some(altar.file("a.ini").into()),
            ..Profile::default()
        };
        assert_eq!(profile.faults(&Uac::default()), []);
    }

    #[test]
    fn every_part_reports_its_own_fault_with_uzdoom_first() {
        let altar = Altar::new("profile-faults");
        let mut profile = Profile {
            wads: vec![wad(&altar, "a.wad"), wad(&altar, "b.pk3")],
            base_wad: Some(wad(&altar, "base.wad")),
            uzdoom: Some(altar.file("no-uzdoom").into()),
            ini: Some(altar.file("no.ini").into()),
            ..Profile::default()
        };
        fs::remove_file(altar.file("b.pk3")).unwrap();
        fs::remove_file(altar.file("base.wad")).unwrap();
        fs::write(altar.file("a.wad"), b"123456789").unwrap();
        let parts: Vec<_> = profile.faults(&Uac::default());
        assert_eq!(
            parts,
            [
                (Part::Uzdoom, Fault::Missing),
                (Part::BaseWad, Fault::Missing),
                (Part::Ini, Fault::Missing),
                (
                    Part::Wad(0),
                    Fault::Changed {
                        expected: 4,
                        found: 9
                    }
                ),
                (Part::Wad(1), Fault::Missing),
            ]
        );
        // only the changed wad is non-blocking
        assert_eq!(parts.iter().filter(|(_, f)| f.is_blocking()).count(), 4);
        profile.wads.clear();
    }

    #[test]
    fn the_global_settings_count_when_the_profile_has_no_overrides() {
        let altar = Altar::new("profile-global");
        let global = Uac {
            uzdoom: Some(altar.file("nowhere").into()),
            ..Uac::default()
        };
        assert_eq!(
            Profile::default().faults(&global),
            [(Part::Uzdoom, Fault::Missing)]
        );
        // no UZDoom chosen at all is not a fault; that has its own warning
        assert_eq!(Profile::default().faults(&Uac::default()), []);
    }

    #[test]
    fn a_profiles_own_arguments_win_over_the_global_ones() {
        let global = Uac {
            uzdoom_args: "-nomusic".into(),
            ..Uac::default()
        };
        assert_eq!(Profile::default().uzdoom_args(&global), "-nomusic");
        let own = Profile {
            uzdoom_args: Some("+sv_cheats 1".into()),
            ..Profile::default()
        };
        assert_eq!(own.uzdoom_args(&global), "+sv_cheats 1");
        // and they are saved with it
        let altar = Altar::new("profile-own-args");
        let path = altar.file("own.rite");
        own.save(&path).unwrap();
        assert_eq!(Profile::open(&path), Some(own));
    }

    #[test]
    fn the_sessions_uzdoom_and_base_wad_win_over_the_profiles_own_and_are_never_saved() {
        let base = |path: &str| Wad::new(path.into(), "ab".repeat(32), 1);
        let own = Profile {
            uzdoom: Some("/own/uzdoom".into()),
            base_wad: Some(base("/own/DOOM2.WAD")),
            ..Profile::default()
        };
        let mut global = Uac {
            uzdoom: Some("/global/uzdoom".into()),
            base_wad: Some(base("/global/DOOM2.WAD")),
            ..Uac::default()
        };
        assert_eq!(own.uzdoom(&global), Some(&"/own/uzdoom".into()));
        global.session_uzdoom = Some("/test/uzdoom-dev".into());
        global.session_base_wad = Some(base("/test/freedoom2.wad"));
        assert_eq!(own.uzdoom(&global), Some(&"/test/uzdoom-dev".into()));
        assert_eq!(
            own.base_wad(&global).map(|w| w.path.clone()),
            Some("/test/freedoom2.wad".into())
        );
        assert_eq!(global.uzdoom_in_effect(), Some(&"/test/uzdoom-dev".into()));
        // the session's never reach the file
        let altar = Altar::new("profile-session");
        let path = Uac::path(&altar.0);
        global.save(&path).unwrap();
        let saved = Uac::open(&path).unwrap();
        assert_eq!(saved.session_uzdoom, None);
        assert_eq!(saved.session_base_wad, None);
        assert_eq!(saved.uzdoom, Some("/global/uzdoom".into()));
    }

    #[test]
    fn an_ini_inside_the_data_dir_follows_a_rename() {
        let data = Path::new("/data");
        let mut profile = Profile {
            name: "new".into(),
            ini: Some("/data/old/uzdoom.ini".into()),
            ..Profile::default()
        };
        profile.follow_rename(data, "old");
        assert_eq!(profile.ini, Some("/data/new/uzdoom.ini".into()));
    }

    #[test]
    fn an_ini_somewhere_else_stays_where_it_is() {
        let mut profile = Profile {
            name: "new".into(),
            ini: Some("/home/me/uzdoom.ini".into()),
            ..Profile::default()
        };
        profile.follow_rename(Path::new("/data"), "old");
        assert_eq!(profile.ini, Some("/home/me/uzdoom.ini".into()));
        // and no ini stays no ini
        let mut none = Profile::default();
        none.follow_rename(Path::new("/data"), "old");
        assert_eq!(none.ini, None);
    }

    // 📋 copies, for pasting

    fn named(name: &str) -> Profile {
        Profile {
            name: name.into(),
            ..Profile::default()
        }
    }

    #[test]
    fn a_tag_skips_the_names_profiles_already_have() {
        let profiles = [named("brutal"), named("brutal_2"), named("brutal_4")];
        assert_eq!(tagged_name("brutal", &profiles), "brutal_3");
        assert_eq!(tagged_name("brutal_4", &profiles), "brutal_5");
    }

    #[test]
    fn a_copy_gets_its_own_ini_and_none_of_the_saves() {
        let altar = Altar::new("profile-copy");
        let data = altar.file("data");
        let ini = data.join("brutal/uzdoom.ini");
        fs::create_dir_all(data.join("brutal/saves")).unwrap();
        fs::write(&ini, b"[Doom.Player]").unwrap();
        fs::write(data.join("brutal/saves/slot1"), b"save").unwrap();
        let original = Profile {
            name: "brutal".into(),
            ini: Some(ini.clone().into()),
            ..spread_out()
        };
        let copy = original.copied_as("brutal_2".into(), &data).unwrap();
        assert_eq!(copy.name, "brutal_2");
        assert_eq!(copy.wads, original.wads);
        assert_eq!(copy.uzdoom, original.uzdoom);
        let copied_ini = data.join("brutal_2/uzdoom.ini");
        assert_eq!(copy.ini, Some(copied_ini.clone().into()));
        assert_eq!(fs::read(copied_ini).unwrap(), b"[Doom.Player]");
        assert!(!data.join("brutal_2/saves/slot1").exists());
        // the original's ini is left where it was
        assert_eq!(fs::read(ini).unwrap(), b"[Doom.Player]");
    }

    #[test]
    fn a_copy_shares_an_ini_kept_somewhere_else() {
        let altar = Altar::new("profile-copy-shared-ini");
        let original = Profile {
            ini: Some("/home/me/uzdoom.ini".into()),
            ..named("brutal")
        };
        let copy = original
            .copied_as("brutal_2".into(), &altar.file("data"))
            .unwrap();
        assert_eq!(copy.ini, original.ini);
    }

    // 📜 the saved shape: dirs listed once, names as text, and every older shape still read

    /// A profile with wads in two dirs, one dir used twice, and a base wad in a third.
    fn spread_out() -> Profile {
        let wad = |path: &str| Wad {
            modified: 7,
            ..Wad::new(path.into(), "ab".repeat(32), 4)
        };
        Profile {
            base_wad: Some(wad("/iwads/DOOM2.WAD")),
            uzdoom: Some("/usr/bin/uzdoom".into()),
            ..Profile::new(
                "spread".into(),
                vec![wad("/wads/a.wad"), wad("/mods/b.pk3"), wad("/wads/c.wad")],
            )
        }
    }

    #[test]
    fn each_dir_is_saved_once_and_the_wads_point_into_it() {
        let saved = SavedProfile::from(spread_out());
        let dirs: Vec<_> = saved
            .dirs
            .iter()
            .map(|dir| OsString::from(dir.clone()))
            .collect();
        assert_eq!(dirs, ["/wads", "/mods", "/iwads"]);
        let numbers: Vec<_> = saved
            .wads
            .iter()
            .map(|entry| match entry {
                SavedEntry::Filed { dir, name, .. } => (*dir, name.as_str()),
                SavedEntry::Loose(_) => panic!("every wad here has a dir and a text name"),
            })
            .collect();
        assert_eq!(numbers, [(0, "a.wad"), (1, "b.pk3"), (0, "c.wad")]);
    }

    #[test]
    fn a_profile_saves_and_loads_back_the_same() {
        let altar = Altar::new("profile-round-trip");
        let path = altar.file("spread.rite");
        let profile = spread_out();
        profile.save(&path).unwrap();
        assert_eq!(Profile::open(&path), Some(profile));
        // nothing in the file is tagged with the OS that wrote it
        let bytes = fs::read(&path).unwrap();
        assert!(!bytes.windows(4).any(|window| window == b"Unix"));
    }

    #[cfg(unix)]
    #[test]
    fn a_wad_whose_name_is_not_text_is_kept_whole() {
        use std::os::unix::ffi::OsStringExt;

        let odd = OsString::from_vec(b"/wads/\xff.wad".to_vec());
        let profile = Profile::new("odd".into(), vec![Wad::new(odd, "ab".repeat(32), 4)]);
        let saved = SavedProfile::from(profile.clone());
        assert!(matches!(saved.wads[0], SavedEntry::Loose(_)));
        let altar = Altar::new("profile-odd-name");
        let path = altar.file("odd.rite");
        profile.save(&path).unwrap();
        assert_eq!(Profile::open(&path), Some(profile));
    }

    /// How edition 1 saved a profile: every wad with its whole path, as a tagged `OsString`.
    #[derive(Serialize)]
    struct EditionOne {
        name: String,
        wads: Vec<EditionOneWad>,
        base_wad: Option<EditionOneWad>,
        uzdoom: Option<OsString>,
        ini: Option<OsString>,
        host_defaults: Option<HostSettings>,
    }

    #[derive(Serialize)]
    struct EditionOneWad {
        path: OsString,
        hash: String,
        size: u64,
        volatile: bool,
    }

    #[test]
    fn a_profile_saved_by_edition_one_loads_and_upgrades() {
        let altar = Altar::new("profile-edition-one");
        let path = altar.file("old.rite");
        let old_wad = |path: &str, volatile| EditionOneWad {
            path: path.into(),
            hash: "cd".repeat(32),
            size: 9,
            volatile,
        };
        let old = EditionOne {
            name: "old".into(),
            wads: vec![
                old_wad("/wads/a.wad", false),
                old_wad("/wads/wip.pk3", true),
            ],
            base_wad: Some(old_wad("/iwads/DOOM2.WAD", false)),
            uzdoom: Some("/usr/bin/uzdoom".into()),
            ini: None,
            host_defaults: None,
        };
        #[derive(Serialize)]
        struct Sealed<T> {
            version: u32,
            data: T,
        }
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: 1,
            data: old,
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();

        let wad = |path: &str, volatile| Wad {
            volatile,
            ..Wad::new(path.into(), "cd".repeat(32), 9)
        };
        let expected = Profile {
            base_wad: Some(wad("/iwads/DOOM2.WAD", false)),
            uzdoom: Some("/usr/bin/uzdoom".into()),
            ..Profile::new(
                "old".into(),
                vec![wad("/wads/a.wad", false), wad("/wads/wip.pk3", true)],
            )
        };
        assert_eq!(Profile::stored_version(&path), Some(1));
        let loaded = Profile::open(&path).unwrap();
        assert_eq!(loaded, expected);
        // and upgrading rewrites it in the new shape, which reads back the same
        assert!(loaded.upgrade(&path).unwrap());
        assert_eq!(Profile::stored_version(&path), Some(Profile::current()));
        assert_eq!(Profile::open(&path), Some(expected));
    }

    #[test]
    fn a_profile_saved_by_edition_two_gets_the_sizes_it_never_recorded() {
        let altar = Altar::new("profile-edition-two");
        fs::write(altar.file("there.wad"), b"1234567").unwrap();
        fs::write(altar.file("known.pk3"), b"12").unwrap();
        fs::write(altar.file("base.wad"), b"123").unwrap();
        let wad = |name: &str, size| Wad::new(altar.file(name).into(), "ab".repeat(32), size);
        let old = Profile {
            base_wad: Some(wad("base.wad", 0)),
            ..Profile::new(
                "old".into(),
                vec![wad("there.wad", 0), wad("gone.wad", 0), wad("known.pk3", 9)],
            )
        };
        // edition 2 saved today's shape (with the old names for its dirs, which still read), only
        // with an older number on it
        #[derive(Serialize)]
        struct Sealed<'a> {
            version: u32,
            data: &'a Profile,
        }
        let path = altar.file("old.rite");
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: 2,
            data: &old,
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();

        let loaded = Profile::open(&path).unwrap();
        let sizes: Vec<_> = loaded.wads.iter().map(|wad| wad.size).collect();
        // the unknown sizes come from the files; a missing file stays unknown, a known size stays
        assert_eq!(sizes, [7, 0, 9]);
        assert_eq!(loaded.base_wad.as_ref().map(|wad| wad.size), Some(3));
        assert!(loaded.wads.iter().all(|wad| wad.modified == 0));
        // the upgrade is written as the new edition and reads back the same
        assert!(loaded.upgrade(&path).unwrap());
        assert_eq!(Profile::stored_version(&path), Some(Profile::current()));
        assert_eq!(Profile::open(&path), Some(loaded));
    }

    /// How edition 3 saved a profile: today's shape, with the dirs still called folders.
    #[derive(Serialize)]
    struct EditionThree {
        name: String,
        wads: Vec<EditionThreeWad>,
        base_wad: Option<EditionThreeWad>,
        uzdoom: Option<String>,
        ini: Option<String>,
        host_defaults: Option<HostSettings>,
        folders: Vec<String>,
        uzdoom_args: Option<String>,
    }

    #[derive(Serialize)]
    struct EditionThreeWad {
        folder: u32,
        name: String,
        hash: String,
        size: u64,
        volatile: bool,
        modified: u64,
    }

    #[test]
    fn a_profile_saved_by_edition_three_keeps_its_folders_as_dirs() {
        let altar = Altar::new("profile-edition-three");
        let path = altar.file("old.rite");
        let old_wad = |folder, name: &str| EditionThreeWad {
            folder,
            name: name.into(),
            hash: "ab".repeat(32),
            size: 4,
            volatile: false,
            modified: 7,
        };
        let old = EditionThree {
            name: "spread".into(),
            wads: vec![
                old_wad(0, "a.wad"),
                old_wad(1, "b.pk3"),
                old_wad(0, "c.wad"),
            ],
            base_wad: Some(old_wad(2, "DOOM2.WAD")),
            uzdoom: Some("/usr/bin/uzdoom".into()),
            ini: None,
            host_defaults: None,
            folders: vec!["/wads".into(), "/mods".into(), "/iwads".into()],
            uzdoom_args: None,
        };
        #[derive(Serialize)]
        struct Sealed<T> {
            version: u32,
            data: T,
        }
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: 3,
            data: old,
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();

        let loaded = Profile::open(&path).unwrap();
        assert_eq!(loaded, spread_out());
        assert!(loaded.upgrade(&path).unwrap());
        assert_eq!(Profile::stored_version(&path), Some(4));
        assert_eq!(Profile::open(&path), Some(spread_out()));
        // the upgraded file says dirs, and nothing is a folder anymore
        let bytes = fs::read(&path).unwrap();
        assert!(!bytes.windows(6).any(|window| window == b"folder"));
    }

    // 👀 a profile that watches a dir

    #[test]
    fn a_watched_dir_brings_in_new_wads_and_lets_go_of_gone_ones_but_not_others() {
        let altar = Altar::new("profile-watch");
        let dir = altar.file("watched");
        fs::create_dir_all(dir.join("deep")).unwrap();
        for name in ["a.wad", "b.pk3", "deep/c.wad", "notes.txt"] {
            fs::write(dir.join(name), name).unwrap();
        }
        fs::write(altar.file("elsewhere.wad"), b"x").unwrap();
        let wad = |path: PathBuf| Wad::new(path.into(), "ab".repeat(32), 1);
        let mut profile = Profile {
            watched: Some(dir.clone().into_os_string()),
            ..Profile::new(
                "watching".into(),
                vec![
                    wad(dir.join("a.wad")),
                    wad(dir.join("gone.wad")),
                    // gone too, but not from the dir: a placeholder as ever
                    wad(altar.file("missing.wad")),
                ],
            )
        };
        profile.dismissed = vec![dir.join("b.pk3").into(), dir.join("long_gone.wad").into()];

        let watching = profile.look().unwrap();
        assert_eq!(watching.gone, [1]);
        assert_eq!(
            watching.new,
            [dir.join("deep/c.wad")],
            "a.wad is in, b.pk3 dismissed"
        );
        profile.take_in(&watching, vec![wad(dir.join("deep/c.wad"))]);
        let paths: Vec<_> = profile
            .wads
            .iter()
            .map(|wad| PathBuf::from(&wad.path))
            .collect();
        assert_eq!(
            paths,
            [
                dir.join("a.wad"),
                altar.file("missing.wad"),
                dir.join("deep/c.wad")
            ]
        );
        assert_eq!(profile.dismissed, [dir.join("b.pk3").into_os_string()]);
        assert_eq!(profile.look(), None, "nothing more to take in");

        // a dir that is gone (an unplugged drive) changes nothing
        let unplugged = Profile {
            watched: Some(altar.file("unplugged").into_os_string()),
            ..profile.clone()
        };
        assert_eq!(unplugged.look(), None);
        assert_eq!(Profile::default().look(), None);
    }

    #[test]
    fn a_wad_taken_out_of_a_watched_dir_is_dismissed_until_it_is_put_back() {
        let dir = PathBuf::from("/watched");
        let wad = |path: &str| Wad::new(path.into(), "ab".repeat(32), 1);
        let before = Profile {
            watched: Some(dir.clone().into_os_string()),
            ..Profile::new(
                "watching".into(),
                vec![
                    wad("/watched/a.wad"),
                    wad("/watched/b.wad"),
                    wad("/other/c.wad"),
                ],
            )
        };
        let mut after = Profile {
            wads: vec![wad("/watched/b.wad")],
            ..before.clone()
        };
        after.note_dismissed(&before);
        assert_eq!(
            after.dismissed,
            [OsString::from("/watched/a.wad")],
            "only the dir's"
        );
        let mut put_back = Profile {
            wads: vec![wad("/watched/a.wad"), wad("/watched/b.wad")],
            ..after.clone()
        };
        put_back.note_dismissed(&after);
        assert!(put_back.dismissed.is_empty());
        // a profile that watches nothing dismisses nothing
        let mut plain = Profile::new("plain".into(), Vec::new());
        plain.note_dismissed(&before);
        assert!(plain.dismissed.is_empty());
    }

    #[test]
    fn a_watched_dir_and_the_dismissed_wads_are_saved_and_follow_a_move() {
        let altar = Altar::new("profile-watch-saved");
        let path = altar.file("watching.rite");
        let mut profile = Profile {
            watched: Some("/watched".into()),
            dismissed: vec!["/watched/a.wad".into()],
            ..spread_out()
        };
        profile.save(&path).unwrap();
        assert_eq!(Profile::open(&path), Some(profile.clone()));
        assert!(profile.follow(Path::new("/watched/a.wad"), Path::new("/mods/a.wad")));
        assert_eq!(profile.dismissed, [OsString::from("/mods/a.wad")]);
    }

    #[test]
    fn a_wad_pointing_at_a_missing_dir_makes_the_file_unreadable() {
        let mut saved = SavedProfile::from(spread_out());
        saved.dirs.truncate(1);
        assert!(Profile::try_from(saved).is_err());
    }
}
