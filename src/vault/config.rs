use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{vault::edition::Edition, vault::profile::Profile, vault::wad::Wad};

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
    /// Goes straight to the launcher without the splash screen. Toggled with `S` in the launcher.
    #[serde(default)]
    pub skip_splash: bool,
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

    /// Writes the settings along with the current profile order.
    pub fn write(&mut self, directory: &Path, profiles: &[Profile]) -> io::Result<()> {
        self.profile_names = profiles.iter().map(|p| p.name.clone()).collect();
        self.save(&Self::path(directory))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_saved_before_the_splash_option_still_shows_the_splash() {
        // the same data, minus the field that did not exist yet
        let old = rmp_serde::to_vec_named(&old_config()).unwrap();
        let config: Config = rmp_serde::from_slice(&old).unwrap();
        assert!(!config.skip_splash);
        assert_eq!(config.profile_names, ["brutal"]);
    }

    #[derive(Serialize)]
    struct OldConfig {
        uzdoom: Option<OsString>,
        base_wad: Option<Wad>,
        profile_names: Vec<String>,
    }

    fn old_config() -> OldConfig {
        OldConfig {
            uzdoom: None,
            base_wad: None,
            profile_names: vec!["brutal".to_string()],
        }
    }
}
