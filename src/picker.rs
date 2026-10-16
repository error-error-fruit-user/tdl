//! The system's file dialogs, always opened in a folder that exists.
//!
//! The dialog opens in the preferred folder if there is one, else the first of the wad folders
//! (from `--wads`, or kept from it), else the working directory if there are no wad folders, else
//! the Downloads folder. If none of those can be found there is nowhere sensible to open it, so
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

/// The session's wad folders, set once at startup. Kept here rather than passed to every picker,
/// since pickers are opened from all over and only ever need to read it.
static WAD_FOLDERS: RwLock<Vec<PathBuf>> = RwLock::new(Vec::new());

/// Makes `folders` the places dialogs open before anything else, instead of the working directory.
pub fn set_wad_folders(folders: &[PathBuf]) {
    if let Ok(mut wad_folders) = WAD_FOLDERS.write() {
        *wad_folders = folders.to_vec();
    }
}

/// Asks for one file. `None` if the user cancels, or if there is no folder to open the dialog in.
pub fn file(
    grimoire: &mut Grimoire,
    filter: Option<Filter>,
    start: Option<&Path>,
) -> Result<Option<PathBuf>> {
    #[cfg(debug_assertions)]
    if let Some(picked) = stand_in() {
        return Ok(picked.and_then(|paths| paths.into_iter().next()));
    }
    Ok(dialog(grimoire, filter, start)?.and_then(|dialog| dialog.pick_file()))
}

/// Asks for any number of files. `None` if the user cancels, or if there is no folder to open
/// the dialog in.
pub fn files(
    grimoire: &mut Grimoire,
    filter: Option<Filter>,
    start: Option<&Path>,
) -> Result<Option<Vec<PathBuf>>> {
    #[cfg(debug_assertions)]
    if let Some(picked) = stand_in() {
        return Ok(picked);
    }
    Ok(dialog(grimoire, filter, start)?.and_then(|dialog| dialog.pick_files()))
}

/// Asks where to save a file, offering `file_name`. `None` if the user cancels, or if there is no
/// folder to open the dialog in.
pub fn save(
    grimoire: &mut Grimoire,
    filter: Option<Filter>,
    file_name: &str,
) -> Result<Option<PathBuf>> {
    #[cfg(debug_assertions)]
    if let Some(picked) = stand_in() {
        return Ok(picked.and_then(|paths| paths.into_iter().next()));
    }
    Ok(dialog(grimoire, filter, None)?
        .and_then(|dialog| dialog.set_file_name(file_name).save_file()))
}

/// The session's wad folders, as set by [`set_wad_folders`].
pub fn wad_folders() -> Vec<PathBuf> {
    WAD_FOLDERS
        .read()
        .map(|folders| folders.clone())
        .unwrap_or_default()
}

/// Asks for a folder. `None` if the user cancels, or if there is no folder to open the dialog in.
pub fn folder(grimoire: &mut Grimoire, start: Option<&Path>) -> Result<Option<PathBuf>> {
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

/// A dialog ready to show, or a warning and `None` if there is no folder to show it in.
fn dialog(
    grimoire: &mut Grimoire,
    filter: Option<Filter>,
    start: Option<&Path>,
) -> Result<Option<FileDialog>> {
    let downloads = UserDirs::new().and_then(|dirs| dirs.download_dir().map(Path::to_path_buf));
    let wad_folders = wad_folders();
    let working = std::env::current_dir().ok();
    let Some(folder) = start_folder(start, &wad_folders, working, downloads) else {
        Warning::no_folder_to_open(grimoire)?;
        return Ok(None);
    };
    let dialog = FileDialog::new().set_directory(folder);
    Ok(Some(match filter {
        Some((name, extensions)) => dialog.add_filter(name, extensions),
        None => dialog,
    }))
}

/// The first of these that is a folder that exists. The wad folders take the working directory's
/// place: when there are any, the working directory isn't tried at all.
fn start_folder(
    preferred: Option<&Path>,
    wad_folders: &[PathBuf],
    working: Option<PathBuf>,
    downloads: Option<PathBuf>,
) -> Option<PathBuf> {
    let places = if wad_folders.is_empty() {
        working.into_iter().collect()
    } else {
        wad_folders.to_vec()
    };
    preferred
        .map(Path::to_path_buf)
        .into_iter()
        .chain(places)
        .chain(downloads)
        .find(|folder| folder.is_dir())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::vault::scribe::tests::Altar;

    #[test]
    fn the_preferred_folder_wins_when_it_exists() {
        let altar = Altar::new("picker-preferred");
        let found = start_folder(Some(&altar.0), &[], Some("/".into()), Some("/".into()));
        assert_eq!(found, Some(altar.0.clone()));
    }

    #[test]
    fn a_missing_preferred_folder_falls_back_to_the_working_directory() {
        let altar = Altar::new("picker-working");
        let gone = altar.file("gone");
        let found = start_folder(Some(&gone), &[], Some(altar.0.clone()), Some("/".into()));
        assert_eq!(found, Some(altar.0.clone()));
    }

    #[test]
    fn a_missing_working_directory_falls_back_to_downloads() {
        let altar = Altar::new("picker-downloads");
        let found = start_folder(None, &[], None, Some(altar.0.clone()));
        assert_eq!(found, Some(altar.0.clone()));
        // a path that exists but is a file is not a folder
        let file = altar.file("file");
        fs::write(&file, b"").unwrap();
        assert_eq!(
            start_folder(Some(&file), &[], Some(file.clone()), None),
            None
        );
    }

    #[test]
    fn with_nowhere_to_open_there_is_nothing() {
        assert_eq!(start_folder(None, &[], None, None), None);
        assert_eq!(
            start_folder(
                None,
                &[],
                Some("/no/such/folder".into()),
                Some("/nor/this".into())
            ),
            None
        );
    }

    #[test]
    fn wad_folders_replace_the_working_directory() {
        let altar = Altar::new("picker-wad-folders");
        let (wads, working) = (altar.file("wads"), altar.file("working"));
        fs::create_dir_all(&wads).unwrap();
        fs::create_dir_all(&working).unwrap();
        let gone = altar.file("gone");
        // the first wad folder that exists wins over the working directory
        assert_eq!(
            start_folder(
                None,
                &[gone.clone(), wads.clone()],
                Some(working.clone()),
                None
            ),
            Some(wads.clone())
        );
        // and if none of them exist, the working directory still isn't used
        assert_eq!(
            start_folder(None, &[gone], Some(working), Some(altar.0.clone())),
            Some(altar.0.clone())
        );
        // a preferred folder, like where a wad used to live, still comes first
        assert_eq!(
            start_folder(Some(&altar.0), &[wads], None, None),
            Some(altar.0.clone())
        );
    }
}
