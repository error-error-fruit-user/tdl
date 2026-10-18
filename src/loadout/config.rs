use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    loadout::edition::Edition,
    loadout::profile::Profile,
    loadout::saved::{optional_path, paths},
    loadout::wad::Wad,
};

const FILE: &str = "config.rite";

/// What belongs to one loadout: the order of its profiles and the wad dirs kept for it. Every
/// setting lives in the global settings instead (see [`Uac`](crate::loadout::uac::Global)), so
/// it is the same whichever loadout is open.
///
/// Stored as `<config>/config.rite`. The field order is the layout of edition 0, where this was a
/// bare `(uzdoom, base_wad, profile_names)` array, so it must not change.
#[derive(Default, Deserialize, Serialize)]
pub struct Config {
    /// The settings a file from before edition 5 still holds, read but never written again: the
    /// launcher hands them to the global settings (see
    /// [`Uac::adopt`](crate::loadout::uac::Global::adopt)).
    #[serde(default, rename = "uzdoom", with = "optional_path", skip_serializing)]
    pub old_uzdoom: Option<OsString>,
    #[serde(default, rename = "base_wad", skip_serializing)]
    pub old_base_wad: Option<Wad>,
    /// The order profiles appear in the launcher. Kept in step with the profiles by [`Config::write`].
    #[serde(default)]
    pub profile_names: Vec<String>,
    #[serde(default, rename = "skip_splash", skip_serializing)]
    pub old_skip_splash: bool,
    #[serde(default, rename = "host_port", skip_serializing)]
    pub old_host_port: Option<u16>,
    /// Wad dirs kept for this loadout from `--wads`, used whenever tdl starts without it.
    /// Missing in older files, which count as none. Called `wad_folders` before edition 4.
    #[serde(default, alias = "wad_folders", with = "paths")]
    pub wad_dirs: Vec<OsString>,
    #[serde(default, rename = "uzdoom_args", skip_serializing)]
    pub old_uzdoom_args: String,
}

/// Two configs are the same if they save the same: the old settings are never written, so they
/// don't count (an upgrade reads its file back and compares it this way).
impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        self.profile_names == other.profile_names && self.wad_dirs == other.wad_dirs
    }
}

impl Edition for Config {
    const UPGRADES: &[fn(Self) -> Self] = &[v0_to_v1, v1_to_v2, v2_to_v3, v3_to_v4, v4_to_v5];
}

/// Edition 1 only wrapped the same data in a named, versioned map, so nothing needs fixing.
fn v0_to_v1(config: Config) -> Config {
    config
}

/// Edition 2 saves paths as text, so the file reads on any OS. Both shapes read into a [`Config`]
/// already (see [`crate::loadout::saved::SavedPath`]); the new edition number keeps older builds
/// from reading a file they can't understand.
fn v1_to_v2(config: Config) -> Config {
    config
}

/// Edition 3 fills in the global base wad's size if it was added before sizes were recorded, as
/// [`crate::loadout::profile`] does for every profile's wads.
fn v2_to_v3(config: Config) -> Config {
    Config {
        old_base_wad: config.old_base_wad.map(Wad::with_size_backfilled),
        ..config
    }
}

/// Edition 4 calls `wad_folders` `wad_dirs`. The old name still reads (see [`Config::wad_dirs`]);
/// the new edition number keeps older builds, which would drop the field, from reading the file.
fn v3_to_v4(config: Config) -> Config {
    config
}

/// Edition 5 keeps no settings: UZDoom, the base wad, UZDoom's arguments, the splash and the last
/// host port moved to `uac.rite`, so they are the same in every loadout. They are still read from
/// older files (as `old_*`), for the launcher to hand over before the file is upgraded.
fn v4_to_v5(config: Config) -> Config {
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
        assert!(!config.old_skip_splash);
        assert_eq!(config.old_host_port, None);
        assert!(config.wad_dirs.is_empty());
        assert!(config.old_uzdoom_args.is_empty());
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

    #[test]
    fn a_config_saved_by_edition_one_loads_and_saves_paths_as_text() {
        use std::fs;

        use crate::loadout::scribe::tests::Altar;

        /// How edition 1 saved the settings: paths as `OsString`s tagged with the OS.
        #[derive(Serialize)]
        struct EditionOne {
            uzdoom: Option<OsString>,
            base_wad: Option<EditionOneWad>,
            profile_names: Vec<String>,
            skip_splash: bool,
            host_port: Option<u16>,
        }
        #[derive(Serialize)]
        struct EditionOneWad {
            path: OsString,
            hash: String,
            size: u64,
            volatile: bool,
        }
        #[derive(Serialize)]
        struct Sealed<T> {
            version: u32,
            data: T,
        }
        let altar = Altar::new("config-edition-one");
        let path = Config::path(&altar.file(""));
        let old = EditionOne {
            uzdoom: Some("/usr/bin/uzdoom".into()),
            base_wad: Some(EditionOneWad {
                path: "/iwads/DOOM2.WAD".into(),
                hash: "ab".repeat(32),
                size: 3,
                volatile: false,
            }),
            profile_names: vec!["brutal".into()],
            skip_splash: true,
            host_port: Some(9000),
        };
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: 1,
            data: old,
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();

        let config = Config::open(&path).unwrap();
        assert_eq!(config.old_uzdoom, Some("/usr/bin/uzdoom".into()));
        assert_eq!(
            config.old_base_wad.as_ref().map(|wad| wad.path.clone()),
            Some("/iwads/DOOM2.WAD".into())
        );
        assert_eq!(config.old_host_port, Some(9000));
        assert!(config.upgrade(&path).unwrap());
        assert_eq!(Config::stored_version(&path), Some(Config::current()));
        let bytes = fs::read(&path).unwrap();
        assert!(!bytes.windows(4).any(|window| window == b"Unix"));
    }

    #[test]
    fn a_config_saved_by_edition_three_keeps_its_wad_folders_as_dirs() {
        use std::fs;

        use crate::loadout::scribe::tests::Altar;

        /// How edition 3 saved the settings: today's shape, with the wad dirs called folders.
        #[derive(Serialize)]
        struct EditionThree {
            uzdoom: Option<String>,
            base_wad: Option<Wad>,
            profile_names: Vec<String>,
            wad_folders: Vec<String>,
        }
        #[derive(Serialize)]
        struct Sealed {
            version: u32,
            data: EditionThree,
        }
        let altar = Altar::new("config-edition-three");
        let path = Config::path(&altar.file(""));
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: 3,
            data: EditionThree {
                uzdoom: None,
                base_wad: None,
                profile_names: vec!["brutal".into()],
                wad_folders: vec!["/wads".into(), "/mods".into()],
            },
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();

        let config = Config::open(&path).unwrap();
        assert_eq!(config.wad_dirs, [OsString::from("/wads"), "/mods".into()]);
        assert!(config.upgrade(&path).unwrap());
        assert_eq!(Config::stored_version(&path), Some(Config::current()));
        assert_eq!(Config::open(&path).unwrap().wad_dirs, config.wad_dirs);
    }

    #[test]
    fn a_config_saved_by_edition_four_gives_up_its_settings_when_upgraded() {
        use std::fs;

        use crate::loadout::scribe::tests::Altar;

        let altar = Altar::new("config-edition-four");
        let path = Config::path(&altar.file(""));
        // edition 4 saved the settings with the profile order and the wad dirs
        #[derive(Serialize)]
        struct EditionFour {
            uzdoom: Option<String>,
            base_wad: Option<Wad>,
            profile_names: Vec<String>,
            skip_splash: bool,
            host_port: Option<u16>,
            wad_dirs: Vec<String>,
            uzdoom_args: String,
        }
        #[derive(Serialize)]
        struct Sealed {
            version: u32,
            data: EditionFour,
        }
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: 4,
            data: EditionFour {
                uzdoom: Some("/usr/bin/uzdoom".into()),
                base_wad: None,
                profile_names: vec!["brutal".into()],
                skip_splash: true,
                host_port: Some(9000),
                wad_dirs: vec!["/wads".into()],
                uzdoom_args: "-nomusic".into(),
            },
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();

        let config = Config::open(&path).unwrap();
        assert_eq!(config.old_uzdoom, Some("/usr/bin/uzdoom".into()));
        assert!(config.old_skip_splash);
        assert_eq!(config.old_host_port, Some(9000));
        assert_eq!(config.old_uzdoom_args, "-nomusic");
        assert!(config.upgrade(&path).unwrap());
        assert_eq!(Config::stored_version(&path), Some(5));
        // the upgraded file keeps only what is the loadout's own
        let upgraded = Config::open(&path).unwrap();
        assert_eq!(upgraded.profile_names, ["brutal"]);
        assert_eq!(upgraded.wad_dirs, [OsString::from("/wads")]);
        assert_eq!(upgraded.old_uzdoom, None);
        assert!(!upgraded.old_skip_splash && upgraded.old_uzdoom_args.is_empty());
    }

    #[test]
    fn a_config_saved_by_edition_two_gets_its_base_wads_size() {
        use std::fs;

        use crate::loadout::scribe::tests::Altar;

        let altar = Altar::new("config-edition-two");
        fs::write(altar.file("DOOM2.WAD"), b"12345").unwrap();
        /// How edition 2 saved the settings, as far as this test needs them.
        #[derive(Serialize)]
        struct EditionTwo {
            base_wad: Option<Wad>,
            profile_names: Vec<String>,
        }
        let old = EditionTwo {
            base_wad: Some(Wad::new(altar.file("DOOM2.WAD").into(), "ab".repeat(32), 0)),
            profile_names: vec!["brutal".into()],
        };
        #[derive(Serialize)]
        struct Sealed<'a> {
            version: u32,
            data: &'a EditionTwo,
        }
        let path = Config::path(&altar.file(""));
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: 2,
            data: &old,
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();

        let config = Config::open(&path).unwrap();
        assert_eq!(config.old_base_wad.as_ref().map(|wad| wad.size), Some(5));
        assert_eq!(config.profile_names, ["brutal"]);
        assert!(config.upgrade(&path).unwrap());
        assert_eq!(Config::stored_version(&path), Some(Config::current()));
    }
}
