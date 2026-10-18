use std::{
    ffi::{OsStr, OsString},
    fmt,
    fs::{self, Metadata},
    io,
    ops::Deref,
    path::Path,
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};

use crate::{
    error::Result,
    loadout::{
        hash,
        saved::{SavedPath, SavedWad},
    },
    tally::Tally,
};

/// A wad or pk3 file, identified across machines by the BLAKE3 hash of its contents.
///
/// `Deref`s to its path so it can be passed straight to a command line. Saved as a
/// [`SavedWad`], so the path reads back on any OS.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(from = "SavedWad", into = "SavedWad")]
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
    /// When the file was last modified as of hashing, in nanoseconds since 1970. Together with the
    /// size it says whether the file can still be trusted to have its hash without reading it
    /// again. 0 means unknown. Only means anything on the machine that recorded it.
    pub modified: u64,
}

impl From<SavedWad> for Wad {
    fn from(saved: SavedWad) -> Self {
        Self {
            path: saved.path.into(),
            hash: saved.hash,
            size: saved.size,
            volatile: saved.volatile,
            modified: saved.modified,
        }
    }
}

impl From<Wad> for SavedWad {
    fn from(wad: Wad) -> Self {
        Self {
            path: SavedPath::from(wad.path.as_os_str()),
            hash: wad.hash,
            size: wad.size,
            volatile: wad.volatile,
            modified: wad.modified,
        }
    }
}

/// When a file was last modified, in nanoseconds since 1970, or 0 if the OS can't say.
pub fn modified(metadata: &Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |time| time.as_nanos() as u64)
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
            modified: 0,
        }
    }

    /// Hashes the file at `path`, which can be slow for large wads, so `tally` is told the file's
    /// size and then how far the hashing has got.
    ///
    /// The modified time is taken before reading, so a file that changes while it is hashed looks
    /// changed afterwards rather than trusted.
    pub fn hashed(path: &Path, tally: &mut impl Tally) -> Result<Self> {
        let metadata = fs::metadata(path)?;
        let size = metadata.len();
        tally.count_file(path.file_name().unwrap_or(path.as_os_str()), size)?;
        Ok(Self {
            modified: modified(&metadata),
            ..Self::new(path.as_os_str().into(), hash::hash(path, tally)?, size)
        })
    }

    /// Whether the file is still the one that was hashed, judged without reading it: there, the
    /// same size, and modified at the same moment. A wad whose modified time was never recorded is
    /// never trusted this way.
    pub fn is_unchanged(&self) -> bool {
        self.modified != 0
            && fs::metadata(&self.path).is_ok_and(|metadata| {
                metadata.is_file()
                    && metadata.len() == self.size
                    && modified(&metadata) == self.modified
            })
    }

    /// Hashes the file at `path` with nobody watching.
    #[cfg(test)]
    pub fn from_path(path: &Path) -> Result<Self> {
        Self::hashed(path, &mut ())
    }

    /// Points the wad at `to` if its file was at `from`, which a move on the wads screen does.
    /// Returns whether it was.
    pub fn follow(&mut self, from: &Path, to: &Path) -> bool {
        if Path::new(&self.path) != from {
            return false;
        }
        self.path = to.as_os_str().to_os_string();
        true
    }

    /// The file's name for display, or the whole path if it has no file name (`..`, empty).
    pub fn file_name(&self) -> std::borrow::Cow<'_, str> {
        Path::new(&self.path)
            .file_name()
            .unwrap_or(&self.path)
            .to_string_lossy()
    }

    /// This wad with its size filled in from the file as it is now, if the size was never recorded
    /// (0) and the file is there. Nothing is hashed, and the modified time stays unknown, so the
    /// file is still never trusted without being read. Changes made before this point can't be
    /// seen, but from now on a change in size is noticed.
    pub fn with_size_backfilled(self) -> Self {
        if self.size != 0 {
            return self;
        }
        match fs::metadata(&self.path) {
            Ok(metadata) if metadata.is_file() => Self {
                size: metadata.len(),
                ..self
            },
            _ => self,
        }
    }

    /// Copies the file to `path`, which must not exist yet, and returns the copy as a wad of its
    /// own with the same hash and settings, for pasting a wad whose name the profile already has.
    /// The copy is only trusted without being read again (see [`Wad::is_unchanged`]) if the
    /// original still could be, since otherwise its hash may already have been stale.
    pub fn copied_to(&self, path: &Path) -> io::Result<Self> {
        let trusted = self.is_unchanged();
        // 🛑 never write over a file that is already there
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        if let Err(error) = fs::copy(&self.path, path) {
            let _ = fs::remove_file(path);
            return Err(error);
        }
        let metadata = fs::metadata(path)?;
        Ok(Self {
            path: path.as_os_str().into(),
            modified: if trusted { modified(&metadata) } else { 0 },
            ..self.clone()
        })
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
    /// `tally` only hears about the wads that are hashed.
    pub fn refreshed(&self, tally: &mut impl Tally) -> Result<Self> {
        if !self.volatile {
            return Ok(self.clone());
        }
        Ok(Self {
            volatile: true,
            ..Self::hashed(Path::new(&self.path), tally)?
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
    /// The path leads to a dir, or something else that isn't a regular file.
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
    use crate::loadout::scribe::tests::Altar;

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
    fn a_dir_is_not_a_file() {
        let altar = Altar::new("wad-dir");
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
        assert_eq!(wad.refreshed(&mut ()).unwrap(), wad);

        wad.volatile = true;
        let fresh = wad.refreshed(&mut ()).unwrap();
        assert_ne!(fresh.hash, before);
        assert_eq!(fresh.size, 10);
        assert!(fresh.volatile);
    }

    #[test]
    fn backfilling_fills_in_only_an_unknown_size_of_a_file_that_is_there() {
        let altar = Altar::new("wad-backfill");
        fs::write(altar.file("a.wad"), b"12345").unwrap();
        let unknown = Wad::new(altar.file("a.wad").into(), "ab".into(), 0);
        let backfilled = unknown.clone().with_size_backfilled();
        assert_eq!(backfilled.size, 5);
        assert_eq!(
            backfilled.modified, 0,
            "still not trusted without reading it"
        );
        // and now a change is noticed
        fs::write(altar.file("a.wad"), b"123456789").unwrap();
        assert!(matches!(backfilled.check(), Some(Fault::Changed { .. })));

        // a known size is left alone, even if the file has since changed
        let known = Wad::new(altar.file("a.wad").into(), "ab".into(), 3);
        assert_eq!(known.clone().with_size_backfilled(), known);
        // and so is a wad with nothing to measure
        let gone = Wad::new(altar.file("gone.wad").into(), "ab".into(), 0);
        assert_eq!(gone.clone().with_size_backfilled(), gone);
        fs::create_dir(altar.file("dir.wad")).unwrap();
        let dir = Wad::new(altar.file("dir.wad").into(), "ab".into(), 0);
        assert_eq!(dir.clone().with_size_backfilled(), dir);
    }

    #[test]
    fn a_copy_is_its_own_file_with_the_same_hash_and_settings() {
        let altar = Altar::new("wad-copied");
        fs::write(altar.file("brutal.pk3"), b"brutal bytes").unwrap();
        let mut original = Wad::from_path(&altar.file("brutal.pk3")).unwrap();
        original.volatile = true;
        let copy = original.copied_to(&altar.file("brutal_2.pk3")).unwrap();
        assert_eq!(copy.path, altar.file("brutal_2.pk3").into_os_string());
        assert_eq!(
            fs::read(altar.file("brutal_2.pk3")).unwrap(),
            b"brutal bytes"
        );
        assert_eq!(
            (copy.hash.as_str(), copy.size),
            (original.hash.as_str(), original.size)
        );
        assert!(copy.volatile);
        assert!(
            copy.is_unchanged(),
            "a faithful copy of a trusted file is trusted too"
        );
        assert_eq!(copy.check(), None);
        // a stale original passes on its doubt
        fs::write(altar.file("brutal.pk3"), b"changed!").unwrap();
        let doubtful = original.copied_to(&altar.file("brutal_3.pk3")).unwrap();
        assert_eq!(doubtful.modified, 0);
    }

    #[test]
    fn copying_never_writes_over_a_file_and_a_missing_one_leaves_nothing() {
        let altar = Altar::new("wad-copied-refused");
        fs::write(altar.file("a.wad"), b"a").unwrap();
        fs::write(altar.file("b.wad"), b"b").unwrap();
        let a = Wad::from_path(&altar.file("a.wad")).unwrap();
        assert!(a.copied_to(&altar.file("b.wad")).is_err());
        assert_eq!(fs::read(altar.file("b.wad")).unwrap(), b"b");
        fs::remove_file(altar.file("a.wad")).unwrap();
        assert!(a.copied_to(&altar.file("c.wad")).is_err());
        assert!(!altar.file("c.wad").exists());
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
