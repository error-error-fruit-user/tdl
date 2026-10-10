use std::{ffi::OsString, path::Path};

use serde::{Deserialize, Serialize};

use crate::{error::Result, hash};

const SUBDIRECTORY: &str = "profiles";

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Profile {
    pub name: String,
    pub mods: Vec<Mod>,
    pub uzdoom: Option<OsString>,
}

impl Profile {
    pub fn new(mods: Vec<Mod>) -> Self {
        Self {
            name: String::new(),
            mods,
            uzdoom: None,
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
        std::fs::write(subdir.join("uzdoom.ini"), "").unwrap();
    }

    pub fn remove(&self, config_dir: &Path, data_dir: &Path, profile_name: String) {
        std::fs::remove_dir_all(data_dir.join(&profile_name)).unwrap();
        std::fs::remove_file(config_dir.join(SUBDIRECTORY).join(profile_name + ".doom")).unwrap();
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Mod {
    pub path: OsString,
    pub hash: String,
}

impl Mod {
    pub fn new(path: OsString, hash: String) -> Self {
        Self { path, hash }
    }

    pub fn from_path(path: OsString) -> Result<Self> {
        let hash = hash::hash(Path::new(&path))?;
        Ok(Self { path, hash })
    }
}
