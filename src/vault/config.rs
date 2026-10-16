use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    vault::edition::Edition,
    vault::profile::Profile,
    vault::saved::{optional_path, paths},
    vault::wad::Wad,
};

const FILE: &str = "config.rite";

/// Global settings, used by any profile that doesn't override them.
///
/// Stored as `<config>/config.rite`. The field order is the layout of edition 0, where this was a
/// bare `(uzdoom, base_wad, profile_names)` array, so it must not change.
#[derive(Default, PartialEq, Deserialize, Serialize)]
pub struct Config {
    #[serde(with = "optional_path")]
    pub uzdoom: Option<OsString>,
    pub base_wad: Option<Wad>,
    /// The order profiles appear in the launcher. Kept in step with the profiles by [`Config::write`].
    #[serde(default)]
    pub profile_names: Vec<String>,
    /// Goes straight to the launcher without the splash screen. Toggled with `S` in the launcher.
    #[serde(default)]
    pub skip_splash: bool,
    /// The port the host served its wads on last time, offered again in the host form. `None`
    /// until a game is hosted on a port other than the default.
    #[serde(default)]
    pub host_port: Option<u16>,
    /// Wad folders kept for this vault from `--wads`, used whenever tdl starts without it.
    /// Missing in older files, which count as none.
    #[serde(default, with = "paths")]
    pub wad_folders: Vec<OsString>,
}

impl Edition for Config {
    const UPGRADES: &[fn(Self) -> Self] = &[v0_to_v1, v1_to_v2];
}

/// Edition 1 only wrapped the same data in a named, versioned map, so nothing needs fixing.
fn v0_to_v1(config: Config) -> Config {
    config
}

/// Edition 2 saves paths as text, so the file reads on any OS. Both shapes read into a [`Config`]
/// already (see [`crate::vault::saved::SavedPath`]); the new edition number keeps older builds
/// from reading a file they can't understand.
fn v1_to_v2(config: Config) -> Config {
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
        assert_eq!(config.host_port, None);
        assert!(config.wad_folders.is_empty());
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

        use crate::vault::scribe::tests::Altar;

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
        assert_eq!(config.uzdoom, Some("/usr/bin/uzdoom".into()));
        assert_eq!(
            config.base_wad.as_ref().map(|wad| wad.path.clone()),
            Some("/iwads/DOOM2.WAD".into())
        );
        assert_eq!(config.host_port, Some(9000));
        assert!(config.upgrade(&path).unwrap());
        assert_eq!(Config::stored_version(&path), Some(2));
        let bytes = fs::read(&path).unwrap();
        assert!(!bytes.windows(4).any(|window| window == b"Unix"));
    }
}
