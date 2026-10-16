//! Where the wads a host asks for might already be on this machine, so joining only downloads
//! what is really missing.
//!
//! Nothing here searches the disk. A wad is looked for in three places, each a direct join and a
//! `stat`: the wads profiles already use (by hash), the downloads folder (stored by hash), and the
//! host's file name in every folder any profile has a wad in.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    net::Entry,
    vault::{
        Vault,
        wad::{self, Wad},
    },
};

/// What this machine knows about where wads are. Built from the profiles only when it's needed
/// (see [`Vault::lookup`]), since most sessions never join a game.
pub struct Lookup {
    /// Every wad in a profile, or a base wad, by hash. Volatile wads are left out, since their
    /// stored hash is expected to be out of date.
    known: HashMap<String, Vec<Wad>>,
    /// Every folder a profile has a wad in, each once, in the order first seen.
    folders: Vec<PathBuf>,
    downloads_dir: PathBuf,
}

/// What the lookup can tell about one of the host's wads without reading any file.
#[derive(Debug, PartialEq)]
pub enum Local {
    /// A file that is this wad: one hashed before and unchanged since, or one in the downloads
    /// folder, which was checked against its hash when it was downloaded.
    Intact(Wad),
    /// Files that may be it, since they have the right size, but that have to be hashed to know.
    /// Empty if there is nothing to try.
    Maybe(Vec<PathBuf>),
}

impl Lookup {
    pub fn build(vault: &Vault) -> Self {
        let mut known: HashMap<String, Vec<Wad>> = HashMap::new();
        let mut folders = Vec::new();
        let wads = vault
            .profiles
            .iter()
            .flat_map(|profile| profile.wads.iter().chain(&profile.base_wad))
            .chain(&vault.config.base_wad);
        for wad in wads {
            if let Some(folder) = Path::new(&wad.path).parent()
                && !folders.iter().any(|known| known == folder)
            {
                folders.push(folder.to_path_buf());
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
            folders,
            downloads_dir: vault.downloads_dir(),
        }
    }

    /// Where `entry` might be. A known wad that has changed, or a file of the right name and size
    /// in a known folder, is only a [`Local::Maybe`], never trusted unread.
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

        for folder in &self.folders {
            let path = folder.join(&entry.name);
            if !maybe.contains(&path) && file_of_size(&path, entry.size).is_some() {
                maybe.push(path);
            }
        }
        // only the ones that could still be it are worth hashing
        maybe.retain(|path| file_of_size(path, entry.size).is_some());
        Local::Maybe(maybe)
    }
}

/// The file's metadata, if `path` is a file of exactly `size` bytes.
fn file_of_size(path: &Path, size: u64) -> Option<fs::Metadata> {
    fs::metadata(path)
        .ok()
        .filter(|metadata| metadata.is_file() && metadata.len() == size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::{profile::Profile, scribe::tests::Altar};

    fn entry_for(wad: &Wad) -> Entry {
        Entry {
            name: wad.file_name().into_owned(),
            size: wad.size,
            hash: wad.hash.clone(),
        }
    }

    /// A vault with one profile holding `wads`.
    fn vault_with(altar: &Altar, wads: Vec<Wad>) -> Vault {
        let mut vault = Vault::new(altar.file("config"), altar.file("data"));
        vault
            .insert(&mut 0, Profile::new("p".into(), wads))
            .unwrap();
        vault
    }

    #[test]
    fn a_known_unchanged_wad_is_intact() {
        let altar = Altar::new("lookup-known");
        fs::write(altar.file("a.wad"), b"aaaa").unwrap();
        let wad = Wad::from_path(&altar.file("a.wad")).unwrap();
        let vault = vault_with(&altar, vec![wad.clone()]);
        assert_eq!(
            Lookup::build(&vault).check(&entry_for(&wad)),
            Local::Intact(wad)
        );
    }

    #[test]
    fn a_known_wad_that_was_touched_has_to_be_hashed_again() {
        let altar = Altar::new("lookup-touched");
        fs::write(altar.file("a.wad"), b"aaaa").unwrap();
        let mut wad = Wad::from_path(&altar.file("a.wad")).unwrap();
        let entry = entry_for(&wad);
        // as if it had been written since, to the same size
        wad.modified -= 1;
        let vault = vault_with(&altar, vec![wad]);
        assert_eq!(
            Lookup::build(&vault).check(&entry),
            Local::Maybe(vec![altar.file("a.wad")])
        );
    }

    #[test]
    fn a_volatile_wad_is_never_trusted_by_its_hash() {
        let altar = Altar::new("lookup-volatile");
        fs::write(altar.file("a.wad"), b"aaaa").unwrap();
        let wad = Wad {
            volatile: true,
            ..Wad::from_path(&altar.file("a.wad")).unwrap()
        };
        let vault = vault_with(&altar, vec![wad.clone()]);
        // still found by name in its folder, but only to be hashed
        assert_eq!(
            Lookup::build(&vault).check(&entry_for(&wad)),
            Local::Maybe(vec![altar.file("a.wad")])
        );
    }

    #[test]
    fn a_wad_no_profile_uses_is_found_by_name_in_a_known_folder() {
        let altar = Altar::new("lookup-folder");
        fs::write(altar.file("used.wad"), b"used").unwrap();
        fs::write(altar.file("unused.wad"), b"unused").unwrap();
        fs::write(altar.file("wrong_size.wad"), b"x").unwrap();
        let vault = vault_with(
            &altar,
            vec![Wad::from_path(&altar.file("used.wad")).unwrap()],
        );
        let lookup = Lookup::build(&vault);
        let unused = Wad::from_path(&altar.file("unused.wad")).unwrap();
        assert_eq!(
            lookup.check(&entry_for(&unused)),
            Local::Maybe(vec![altar.file("unused.wad")])
        );
        // the size is checked before anything is read
        let wrong = Entry {
            name: "wrong_size.wad".into(),
            size: 99,
            hash: "ab".repeat(32),
        };
        assert_eq!(lookup.check(&wrong), Local::Maybe(vec![]));
    }

    #[test]
    fn a_downloaded_wad_is_intact() {
        let altar = Altar::new("lookup-downloads");
        let vault = vault_with(&altar, vec![]);
        let hash = blake3::hash(b"dl").to_hex().to_string();
        let folder = vault.downloads_dir().join(&hash);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("dl.wad"), b"dl").unwrap();
        let entry = Entry {
            name: "dl.wad".into(),
            size: 2,
            hash,
        };
        let Local::Intact(wad) = Lookup::build(&vault).check(&entry) else {
            panic!("the download should be found");
        };
        assert_eq!(wad.path, folder.join("dl.wad").into_os_string());
    }
}
