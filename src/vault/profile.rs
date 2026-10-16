use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{vault::config::Config, vault::edition::Edition, vault::scribe, vault::wad::Wad};

/// The profiles folder inside the config folder. The dot keeps it from ever matching a profile's
/// name (names can't contain one), which matters where the config and data folders are the same.
pub const SUBDIRECTORY: &str = ".profiles";

/// A named, ordered list of wads (load order matters) plus optional per-profile overrides.
///
/// Stored as `<config>/.profiles/<name>.rite` (MessagePack); saves live in `<data>/<name>/saves`.
/// The name is also the file name, so it is restricted to `A-Z a-z 0-9 _`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Profile {
    pub name: String,
    pub wads: Vec<Wad>,
    /// Overrides [`Config::base_wad`].
    pub base_wad: Option<Wad>,
    /// Overrides [`Config::uzdoom`].
    pub uzdoom: Option<OsString>,
    pub ini: Option<OsString>,
}

impl Edition for Profile {
    const UPGRADES: &[fn(Self) -> Self] = &[v0_to_v1];
}

/// Edition 1 only wrapped the same data in a named, versioned map, so nothing needs fixing.
fn v0_to_v1(profile: Profile) -> Profile {
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
        }
    }

    /// The profile's own UZDoom path, falling back to the global one.
    pub fn uzdoom<'a>(&'a self, config: &'a Config) -> Option<&'a OsString> {
        self.uzdoom.as_ref().or(config.uzdoom.as_ref())
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

    /// Moves the saves and deletes the old file; the caller then writes the profile under its new name.
    pub fn rename(&self, new_name: &str, config_dir: &Path, data_dir: &Path) {
        if self.name.is_empty() {
            return;
        }
        let _ = std::fs::rename(data_dir.join(&self.name), data_dir.join(new_name));
        scribe::burn(&Self::path(config_dir, &self.name));
    }
}
