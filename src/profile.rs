use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{error::Result, hash};

const SUBDIRECTORY: &'static str = "profiles";

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Profile {
    pub name: String,
    pub mods: Vec<Mod>,
}

impl Profile {
    pub fn new(mods: Vec<Mod>) -> Self {
        Self {
            name: String::new(),
            mods,
        }
    }

    pub fn read(config_dir: &Path, profile_name: &str) -> Profile {
        let Ok(profile) = std::fs::read_to_string(
            config_dir
                .join(SUBDIRECTORY)
                .join(profile_name.to_string() + ".toml"),
        ) else {
            return Profile::default();
        };
        toml::from_str(&profile).unwrap_or_default()
    }

    pub fn write(&self, config_dir: &Path, data_dir: &Path) {
        std::fs::write(
            config_dir
                .join(SUBDIRECTORY)
                .join(self.name.clone() + ".toml"),
            toml::to_string_pretty(self).unwrap(),
        )
        .unwrap();
        let subdir = data_dir.join(&self.name);
        std::fs::create_dir_all(subdir.join("saves")).unwrap();
        std::fs::write(subdir.join("uzdoom.ini"), "").unwrap();
    }

    pub fn remove(&self, config_dir: &Path, data_dir: &Path, profile_name: String) {
        std::fs::remove_dir_all(data_dir.join(&profile_name)).unwrap();
        std::fs::remove_file(config_dir.join(SUBDIRECTORY).join(profile_name + ".toml")).unwrap();
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Mod {
    pub path: String,
    pub hash: String,
}

impl Mod {
    pub fn new(path: String, hash: String) -> Self {
        Self { path, hash }
    }

    pub fn from_path(path: String) -> Result<Self> {
        let hash = hash::hash(Path::new(&path))?;
        Ok(Self { path, hash })
    }
}
