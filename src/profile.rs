use std::{ffi::OsString, path::Path};

use serde::{Deserialize, Serialize};

use crate::{config::Config, wad::Wad};

const SUBDIRECTORY: &str = "profiles";

/// A named, ordered list of wads (load order matters) plus optional per-profile overrides.
///
/// Stored as `<config>/profiles/<name>.doom` (MessagePack); saves live in `<data>/<name>/saves`.
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

    /// The name is taken from the caller, not the file, since a bad file reads as a default profile.
    pub fn read(config_dir: &Path, profile_name: &str) -> Profile {
        let profile_data = std::fs::read(
            config_dir
                .join(SUBDIRECTORY)
                .join(profile_name.to_string() + ".doom"),
        )
        .unwrap_or_default();
        rmp_serde::from_slice(&profile_data).unwrap_or_default()
    }

    /// Writes the profile file and makes sure its `saves` folder exists. Panics if the disk write fails.
    pub fn write(&self, config_dir: &Path, data_dir: &Path) {
        std::fs::write(
            config_dir
                .join(SUBDIRECTORY)
                .join(self.name.clone() + ".doom"),
            rmp_serde::to_vec(self).unwrap(),
        )
        .unwrap();
        let subdir = data_dir.join(&self.name);
        std::fs::create_dir_all(subdir.join("saves")).unwrap();
    }

    /// Does nothing for an empty name, which would otherwise resolve to the whole data directory.
    pub fn remove(&self, config_dir: &Path, data_dir: &Path) {
        if self.name.is_empty() {
            return;
        }
        let _ = std::fs::remove_dir_all(data_dir.join(&self.name));
        let _ = std::fs::remove_file(
            config_dir
                .join(SUBDIRECTORY)
                .join(self.name.clone() + ".doom"),
        );
    }

    /// Moves the saves and deletes the old file; the caller then writes the profile under its new name.
    pub fn rename(&self, new_name: &str, config_dir: &Path, data_dir: &Path) {
        if self.name.is_empty() {
            return;
        }
        let _ = std::fs::rename(data_dir.join(&self.name), data_dir.join(new_name));
        let _ = std::fs::remove_file(
            config_dir
                .join(SUBDIRECTORY)
                .join(self.name.clone() + ".doom"),
        );
    }
}
