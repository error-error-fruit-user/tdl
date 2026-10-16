pub mod config;
pub mod edition;
pub mod hash;
pub mod host_settings;
pub mod lookup;
pub mod profile;
pub mod saved;
pub mod scribe;
pub mod wad;

use std::path::PathBuf;

use crate::{
    error::Result,
    vault::{
        config::Config,
        edition::{Edition, Loaded},
        lookup::Lookup,
        profile::{Profile, SUBDIRECTORY},
    },
};

/// The vault: the config and data folders on disk, and the settings and profiles loaded from them,
/// kept in sync with it.
///
/// Every mutating method writes through immediately, except reordering (done in the
/// launcher), which is saved when the launcher quits.
pub struct Vault {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config: Config,
    pub profiles: Vec<Profile>,
    /// Things that went wrong while loading and were dealt with, for the user to be told about
    /// once. Nothing unreadable is ever overwritten: it is kept next to where it was as `.corrupt`.
    pub notices: Vec<String>,
    /// Where wads already on this machine are, built the first time it is asked for and thrown
    /// away whenever the profiles change, so it is never out of date.
    lookup: Option<Lookup>,
}

impl Vault {
    /// Loads everything, recovering from what it can. A profile whose file is unreadable falls back
    /// to its `.bak`, or failing that to an empty profile that keeps its name, so it can still be
    /// seen and deleted from the launcher. If the settings are lost, the profile list is rebuilt from
    /// the files in the profiles folder. A `profiles` folder from before the dot is renamed first.
    pub fn new(config_dir: PathBuf, data_dir: PathBuf) -> Self {
        let profiles_dir = config_dir.join(SUBDIRECTORY);
        if !profiles_dir.exists() {
            let _ = std::fs::rename(config_dir.join("profiles"), &profiles_dir);
        }
        let mut notices = Vec::new();
        let config = match Config::load(&Config::path(&config_dir)) {
            Loaded::Fine(config) => config,
            Loaded::Restored(config) => {
                notices.push("Restored your settings from their backup".to_string());
                config
            }
            Loaded::Lost => {
                notices.push("Your settings were unreadable, saved as .corrupt".to_string());
                let profile_names = names_on_disk(&profiles_dir);
                if !profile_names.is_empty() {
                    notices.push("Profile list rebuilt from the files on disk".to_string());
                }
                Config {
                    profile_names,
                    ..Config::default()
                }
            }
            Loaded::Missing | Loaded::TooNew => Config::default(),
        };
        let profiles = config
            .profile_names
            .iter()
            .map(|name| {
                let mut profile = match Profile::load(&Profile::path(&config_dir, name)) {
                    Loaded::Fine(profile) => profile,
                    Loaded::Restored(profile) => {
                        notices.push(format!("Restored '{name}' from its backup"));
                        profile
                    }
                    Loaded::Lost => {
                        notices.push(format!("'{name}' was unreadable, saved as .corrupt"));
                        Profile::default()
                    }
                    Loaded::Missing => {
                        notices.push(format!("'{name}' has no file, starting empty"));
                        Profile::default()
                    }
                    Loaded::TooNew => Profile::default(),
                };
                profile.name = name.clone();
                profile
            })
            .collect();
        let _ = std::fs::create_dir_all(&profiles_dir);
        Self {
            config_dir,
            data_dir,
            config,
            profiles,
            notices,
            lookup: None,
        }
    }

    /// Where wads already on this machine are, built now if it hasn't been since the profiles
    /// last changed.
    pub fn lookup(&mut self) -> &Lookup {
        if self.lookup.is_none() {
            self.lookup = Some(Lookup::build(self));
        }
        self.lookup.as_ref().expect("built just above")
    }

    /// Throws the lookup away, for a change to the wads it was built from that doesn't go through
    /// the vault, such as setting the global base wad.
    pub fn forget_lookup(&mut self) {
        self.lookup = None;
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
    ///
    /// If the settings can't be saved the profile is taken back out of the list, so what is on
    /// screen always matches what is on disk. (Its file is left behind, harmlessly.)
    pub fn insert(&mut self, index: &mut usize, profile: Profile) -> Result<()> {
        self.forget_lookup();
        profile.write(&self.config_dir, &self.data_dir)?;
        self.profiles.insert(*index, profile);
        if let Err(error) = self.config.write(&self.config_dir, &self.profiles) {
            self.profiles.remove(*index);
            return Err(error.into());
        }
        if *index == self.profiles.len() - 1 {
            *index += 1;
        }
        Ok(())
    }

    /// Overwrites the profile at `index`, moving its saves if it was renamed.
    ///
    /// The order matters: the new file is written first, then the settings, and only then are the
    /// old saves moved and the old file deleted. A failure at any point leaves the old profile whole.
    ///
    /// Nothing is written if nothing changed, and the settings are only rewritten when the name did,
    /// since all they hold about a profile is its name. Every save waits for the disk, so skipping
    /// the needless ones is what keeps leaving the editor quick.
    pub fn replace(&mut self, index: usize, profile: Profile) -> Result<()> {
        self.forget_lookup();
        if self.profiles[index] == profile {
            return Ok(());
        }
        let renamed = self.profiles[index].name != profile.name;
        let mut profile = profile;
        if renamed {
            profile.follow_rename(&self.data_dir, &self.profiles[index].name);
        }
        profile.save(&Profile::path(&self.config_dir, &profile.name))?;
        let old = std::mem::replace(&mut self.profiles[index], profile);
        if renamed && let Err(error) = self.config.write(&self.config_dir, &self.profiles) {
            self.profiles[index] = old;
            return Err(error.into());
        }
        let profile = &self.profiles[index];
        if renamed {
            old.rename(&profile.name, &self.config_dir, &self.data_dir);
        }
        Ok(profile.create_saves_dir(&self.data_dir)?)
    }

    /// Deletes the profile and its saves, then keeps `index` in range.
    ///
    /// The settings stop listing it first, and its files are only deleted once that has been saved.
    pub fn remove(&mut self, index: &mut usize) -> Result<()> {
        self.forget_lookup();
        // 🪦 the list forgets first, and only then does the profile get buried
        let removed = self.profiles.remove(*index);
        if let Err(error) = self.config.write(&self.config_dir, &self.profiles) {
            self.profiles.insert(*index, removed);
            return Err(error.into());
        }
        removed.remove(&self.config_dir, &self.data_dir);
        if *index == self.profiles.len() && *index != 0 {
            *index -= 1;
        }
        Ok(())
    }
}

/// The profile names that have a file in `dir`, in alphabetical order. Backups and other leftovers
/// have different extensions, so they are not counted.
fn names_on_disk(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let extension = path.extension()?.to_str()?;
            let stem = path.file_stem()?.to_str()?;
            let valid =
                !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
            (valid && [scribe::EXTENSION, scribe::OLD_EXTENSION].contains(&extension))
                .then(|| stem.to_string())
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs};

    use super::*;
    use crate::{
        vault::profile::Part, vault::scribe, vault::scribe::tests::Altar, vault::wad::Wad,
    };

    /// A config dir laid out exactly as the app saved it before editions existed.
    fn legacy_altar(name: &str) -> (Altar, Profile) {
        let altar = Altar::new(name);
        let config = altar.file("config");
        fs::create_dir_all(config.join("profiles")).unwrap();
        let profile = Profile {
            name: "brutal".into(),
            wads: vec![Wad::new("/wads/brutal.pk3".into(), "ab".repeat(32), 0)],
            base_wad: None,
            uzdoom: Some(OsString::from("/usr/bin/uzdoom")),
            ini: None,
            host_defaults: None,
        };
        // the old profile layout, from before host defaults: a bare five-field array
        let old_profile = (
            &profile.name,
            &profile.wads,
            &profile.base_wad,
            &profile.uzdoom,
            &profile.ini,
        );
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
            rmp_serde::to_vec(&old_profile).unwrap(),
        )
        .unwrap();
        (altar, profile)
    }

    fn vault(altar: &Altar) -> Vault {
        Vault::new(altar.file("config"), altar.file("data"))
    }

    #[test]
    fn legacy_files_load_and_are_reported_as_outdated() {
        let (altar, profile) = legacy_altar("vault-legacy-load");
        let vault = vault(&altar);
        assert_eq!(vault.profiles, [profile]);
        assert_eq!(vault.config.uzdoom, Some("/usr/bin/uzdoom".into()));
        assert!(vault.is_outdated());
        assert!(!vault.is_too_new());
        assert!(vault.notices.is_empty());
        // the old folder was renamed, not copied
        assert!(altar.file("config/.profiles").is_dir());
        assert!(!altar.file("config/profiles").exists());
    }

    #[test]
    fn upgrading_converts_every_file_and_keeps_the_originals_until_retired() {
        let (altar, profile) = legacy_altar("vault-upgrade");
        let config_dir = altar.file("config");
        let original = fs::read(config_dir.join("profiles/brutal.doom")).unwrap();

        let upgraded = vault(&altar).upgrade().unwrap();
        assert_eq!(upgraded.len(), 2);
        assert!(config_dir.join("config.rite").exists());
        assert!(config_dir.join(".profiles/brutal.rite").exists());
        // originals are still there, because nobody has agreed to delete them
        assert_eq!(
            fs::read(config_dir.join(".profiles/brutal.doom")).unwrap(),
            original
        );
        assert!(config_dir.join("config.doom").exists());

        let reloaded = vault(&altar);
        assert!(!reloaded.is_outdated());
        assert_eq!(reloaded.profiles, [profile]);
        assert!(reloaded.upgrade().unwrap().is_empty());

        upgraded.iter().for_each(|path| scribe::retire(path));
        assert!(!config_dir.join("config.doom").exists());
        assert!(!config_dir.join(".profiles/brutal.doom").exists());
        assert_eq!(vault(&altar).profiles.len(), 1);
    }

    #[test]
    fn a_newer_file_is_reported_and_left_alone() {
        let (altar, _) = legacy_altar("vault-too-new");
        vault(&altar).upgrade().unwrap();
        let path = altar.file("config/.profiles/brutal.rite");
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
        let vault = vault(&altar);
        assert!(vault.is_too_new());
        assert!(vault.upgrade().unwrap().is_empty());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn a_fresh_install_has_nothing_to_upgrade() {
        let altar = Altar::new("vault-fresh");
        let vault = vault(&altar);
        assert!(!vault.is_outdated());
        assert!(!vault.is_too_new());
    }

    /// Upgrades the legacy layout, so there is a `.rite` and a `.bak` to break.
    fn upgraded_altar(name: &str) -> (Altar, Profile) {
        let (altar, profile) = legacy_altar(name);
        vault(&altar).upgrade().unwrap();
        (altar, profile)
    }

    #[test]
    fn a_corrupt_profile_is_restored_from_its_backup_and_the_user_is_told() {
        let (altar, profile) = upgraded_altar("vault-restore");
        let path = altar.file("config/.profiles/brutal.rite");
        fs::write(&path, b"not a profile").unwrap();
        let vault = vault(&altar);
        assert_eq!(vault.profiles, [profile]);
        assert_eq!(vault.notices, ["Restored 'brutal' from its backup"]);
        assert_eq!(
            fs::read(altar.file("config/.profiles/brutal.rite.corrupt")).unwrap(),
            b"not a profile"
        );
    }

    #[test]
    fn a_corrupt_profile_without_a_backup_is_empty_but_keeps_its_name_and_its_bytes() {
        let (altar, _) = upgraded_altar("vault-lost");
        let path = altar.file("config/.profiles/brutal.rite");
        fs::remove_file(altar.file("config/.profiles/brutal.rite.bak")).unwrap();
        fs::write(&path, b"not a profile").unwrap();
        let vault = vault(&altar);
        assert_eq!(vault.profiles.len(), 1);
        assert_eq!(vault.profiles[0].name, "brutal");
        assert!(vault.profiles[0].wads.is_empty());
        assert_eq!(
            vault.notices,
            ["'brutal' was unreadable, saved as .corrupt"]
        );

        // saving the empty profile must not touch the unreadable original
        vault.profiles[0]
            .write(&vault.config_dir, &vault.data_dir)
            .unwrap();
        assert_eq!(
            fs::read(altar.file("config/.profiles/brutal.rite.corrupt")).unwrap(),
            b"not a profile"
        );
    }

    #[test]
    fn a_profile_with_no_file_is_reported_and_starts_empty() {
        let (altar, _) = upgraded_altar("vault-missing");
        burn_profile(&altar);
        let vault = vault(&altar);
        assert_eq!(vault.profiles[0].name, "brutal");
        assert_eq!(vault.notices, ["'brutal' has no file, starting empty"]);
    }

    #[test]
    fn lost_settings_are_set_aside_and_the_profile_list_is_rebuilt_from_the_files() {
        let (altar, profile) = upgraded_altar("vault-config-lost");
        fs::remove_file(altar.file("config/config.rite.bak")).unwrap();
        fs::write(altar.file("config/config.rite"), b"not settings").unwrap();
        let vault = vault(&altar);
        assert_eq!(vault.profiles, [profile]);
        assert_eq!(vault.config.uzdoom, None);
        assert_eq!(
            vault.notices,
            [
                "Your settings were unreadable, saved as .corrupt",
                "Profile list rebuilt from the files on disk"
            ]
        );
        assert_eq!(
            fs::read(altar.file("config/config.rite.corrupt")).unwrap(),
            b"not settings"
        );
    }

    #[test]
    fn only_real_profile_files_count_when_rebuilding_the_list() {
        let altar = Altar::new("vault-names");
        let dir = altar.file(".profiles");
        fs::create_dir_all(&dir).unwrap();
        for name in [
            "b.rite",
            "a.rite",
            "a.rite.bak",
            "c.rite.corrupt",
            "d.rite.tmp",
            "e.doom",
            "e.rite",
            "bad name.rite",
            "notes.txt",
        ] {
            fs::write(dir.join(name), b"x").unwrap();
        }
        assert_eq!(names_on_disk(&dir), ["a", "b", "e"]);
        assert!(names_on_disk(&altar.file("nowhere")).is_empty());
    }

    fn burn_profile(altar: &Altar) {
        scribe::burn(&altar.file("config/.profiles/brutal.rite"));
    }

    /// Makes a folder read-only until dropped, so writes into it fail. `None` where that can't be
    /// done (not unix, or running as root, who is allowed to write anywhere).
    #[cfg(unix)]
    struct Frozen(PathBuf);

    #[cfg(unix)]
    impl Frozen {
        fn new(dir: PathBuf) -> Option<Self> {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
            let frozen = Self(dir);
            // 🧊 if even a frozen folder lets us write, nothing here can fail, so skip
            let probe = frozen.0.join(".probe");
            match fs::write(&probe, b"") {
                Ok(()) => {
                    let _ = fs::remove_file(probe);
                    None
                }
                Err(_) => Some(frozen),
            }
        }
    }

    #[cfg(unix)]
    impl Drop for Frozen {
        fn drop(&mut self) {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
        }
    }

    fn named(name: &str) -> Profile {
        Profile {
            name: name.into(),
            ..Profile::default()
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_insert_leaves_the_list_and_the_disk_alone() {
        let (altar, profile) = upgraded_altar("vault-insert-fails");
        let mut vault = vault(&altar);
        let Some(_frozen) = Frozen::new(altar.file("config/.profiles")) else {
            return;
        };
        let mut index = 0;
        assert!(vault.insert(&mut index, named("new")).is_err());
        assert_eq!(vault.profiles, [profile]);
        assert_eq!(index, 0);
        assert!(!altar.file("config/.profiles/new.rite").exists());
    }

    #[cfg(unix)]
    #[test]
    fn an_insert_whose_settings_cannot_be_saved_is_taken_back() {
        let (altar, profile) = upgraded_altar("vault-insert-config-fails");
        let mut vault = vault(&altar);
        let Some(_frozen) = Frozen::new(altar.file("config")) else {
            return;
        };
        let mut index = 0;
        assert!(vault.insert(&mut index, named("new")).is_err());
        assert_eq!(vault.profiles, [profile]);
        assert_eq!(index, 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_remove_deletes_nothing() {
        let (altar, profile) = upgraded_altar("vault-remove-fails");
        let mut vault = vault(&altar);
        profile.write(&vault.config_dir, &vault.data_dir).unwrap();
        fs::write(altar.file("data/brutal/saves/slot1"), b"my save").unwrap();
        let Some(_frozen) = Frozen::new(altar.file("config")) else {
            return;
        };
        let mut index = 0;
        assert!(vault.remove(&mut index).is_err());
        assert_eq!(vault.profiles, [profile]);
        assert!(altar.file("config/.profiles/brutal.rite").exists());
        assert_eq!(
            fs::read(altar.file("data/brutal/saves/slot1")).unwrap(),
            b"my save"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_rename_leaves_the_old_profile_and_its_saves_in_place() {
        let (altar, profile) = upgraded_altar("vault-rename-fails");
        let mut vault = vault(&altar);
        profile.write(&vault.config_dir, &vault.data_dir).unwrap();
        fs::write(altar.file("data/brutal/saves/slot1"), b"my save").unwrap();
        let Some(_frozen) = Frozen::new(altar.file("config")) else {
            return;
        };
        let renamed = Profile {
            name: "renamed".into(),
            ..profile.clone()
        };
        assert!(vault.replace(0, renamed).is_err());
        assert_eq!(vault.profiles, [profile]);
        assert!(altar.file("config/.profiles/brutal.rite").exists());
        assert_eq!(
            fs::read(altar.file("data/brutal/saves/slot1")).unwrap(),
            b"my save"
        );
    }

    #[test]
    fn renaming_moves_the_saves_and_retires_the_old_file() {
        let (altar, profile) = upgraded_altar("vault-rename");
        let mut vault = vault(&altar);
        profile.write(&vault.config_dir, &vault.data_dir).unwrap();
        fs::write(altar.file("data/brutal/saves/slot1"), b"my save").unwrap();
        let renamed = Profile {
            name: "renamed".into(),
            ..profile
        };
        vault.replace(0, renamed.clone()).unwrap();
        assert_eq!(vault.profiles, std::slice::from_ref(&renamed));
        assert_eq!(vault.config.profile_names, ["renamed"]);
        assert!(!altar.file("config/.profiles/brutal.rite").exists());
        assert!(altar.file("config/.profiles/renamed.rite").exists());
        assert!(!altar.file("data/brutal").exists());
        assert_eq!(
            fs::read(altar.file("data/renamed/saves/slot1")).unwrap(),
            b"my save"
        );
        // and it survives a restart
        assert_eq!(self::vault(&altar).profiles, [renamed]);
    }

    #[test]
    fn leaving_the_editor_without_changes_writes_nothing() {
        let (altar, profile) = upgraded_altar("vault-replace-unchanged");
        let mut vault = vault(&altar);
        let files = [
            "config/.profiles/brutal.rite",
            "config/.profiles/brutal.rite.bak",
        ];
        let snapshot = || -> Vec<_> {
            files
                .iter()
                .map(|f| {
                    let modified = fs::metadata(altar.file(f)).unwrap().modified().unwrap();
                    (fs::read(altar.file(f)).unwrap(), modified)
                })
                .collect()
        };
        let before = snapshot();
        vault.replace(0, profile).unwrap();
        assert_eq!(before, snapshot());
        assert!(!altar.file("config/.profiles/brutal.rite.tmp").exists());
    }

    #[test]
    fn an_edit_that_keeps_the_name_does_not_rewrite_the_settings() {
        let (altar, profile) = upgraded_altar("vault-replace-keeps-config");
        let mut vault = vault(&altar);
        // saving the settings again would copy the current config over its backup
        let backup = fs::read(altar.file("config/config.rite.bak")).unwrap();
        let modified = fs::metadata(altar.file("config/config.rite"))
            .unwrap()
            .modified()
            .unwrap();

        let mut edited = profile;
        edited
            .wads
            .push(Wad::new("/wads/extra.pk3".into(), "cd".repeat(32), 0));
        vault.replace(0, edited.clone()).unwrap();

        assert_eq!(
            fs::read(altar.file("config/config.rite.bak")).unwrap(),
            backup
        );
        let now = fs::metadata(altar.file("config/config.rite"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(now, modified);
        // while the profile itself was saved
        assert_eq!(self::vault(&altar).profiles, [edited]);
    }

    #[test]
    fn renaming_a_profile_keeps_its_own_ini_pointing_at_the_moved_file() {
        let (altar, profile) = upgraded_altar("vault-rename-ini");
        let mut vault = vault(&altar);
        let ini = altar.file("data/brutal/uzdoom.ini");
        fs::create_dir_all(altar.file("data/brutal")).unwrap();
        fs::write(&ini, b"[Doom.Player]").unwrap();
        let with_ini = Profile {
            ini: Some(ini.into()),
            ..profile
        };
        vault.replace(0, with_ini.clone()).unwrap();

        let renamed = Profile {
            name: "renamed".into(),
            ..with_ini
        };
        vault.replace(0, renamed).unwrap();

        let moved = altar.file("data/renamed/uzdoom.ini");
        assert_eq!(vault.profiles[0].ini, Some(moved.clone().into()));
        assert_eq!(fs::read(moved).unwrap(), b"[Doom.Player]");
        // and a restart sees the same, with no broken ini to complain about
        let reloaded = self::vault(&altar);
        assert_eq!(reloaded.profiles[0].ini, vault.profiles[0].ini);
        // (the wad and UZDoom paths in this fixture are made up, so only the ini is checked)
        let faults = reloaded.profiles[0].faults(&reloaded.config);
        assert!(faults.iter().all(|(part, _)| *part != Part::Ini));
    }
}
