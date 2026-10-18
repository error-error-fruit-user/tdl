use std::{io::Read, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    net::{Address, MAX_FILES, is_safe, new_agent},
    vault::profile::Profile,
    vault::wad::Wad,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, hash: &str) -> Entry {
        Entry::new(name.into(), 1, hash.into())
    }

    fn manifest(entries: Vec<Entry>) -> Manifest {
        Manifest {
            entries,
            base_wad_hash: String::new(),
        }
    }

    fn wad(path: &str, hash: &str, volatile: bool) -> Wad {
        Wad {
            volatile,
            ..Wad::new(path.into(), hash.into(), 1)
        }
    }

    fn profile(wads: Vec<Wad>) -> Profile {
        Profile::new("p".into(), wads)
    }

    #[test]
    fn identical_wads_in_order_are_compatible() {
        let host = manifest(vec![entry("a.pk3", "h1"), entry("b.wad", "h2")]);
        let mine = profile(vec![
            wad("/x/a.pk3", "h1", false),
            wad("/y/b.wad", "h2", false),
        ]);
        assert!(host.is_compatible_with(&mine));
        assert!(host.differing_wads(&mine).is_empty());
    }

    #[test]
    fn a_different_hash_is_incompatible_unless_the_wad_is_volatile() {
        let host = manifest(vec![entry("a.pk3", "h1")]);
        assert!(!host.is_compatible_with(&profile(vec![wad("/x/a.pk3", "other", false)])));

        let volatile = profile(vec![wad("/x/a.pk3", "other", true)]);
        assert!(host.is_compatible_with(&volatile));
        // ... and is brought up to the host's when joining
        assert_eq!(host.differing_wads(&volatile), [0]);
    }

    #[test]
    fn a_volatile_wad_only_stands_in_for_a_wad_of_the_same_name() {
        let host = manifest(vec![entry("a.pk3", "h1")]);
        let imposter = profile(vec![wad("/x/b.pk3", "other", true)]);
        assert!(!host.is_compatible_with(&imposter));
    }

    #[test]
    fn the_number_of_wads_must_still_match() {
        let host = manifest(vec![entry("a.pk3", "h1"), entry("b.wad", "h2")]);
        assert!(!host.is_compatible_with(&profile(vec![wad("/x/a.pk3", "h1", true)])));
    }

    #[test]
    fn a_matching_volatile_wad_is_not_reported_as_differing() {
        let host = manifest(vec![entry("a.pk3", "h1")]);
        assert!(
            host.differing_wads(&profile(vec![wad("/x/a.pk3", "h1", true)]))
                .is_empty()
        );
    }

    // 🧱 the base wad: the host's hash against the joiner's own

    fn hosting_base_wad(hash: &str) -> Manifest {
        Manifest {
            base_wad_hash: hash.into(),
            ..manifest(Vec::new())
        }
    }

    #[test]
    fn a_host_with_no_base_wad_matches_any_base_wad_or_none() {
        let host = hosting_base_wad("");
        assert!(host.base_wad_matches(None));
        assert!(host.base_wad_matches(Some(&wad("/iwads/DOOM2.WAD", "h1", false))));
    }

    #[test]
    fn the_same_base_wad_matches_wherever_it_is() {
        let host = hosting_base_wad("h1");
        assert!(host.base_wad_matches(Some(&wad("/somewhere/else/doom2.wad", "h1", false))));
    }

    #[test]
    fn a_different_base_wad_does_not_match() {
        let host = hosting_base_wad("h1");
        assert!(!host.base_wad_matches(Some(&wad("/iwads/DOOM2.WAD", "h2", false))));
        // being volatile is no excuse: only profile wads stand in by name
        assert!(!host.base_wad_matches(Some(&wad("/iwads/DOOM2.WAD", "h2", true))));
    }

    #[test]
    fn a_joiner_with_no_base_wad_of_their_own_is_let_through() {
        // UZDoom then picks its own, which is the joiner's problem rather than a mismatch
        assert!(hosting_base_wad("h1").base_wad_matches(None));
    }
}
