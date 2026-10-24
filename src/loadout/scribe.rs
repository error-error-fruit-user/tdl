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
