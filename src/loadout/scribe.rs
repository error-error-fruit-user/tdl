//! Every file this app keeps goes through the scribe, who never strikes out the old page until
//! the new one is fully dry.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// The extension of every file the app writes: a profile is a rite, and so is the config.
pub const EXTENSION: &str = "rite";

/// `brutal.rite` becomes `brutal.rite.tmp`, `brutal.rite.bak`, and so on.
pub fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    name.into()
}

/// Replaces `path` with `bytes` without ever leaving a half-written file behind.
///
/// The new contents are written and synced to a `.tmp` beside the file, the file being replaced
/// is copied to `.bak`, and only then is the `.tmp` renamed over it. A rename within a dir is
/// atomic, so a crash leaves either the old file or the new one, never a torn page.
///
/// The directory itself is not synced: that is Unix-only, and a launcher's config isn't worth it.
pub fn inscribe(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let ink = suffixed(path, ".tmp");
    let result = (|| {
        let mut scroll = fs::File::create(&ink)?;
        scroll.write_all(bytes)?;
        scroll.sync_all()?;
        // 🗝️ the backup is the file as it was
        if path.exists() {
            fs::copy(path, suffixed(path, ".bak"))?;
        }
        fs::rename(&ink, path)
    })();
    // 🕯️ don't leave candle wax on the floor
    if result.is_err() {
        let _ = fs::remove_file(ink);
    }
    result
}

/// Deletes what an upgrade left behind: the `.bak` copy of the file as it was. The current file is
/// never touched.
pub fn retire(path: &Path) {
    let _ = fs::remove_file(suffixed(path, ".bak"));
}

/// Destroys the file and every trace of it: the backup and a stray `.tmp`.
pub fn burn(path: &Path) {
    for ashes in [
        path.to_path_buf(),
        suffixed(path, ".bak"),
        suffixed(path, ".tmp"),
    ] {
        let _ = fs::remove_file(ashes);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A fresh empty dir per test, removed again when dropped.
    pub struct Altar(pub PathBuf);

    impl Altar {
        pub fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("tdl-scribe-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        pub fn file(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Altar {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn inscribing_creates_the_file_and_leaves_no_tmp() {
        let altar = Altar::new("create");
        let path = altar.file("a.rite");
        inscribe(&path, b"first").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first");
        assert!(!altar.file("a.rite.tmp").exists());
        assert!(!altar.file("a.rite.bak").exists());
    }

    #[test]
    fn inscribing_again_keeps_the_previous_version_as_the_backup() {
        let altar = Altar::new("backup");
        let path = altar.file("a.rite");
        inscribe(&path, b"first").unwrap();
        inscribe(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(fs::read(altar.file("a.rite.bak")).unwrap(), b"first");
    }

    #[test]
    fn a_stray_tmp_from_a_crash_does_not_get_in_the_way() {
        let altar = Altar::new("stray");
        let path = altar.file("a.rite");
        fs::write(altar.file("a.rite.tmp"), b"half a pa").unwrap();
        inscribe(&path, b"whole").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"whole");
        assert!(!altar.file("a.rite.tmp").exists());
    }

    #[test]
    fn a_failed_inscription_keeps_the_old_file_and_cleans_up() {
        let altar = Altar::new("fail");
        let path = altar.file("a.rite");
        inscribe(&path, b"keep me").unwrap();
        // a dir where the backup should go makes the copy fail after the .tmp is written
        fs::remove_file(altar.file("a.rite.bak")).ok();
        fs::create_dir(altar.file("a.rite.bak")).unwrap();
        assert!(inscribe(&path, b"lose me").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"keep me");
        assert!(!altar.file("a.rite.tmp").exists());
    }

    #[test]
    fn retiring_deletes_the_backup_and_keeps_the_file() {
        let altar = Altar::new("retire");
        let path = altar.file("a.rite");
        inscribe(&path, b"old").unwrap();
        inscribe(&path, b"new").unwrap();
        retire(&path);
        assert!(!altar.file("a.rite.bak").exists());
        assert_eq!(fs::read(&path).unwrap(), b"new");
    }

    #[test]
    fn burning_removes_every_trace() {
        let altar = Altar::new("burn");
        let path = altar.file("a.rite");
        inscribe(&path, b"one").unwrap();
        inscribe(&path, b"two").unwrap();
        fs::write(altar.file("a.rite.tmp"), b"x").unwrap();
        burn(&path);
        assert_eq!(fs::read_dir(&altar.0).unwrap().count(), 0);
    }
}
