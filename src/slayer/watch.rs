//! Profiles that watch a dir: made with `F` in the launcher (or given a dir with `T` in the
//! editor), they take in the wads put in their dir, and let go of the ones that leave it, whenever
//! tdl starts, before they are launched or hosted, and when they are opened in the editor.

use crate::{
    error::Result,
    picker,
    popups::{hashing, warning::Warning},
    ritual::Grimoire,
    screens::profile_namer::ProfileNamer,
    vault::{Vault, profile::Profile, wad::Wad},
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

/// [`refresh`] for the profile at `index` in `vault`, saved if it changed.
pub fn refresh_saved(grimoire: &mut Grimoire, vault: &mut Vault, index: usize) -> Result<()> {
    let mut profile = vault.profiles[index].clone();
    if refresh(grimoire, &mut profile)? && vault.replace(index, profile).is_err() {
        Warning::save_failed(grimoire)?;
    }
    Ok(())
}

/// [`refresh_saved`] for every profile that watches a dir.
pub fn refresh_all(grimoire: &mut Grimoire, vault: &mut Vault) -> Result<()> {
    for index in 0..vault.profiles.len() {
        if vault.profiles[index].watched.is_some() {
            refresh_saved(grimoire, vault, index)?;
        }
    }
    Ok(())
}

/// [`refresh_saved`] with no windows, for `--headless`: the new wads are hashed without a progress
/// window, and `Err` says what went wrong.
pub fn refresh_plain(vault: &mut Vault, index: usize) -> std::result::Result<(), String> {
    let mut profile = vault.profiles[index].clone();
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
    vault
        .replace(index, profile)
        .map_err(|error| format!("Could not save the profile: {error}"))
}

/// Makes a profile that watches a dir the user picks, named after the dir unless they change it,
/// with every wad already in it, and puts it at `index`.
pub fn create(grimoire: &mut Grimoire, vault: &mut Vault, index: &mut usize) -> Result<()> {
    let Some(dir) = picker::dir(grimoire, None)? else {
        return Ok(());
    };
    let dir = std::path::absolute(&dir).unwrap_or(dir);
    let suggested = dir
        .file_name()
        .map(|name| name.to_string_lossy().replace(['-', ' ', '.'], "_"))
        .unwrap_or_default();
    let Some(name) = ProfileNamer::summon_suggesting(grimoire, &vault.profiles, &suggested)? else {
        return Ok(());
    };
    let mut profile = Profile {
        watched: Some(dir.into_os_string()),
        ..Profile::new(name, Vec::new())
    };
    // ⏳ backing out of the hashing still makes the profile: the wads come in when it next looks
    refresh(grimoire, &mut profile)?;
    if vault.insert(index, profile).is_err() {
        Warning::save_failed(grimoire)?;
    }
    Ok(())
}
