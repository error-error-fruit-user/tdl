use std::{ffi::OsString, path::Path};

use serde::{Deserialize, Serialize};

use crate::wad::Wad;

const FILE: &str = "config.doom";

#[derive(Default, Deserialize, Serialize)]
pub struct Config {
    pub uzdoom: Option<OsString>,
    pub base_wad: Option<Wad>,
    pub profiles: Vec<String>,
}

impl Config {
    pub fn read(directory: &Path) -> Self {
        let Ok(config_data) = std::fs::read(directory.join(FILE)) else {
            return Self::default();
        };
        rmp_serde::from_slice(&config_data).unwrap_or_default()
    }

    pub fn write(&self, directory: &Path) {
        std::fs::write(directory.join(FILE), rmp_serde::to_vec(self).unwrap()).unwrap();
    }
}
