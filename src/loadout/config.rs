use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{loadout::edition::Edition, loadout::profile::Profile, loadout::saved::paths};

const FILE: &str = "config.rite";

/// What belongs to one loadout: the order of its profiles and the wad dirs kept for it. Every
/// setting lives in the global settings instead (see [`Uac`](crate::loadout::uac::Uac)), so it is
/// the same whichever loadout is open.
///
/// Stored as `<config>/config.rite`.
#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Config {
    /// The order profiles appear in the launcher. Kept in step with the profiles by [`Config::write`].
    #[serde(default)]
    pub profile_names: Vec<String>,
    /// Wad dirs kept for this loadout from `--wads`, used whenever tdl starts without it.
    #[serde(default, with = "paths")]
    pub wad_dirs: Vec<OsString>,
}

impl Edition for Config {
    /// Edition 5 is the one tdl 1.0.0 writes; older ones held settings, which `upgrade_legacy.py`
    /// hands over to `uac.rite`.
    const FIRST: u32 = 5;
    const UPGRADES: &[fn(Self) -> Self] = &[];
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
    use std::fs;

    use super::*;
    use crate::loadout::{edition::Loaded, scribe::tests::Altar};

    #[test]
    fn a_config_saves_and_loads_back_the_same() {
        let altar = Altar::new("config-round-trip");
        let mut config = Config {
            wad_dirs: vec!["/wads".into(), "/mods".into()],
            ..Config::default()
        };
        config
            .write(&altar.0, &[Profile::new("brutal".into(), Vec::new())])
            .unwrap();
        let loaded = Config::open(&Config::path(&altar.0)).unwrap();
        assert_eq!(loaded.profile_names, ["brutal"]);
        assert_eq!(loaded, config);
        assert_eq!(Config::stored_version(&Config::path(&altar.0)), Some(5));
    }

    #[test]
    fn a_config_saved_before_tdl_1_is_legacy_and_left_alone() {
        /// How edition 4 saved the settings: with UZDoom and the rest that went global.
        #[derive(Serialize)]
        struct EditionFour {
            uzdoom: Option<String>,
            profile_names: Vec<String>,
            skip_splash: bool,
        }
        #[derive(Serialize)]
        struct Sealed {
            version: u32,
            data: EditionFour,
        }
        let altar = Altar::new("config-legacy");
        let path = Config::path(&altar.0);
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: 4,
            data: EditionFour {
                uzdoom: Some("/usr/bin/uzdoom".into()),
                profile_names: vec!["brutal".into()],
                skip_splash: true,
            },
        })
        .unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(Config::is_legacy(&path));
        assert_eq!(Config::load(&path), Loaded::Legacy);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
