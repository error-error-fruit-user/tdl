//! Profiles that watch a dir: made with `F` in the launcher (or given a dir with `T` in the
//! editor), they take in the wads put in their dir, and let go of the ones that leave it, whenever
//! tdl starts, before they are launched or hosted, and when they are opened in the editor.

use crate::{
    error::Result,
    loadout::{Loadout, profile::Profile, wad::Wad},
    picker,
    popups::{hashing, warning::Warning},
    ritual::Grimoire,
    screens::profile_namer::ProfileNamer,
};

/// Brings `profile` up to date with its watched dir, hashing the new wads (which `Esc` stops,
/// leaving it as it was). Returns whether it changed.
pub fn refresh(grimoire: &mut Grimoire, profile: &mut Profile) -> Result<bool> {
    let Some(watching) = profile.look() else {
        return Ok(false);
    };
    let new = if watching.new.is_empty() {
        Vec::new()
    } else {
        match hashing::wads(grimoire, &watching.new)? {
            Some(new) => new,
            None => return Ok(false),
        }
    };
    profile.take_in(&watching, new);
    Ok(true)
}

/// [`refresh`] for the profile at `index` in `loadout`, saved if it changed.
pub fn refresh_saved(grimoire: &mut Grimoire, loadout: &mut Loadout, index: usize) -> Result<()> {
    let mut profile = loadout.profiles[index].clone();
    if refresh(grimoire, &mut profile)?
        && let Err(error) = loadout.replace(index, profile)
    {
        Warning::save_failed(grimoire, &error)?;
    }
    Ok(())
}

/// [`refresh_saved`] for every profile that watches a dir.
pub fn refresh_all(grimoire: &mut Grimoire, loadout: &mut Loadout) -> Result<()> {
    for index in 0..loadout.profiles.len() {
        if loadout.profiles[index].watched.is_some() {
            refresh_saved(grimoire, loadout, index)?;
        }
    }
    Ok(())
}

/// [`refresh_saved`] with no windows, for `--headless`: the new wads are hashed without a progress
/// window, and `Err` says what went wrong.
pub fn refresh_plain(loadout: &mut Loadout, index: usize) -> std::result::Result<(), String> {
    let mut profile = loadout.profiles[index].clone();
    let Some(watching) = profile.look() else {
        return Ok(());
    };
    let new = watching
        .new
        .iter()
        .map(|path| Wad::hashed(path, &mut ()))
        .collect::<Result<Vec<_>>>()
        .map_err(|error| format!("Could not read a wad in the watched dir: {error}"))?;
    profile.take_in(&watching, new);
    loadout
        .replace(index, profile)
        .map_err(|error| format!("Could not save the profile: {error}"))
}

/// Makes a profile that watches a dir the user picks, named after the dir unless they change it,
/// with every wad already in it, and puts it at `index`.
pub fn create(grimoire: &mut Grimoire, loadout: &mut Loadout, index: &mut usize) -> Result<()> {
    let Some(dir) = picker::dir(grimoire, None)? else {
        return Ok(());
    };
    let dir = std::path::absolute(&dir).unwrap_or(dir);
    let suggested = dir
        .file_name()
        .map(|name| name.to_string_lossy().replace('.', "_"))
        .unwrap_or_default();
    let Some(name) = ProfileNamer::summon_suggesting(grimoire, &loadout.profiles, &suggested)?
    else {
        return Ok(());
    };
    let mut profile = Profile {
        watched: Some(dir.into_os_string()),
        ..Profile::new(name, Vec::new())
    };
    // ⏳ backing out of the hashing still makes the profile: the wads come in when it next looks
    refresh(grimoire, &mut profile)?;
    if let Err(error) = loadout.insert(index, profile) {
        Warning::save_failed(grimoire, &error)?;
    }
    Ok(())
}
