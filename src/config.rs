use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{edition::Edition, profile::Profile, wad::Wad};

const FILE: &str = "config.rite";

/// Global settings, used by any profile that doesn't override them.
///
/// Stored as `<config>/config.rite`. The field order is the layout of edition 0, where this was a
/// bare `(uzdoom, base_wad, profile_names)` array, so it must not change.
#[derive(Default, PartialEq, Deserialize, Serialize)]
pub struct Config {
    pub uzdoom: Option<OsString>,
    pub base_wad: Option<Wad>,
    /// The order profiles appear in the launcher. Kept in step with the profiles by [`Config::write`].
    #[serde(default)]
    pub profile_names: Vec<String>,
}

impl Edition for Config {
    const UPGRADES: &[fn(Self) -> Self] = &[v0_to_v1];
}

/// Edition 1 only wrapped the same data in a named, versioned map, so nothing needs fixing.
fn v0_to_v1(config: Config) -> Config {
    config
}

impl Config {
    /// Where the config is kept.
    pub fn path(directory: &Path) -> PathBuf {
        directory.join(FILE)
    }

    /// Writes the settings along with the current profile order. Panics if the disk write fails.
    pub fn write(&mut self, directory: &Path, profiles: &[Profile]) {
        self.profile_names = profiles.iter().map(|p| p.name.clone()).collect();
        self.save(&Self::path(directory)).unwrap();
    }
}
