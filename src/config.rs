use std::path::Path;

use serde::{Deserialize, Serialize};

const FILE: &'static str = "config.toml";

#[derive(Default, Deserialize, Serialize)]
pub struct Config {
    pub uzdoom: String,
    pub profiles: Vec<String>,
}

impl Config {
    pub fn read(directory: &Path) -> Self {
        let Ok(toml) = std::fs::read_to_string(directory.join(FILE)) else {
            return Self::default();
        };
        toml::from_str(&toml).unwrap_or_default()
    }

    pub fn write(&self, directory: &Path) {
        std::fs::write(directory.join(FILE), toml::to_string_pretty(self).unwrap()).unwrap();
    }
}
