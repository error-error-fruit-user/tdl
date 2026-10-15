use std::path::PathBuf;

use crate::{config::Config, edition::Edition, error::Result, profile::Profile};

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
        let config = Config::open(&Config::path(&config_dir)).unwrap_or_default();
        let profiles = config
            .profile_names
            .iter()
            .map(|name| {
                let mut profile =
                    Profile::open(&Profile::path(&config_dir, name)).unwrap_or_default();
                profile.name = name.clone();
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

    /// The edition each file on disk was saved as, paired with the edition this build writes.
    /// Files that don't exist yet are left out.
    fn editions(&self) -> impl Iterator<Item = (u32, u32)> {
        let config = Config::stored_version(&Config::path(&self.config_dir))
            .map(|version| (version, Config::current()));
        let profiles = self.profiles.iter().filter_map(|p| {
            Profile::stored_version(&Profile::path(&self.config_dir, &p.name))
                .map(|version| (version, Profile::current()))
        });
        config.into_iter().chain(profiles)
    }

    /// Whether any file on disk was saved by an older edition, and still needs upgrading.
    pub fn is_outdated(&self) -> bool {
        self.editions().any(|(stored, current)| stored < current)
    }

    /// Whether any file on disk was saved by a newer edition than this build understands.
    /// Nothing should be saved while this is true, or those files would be overwritten.
    pub fn is_too_new(&self) -> bool {
        self.editions().any(|(stored, current)| stored > current)
    }

    /// Rewrites every outdated file as the current edition, checking each by reading it back.
    /// Returns the files it rewrote; their originals are untouched, for the caller to retire.
    pub fn upgrade(&self) -> Result<Vec<PathBuf>> {
        let mut upgraded = Vec::new();
        let path = Config::path(&self.config_dir);
        if self.config.upgrade(&path)? {
            upgraded.push(path);
        }
        for profile in &self.profiles {
            let path = Profile::path(&self.config_dir, &profile.name);
            if profile.upgrade(&path)? {
                upgraded.push(path);
            }
        }
        Ok(upgraded)
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

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs};

    use super::*;
    use crate::{scribe, scribe::tests::Altar, wad::Wad};

    /// A config dir laid out exactly as the app saved it before editions existed.
    fn legacy_altar(name: &str) -> (Altar, Profile) {
        let altar = Altar::new(name);
        let config = altar.file("config");
        fs::create_dir_all(config.join("profiles")).unwrap();
        let profile = Profile {
            name: "brutal".into(),
            wads: vec![Wad::new("/wads/brutal.pk3".into(), "ab".repeat(32))],
            base_wad: None,
            uzdoom: Some(OsString::from("/usr/bin/uzdoom")),
            ini: None,
        };
        // the old tuple layout: (uzdoom, base_wad, profile_names)
        let old_config = (
            Some(OsString::from("/usr/bin/uzdoom")),
            None::<Wad>,
            ["brutal"],
        );
        fs::write(
            config.join("config.doom"),
            rmp_serde::to_vec(&old_config).unwrap(),
        )
        .unwrap();
        fs::write(
            config.join("profiles/brutal.doom"),
            rmp_serde::to_vec(&profile).unwrap(),
        )
        .unwrap();
        (altar, profile)
    }

    fn library(altar: &Altar) -> Library {
        Library::new(altar.file("config"), altar.file("data"))
    }

    #[test]
    fn legacy_files_load_and_are_reported_as_outdated() {
        let (altar, profile) = legacy_altar("library-legacy-load");
        let library = library(&altar);
        assert_eq!(library.profiles, [profile]);
        assert_eq!(library.config.uzdoom, Some("/usr/bin/uzdoom".into()));
        assert!(library.is_outdated());
        assert!(!library.is_too_new());
    }

    #[test]
    fn upgrading_converts_every_file_and_keeps_the_originals_until_retired() {
        let (altar, profile) = legacy_altar("library-upgrade");
        let config_dir = altar.file("config");
        let original = fs::read(config_dir.join("profiles/brutal.doom")).unwrap();

        let upgraded = library(&altar).upgrade().unwrap();
        assert_eq!(upgraded.len(), 2);
        assert!(config_dir.join("config.rite").exists());
        assert!(config_dir.join("profiles/brutal.rite").exists());
        // originals are still there, because nobody has agreed to delete them
        assert_eq!(
            fs::read(config_dir.join("profiles/brutal.doom")).unwrap(),
            original
        );
        assert!(config_dir.join("config.doom").exists());

        let reloaded = library(&altar);
        assert!(!reloaded.is_outdated());
        assert_eq!(reloaded.profiles, [profile]);
        assert!(reloaded.upgrade().unwrap().is_empty());

        upgraded.iter().for_each(|path| scribe::retire(path));
        assert!(!config_dir.join("config.doom").exists());
        assert!(!config_dir.join("profiles/brutal.doom").exists());
        assert_eq!(library(&altar).profiles.len(), 1);
    }

    #[test]
    fn a_newer_file_is_reported_and_left_alone() {
        let (altar, _) = legacy_altar("library-too-new");
        library(&altar).upgrade().unwrap();
        let path = altar.file("config/profiles/brutal.rite");
        let newer = br#"placeholder"#;
        // replace the profile with a file from a "future" edition
        #[derive(serde::Serialize)]
        struct Future {
            version: u32,
            data: Vec<u8>,
        }
        let bytes = rmp_serde::to_vec_named(&Future {
            version: 99,
            data: newer.to_vec(),
        })
        .unwrap();
        fs::write(&path, &bytes).unwrap();
        let library = library(&altar);
        assert!(library.is_too_new());
        assert!(library.upgrade().unwrap().is_empty());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn a_fresh_install_has_nothing_to_upgrade() {
        let altar = Altar::new("library-fresh");
        let library = library(&altar);
        assert!(!library.is_outdated());
        assert!(!library.is_too_new());
    }
}
