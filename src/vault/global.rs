//! Settings that belong to no vault: which vault to open when none is passed, and wad folders every
//! vault uses. Kept in `global.rite` in the platform's standard config folder, the one place every
//! vault can find.

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::vault::{
    edition::Edition,
    saved::{optional_path, paths},
};

const FILE: &str = "global.rite";

#[derive(Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Global {
    /// The vault to open when tdl starts without `--vault`, kept from an earlier `--vault`.
    #[serde(default, with = "optional_path")]
    pub vault: Option<OsString>,
    /// Wad folders kept from `--wads` for every vault, used after the vault's own.
    #[serde(default, with = "paths")]
    pub wad_folders: Vec<OsString>,
}

impl Edition for Global {
    /// Born after editions existed, so it starts at the first one and has nothing to upgrade.
    const UPGRADES: &[fn(Self) -> Self] = &[];
}

impl Global {
    /// Where the global settings are kept, given the standard config folder.
    pub fn path(standard_config_dir: &Path) -> PathBuf {
        standard_config_dir.join(FILE)
    }

    /// Saves to `path`, making its folder first: someone who only ever opens other vaults may
    /// never have had the standard config folder made.
    pub fn keep(&self, path: &Path) -> io::Result<()> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        self.save(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::scribe::tests::Altar;

    #[test]
    fn global_settings_save_and_load_back() {
        let altar = Altar::new("global-round-trip");
        // in a folder that doesn't exist yet
        let path = Global::path(&altar.file("standard"));
        let global = Global {
            vault: Some("/home/me/doom".into()),
            wad_folders: vec!["/wads".into(), "/mods".into()],
        };
        global.keep(&path).unwrap();
        assert_eq!(Global::open(&path), Some(global));
    }
}
