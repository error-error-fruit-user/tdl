use std::path::PathBuf;

use crate::{config::Config, profile::Profile};

/// The profiles and settings loaded from disk, kept in sync with it.
///
/// Every mutating method writes through immediately, except reordering (done in the
/// launcher), which is saved when the launcher quits.
pub struct Library {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config: Config,
    pub profiles: Vec<Profile>,
}

impl Library {
    /// Missing or corrupt profile files still yield a profile that keeps its name, so it can be
    /// seen and deleted from the launcher.
    pub fn new(config_dir: PathBuf, data_dir: PathBuf) -> Self {
        let (config, profile_names) = Config::read(&config_dir);
        let profiles = profile_names
            .into_iter()
            .map(|name| {
                let mut profile = Profile::read(&config_dir, &name);
                profile.name = name;
                profile
            })
            .collect();
        std::fs::create_dir_all(config_dir.join("profiles")).unwrap();
        Self {
            config_dir,
            data_dir,
            config,
            profiles,
        }
    }

    /// Wads fetched from hosts, stored as `<hash>/<name>` and shared by every downloaded profile.
    pub fn downloads_dir(&self) -> PathBuf {
        self.data_dir.join(".downloads")
    }

    /// Inserts at `index`; if that was the last row, moves `index` down so the `+` row stays last.
    pub fn insert(&mut self, index: &mut usize, profile: Profile) {
        profile.write(&self.config_dir, &self.data_dir);
        self.profiles.insert(*index, profile);
        self.config.write(&self.config_dir, &self.profiles);
        if *index == self.profiles.len() - 1 {
            *index += 1;
        }
    }

    /// Overwrites the profile at `index`, moving its saves if it was renamed.
    pub fn replace(&mut self, index: usize, profile: Profile) {
        let old = &self.profiles[index];
        if old.name != profile.name {
            old.rename(&profile.name, &self.config_dir, &self.data_dir);
        }
        profile.write(&self.config_dir, &self.data_dir);
        self.profiles[index] = profile;
        self.config.write(&self.config_dir, &self.profiles);
    }

    /// Deletes the profile and its saves, then keeps `index` in range.
    pub fn remove(&mut self, index: &mut usize) {
        self.profiles[*index].remove(&self.config_dir, &self.data_dir);
        self.profiles.remove(*index);
        self.config.write(&self.config_dir, &self.profiles);
        if *index == self.profiles.len() && *index != 0 {
            *index -= 1;
        }
    }
}
