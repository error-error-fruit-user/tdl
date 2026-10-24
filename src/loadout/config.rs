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
