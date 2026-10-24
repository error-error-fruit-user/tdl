use std::{io::Read, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    loadout::profile::Profile,
    loadout::wad::Wad,
    net::{Address, MAX_FILES, is_safe, new_agent},
};

/// What a host serves at `/manifest`: its wads in load order, identified by hash.
///
/// Files are then requested by hash at `/file/<hash>`. The manifest comes from an untrusted peer,
/// so [`Manifest::fetch`] validates it and downloads are verified against it.
#[derive(Deserialize, Serialize)]
pub struct Manifest {
    pub entries: Vec<Entry>,
    pub base_wad_hash: String,
}

impl Manifest {
    /// Describes `wads` for joiners. Fails if a wad can't be read, since the host couldn't serve it.
    pub fn build(wads: &[Wad], base_wad_hash: String) -> Result<Self> {
        let entries = wads
            .iter()
            .map(|w| {
                let path = Path::new(&w.path);
                // 🌍 text, so a joiner on another OS can read it; a name that isn't valid UTF-8
                // (rare) goes out with its odd bytes replaced, which only costs matching it by name
                let name = path
                    .file_name()
                    .ok_or(Error::Download("Invalid wad path"))?
                    .to_string_lossy()
                    .into_owned();
                let size = std::fs::metadata(path)?.len();
                let hash = w.hash.clone();
                Ok(Entry::new(name, size, hash))
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            entries,
            base_wad_hash,
        })
    }

    /// Downloads and validates the host's manifest. Rejects too many files and unsafe names or hashes.
    /// A host that turns the code down is [`Error::Refused`], so the user can be told why.
    pub fn fetch(address: &Address) -> Result<Self> {
        let response = match new_agent().get(&address.url("manifest")).call() {
            Err(ureq::Error::StatusCode(403)) => return Err(Error::Refused),
            other => other?,
        };
        let mut body = Vec::new();
        response.into_body().into_reader().read_to_end(&mut body)?;
        let manifest: Manifest = rmp_serde::from_slice(&body)?;
        if manifest.entries.len() > MAX_FILES {
            return Err(Error::Download("Too many files trying to be downloaded"));
        }
        if !manifest.entries.iter().all(Entry::is_safe) {
            return Err(Error::Download("Host sent an invalid file list"));
        }
        Ok(manifest)
    }

    /// An empty hash means the host has no base wad selected, so anything matches.
    pub fn base_wad_matches(&self, base_wad: Option<&Wad>) -> bool {
        self.base_wad_hash.is_empty() || base_wad.is_none_or(|w| w.hash == self.base_wad_hash)
    }

    /// Compatible means identical wads in identical order; paths don't matter. A volatile wad
    /// matches by file name instead of hash.
    pub fn is_compatible_with(&self, profile: &Profile) -> bool {
        self.entries.len() == profile.wads.len()
            && self
                .entries
                .iter()
                .zip(&profile.wads)
                .all(|(entry, wad)| entry.hash == wad.hash || entry.is_same_name_as_volatile(wad))
    }

    /// Where the profile's wads differ from the host's, by their place in the list. A compatible
    /// profile can only differ in volatile wads, which joining brings up to the host's version.
    pub fn differing_wads(&self, profile: &Profile) -> Vec<usize> {
        self.entries
            .iter()
            .zip(&profile.wads)
            .enumerate()
            .filter(|(_, (entry, wad))| entry.hash != wad.hash)
            .map(|(index, _)| index)
            .collect()
    }
}

/// One wad in a [`Manifest`].
#[derive(Clone, Deserialize, Serialize)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub hash: String,
}

impl Entry {
    pub(super) fn new(name: String, size: u64, hash: String) -> Self {
        Self { name, size, hash }
    }

    /// A volatile wad stands in for the host's wad of the same name, whatever its hash.
    fn is_same_name_as_volatile(&self, wad: &Wad) -> bool {
        wad.volatile && wad.file_name() == self.name
    }

    // name and hash become path components when downloading, so a host must not choose them freely
    /// Whether `name` is a bare file name and `hash` a BLAKE3 hex digest.
    fn is_safe(&self) -> bool {
        is_safe(&self.name, &self.hash)
    }
}
