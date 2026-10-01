use std::path::Path;

use serde::{Deserialize, Serialize};

const SUBDIRECTORY: &'static str = "profiles";

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Profile {
    pub mods: Vec<String>,
}

impl Profile {
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

    pub fn write(&self, config_dir: &Path, data_dir: &Path, profile_name: &str) {
        std::fs::write(
            config_dir
                .join(SUBDIRECTORY)
                .join(profile_name.to_string() + ".toml"),
            toml::to_string_pretty(self).unwrap(),
        )
        .unwrap();
        let subdir = data_dir.join(profile_name);
        std::fs::create_dir_all(subdir.join("saves")).unwrap();
        std::fs::write(subdir.join("uzdoom.ini"), "").unwrap();
    }

    pub fn remove(&self, config_dir: &Path, data_dir: &Path, profile_name: String) {
        std::fs::remove_dir_all(data_dir.join(&profile_name)).unwrap();
        std::fs::remove_file(config_dir.join(SUBDIRECTORY).join(profile_name + ".toml")).unwrap();
    }
}
