use std::{ffi::OsString, path::Path};

use serde::{Deserialize, Serialize};

use crate::wad::Wad;

const SUBDIRECTORY: &str = "profiles";

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Profile {
    pub name: String,
    pub wads: Vec<Wad>,
    pub base_wad: Option<Wad>,
    pub uzdoom: Option<OsString>,
    pub ini: Option<OsString>,
}

impl Profile {
    pub fn new(name: String, wads: Vec<Wad>) -> Self {
        Self {
            name,
            wads,
            base_wad: None,
            uzdoom: None,
            ini: None,
        }
    }

    pub fn read(config_dir: &Path, profile_name: &str) -> Profile {
        let profile_data = std::fs::read(
            config_dir
                .join(SUBDIRECTORY)
                .join(profile_name.to_string() + ".doom"),
        )
        .unwrap_or_default();
        rmp_serde::from_slice(&profile_data).unwrap_or_default()
    }

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

    pub fn remove(&self, config_dir: &Path, data_dir: &Path) {
        std::fs::remove_dir_all(data_dir.join(&self.name)).unwrap();
        std::fs::remove_file(
            config_dir
                .join(SUBDIRECTORY)
                .join(self.name.clone() + ".doom"),
        )
        .unwrap();
    }
}
