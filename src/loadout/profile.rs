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

/// A profile as it is saved.
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
    /// Every dir the wads are in, each listed once.
    #[serde(default)]
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

/// One wad in a saved profile. Untagged: a wad with a `dir` is [`SavedEntry::Filed`].
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum SavedEntry {
    /// `name` in the profile's dir number `dir`: the usual case.
    Filed {
        dir: u32,
        name: String,
        hash: String,
        size: u64,
        volatile: bool,
        modified: u64,
        /// Missing in files from before folders could be wads.
        #[serde(default)]
        folder: bool,
    },
    /// A wad saved with its whole path, for one whose file name isn't valid UTF-8 (or that has no
    /// dir to speak of).
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
                        folder: wad.folder,
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
                    folder,
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
                        folder,
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

/// The longest a profile's name may be, in characters.
pub const MAX_NAME: usize = 64;

/// Whether a profile's name may hold `c`: letters, digits, `_`, `-` and spaces. The name is also a
/// file and a dir name, so nothing a file system or a command line would treat differently.
pub fn is_name_character(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ')
}

/// `name` as a profile's name: with the spaces around it taken off, if that leaves one that is
/// not empty, not too long, only of the characters a name may hold and not starting with a `-`
/// (which a command line would take for an option). `None` otherwise.
pub fn clean_name(name: &str) -> Option<String> {
    let name = name.trim_matches(' ');
    (!name.is_empty()
        && name.chars().count() <= MAX_NAME
        && !name.starts_with('-')
        && name.chars().all(is_name_character))
    .then(|| name.to_string())
}

/// Whether `name` is a profile's name as it is (see [`clean_name`]): nothing to trim.
pub fn is_valid_name(name: &str) -> bool {
    clean_name(name).as_deref() == Some(name)
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
    /// Edition 4 is the one tdl 1.0.0 writes; older ones are upgraded by `upgrade_legacy.py`.
    const FIRST: u32 = 4;
    const UPGRADES: &[fn(Self) -> Self] = &[];
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

    /// The ini to play with: this session's from `--ini` if there is one, else the profile's own,
    /// else the global one. `None` is UZDoom's usual one.
    pub fn ini<'a>(&'a self, global: &'a Uac) -> Option<&'a OsString> {
        global
            .session_ini
            .as_ref()
            .or(self.ini.as_ref())
            .or(global.ini.as_ref())
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
            .ini(global)
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
