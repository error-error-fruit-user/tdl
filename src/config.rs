use std::{ffi::OsString, path::Path};

use crate::{profile::Profile, wad::Wad};

const FILE: &str = "config.doom";

#[derive(Default)]
pub struct Config {
    pub uzdoom: Option<OsString>,
    pub base_wad: Option<Wad>,
}

impl Config {
    pub fn read(directory: &Path) -> (Self, Vec<String>) {
        let (uzdoom, base_wad, profile_names) = std::fs::read(directory.join(FILE))
            .ok()
            .and_then(|data| rmp_serde::from_slice(&data).ok())
            .unwrap_or_default();
        (Self { uzdoom, base_wad }, profile_names)
    }

    pub fn write(&self, directory: &Path, profiles: &[Profile]) {
        let profile_names: Vec<_> = profiles.iter().map(|p| &p.name).collect();
        std::fs::write(
            directory.join(FILE),
            rmp_serde::to_vec(&(&self.uzdoom, &self.base_wad, &profile_names)).unwrap(),
        )
        .unwrap();
    }
}
