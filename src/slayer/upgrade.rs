//! Bringing the joiner's own wads up to the host's version when joining. A file here with the name
//! of one of the host's wads but other contents is replaced by the host's (the old one kept beside
//! it as `.bak`), renamed so both are kept, or left alone. A volatile wad is replaced without
//! asking, since it is expected to change; any other is the joiner's call.

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    error::Result,
    net::Manifest,
    vault::{
        Vault, shelf,
        wad::{self, Wad},
    },
};

/// A file here with the name of one of the host's wads, but not its contents.
#[derive(Clone, Debug, PartialEq)]
pub struct Outdated {
    /// Which of the host's wads it is, by its place in the manifest.
    pub entry: usize,
    pub path: PathBuf,
    /// Whether a profile has it as a volatile wad, so it is replaced without asking.
    pub volatile: bool,
}

impl Outdated {
    /// Whether the file is a download, kept under its hash, which is never renamed: replacing it
    /// puts the host's under its own hash instead, and the old one stays where it is.
    pub fn is_download(&self, vault: &Vault) -> bool {
        shelf::is_download(&self.path, &vault.downloads_dir())
    }

    /// The file's name, for questions about it.
    pub fn file_name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }
}

/// What to do with an [`Outdated`] wad.
#[derive(Clone, Debug, PartialEq)]
pub enum Choice {
    /// Put the host's in its place, keeping the old file as `.bak`.
    Replace,
    /// Rename it to this file name, in the same dir, and put the host's where it was.
    Rename(String),
    /// Leave it be; the host's is downloaded on its own, as for any wad that isn't here.
    Skip,
}

/// The files here with the name of one of the host's wads that `local` has no copy of. Each
/// differs from the host's, since any of the right size was hashed while looking for a copy.
pub fn outdated(vault: &mut Vault, manifest: &Manifest, local: &[Option<Wad>]) -> Vec<Outdated> {
    let mut found = Vec::new();
    for (entry_index, (entry, wad)) in manifest.entries.iter().zip(local).enumerate() {
        if wad.is_some() {
            continue;
        }
        for path in vault.lookup().named(&entry.name) {
            found.push(Outdated {
                entry: entry_index,
                volatile: is_volatile(vault, &path),
                path,
            });
        }
    }
    found
}

/// Whether any profile has the file at `path` as a volatile wad.
fn is_volatile(vault: &Vault, path: &Path) -> bool {
    vault
        .profiles
        .iter()
        .flat_map(|profile| &profile.wads)
        .any(|wad| wad.volatile && Path::new(&wad.path) == path)
}

/// The manifest's wads that `choices` need downloaded, by their place in it, each once, in order.
pub fn wanted(outdated: &[Outdated], choices: &[Choice]) -> Vec<usize> {
    let mut wanted: Vec<_> = outdated
        .iter()
        .zip(choices)
        .filter(|(_, choice)| **choice != Choice::Skip)
        .map(|(outdated, _)| outdated.entry)
        .collect();
    wanted.sort_unstable();
    wanted.dedup();
    wanted
}

/// What [`settle`] came to.
#[derive(Debug, Default)]
pub struct Settled {
    /// For each of the host's wads, its copy here now, if one was put in place.
    pub placed: Vec<Option<Wad>>,
    /// What went wrong, one line each, for the user.
    pub problems: Vec<String>,
}

/// Does each of `choices` with its [`Outdated`] wad, given the host's wads `downloaded` (paired
/// with their place in the manifest of `entries` wads). A download no profile ends up using is
/// deleted again afterwards, since the host's is now where the old one was.
pub fn settle(
    vault: &mut Vault,
    entries: usize,
    outdated: &[Outdated],
    choices: &[Choice],
    downloaded: &[(usize, Wad)],
) -> Settled {
    let mut settled = Settled {
        placed: vec![None; entries],
        problems: Vec::new(),
    };
    for (outdated, choice) in outdated.iter().zip(choices) {
        let Some((_, theirs)) = downloaded
            .iter()
            .find(|(entry, _)| *entry == outdated.entry)
        else {
            continue;
        };
        match apply(vault, outdated, theirs, choice) {
            Ok(Some(placed)) => {
                settled.placed[outdated.entry].get_or_insert(placed);
            }
            Ok(None) => {}
            Err(error) => settled.problems.push(format!(
                "Couldn't bring {} up to the host's: {error}",
                outdated.path.display()
            )),
        }
    }
    let downloads = vault.downloads_dir();
    for (_, theirs) in downloaded {
        let path = Path::new(&theirs.path);
        let used = vault
            .profiles
            .iter()
            .flat_map(|profile| &profile.wads)
            .any(|wad| Path::new(&wad.path) == path);
        if !used {
            let _ = shelf::delete(path, &downloads);
        }
    }
    settled
}

/// Does `choice` with the file at `outdated.path`, given the host's version of it, `theirs`,
/// downloaded. Every wad that points at the file keeps up: replaced, it becomes the host's
/// (staying volatile or not); renamed, it follows the old file to its new name. Returns the
/// host's wad where it ended up, or `None` if skipped.
pub fn apply(
    vault: &mut Vault,
    outdated: &Outdated,
    theirs: &Wad,
    choice: &Choice,
) -> Result<Option<Wad>> {
    let path = &outdated.path;
    match choice {
        Choice::Skip => return Ok(None),
        Choice::Replace if outdated.is_download(vault) => {
            // 📦 a download is kept under its hash, so the host's stays under its own
            vault.upgrade_wad(path, theirs)?;
            return Ok(Some(theirs.clone()));
        }
        Choice::Replace => {
            let backup = backup(path);
            // 🪟 renaming onto a file fails on Windows, so the last backup goes first
            let _ = fs::remove_file(&backup);
            fs::rename(path, &backup)?;
            if let Err(error) = shelf::copy(Path::new(&theirs.path), path) {
                let _ = fs::rename(&backup, path);
                return Err(error.into());
            }
        }
        Choice::Rename(name) => {
            let renamed = path.with_file_name(name);
            if renamed.exists() {
                return Err(io::Error::from(io::ErrorKind::AlreadyExists).into());
            }
            fs::rename(path, &renamed)?;
            let followed = vault.follow_wad(path, &renamed);
            if let Err(error) = shelf::copy(Path::new(&theirs.path), path) {
                if fs::rename(&renamed, path).is_ok() {
                    let _ = vault.follow_wad(&renamed, path);
                }
                return Err(error.into());
            }
            followed?;
        }
    }
    let placed = Wad {
        modified: wad::modified(&fs::metadata(path)?),
        ..Wad::new(path.as_os_str().into(), theirs.hash.clone(), theirs.size)
    };
    if *choice == Choice::Replace {
        vault.upgrade_wad(path, &placed)?;
    }
    Ok(Some(placed))
}

/// Where a replaced wad's old file is kept: beside it, with `.bak` on the end.
pub fn backup(path: &Path) -> PathBuf {
    let mut backup = OsString::from(path.as_os_str());
    backup.push(".bak");
    PathBuf::from(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        net::Entry,
        vault::{profile::Profile, scribe::tests::Altar},
    };

    /// A vault in `altar` with one profile, `p`, holding the wad at `path` (volatile or not).
    fn vault_with(altar: &Altar, path: &Path, volatile: bool) -> Vault {
        let mut vault = Vault::new(altar.file("config"), altar.file("data"));
        let wad = Wad {
            volatile,
            ..Wad::from_path(path).unwrap()
        };
        vault
            .insert(&mut 0, Profile::new("p".into(), vec![wad]))
            .unwrap();
        vault
    }

    /// The host's version of a wad, downloaded under its hash in `vault`'s downloads.
    fn downloaded(vault: &Vault, name: &str, contents: &[u8]) -> Wad {
        let hash = blake3::hash(contents).to_hex().to_string();
        let dir = vault.downloads_dir().join(&hash);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), contents).unwrap();
        Wad::from_path(&dir.join(name)).unwrap()
    }

    fn manifest_of(theirs: &Wad) -> Manifest {
        Manifest {
            entries: vec![Entry {
                name: theirs.file_name().into_owned(),
                size: theirs.size,
                hash: theirs.hash.clone(),
            }],
            base_wad_hash: String::new(),
        }
    }

    #[test]
    fn a_file_of_the_hosts_name_with_other_contents_is_outdated() {
        let altar = Altar::new("upgrade-outdated");
        let mine = altar.file("wads/brutal.pk3");
        fs::create_dir_all(mine.parent().unwrap()).unwrap();
        fs::write(&mine, b"old").unwrap();
        let mut vault = vault_with(&altar, &mine, true);
        let theirs = downloaded(&vault, "brutal.pk3", b"newer");
        let manifest = manifest_of(&theirs);
        assert_eq!(
            outdated(&mut vault, &manifest, &[None]),
            [Outdated {
                entry: 0,
                path: mine,
                volatile: true,
            }]
        );
        // 🟰 a wad that is already here has nothing to bring up to date
        assert!(outdated(&mut vault, &manifest, &[Some(theirs)]).is_empty());
    }

    #[test]
    fn replacing_keeps_the_old_file_as_bak_and_every_profile_takes_the_hosts() {
        let altar = Altar::new("upgrade-replace");
        let mine = altar.file("brutal.pk3");
        fs::write(&mine, b"old").unwrap();
        let mut vault = vault_with(&altar, &mine, true);
        let theirs = downloaded(&vault, "brutal.pk3", b"newer");
        let outdated = outdated(&mut vault, &manifest_of(&theirs), &[None]);
        let settled = settle(
            &mut vault,
            1,
            &outdated,
            &[Choice::Replace],
            &[(0, theirs.clone())],
        );
        assert!(settled.problems.is_empty());
        assert_eq!(fs::read(&mine).unwrap(), b"newer");
        assert_eq!(fs::read(backup(&mine)).unwrap(), b"old");
        let wad = &vault.profiles[0].wads[0];
        assert_eq!(
            (wad.hash.as_str(), wad.volatile),
            (theirs.hash.as_str(), true)
        );
        assert_eq!(
            settled.placed,
            [Some(Wad {
                volatile: false,
                ..wad.clone()
            })]
        );
        // 🧹 the download did its job and isn't used by anything
        assert!(!Path::new(&theirs.path).exists());
        // ... and the profile is saved that way
        let saved = Vault::new(altar.file("config"), altar.file("data"));
        assert_eq!(saved.profiles[0].wads[0].hash, theirs.hash);
    }

    #[test]
    fn renaming_keeps_the_old_file_under_its_new_name_for_the_profiles_using_it() {
        let altar = Altar::new("upgrade-rename");
        let mine = altar.file("brutal.pk3");
        fs::write(&mine, b"old").unwrap();
        let mut vault = vault_with(&altar, &mine, false);
        let old_hash = vault.profiles[0].wads[0].hash.clone();
        let theirs = downloaded(&vault, "brutal.pk3", b"newer");
        let outdated = outdated(&mut vault, &manifest_of(&theirs), &[None]);
        let settled = settle(
            &mut vault,
            1,
            &outdated,
            &[Choice::Rename("brutal_old.pk3".into())],
            &[(0, theirs.clone())],
        );
        assert!(settled.problems.is_empty());
        assert_eq!(fs::read(&mine).unwrap(), b"newer");
        assert_eq!(fs::read(altar.file("brutal_old.pk3")).unwrap(), b"old");
        let wad = &vault.profiles[0].wads[0];
        assert_eq!(Path::new(&wad.path), altar.file("brutal_old.pk3"));
        assert_eq!(wad.hash, old_hash);
        let placed = settled.placed[0].as_ref().unwrap();
        assert_eq!(
            (Path::new(&placed.path), &placed.hash),
            (mine.as_path(), &theirs.hash)
        );
    }

    #[test]
    fn a_rename_onto_a_file_already_there_is_refused_and_changes_nothing() {
        let altar = Altar::new("upgrade-rename-taken");
        let mine = altar.file("brutal.pk3");
        fs::write(&mine, b"old").unwrap();
        fs::write(altar.file("taken.pk3"), b"taken").unwrap();
        let mut vault = vault_with(&altar, &mine, false);
        let theirs = downloaded(&vault, "brutal.pk3", b"newer");
        let outdated = outdated(&mut vault, &manifest_of(&theirs), &[None]);
        let settled = settle(
            &mut vault,
            1,
            &outdated,
            &[Choice::Rename("taken.pk3".into())],
            &[(0, theirs)],
        );
        assert_eq!(settled.problems.len(), 1);
        assert_eq!(settled.placed, [None]);
        assert_eq!(fs::read(&mine).unwrap(), b"old");
        assert_eq!(fs::read(altar.file("taken.pk3")).unwrap(), b"taken");
    }

    #[test]
    fn replacing_a_download_points_its_profiles_at_the_hosts_and_keeps_the_old_one() {
        let altar = Altar::new("upgrade-download");
        let probe = Vault::new(altar.file("config"), altar.file("data"));
        let old = downloaded(&probe, "brutal.pk3", b"old");
        let mut vault = vault_with(&altar, Path::new(&old.path), false);
        let theirs = downloaded(&vault, "brutal.pk3", b"newer");
        // only the old download is in a dir any profile uses, so only it is outdated
        let outdated = outdated(&mut vault, &manifest_of(&theirs), &[None]);
        assert_eq!(outdated.len(), 1);
        assert!(outdated[0].is_download(&vault));
        let settled = settle(
            &mut vault,
            1,
            &outdated,
            &[Choice::Replace],
            &[(0, theirs.clone())],
        );
        assert!(settled.problems.is_empty());
        assert_eq!(vault.profiles[0].wads[0].path, theirs.path);
        assert!(Path::new(&old.path).exists() && Path::new(&theirs.path).exists());
    }

    #[test]
    fn skipping_downloads_nothing_and_touches_nothing() {
        let outdated = [Outdated {
            entry: 0,
            path: "brutal.pk3".into(),
            volatile: false,
        }];
        assert!(wanted(&outdated, &[Choice::Skip]).is_empty());
        assert_eq!(wanted(&outdated, &[Choice::Replace]), [0]);
    }
}
