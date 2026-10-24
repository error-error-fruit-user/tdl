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
    loadout::{
        Loadout, shelf,
        wad::{self, Wad},
    },
    net::Manifest,
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
    pub fn is_download(&self, loadout: &Loadout) -> bool {
        shelf::is_download(&self.path, &loadout.downloads_dir())
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
pub fn outdated(
    loadout: &mut Loadout,
    manifest: &Manifest,
    local: &[Option<Wad>],
) -> Vec<Outdated> {
    let mut found = Vec::new();
    for (entry_index, (entry, wad)) in manifest.entries.iter().zip(local).enumerate() {
        if wad.is_some() {
            continue;
        }
        for path in loadout.lookup().named(&entry.name) {
            found.push(Outdated {
                entry: entry_index,
                volatile: is_volatile(loadout, &path),
                path,
            });
        }
    }
    found
}

/// Whether any profile has the file at `path` as a volatile wad.
fn is_volatile(loadout: &Loadout, path: &Path) -> bool {
    loadout
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
    loadout: &mut Loadout,
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
        match apply(loadout, outdated, theirs, choice) {
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
    let downloads = loadout.downloads_dir();
    for (_, theirs) in downloaded {
        let path = Path::new(&theirs.path);
        let used = loadout
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
    loadout: &mut Loadout,
    outdated: &Outdated,
    theirs: &Wad,
    choice: &Choice,
) -> Result<Option<Wad>> {
    let path = &outdated.path;
    match choice {
        Choice::Skip => return Ok(None),
        Choice::Replace if outdated.is_download(loadout) => {
            // 📦 a download is kept under its hash, so the host's stays under its own
            loadout.upgrade_wad(path, theirs)?;
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
            let followed = loadout.follow_wad(path, &renamed);
            if let Err(error) = shelf::copy(Path::new(&theirs.path), path) {
                if fs::rename(&renamed, path).is_ok() {
                    let _ = loadout.follow_wad(&renamed, path);
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
        loadout.upgrade_wad(path, &placed)?;
    }
    Ok(Some(placed))
}

/// Where a replaced wad's old file is kept: beside it, with `.bak` on the end.
pub fn backup(path: &Path) -> PathBuf {
    let mut backup = OsString::from(path.as_os_str());
    backup.push(".bak");
    PathBuf::from(backup)
}
