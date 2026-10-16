use std::{
    ops::Deref,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::RecvTimeoutError,
    },
};

use ratatui::style::Color;

use crate::{
    error::{Error, Result},
    net::{self, Address, Entry, Manifest, Report},
    popups::{
        cancel::CancelHandler,
        dialog::Dialog,
        hashing,
        progress::{Kind, Progress},
        running, status,
        warning::Warning,
    },
    ritual::Grimoire,
    screens::{profile_namer::ProfileNamer, profile_picker::ProfilePicker},
    tally::Tally,
    uzdoom,
    vault::{Vault, lookup::Local, profile::Profile, wad::Wad},
};

const BORDER_COLOR: Color = Color::LightMagenta;

/// Fetches the host's manifest and picks a profile to join with: the only compatible one, a choice
/// among several, or a new profile downloaded from the host when none match.
pub fn summon(
    grimoire: &mut Grimoire,
    vault: &mut Vault,
    index: &mut usize,
    address: &Address,
) -> Result<()> {
    status::summon(grimoire, "Joining", "Joining...", BORDER_COLOR)?;
    let manifest = match Manifest::fetch(address) {
        Ok(manifest) => manifest,
        Err(Error::Refused) => return Warning::code_refused(grimoire),
        Err(_) => return Warning::host_unreachable(grimoire),
    };
    let Some(profile) = resolve_profile(grimoire, vault, index, address, &manifest)? else {
        return Ok(());
    };
    let Some(uzdoom) = profile.uzdoom(&vault.config) else {
        return Warning::uzdoom_not_selected(grimoire);
    };
    if Warning::faults(grimoire, &profile.faults(&vault.config))? {
        return Ok(());
    }
    // 🌀 volatile wads may legitimately differ from the host's, but not without the user knowing
    let differing = manifest.differing_wads(&profile);
    if !differing.is_empty() && !Dialog::should_join_with_different_wads(grimoire, &differing)? {
        return Ok(());
    }
    let base_wad = profile.base_wad(&vault.config);
    if manifest.base_wad_matches(base_wad)
        || Dialog::summon(
            grimoire,
            "Selected base wad does not match, do you want to continue?",
        )?
    {
        let details = [format!("host {}", address.host)];
        let result = uzdoom::join(
            uzdoom,
            base_wad.map(Wad::deref),
            &profile,
            &address.host,
            profile.uzdoom_args(&vault.config),
            &vault.data_dir.join(&profile.name),
        )
        .and_then(|game| running::summon(grimoire, "joining", &details, game));
        Warning::report_spawn(grimoire, result)
    } else {
        Ok(())
    }
}

/// The profile to join with: the single compatible one, a pick among several, or a fresh download.
/// `None` if the user backs out.
fn resolve_profile(
    grimoire: &mut Grimoire,
    vault: &mut Vault,
    index: &mut usize,
    address: &Address,
    manifest: &Manifest,
) -> Result<Option<Profile>> {
    let compatible_profiles: Vec<_> = vault
        .profiles
        .iter()
        .filter(|p| manifest.is_compatible_with(p))
        .collect();
    match compatible_profiles.len() {
        0 => download_profile(grimoire, vault, index, address, manifest),
        1 => Ok(Some(compatible_profiles[0].clone())),
        _ => ProfilePicker::summon(grimoire, &compatible_profiles),
    }
}

/// Offers to make a new profile, named by the user, from the host's wads: the ones already on this
/// machine are used where they are, and only the rest are downloaded.
fn download_profile(
    grimoire: &mut Grimoire,
    vault: &mut Vault,
    index: &mut usize,
    address: &Address,
    manifest: &Manifest,
) -> Result<Option<Profile>> {
    if vault.config.uzdoom.is_none() {
        Warning::uzdoom_not_selected(grimoire)?;
        return Ok(None);
    }
    let Some(local) = find_local(grimoire, vault, manifest)? else {
        return Ok(None);
    };
    let total = local.len();
    let found = local.iter().flatten().count();
    let missing = total - found;
    let question = match (found, missing) {
        (0, 0) => "The host plays without wads. Make a profile for it?".to_string(),
        (_, 0) => format!("All {total} wads are already here. Make a profile from them?"),
        (0, _) => "You don't have the profile, would you like to download?".to_string(),
        _ => format!("{found} of {total} wads are already here. Download the other {missing}?"),
    };
    if !Dialog::summon(grimoire, &question)? {
        return Ok(None);
    }
    let Some(profile_name) = ProfileNamer::summon(grimoire, &vault.profiles)? else {
        return Ok(None);
    };
    let wanted: Vec<_> = manifest
        .entries
        .iter()
        .zip(&local)
        .filter(|(_, wad)| wad.is_none())
        .map(|(entry, _)| entry.clone())
        .collect();
    let downloaded = if wanted.is_empty() {
        Vec::new()
    } else {
        status::summon(grimoire, "Downloading", "Downloading...", BORDER_COLOR)?;
        // 🚪 Esc goes back to the launcher quietly: partial files are already cleaned up, and
        // finished ones stay in the downloads folder, so trying again picks up where this stopped
        match download_wads(grimoire, address, wanted, vault.downloads_dir()) {
            Ok(wads) => wads,
            Err(Error::Cancelled) => return Ok(None),
            Err(Error::Exit) => return Err(Error::Exit),
            Err(_) => {
                Warning::download_failed(grimoire)?;
                return Ok(None);
            }
        }
    };
    let wads = fill_gaps(local, downloaded)
        .ok_or(Error::Download("Fewer wads arrived than were asked for"))?;
    let profile = Profile::new(profile_name, wads);
    if vault.insert(index, profile.clone()).is_err() {
        Warning::save_failed(grimoire)?;
        return Ok(None);
    }
    Ok(Some(profile))
}

/// The host's wads in load order: the local ones where they were found, and each gap filled by the
/// next of `downloaded` (which are in the same order). `None` if there weren't enough downloads.
fn fill_gaps(local: Vec<Option<Wad>>, downloaded: Vec<Wad>) -> Option<Vec<Wad>> {
    let mut downloaded = downloaded.into_iter();
    local
        .into_iter()
        .map(|wad| wad.or_else(|| downloaded.next()))
        .collect()
}

/// The host's wads that are already on this machine, in the manifest's order, with `None` for each
/// one that isn't. A file that only might be one is hashed to make sure, in the hashing window.
/// `None` if Esc was pressed.
fn find_local(
    grimoire: &mut Grimoire,
    vault: &mut Vault,
    manifest: &Manifest,
) -> Result<Option<Vec<Option<Wad>>>> {
    let lookup = vault.lookup();
    let checks: Vec<_> = manifest
        .entries
        .iter()
        .map(|entry| lookup.check(entry))
        .collect();
    let to_hash = checks
        .iter()
        .map(|check| match check {
            Local::Maybe(paths) => paths.len(),
            Local::Intact(_) => 0,
        })
        .sum();
    hashing::watch(grimoire, to_hash, |tally| {
        checks
            .into_iter()
            .zip(&manifest.entries)
            .map(|(check, entry)| match check {
                Local::Intact(wad) => Ok(Some(wad)),
                Local::Maybe(paths) => first_match(paths, entry, tally),
            })
            .collect()
    })
}

/// The first of `paths` whose contents hash to `entry`'s hash. A file that can't be read just isn't
/// the one; only Esc or Ctrl+C stop the search.
fn first_match(paths: Vec<PathBuf>, entry: &Entry, tally: &mut impl Tally) -> Result<Option<Wad>> {
    for path in paths {
        match Wad::hashed(&path, tally) {
            Ok(wad) if wad.hash == entry.hash => return Ok(Some(wad)),
            Err(error @ (Error::Cancelled | Error::Exit)) => return Err(error),
            _ => {}
        }
    }
    Ok(None)
}

/// Downloads the host's wads on a worker thread while this one draws the progress and listens for
/// Esc, so neither a slow host nor a dead one can freeze the screen.
fn download_wads(
    grimoire: &mut Grimoire,
    address: &Address,
    entries: Vec<Entry>,
    downloads_dir: PathBuf,
) -> Result<Vec<Wad>> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let count = entries.len() as u64;
    let reports =
        net::download_in_background(address.clone(), entries, downloads_dir, cancelled.clone());
    let mut progress =
        Progress::new(grimoire, Kind::Downloading, count).with_footer(CancelHandler::HINT);
    loop {
        let step = match reports.recv_timeout(Progress::FRAME) {
            Ok(Report::File(name, size)) => progress.count_file(&name, size),
            Ok(Report::Bytes(amount)) => progress.count_bytes(amount),
            Ok(Report::Done(result)) => return result,
            Err(RecvTimeoutError::Timeout) => progress.tick(),
            Err(RecvTimeoutError::Disconnected) => {
                return Err(Error::Download("The download stopped unexpectedly"));
            }
        };
        // 🛑 the worker notices at its next chunk, and cleans up after itself
        if let Err(error) = step.and_then(|()| CancelHandler::check()) {
            cancelled.store(true, Ordering::SeqCst);
            return Err(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wad(name: &str) -> Wad {
        Wad::new(name.into(), String::new(), 0)
    }

    #[test]
    fn downloads_fill_the_gaps_in_the_hosts_order() {
        let local = vec![None, Some(wad("here.wad")), None];
        assert_eq!(
            fill_gaps(local, vec![wad("first.wad"), wad("last.wad")]),
            Some(vec![wad("first.wad"), wad("here.wad"), wad("last.wad")])
        );
    }

    #[test]
    fn too_few_downloads_is_noticed() {
        assert_eq!(fill_gaps(vec![None, None], vec![wad("only.wad")]), None);
    }
}
