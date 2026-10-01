use std::path::Path;

use serde::{Deserialize, Serialize};

const SUBDIRECTORY: &'static str = "profiles";

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Profile {
    pub mods: Vec<String>,
}

impl Profile {
    pub fn read(directory: &Path, profile_name: &str) -> Profile {
        let Ok(profile) = std::fs::read_to_string(
            directory
                .join(SUBDIRECTORY)
                .join(profile_name.to_string() + ".toml"),
        ) else {
            return Profile::default();
        };
        toml::from_str(&profile).unwrap_or_default()
    }

    pub fn write(&self, directory: &Path, profile_name: &str) {
        std::fs::write(
            directory
                .join(SUBDIRECTORY)
                .join(profile_name.to_string() + ".toml"),
            toml::to_string_pretty(self).unwrap(),
        )
        .unwrap();
    }

    pub fn remove(&self, directory: &Path, profile_name: String) {
        std::fs::remove_file(directory.join(SUBDIRECTORY).join(profile_name + ".toml")).unwrap();
    }
}
