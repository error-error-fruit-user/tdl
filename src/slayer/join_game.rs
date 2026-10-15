use std::ops::Deref;

use ratatui::style::Color;

use crate::{
    error::Result,
    net::{self, Manifest},
    popups::{dialog::Dialog, progress::Progress, status, warning::Warning},
    ritual::Grimoire,
    screens::{profile_namer::ProfileNamer, profile_picker::ProfilePicker},
    uzdoom,
    vault::{Vault, profile::Profile, wad::Wad},
};

const BORDER_COLOR: Color = Color::LightMagenta;

/// Fetches the host's manifest and picks a profile to join with: the only compatible one, a choice
/// among several, or a new profile downloaded from the host when none match.
pub fn summon(
    grimoire: &mut Grimoire,
    vault: &mut Vault,
    index: &mut usize,
    ip: &str,
) -> Result<()> {
    status::summon(grimoire, "Joining", "Joining...", BORDER_COLOR)?;
    let Ok(manifest) = Manifest::fetch(ip) else {
        return Warning::host_unreachable(grimoire);
    };
    let Some(profile) = resolve_profile(grimoire, vault, index, ip, &manifest)? else {
        return Ok(());
    };
    let Some(uzdoom) = profile.uzdoom(&vault.config) else {
        return Warning::uzdoom_not_selected(grimoire);
    };
    let base_wad = profile.base_wad(&vault.config);
    if manifest.base_wad_matches(base_wad)
        || Dialog::summon(
            grimoire,
            "Selected base wad does not match, do you want to continue?",
        )?
    {
        uzdoom::join(
            uzdoom,
            base_wad.map(Wad::deref),
            &profile,
            ip,
            &vault.data_dir.join(&profile.name),
        )
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
    ip: &str,
    manifest: &Manifest,
) -> Result<Option<Profile>> {
    let compatible_profiles: Vec<_> = vault
        .profiles
        .iter()
        .filter(|p| manifest.is_compatible_with(p))
        .collect();
    match compatible_profiles.len() {
        0 => download_profile(grimoire, vault, index, ip, manifest),
        1 => Ok(Some(compatible_profiles[0].clone())),
        _ => ProfilePicker::summon(grimoire, &compatible_profiles),
    }
}

/// Offers to download the host's wads into a new profile, named by the user.
fn download_profile(
    grimoire: &mut Grimoire,
    vault: &mut Vault,
    index: &mut usize,
    ip: &str,
    manifest: &Manifest,
) -> Result<Option<Profile>> {
    if vault.config.uzdoom.is_none() {
        Warning::uzdoom_not_selected(grimoire)?;
        return Ok(None);
    }
    if !Dialog::summon(
        grimoire,
        "You don't have the profile, would you like to download?",
    )? {
        return Ok(None);
    }
    let Some(profile_name) = ProfileNamer::summon(grimoire, &vault.profiles)? else {
        return Ok(None);
    };
    status::summon(grimoire, "Downloading", "Downloading...", BORDER_COLOR)?;
    let mut progress = Progress::new(grimoire, manifest.entries.len() as u64);
    let Ok(wads) = net::download(ip, &manifest.entries, &vault.downloads_dir(), &mut progress)
    else {
        Warning::download_failed(grimoire)?;
        return Ok(None);
    };
    let profile = Profile::new(profile_name, wads);
    vault.insert(index, profile.clone());
    Ok(Some(profile))
}
