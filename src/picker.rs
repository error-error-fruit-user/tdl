//! The system's file dialogs, always opened in a dir that exists.
//!
//! The dialog opens in the preferred dir if there is one, else the first of the wad dirs
//! (from `--wads`, or kept from it), else the working directory if there are no wad dirs, else
//! the Downloads dir. If none of those can be found there is nowhere sensible to open it, so
//! the user is told and the pick is cancelled, instead of the program panicking.

use std::{
    path::{Path, PathBuf},
    sync::RwLock,
};

use directories::UserDirs;
use rfd::FileDialog;

use crate::{error::Result, popups::warning::Warning, ritual::Grimoire};

/// A named set of file extensions to show, like `("wads", &["wad", "pk3"])`.
pub type Filter<'a> = (&'a str, &'a [&'a str]);

/// The session's wad dirs, set once at startup. Kept here rather than passed to every picker,
/// since pickers are opened from all over and only ever need to read it.
static WAD_DIRS: RwLock<Vec<PathBuf>> = RwLock::new(Vec::new());

/// Makes `dirs` the places dialogs open before anything else, instead of the working directory.
pub fn set_wad_dirs(dirs: &[PathBuf]) {
    if let Ok(mut wad_dirs) = WAD_DIRS.write() {
        *wad_dirs = dirs.to_vec();
    }
}

/// Asks for one file. `None` if the user cancels, or if there is no dir to open the dialog in.
pub fn file(
    grimoire: &mut Grimoire,
    filter: Option<Filter>,
    start: Option<&Path>,
) -> Result<Option<PathBuf>> {
    if crate::script::playing() {
        return Ok(crate::script::pick()?.and_then(|paths| paths.into_iter().next()));
    }
    #[cfg(debug_assertions)]
    if let Some(picked) = stand_in() {
        return Ok(picked.and_then(|paths| paths.into_iter().next()));
    }
    Ok(dialog(grimoire, filter, start)?.and_then(|dialog| dialog.pick_file()))
}

/// Asks for any number of files. `None` if the user cancels, or if there is no dir to open
/// the dialog in.
pub fn files(
    grimoire: &mut Grimoire,
    filter: Option<Filter>,
    start: Option<&Path>,
) -> Result<Option<Vec<PathBuf>>> {
    // 📜 a script answers with its pick line
    if crate::script::playing() {
        return crate::script::pick();
    }
    #[cfg(debug_assertions)]
    if let Some(picked) = stand_in() {
        return Ok(picked);
    }
    Ok(dialog(grimoire, filter, start)?.and_then(|dialog| dialog.pick_files()))
}

/// Asks where to save a file, offering `file_name`. `None` if the user cancels, or if there is no
/// dir to open the dialog in.
pub fn save(
    grimoire: &mut Grimoire,
    filter: Option<Filter>,
    file_name: &str,
) -> Result<Option<PathBuf>> {
    if crate::script::playing() {
        return Ok(crate::script::pick()?.and_then(|paths| paths.into_iter().next()));
    }
    #[cfg(debug_assertions)]
    if let Some(picked) = stand_in() {
        return Ok(picked.and_then(|paths| paths.into_iter().next()));
    }
    Ok(dialog(grimoire, filter, None)?
        .and_then(|dialog| dialog.set_file_name(file_name).save_file()))
}

/// The session's wad dirs, as set by [`set_wad_dirs`].
pub fn wad_dirs() -> Vec<PathBuf> {
    WAD_DIRS.read().map(|dirs| dirs.clone()).unwrap_or_default()
}

/// Asks for a dir. `None` if the user cancels, or if there is no dir to open the dialog in.
pub fn dir(grimoire: &mut Grimoire, start: Option<&Path>) -> Result<Option<PathBuf>> {
    if crate::script::playing() {
        return Ok(crate::script::pick()?.and_then(|paths| paths.into_iter().next()));
    }
    #[cfg(debug_assertions)]
    if let Some(picked) = stand_in() {
        return Ok(picked.and_then(|paths| paths.into_iter().next()));
    }
    Ok(dialog(grimoire, None, start)?.and_then(|dialog| dialog.pick_folder()))
}

/// For automated runs of debug builds only: when `TDL_PICKS` names a file, each pick takes the
/// next line of that file instead of opening a dialog. A line holds the picked paths, separated
/// like `PATH` entries (`:` on Unix); an empty line is a cancelled dialog. Release builds don't
/// have this at all.
#[cfg(debug_assertions)]
fn stand_in() -> Option<Option<Vec<PathBuf>>> {
    let queue = std::env::var_os("TDL_PICKS")?;
    let text = std::fs::read_to_string(&queue).ok()?;
    let (line, rest) = text.split_once('\n').unwrap_or((&text, ""));
    std::fs::write(&queue, rest).ok()?;
    let paths: Vec<_> = std::env::split_paths(line)
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    Some((!paths.is_empty()).then_some(paths))
}

/// A dialog ready to show, or a warning and `None` if there is no dir to show it in.
fn dialog(
    grimoire: &mut Grimoire,
    filter: Option<Filter>,
    start: Option<&Path>,
) -> Result<Option<FileDialog>> {
    let downloads = UserDirs::new().and_then(|dirs| dirs.download_dir().map(Path::to_path_buf));
    let wad_dirs = wad_dirs();
    let working = std::env::current_dir().ok();
    let Some(dir) = start_dir(start, &wad_dirs, working, downloads) else {
        Warning::no_dir_to_open(grimoire)?;
        return Ok(None);
    };
    let dialog = FileDialog::new().set_directory(dir);
    Ok(Some(match filter {
        Some((name, extensions)) => dialog.add_filter(name, extensions),
        None => dialog,
    }))
}

/// The first of these that is a dir that exists. The wad dirs take the working directory's
/// place: when there are any, the working directory isn't tried at all.
fn start_dir(
    preferred: Option<&Path>,
    wad_dirs: &[PathBuf],
    working: Option<PathBuf>,
    downloads: Option<PathBuf>,
) -> Option<PathBuf> {
    let places = if wad_dirs.is_empty() {
        working.into_iter().collect()
    } else {
        wad_dirs.to_vec()
    };
    preferred
        .map(Path::to_path_buf)
        .into_iter()
        .chain(places)
        .chain(downloads)
        .find(|dir| dir.is_dir())
}
