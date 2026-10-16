use std::{
    ffi::{OsStr, OsString},
    fmt, fs, io,
    ops::Deref,
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{error::Result, vault::hash};

/// A wad or pk3 file, identified across machines by the BLAKE3 hash of its contents.
///
/// `Deref`s to its path so it can be passed straight to a command line.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Wad {
    pub path: OsString,
    pub hash: String,
    /// The size in bytes when the wad was added, to notice when the file has since changed.
    /// 0 means unknown, as for profiles saved before this was recorded.
    #[serde(default)]
    pub size: u64,
    /// For mods still being worked on, whose file changes all the time. A volatile wad's stored
    /// hash and size are not trusted: its changes aren't flagged, the hash is taken fresh when
    /// hosting, and a hash mismatch when joining is a question instead of a dead end.
    #[serde(default)]
    pub volatile: bool,
}

impl Wad {
    /// File extensions the pickers accept.
    pub const EXTENSIONS: &[&str] = &["wad", "WAD", "pk3"];

    pub fn new(path: OsString, hash: String, size: u64) -> Self {
        Self {
            path,
            hash,
            size,
            volatile: false,
        }
    }

    /// Hashes the file at `path`, which can be slow for large wads.
    pub fn from_path(path: &Path) -> Result<Self> {
        let size = fs::metadata(path)?.len();
        Ok(Self::new(path.as_os_str().into(), hash::hash(path)?, size))
    }

    /// The file's name for display, or the whole path if it has no file name (`..`, empty).
    pub fn file_name(&self) -> std::borrow::Cow<'_, str> {
        Path::new(&self.path)
            .file_name()
            .unwrap_or(&self.path)
            .to_string_lossy()
    }

    /// What is wrong with the file this wad points at, if anything. One look at the disk, no hashing.
    /// A volatile wad is expected to change, so its size is not compared.
    pub fn check(&self) -> Option<Fault> {
        Fault::of(
            Path::new(&self.path),
            if self.volatile { 0 } else { self.size },
        )
    }

    /// This wad as the file is right now: a volatile wad is hashed again, anything else is
    /// trusted as stored. Used when hosting, so the manifest describes the bytes that get served.
    pub fn refreshed(&self) -> Result<Self> {
        if !self.volatile {
            return Ok(self.clone());
        }
        Ok(Self {
            volatile: true,
            ..Self::from_path(Path::new(&self.path))?
        })
    }
}

impl Deref for Wad {
    type Target = OsStr;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

/// What is wrong with a file that a profile points at.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Fault {
    /// There is nothing at that path, usually because the file was moved or renamed.
    Missing,
    /// The path leads to a folder, or something else that isn't a regular file.
    NotAFile,
    /// It exists but can't be opened, with the reason from the OS.
    Unreadable(String),
    /// It is a different size than when it was added, so the stored hash may be stale.
    Changed { expected: u64, found: u64 },
}

impl Fault {
    /// Looks at the file at `path`. `expected` is the size it should have, or 0 if unknown.
    pub fn of(path: &Path, expected: u64) -> Option<Self> {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Some(Self::Missing),
            Err(error) => return Some(Self::Unreadable(error.to_string())),
        };
        if !metadata.is_file() {
            return Some(Self::NotAFile);
        }
        // 🔦 a file can exist and still refuse to be opened, so try
        if let Err(error) = fs::File::open(path) {
            return Some(Self::Unreadable(error.to_string()));
        }
        let found = metadata.len();
        (expected != 0 && expected != found).then_some(Self::Changed { expected, found })
    }

    /// One or two words for tight spaces, like the launcher's status panel.
    pub fn short(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::NotAFile => "not a file",
            Self::Unreadable(_) => "unreadable",
            Self::Changed { .. } => "changed",
        }
    }

    /// Whether the file can't be used at all. A changed file still runs, so it is only marked.
    pub fn is_blocking(&self) -> bool {
        !matches!(self, Self::Changed { .. })
    }
}

/// Reads after a subject: "UZDoom was not found", "It is not a file".
impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Missing => write!(f, "was not found"),
            Self::NotAFile => write!(f, "is not a file"),
            Self::Unreadable(reason) => write!(f, "can't be read ({reason})"),
            Self::Changed { expected, found } => {
                write!(f, "changed size (was {expected} bytes, now {found})")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::scribe::tests::Altar;

    #[test]
    fn a_healthy_file_has_no_fault() {
        let altar = Altar::new("wad-healthy");
        fs::write(altar.file("a.wad"), b"1234").unwrap();
        assert_eq!(Fault::of(&altar.file("a.wad"), 4), None);
    }

    #[test]
    fn an_unknown_size_never_counts_as_changed() {
        let altar = Altar::new("wad-unknown-size");
        fs::write(altar.file("a.wad"), b"1234").unwrap();
        assert_eq!(Fault::of(&altar.file("a.wad"), 0), None);
    }

    #[test]
    fn a_different_size_is_changed_but_not_blocking() {
        let altar = Altar::new("wad-changed");
        fs::write(altar.file("a.wad"), b"1234").unwrap();
        let fault = Fault::of(&altar.file("a.wad"), 9).unwrap();
        assert_eq!(
            fault,
            Fault::Changed {
                expected: 9,
                found: 4
            }
        );
        assert!(!fault.is_blocking());
    }

    #[test]
    fn a_missing_file_is_missing_and_blocking() {
        let altar = Altar::new("wad-missing");
        let fault = Fault::of(&altar.file("gone.wad"), 4).unwrap();
        assert_eq!(fault, Fault::Missing);
        assert!(fault.is_blocking());
    }

    #[test]
    fn a_folder_is_not_a_file() {
        let altar = Altar::new("wad-folder");
        fs::create_dir(altar.file("a.wad")).unwrap();
        assert_eq!(Fault::of(&altar.file("a.wad"), 0), Some(Fault::NotAFile));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_cannot_be_opened_is_unreadable() {
        use std::os::unix::fs::PermissionsExt;
        let altar = Altar::new("wad-unreadable");
        let path = altar.file("a.wad");
        fs::write(&path, b"1234").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        // root can open anything, so there is nothing to check there
        if fs::File::open(&path).is_err() {
            assert!(matches!(Fault::of(&path, 4), Some(Fault::Unreadable(_))));
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[test]
    fn faults_read_after_a_subject() {
        assert_eq!(format!("UZDoom {}", Fault::Missing), "UZDoom was not found");
        assert_eq!(format!("It {}", Fault::NotAFile), "It is not a file");
    }

    #[test]
    fn from_path_records_the_size() {
        let altar = Altar::new("wad-from-path");
        fs::write(altar.file("a.wad"), b"12345").unwrap();
        let wad = Wad::from_path(&altar.file("a.wad")).unwrap();
        assert_eq!(wad.size, 5);
        assert_eq!(wad.check(), None);
    }

    #[test]
    fn a_volatile_wad_is_not_flagged_when_it_changes_but_still_when_it_goes_missing() {
        let altar = Altar::new("wad-volatile");
        fs::write(altar.file("a.wad"), b"1234").unwrap();
        let mut wad = Wad::from_path(&altar.file("a.wad")).unwrap();
        fs::write(altar.file("a.wad"), b"1234-grown").unwrap();
        assert!(matches!(wad.check(), Some(Fault::Changed { .. })));
        wad.volatile = true;
        assert_eq!(wad.check(), None);
        fs::remove_file(altar.file("a.wad")).unwrap();
        assert_eq!(wad.check(), Some(Fault::Missing));
    }

    #[test]
    fn refreshing_rehashes_only_volatile_wads() {
        let altar = Altar::new("wad-refreshed");
        fs::write(altar.file("a.wad"), b"1234").unwrap();
        let mut wad = Wad::from_path(&altar.file("a.wad")).unwrap();
        let before = wad.hash.clone();
        fs::write(altar.file("a.wad"), b"1234-grown").unwrap();

        // not volatile: trusted as stored, even though it is stale
        assert_eq!(wad.refreshed().unwrap(), wad);

        wad.volatile = true;
        let fresh = wad.refreshed().unwrap();
        assert_ne!(fresh.hash, before);
        assert_eq!(fresh.size, 10);
        assert!(fresh.volatile);
    }

    #[test]
    fn a_wad_saved_before_sizes_existed_reads_as_unknown_size() {
        #[derive(Serialize)]
        struct Old {
            path: OsString,
            hash: String,
        }
        let bytes = rmp_serde::to_vec_named(&Old {
            path: "x.wad".into(),
            hash: "ab".into(),
        })
        .unwrap();
        let wad: Wad = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(wad.size, 0);
    }
}
