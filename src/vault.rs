pub mod config;
pub mod edition;
pub mod global;
pub mod hash;
pub mod host_settings;
pub mod lookup;
pub mod profile;
pub mod saved;
pub mod scribe;
pub mod scroll;
pub mod shelf;
pub mod wad;

use std::path::{Path, PathBuf};

use crate::{
    clipboard::Clipboard,
    error::Result,
    vault::{
        config::Config,
        edition::{Edition, Loaded},
        global::Global,
        lookup::Lookup,
        profile::{Profile, SUBDIRECTORY, tagged_name},
        shelf::Shelf,
        wad::Wad,
    },
};

/// The vault: the config and data dirs on disk, and the settings and profiles loaded from them,
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
    /// Dirs the user keeps wads in, for this session: the ones passed with `--wads`, or else
    /// the ones kept for this vault and then the global ones. Joining looks in them for wads it
    /// already has.
    pub wad_dirs: Vec<PathBuf>,
    /// Wads or profiles copied with `Y` in the launcher or the editor, for `P` to paste. Kept for
    /// the session only, never saved.
    pub clipboard: Clipboard,
    /// The settings that belong to no vault, loaded by the launcher from `global_path`.
    pub global: Global,
    pub global_path: PathBuf,
    /// False if `global.rite` is from a newer tdl (or was never loaded), so it is never written.
    pub global_writable: bool,
}

impl Vault {
    /// Loads everything, recovering from what it can. A profile whose file is unreadable falls back
    /// to its `.bak`, or failing that to an empty profile that keeps its name, so it can still be
    /// seen and deleted from the launcher. If the settings are lost, the profile list is rebuilt from
    /// the files in the profiles dir. A `profiles` dir from before the dot is renamed first.
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
            wad_dirs: Vec::new(),
            clipboard: Clipboard::default(),
            global: Global::default(),
            global_path: PathBuf::new(),
            global_writable: false,
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

    /// What the wads screen starts from: this session's wad dirs, the downloads, and the dirs
    /// kept for this vault and for every vault.
    pub fn shelf(&self) -> Shelf {
        let paths = |dirs: &[std::ffi::OsString]| dirs.iter().map(PathBuf::from).collect();
        Shelf {
            dirs: self.wad_dirs.clone(),
            downloads: self.downloads_dir(),
            kept_vault: paths(&self.config.wad_dirs),
            kept_global: paths(&self.global.wad_dirs),
            global_writable: self.global_writable,
            moves: Vec::new(),
        }
    }

    /// Saves the global settings, which every vault shares. Refused if `global.rite` is from a
    /// newer tdl, so it is never written over.
    pub fn keep_global(&self) -> Result<()> {
        if !self.global_writable {
            return Err(crate::error::Error::GlobalTooNew);
        }
        Ok(self.global.keep(&self.global_path)?)
    }

    /// Takes on what was done on the wads screen: its wad dirs become this session's, the kept
    /// ones are saved if they changed, and every wad (in the profiles, and the global base wad)
    /// whose file was moved is pointed at where it went, and saved. Everything is tried even if a
    /// save fails; the first failure is returned.
    pub fn restock(&mut self, shelf: Shelf) -> Result<()> {
        let mut outcome = Ok(());
        let mut note = |saved: std::io::Result<()>| {
            if let Err(error) = saved
                && outcome.is_ok()
            {
                outcome = Err(error.into());
            }
        };
        if shelf.dirs != self.wad_dirs || !shelf.moves.is_empty() {
            self.forget_lookup();
        }
        // 📂 the file pickers open in the wad dirs, so they hear about the change too
        crate::picker::set_wad_dirs(&shelf.dirs);
        self.wad_dirs = shelf.dirs;
        let saved = |dirs: Vec<PathBuf>| -> Vec<std::ffi::OsString> {
            dirs.into_iter().map(PathBuf::into_os_string).collect()
        };
        let kept_global = saved(shelf.kept_global);
        let mut global_changed = self.global_writable && kept_global != self.global.wad_dirs;
        if global_changed {
            self.global.wad_dirs = kept_global;
        }
        for (from, to) in &shelf.moves {
            if let Some(base_wad) = &mut self.global.base_wad {
                global_changed |= base_wad.follow(from, to);
            }
        }
        if global_changed {
            note(self.keep_global().map_err(std::io::Error::other));
        }
        let kept_vault = saved(shelf.kept_vault);
        let config_changed = kept_vault != self.config.wad_dirs;
        self.config.wad_dirs = kept_vault;
        for profile in &mut self.profiles {
            let mut followed = false;
            for (from, to) in &shelf.moves {
                followed |= profile.follow(from, to);
            }
            if followed {
                note(profile.write(&self.config_dir, &self.data_dir));
            }
        }
        if config_changed {
            note(self.config.write(&self.config_dir, &self.profiles));
        }
        outcome
    }

    /// Puts `to` in place of every wad whose file is at `from`, in the profiles and the global base
    /// wad, each staying volatile or not, and saves what changed: for a wad brought up to the
    /// host's version when joining. Everything is tried even if a save fails; the first failure
    /// is returned.
    pub fn upgrade_wad(&mut self, from: &Path, to: &Wad) -> Result<()> {
        let upgrade = |wad: &mut Wad| {
            if Path::new(&wad.path) != from {
                return false;
            }
            *wad = Wad {
                volatile: wad.volatile,
                ..to.clone()
            };
            true
        };
        self.rework_wads(upgrade, |profile| {
            let mut changed = false;
            for wad in profile.wads.iter_mut().chain(&mut profile.base_wad) {
                changed |= upgrade(wad);
            }
            changed
        })
    }

    /// Points every wad whose file was at `from` at `to`, in the profiles and the global base
    /// wad, and saves what changed: for a wad renamed to make room for the host's when joining.
    /// Everything is tried even if a save fails; the first failure is returned.
    pub fn follow_wad(&mut self, from: &Path, to: &Path) -> Result<()> {
        self.rework_wads(
            |wad| wad.follow(from, to),
            |profile| profile.follow(from, to),
        )
    }

    /// Changes the global base wad with `base_wad` and each profile with `profile` (each says
    /// whether it changed anything), and saves the ones that changed.
    fn rework_wads(
        &mut self,
        base_wad: impl Fn(&mut Wad) -> bool,
        profile: impl Fn(&mut Profile) -> bool,
    ) -> Result<()> {
        self.forget_lookup();
        let mut outcome = Ok(());
        if self.global.base_wad.as_mut().is_some_and(base_wad)
            && let Err(error) = self.keep_global()
        {
            outcome = Err(error);
        }
        for changed in &mut self.profiles {
            if profile(changed)
                && let Err(error) = changed.write(&self.config_dir, &self.data_dir)
                && outcome.is_ok()
            {
                outcome = Err(error.into());
            }
        }
        outcome
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

    /// Puts copies of `profiles` in at `index`, in their order: in front of the profile there, or at
    /// the end from the new-profile row (where `index` moves down with the row). A copy keeps its
    /// name if no profile has it; otherwise `choose` is told the profiles so far, the name, and a
    /// tagged one (`brutal_2`), and gives the copy's name, or `None` to leave it out. A copy gets an
    /// ini of its own, as [`Profile::copied_as`] says.
    pub fn insert_copies(
        &mut self,
        index: &mut usize,
        profiles: Vec<Profile>,
        mut choose: impl FnMut(&[Profile], &str, &str) -> Result<Option<String>>,
    ) -> Result<InsertedCopies> {
        let mut inserted = InsertedCopies::default();
        for profile in profiles {
            let taken = |name: &str| self.profiles.iter().any(|p| p.name == name);
            let name = if taken(&profile.name) {
                let tagged = tagged_name(&profile.name, &self.profiles);
                match choose(&self.profiles, &profile.name, &tagged)? {
                    Some(name) if !taken(&name) => name,
                    Some(_) => {
                        inserted.failed = true;
                        continue;
                    }
                    None => continue,
                }
            } else {
                profile.name.clone()
            };
            let Ok(copy) = profile.copied_as(name, &self.data_dir) else {
                inserted.failed = true;
                continue;
            };
            let on_new_row = *index == self.profiles.len();
            let landed = *index;
            if self.insert(index, copy).is_err() {
                inserted.failed = true;
                continue;
            }
            // 📋 the next copy goes after this one; on the new-profile row `insert` moved along already
            if !on_new_row {
                *index += 1;
            }
            inserted.first.get_or_insert(landed);
            inserted.count += 1;
        }
        Ok(inserted)
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

    /// Puts `profile` in place of the profile at `replaced`, which has its name: that one is
    /// deleted first, saves and all, so the two never share a file or a data dir. A new profile
    /// (`edited` is `None`) takes the replaced one's place in the list; an edited one (from the
    /// profile at `edited`) keeps its own place and moves its saves over. Returns where `profile`
    /// ended up.
    ///
    /// If the replaced profile can't be deleted, nothing changes. If it can but `profile` then
    /// can't be saved, the replaced profile stays deleted.
    pub fn supplant(
        &mut self,
        replaced: usize,
        edited: Option<usize>,
        profile: Profile,
    ) -> Result<usize> {
        let mut removed = replaced;
        self.remove(&mut removed)?;
        match edited {
            Some(edited) => {
                let index = if replaced < edited {
                    edited - 1
                } else {
                    edited
                };
                self.replace(index, profile)?;
                Ok(index)
            }
            None => {
                let mut index = replaced;
                self.insert(&mut index, profile)?;
                Ok(replaced)
            }
        }
    }

    /// Deletes every profile, each with its file, backup and data dir (saves and all). The
    /// emptied list is saved first, so a failed save leaves every profile as it was.
    pub fn remove_all(&mut self) -> Result<()> {
        self.forget_lookup();
        let removed = std::mem::take(&mut self.profiles);
        if let Err(error) = self.config.write(&self.config_dir, &self.profiles) {
            self.profiles = removed;
            return Err(error.into());
        }
        for profile in removed {
            profile.remove(&self.config_dir, &self.data_dir);
        }
        Ok(())
    }
}

/// What [`Vault::insert_copies`] came to.
#[derive(Debug, Default, PartialEq)]
pub struct InsertedCopies {
    /// Where the first copy went, if any did.
    pub first: Option<usize>,
    /// How many went in.
    pub count: usize,
    /// Whether any copy couldn't be saved (or was given a name that is taken), and was left out.
    pub failed: bool,
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
        vault::profile::Part, vault::scribe, vault::scribe::tests::Altar, vault::shelf,
        vault::wad::Wad,
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
            uzdoom_args: None,
            watched: None,
            dismissed: Vec::new(),
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
        // the UZDoom it kept is still read, for the launcher to hand to the global settings
        assert_eq!(vault.config.old_uzdoom, Some("/usr/bin/uzdoom".into()));
        assert!(vault.is_outdated());
        assert!(!vault.is_too_new());
        assert!(vault.notices.is_empty());
        // the old dir was renamed, not copied
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
        assert_eq!(vault.global.uzdoom, None);
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

    /// Makes a dir read-only until dropped, so writes into it fail. `None` where that can't be
    /// done (not unix, or running as root, who is allowed to write anywhere).
    #[cfg(unix)]
    struct Frozen(PathBuf);

    #[cfg(unix)]
    impl Frozen {
        fn new(dir: PathBuf) -> Option<Self> {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
            let frozen = Self(dir);
            // 🧊 if even a frozen dir lets us write, nothing here can fail, so skip
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

    #[test]
    fn removing_them_all_takes_every_profile_and_its_saves_but_not_the_downloads() {
        let (altar, profile) = upgraded_altar("vault-remove-all");
        let mut vault = vault(&altar);
        profile.write(&vault.config_dir, &vault.data_dir).unwrap();
        let mut index = 1;
        vault.insert(&mut index, named("second")).unwrap();
        fs::write(altar.file("data/brutal/saves/slot1"), b"my save").unwrap();
        fs::create_dir_all(vault.downloads_dir()).unwrap();
        fs::write(vault.downloads_dir().join("got.wad"), b"wad").unwrap();

        vault.remove_all().unwrap();
        assert!(vault.profiles.is_empty());
        assert!(vault.config.profile_names.is_empty());
        for gone in [
            "config/.profiles/brutal.rite",
            "config/.profiles/second.rite",
            "data/brutal",
        ] {
            assert!(!altar.file(gone).exists(), "{gone}");
        }
        assert!(vault.downloads_dir().join("got.wad").is_file());
        assert!(
            self::vault(&altar).profiles.is_empty(),
            "and it stays empty"
        );
    }

    #[test]
    fn restocking_keeps_the_dirs_and_points_moved_wads_at_where_they_went() {
        let altar = Altar::new("vault-restock");
        let root = crate::testing::example_vault(&altar);
        let mut vault = Vault::new(root.join("config"), root.join("data"));
        // 📂 the example's wads are beside the vault
        let wads = altar.file("");
        vault.global_path = root.join("global.rite");
        vault.global_writable = true;
        let mut shelf = vault.shelf();
        assert_eq!(shelf.downloads, vault.downloads_dir());
        shelf.add(wads.join("wads"), shelf::Keep::Vault);
        shelf.add(wads.join("mods"), shelf::Keep::Global);
        let (from, to) = (
            wads.join("wads/brutalv22.pk3"),
            wads.join("mods/brutalv22.pk3"),
        );
        fs::create_dir_all(wads.join("mods")).unwrap();
        fs::rename(&from, &to).unwrap();
        shelf.moves.push((from.clone(), to.clone()));

        vault.restock(shelf).unwrap();
        assert_eq!(vault.wad_dirs, [wads.join("wads"), wads.join("mods")]);
        assert_eq!(vault.config.wad_dirs, [wads.join("wads").into_os_string()]);
        assert_eq!(vault.global.wad_dirs, [wads.join("mods").into_os_string()]);
        assert_eq!(vault.profiles[0].wads[0].path, to.as_os_str());
        // all of it saved, the moved wad's profile too
        let reloaded = Vault::new(root.join("config"), root.join("data"));
        assert_eq!(reloaded.config.wad_dirs, vault.config.wad_dirs);
        assert_eq!(reloaded.profiles[0].wads[0].path, to.as_os_str());
        assert_eq!(
            Global::open(&root.join("global.rite")).unwrap().wad_dirs,
            vault.global.wad_dirs
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_remove_all_deletes_nothing() {
        let (altar, profile) = upgraded_altar("vault-remove-all-fails");
        let mut vault = vault(&altar);
        profile.write(&vault.config_dir, &vault.data_dir).unwrap();
        fs::write(altar.file("data/brutal/saves/slot1"), b"my save").unwrap();
        let Some(_frozen) = Frozen::new(altar.file("config")) else {
            return;
        };
        assert!(vault.remove_all().is_err());
        assert_eq!(vault.profiles, [profile]);
        assert!(altar.file("config/.profiles/brutal.rite").exists());
        assert!(altar.file("data/brutal/saves/slot1").exists());
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

    // 📇 where the cursor lands after a successful insert or remove

    #[test]
    fn inserting_in_the_middle_keeps_the_cursor_on_the_new_profile() {
        let (altar, profile) = upgraded_altar("vault-insert-middle");
        let mut vault = vault(&altar);
        let mut index = 0;
        vault.insert(&mut index, named("first")).unwrap();
        assert_eq!(index, 0);
        let names: Vec<_> = vault.profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["first", &profile.name]);
        assert_eq!(vault.config.profile_names, names);
        assert!(altar.file("config/.profiles/first.rite").exists());
    }

    #[test]
    fn inserting_on_the_new_profile_row_moves_the_cursor_down_with_it() {
        let (altar, profile) = upgraded_altar("vault-insert-last");
        let mut vault = vault(&altar);
        // the `+` row sits just past the last profile
        let mut index = vault.profiles.len();
        vault.insert(&mut index, named("last")).unwrap();
        assert_eq!(vault.profiles[1].name, "last");
        assert_eq!(index, 2, "the cursor stays on the `+` row");
        assert_eq!(vault.config.profile_names, [profile.name.as_str(), "last"]);
        // and the order survives a restart
        let names: Vec<_> = self::vault(&altar)
            .profiles
            .iter()
            .map(|p| p.name.clone())
            .collect();
        assert_eq!(names, [profile.name.as_str(), "last"]);
    }

    #[test]
    fn removing_keeps_the_cursor_in_range() {
        let (altar, _) = upgraded_altar("vault-remove-ok");
        let mut vault = vault(&altar);
        let mut index = 1;
        vault.insert(&mut index, named("second")).unwrap();
        vault.insert(&mut index, named("third")).unwrap();
        assert_eq!(index, 3);

        // the last profile: the cursor moves up onto the one before it
        let mut index = 2;
        vault.remove(&mut index).unwrap();
        assert_eq!(index, 1);
        assert!(!altar.file("config/.profiles/third.rite").exists());
        assert!(!altar.file("data/third").exists());

        // one in the middle: the cursor stays, now on the profile that followed it
        let mut index = 0;
        vault.remove(&mut index).unwrap();
        assert_eq!(index, 0);
        assert_eq!(vault.profiles[0].name, "second");

        // the only one left: the cursor stays at the top, on the `+` row
        vault.remove(&mut index).unwrap();
        assert_eq!(index, 0);
        assert!(vault.profiles.is_empty());
        assert!(vault.config.profile_names.is_empty());
        assert!(self::vault(&altar).profiles.is_empty());
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
    fn a_new_profile_replacing_one_of_its_name_takes_its_place_and_not_its_saves() {
        let (altar, profile) = upgraded_altar("vault-supplant-new");
        let mut vault = vault(&altar);
        profile.write(&vault.config_dir, &vault.data_dir).unwrap();
        let mut index = 1;
        vault.insert(&mut index, named("second")).unwrap();
        fs::write(altar.file("data/brutal/saves/slot1"), b"old save").unwrap();
        let replacement = Profile {
            wads: vec![Wad::new("/wads/new.pk3".into(), "cd".repeat(32), 0)],
            ..named("brutal")
        };

        assert_eq!(vault.supplant(0, None, replacement.clone()).unwrap(), 0);
        assert_eq!(vault.profiles, [replacement.clone(), named("second")]);
        assert_eq!(vault.config.profile_names, ["brutal", "second"]);
        assert!(!altar.file("data/brutal/saves/slot1").exists());
        assert_eq!(self::vault(&altar).profiles, [replacement, named("second")]);
    }

    #[test]
    fn an_edited_profile_renamed_over_another_keeps_its_place_and_its_saves() {
        let (altar, profile) = upgraded_altar("vault-supplant-edited");
        let mut vault = vault(&altar);
        profile.write(&vault.config_dir, &vault.data_dir).unwrap();
        let mut index = 1;
        vault.insert(&mut index, named("second")).unwrap();
        vault.insert(&mut index, named("third")).unwrap();
        fs::write(altar.file("data/brutal/saves/slot1"), b"replaced").unwrap();
        fs::write(altar.file("data/third/saves/slot1"), b"kept").unwrap();
        let renamed = named("brutal");

        // the replaced profile was in front, so the edited one moves up a place with the list
        assert_eq!(vault.supplant(0, Some(2), renamed.clone()).unwrap(), 1);
        assert_eq!(vault.profiles, [named("second"), renamed.clone()]);
        assert_eq!(vault.config.profile_names, ["second", "brutal"]);
        assert!(!altar.file("config/.profiles/third.rite").exists());
        assert!(!altar.file("data/third").exists());
        assert_eq!(
            fs::read(altar.file("data/brutal/saves/slot1")).unwrap(),
            b"kept"
        );
        assert_eq!(self::vault(&altar).profiles, [named("second"), renamed]);
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
        let faults = reloaded.profiles[0].faults(&reloaded.global);
        assert!(faults.iter().all(|(part, _)| *part != Part::Ini));
    }
}
