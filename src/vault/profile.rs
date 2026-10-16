use std::{
    collections::HashMap,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    vault::config::Config,
    vault::edition::Edition,
    vault::host_settings::HostSettings,
    vault::saved::{SavedPath, SavedWad},
    vault::scribe,
    vault::wad::{Fault, Wad},
};

/// The profiles folder inside the config folder. The dot keeps it from ever matching a profile's
/// name (names can't contain one), which matters where the config and data folders are the same.
pub const SUBDIRECTORY: &str = ".profiles";

/// A named, ordered list of wads (load order matters) plus optional per-profile overrides.
///
/// Stored as `<config>/.profiles/<name>.rite` (MessagePack); saves live in `<data>/<name>/saves`.
/// The name is also the file name, so it is restricted to `A-Z a-z 0-9 _`.
///
/// In memory every wad has its whole path; on disk the folders are listed once and each wad names
/// its folder and file (see [`SavedProfile`]), so the file reads on any OS and a profile can later be
/// shared by dropping just the folders.
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
    /// Every folder the wads are in, each listed once. Missing in files from before edition 2,
    /// whose wads all carry their whole path.
    #[serde(default)]
    folders: Vec<SavedPath>,
}

/// One wad in a saved profile.
///
/// Untagged, so files from before edition 2 still read: their wads have no `folder`, so they land
/// in [`SavedEntry::Loose`].
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum SavedEntry {
    /// `name` in the profile's folder number `folder`: the usual case.
    Filed {
        folder: u32,
        name: String,
        hash: String,
        size: u64,
        volatile: bool,
        modified: u64,
    },
    /// A wad saved with its whole path, for one whose file name isn't valid UTF-8 (or that has no
    /// folder to speak of), and for every wad saved before edition 2.
    Loose(SavedWad),
}

impl From<Profile> for SavedProfile {
    fn from(profile: Profile) -> Self {
        let mut folders = Vec::new();
        let mut numbers: HashMap<PathBuf, u32> = HashMap::new();
        let mut entry = |wad: Wad| {
            let path = Path::new(&wad.path);
            match (
                path.parent(),
                path.file_name().and_then(|name| name.to_str()),
            ) {
                (Some(folder), Some(name)) if !folder.as_os_str().is_empty() => {
                    let folder = *numbers.entry(folder.to_path_buf()).or_insert_with(|| {
                        folders.push(SavedPath::from(folder.as_os_str()));
                        folders.len() as u32 - 1
                    });
                    SavedEntry::Filed {
                        folder,
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
            folders,
        }
    }
}

impl TryFrom<SavedProfile> for Profile {
    type Error = &'static str;

    /// Fails if a wad names a folder the profile doesn't list, which only a damaged file does.
    fn try_from(saved: SavedProfile) -> Result<Self, Self::Error> {
        let folders: Vec<PathBuf> = saved
            .folders
            .into_iter()
            .map(|folder| PathBuf::from(OsString::from(folder)))
            .collect();
        let wad = |entry: SavedEntry| -> Result<Wad, Self::Error> {
            match entry {
                SavedEntry::Filed {
                    folder,
                    name,
                    hash,
                    size,
                    volatile,
                    modified,
                } => {
                    let folder = folders
                        .get(folder as usize)
                        .ok_or("a wad names a folder the profile doesn't list")?;
                    Ok(Wad {
                        path: folder.join(name).into_os_string(),
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
        })
    }
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
    const UPGRADES: &[fn(Self) -> Self] = &[v0_to_v1, v1_to_v2];
}

/// Edition 1 only wrapped the same data in a named, versioned map, so nothing needs fixing.
fn v0_to_v1(profile: Profile) -> Profile {
    profile
}

/// Edition 2 saves wads as a folder table and text names, and paths as text, so the file reads on
/// any OS. Reading either shape into a [`Profile`] already did the work (see [`SavedEntry`]); the
/// new edition number is what keeps older builds from reading a file they can't understand.
fn v1_to_v2(profile: Profile) -> Profile {
    profile
}

impl Profile {
    /// A profile with no overrides, as created when downloading from a host.
    pub fn new(name: String, wads: Vec<Wad>) -> Self {
        Self {
            name,
            wads,
            base_wad: None,
            uzdoom: None,
            ini: None,
            host_defaults: None,
        }
    }

    /// The profile's own UZDoom path, falling back to the global one.
    pub fn uzdoom<'a>(&'a self, config: &'a Config) -> Option<&'a OsString> {
        self.uzdoom.as_ref().or(config.uzdoom.as_ref())
    }

    /// Everything wrong with the files this profile needs to run, UZDoom first. The UZDoom path,
    /// base wad and ini are the effective ones, so the global settings count too. This only looks
    /// at the disk; nothing is hashed.
    pub fn faults(&self, config: &Config) -> Vec<(Part, Fault)> {
        let uzdoom = self
            .uzdoom(config)
            .and_then(|path| Fault::of(Path::new(path), 0))
            .map(|fault| (Part::Uzdoom, fault));
        let base_wad = self
            .base_wad(config)
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

    /// The profile's own base wad, falling back to the global one.
    pub fn base_wad<'a>(&'a self, config: &'a Config) -> Option<&'a Wad> {
        self.base_wad.as_ref().or(config.base_wad.as_ref())
    }

    /// Where the profile called `name` is kept.
    pub fn path(config_dir: &Path, name: &str) -> PathBuf {
        config_dir
            .join(SUBDIRECTORY)
            .join(format!("{name}.{}", scribe::EXTENSION))
    }

    /// Writes the profile file and makes sure its `saves` folder exists.
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

    /// After the data folder moved from `old_name` to this profile's name, an ini that lived inside
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
    use crate::vault::scribe::tests::Altar;

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
        assert_eq!(profile.faults(&Config::default()), []);
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
        let parts: Vec<_> = profile.faults(&Config::default());
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
        let config = Config {
            uzdoom: Some(altar.file("nowhere").into()),
            ..Config::default()
        };
        assert_eq!(
            Profile::default().faults(&config),
            [(Part::Uzdoom, Fault::Missing)]
        );
        // no UZDoom chosen at all is not a fault; that has its own warning
        assert_eq!(Profile::default().faults(&Config::default()), []);
    }

    #[test]
    fn an_ini_inside_the_data_folder_follows_a_rename() {
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

    // 📜 the saved shape: folders listed once, names as text, and every older shape still read

    /// A profile with wads in two folders, one folder used twice, and a base wad in a third.
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
    fn each_folder_is_saved_once_and_the_wads_point_into_it() {
        let saved = SavedProfile::from(spread_out());
        let folders: Vec<_> = saved
            .folders
            .iter()
            .map(|folder| OsString::from(folder.clone()))
            .collect();
        assert_eq!(folders, ["/wads", "/mods", "/iwads"]);
        let numbers: Vec<_> = saved
            .wads
            .iter()
            .map(|entry| match entry {
                SavedEntry::Filed { folder, name, .. } => (*folder, name.as_str()),
                SavedEntry::Loose(_) => panic!("every wad here has a folder and a text name"),
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
        assert_eq!(Profile::stored_version(&path), Some(2));
        assert_eq!(Profile::open(&path), Some(expected));
    }

    #[test]
    fn a_wad_pointing_at_a_missing_folder_makes_the_file_unreadable() {
        let mut saved = SavedProfile::from(spread_out());
        saved.folders.truncate(1);
        assert!(Profile::try_from(saved).is_err());
    }
}
