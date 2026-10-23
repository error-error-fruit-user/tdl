use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{loadout::edition::Edition, loadout::profile::Profile, loadout::saved::paths};

const FILE: &str = "config.rite";

/// What belongs to one loadout: the order of its profiles, the wad dirs kept for it, and whether it
/// uses settings of its own. The settings themselves are a [`Uac`](crate::loadout::uac::Uac): the
/// global ones, the same whichever loadout is open, unless `own_settings` is on, when they are the
/// loadout's own (`settings.rite`, see [`Uac::loadout_path`](crate::loadout::uac::Uac::loadout_path)).
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
    /// Uses the loadout's own settings instead of the global ones. Switched in the settings;
    /// missing in older files, which count as using the global ones.
    #[serde(default)]
    pub own_settings: bool,
    /// Dirs with `wad` in their name, found next to tdl, that the user said never to keep as wad
    /// dirs, so tdl stops asking about them. Missing in older files, which count as none.
    #[serde(default, with = "paths")]
    pub ignored_wad_dirs: Vec<OsString>,
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
            own_settings: true,
            ignored_wad_dirs: vec!["/tdl/old_wads".into()],
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
    fn a_config_saved_before_its_own_settings_uses_the_global_ones() {
        /// How edition 5 was saved before a loadout could have settings of its own.
        #[derive(Serialize)]
        struct Before {
            profile_names: Vec<String>,
            wad_dirs: Vec<String>,
        }
        #[derive(Serialize)]
        struct Sealed {
            version: u32,
            data: Before,
        }
        let altar = Altar::new("config-before-own-settings");
        let path = Config::path(&altar.0);
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: 5,
            data: Before {
                profile_names: vec!["brutal".into()],
                wad_dirs: vec!["/wads".into()],
            },
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();
        let loaded = Config::open(&path).unwrap();
        assert_eq!(loaded.wad_dirs, [OsString::from("/wads")]);
        assert!(!loaded.own_settings);
        assert!(loaded.ignored_wad_dirs.is_empty());
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
