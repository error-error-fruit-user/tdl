//! Where the wads a host asks for might already be on this machine, so joining only downloads
//! what is really missing.
//!
//! Nothing here searches the disk. A wad is looked for in three places, each a direct join and a
//! `stat`: the wads profiles already use (by hash), the downloads dir (stored by hash), and the
//! host's file name in the session's wad dirs and every dir any profile has a wad in.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    loadout::{
        Loadout,
        wad::{self, Wad},
    },
    net::Entry,
};

/// What this machine knows about where wads are. Built from the profiles only when it's needed
/// (see [`Loadout::lookup`]), since most sessions never join a game.
pub struct Lookup {
    /// Every wad in a profile, or a base wad, by hash. Volatile wads are left out, since their
    /// stored hash is expected to be out of date.
    known: HashMap<String, Vec<Wad>>,
    /// The session's wad dirs, then every dir a profile has a wad in, each once.
    dirs: Vec<PathBuf>,
    downloads_dir: PathBuf,
}

/// What the lookup can tell about one of the host's wads without reading any file.
#[derive(Debug, PartialEq)]
pub enum Local {
    /// A file that is this wad: one hashed before and unchanged since, or one in the downloads
    /// dir, which was checked against its hash when it was downloaded.
    Intact(Wad),
    /// Files that may be it, since they have the right size, but that have to be hashed to know.
    /// Empty if there is nothing to try.
    Maybe(Vec<PathBuf>),
}

impl Lookup {
    pub fn build(loadout: &Loadout) -> Self {
        let mut known: HashMap<String, Vec<Wad>> = HashMap::new();
        // 📂 the dirs the user named come first: that's where they said their wads are
        let mut dirs = Vec::new();
        for dir in &loadout.wad_dirs {
            if !dirs.contains(dir) {
                dirs.push(dir.clone());
            }
        }
        let wads = loadout
            .profiles
            .iter()
            .flat_map(|profile| profile.wads.iter().chain(&profile.base_wad))
            .chain(&loadout.uac.base_wad)
            .chain(&loadout.uac.session_base_wad);
        for wad in wads {
            if let Some(dir) = Path::new(&wad.path).parent()
                && !dirs.iter().any(|known| known == dir)
            {
                dirs.push(dir.to_path_buf());
            }
            if !wad.volatile && !wad.hash.is_empty() {
                let copies = known.entry(wad.hash.clone()).or_default();
                if !copies.iter().any(|copy| copy.path == wad.path) {
                    copies.push(wad.clone());
                }
            }
        }
        Self {
            known,
            dirs,
            downloads_dir: loadout.downloads_dir(),
        }
    }

    /// Where `entry` might be. A known wad that has changed, or a file of the right name and size
    /// in a known dir, is only a [`Local::Maybe`], never trusted unread.
    pub fn check(&self, entry: &Entry) -> Local {
        let mut maybe = Vec::new();
        for wad in self.known.get(&entry.hash).into_iter().flatten() {
            if wad.size == entry.size && wad.is_unchanged() {
                return Local::Intact(wad.clone());
            }
            maybe.push(PathBuf::from(&wad.path));
        }

        let downloaded = self.downloads_dir.join(&entry.hash).join(&entry.name);
        if let Some(metadata) = file_of_size(&downloaded, entry.size) {
            return Local::Intact(Wad {
                modified: wad::modified(&metadata),
                ..Wad::new(downloaded.into(), entry.hash.clone(), entry.size)
            });
        }

        for dir in &self.dirs {
            let path = dir.join(&entry.name);
            if !maybe.contains(&path) && file_of_size(&path, entry.size).is_some() {
                maybe.push(path);
            }
        }
        // only the ones that could still be it are worth hashing
        maybe.retain(|path| file_of_size(path, entry.size).is_some());
        Local::Maybe(maybe)
    }

    /// Every file named `name` in the dirs looked in, whatever its size or contents: the wads a
    /// host's wad of that name may be a newer version of.
    pub fn named(&self, name: &str) -> Vec<PathBuf> {
        self.dirs
            .iter()
            .map(|dir| dir.join(name))
            .filter(|path| path.is_file())
            .collect()
    }
}

/// The file's metadata, if `path` is a file of exactly `size` bytes.
fn file_of_size(path: &Path, size: u64) -> Option<fs::Metadata> {
    fs::metadata(path)
        .ok()
        .filter(|metadata| metadata.is_file() && metadata.len() == size)
}
