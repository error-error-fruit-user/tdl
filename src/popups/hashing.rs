//! Hashing wads behind a progress window. The window only shows once the wait is long enough to
//! notice, so adding a small wad looks the same as it always did.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use crate::{
    error::{Error, Result},
    popups::{
        cancel::CancelHandler,
        progress::{Kind, Progress},
    },
    ritual::Grimoire,
    tally::Tally,
    vault::wad::Wad,
};

/// Hashes the files at `paths`, in order. `None` if Esc was pressed, in which case none are kept.
pub fn wads(grimoire: &mut Grimoire, paths: &[PathBuf]) -> Result<Option<Vec<Wad>>> {
    watch(grimoire, paths.len(), |tally| {
        paths.iter().map(|path| Wad::hashed(path, tally)).collect()
    })
}

/// Hashes the file at `path`. `None` if Esc was pressed.
pub fn wad(grimoire: &mut Grimoire, path: &Path) -> Result<Option<Wad>> {
    watch(grimoire, 1, |tally| Wad::hashed(path, tally))
}

/// `wads` as their files are now, with the volatile ones hashed again (see [`Wad::refreshed`]).
/// `None` if Esc was pressed.
pub fn refresh(grimoire: &mut Grimoire, wads: &[Wad]) -> Result<Option<Vec<Wad>>> {
    let volatile = wads.iter().filter(|wad| wad.volatile).count();
    watch(grimoire, volatile, |tally| {
        wads.iter().map(|wad| wad.refreshed(tally)).collect()
    })
}

/// Runs `work` with a hashing window watching it, and turns Esc into `None`. `files` is how many
/// files `work` may hash, for the window's count.
pub fn watch<T>(
    grimoire: &mut Grimoire,
    files: usize,
    work: impl FnOnce(&mut Watched) -> Result<T>,
) -> Result<Option<T>> {
    watch_as(grimoire, Kind::Hashing, files, work)
}

/// [`watch`], in a window of another `kind`, for work that does more than hash.
pub fn watch_as<T>(
    grimoire: &mut Grimoire,
    kind: Kind,
    files: usize,
    work: impl FnOnce(&mut Watched) -> Result<T>,
) -> Result<Option<T>> {
    let progress = Progress::new(grimoire, kind, files as u64).with_footer(CancelHandler::HINT);
    match work(&mut Watched(progress)) {
        Ok(done) => Ok(Some(done)),
        Err(Error::Cancelled) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Shows the hashing in the window, and stops it after any chunk if Esc was pressed.
pub struct Watched<'a>(Progress<'a>);

impl Tally for Watched<'_> {
    fn count_file(&mut self, name: &OsStr, size: u64) -> Result<()> {
        self.0.count_file(name, size)?;
        CancelHandler::check()
    }

    fn count_bytes(&mut self, amount: u64) -> Result<()> {
        self.0.count_bytes(amount)?;
        CancelHandler::check()
    }
}
