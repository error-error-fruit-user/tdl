use std::ops::Deref;

use ratatui::style::Color;

use crate::{
    error::Result, net, popups::status, popups::warning::Warning, ritual::Grimoire,
    screens::host_settings::HostSettings, uzdoom, vault::Vault, vault::wad::Wad,
};

const TITLE: &str = "Hosting";
const BORDER_COLOR: Color = Color::LightMagenta;

/// Serves the profile's wads on [`net::DEFAULT_PORT`], then runs the game; the server stops when
/// the game exits.
pub fn summon(
    grimoire: &mut Grimoire,
    vault: &Vault,
    index: usize,
    settings: HostSettings,
) -> Result<()> {
    let profile = &vault.profiles[index];
    let Some(uzdoom) = profile.uzdoom(&vault.config) else {
        return Warning::uzdoom_not_selected(grimoire);
    };
    let base_wad = profile.base_wad(&vault.config);
    status::summon(grimoire, TITLE, "Hosting...", BORDER_COLOR)?;

    // 🌀 volatile wads are hashed fresh, so what the manifest says is what actually gets served
    let Ok(wads) = profile
        .wads
        .iter()
        .map(Wad::refreshed)
        .collect::<Result<Vec<_>>>()
    else {
        return Warning::wad_unreadable(grimoire);
    };
    let Ok(manifest) =
        net::Manifest::build(&wads, base_wad.map(|w| w.hash.clone()).unwrap_or_default())
    else {
        return Warning::wad_unreadable(grimoire);
    };
    let Ok(_server) = manifest.serve(
        net::DEFAULT_PORT,
        wads.iter().map(|w| w.path.clone()).collect(),
    ) else {
        return Warning::host_failed(grimoire);
    };

    let result = uzdoom::host(
        uzdoom,
        base_wad.map(Wad::deref),
        profile,
        &settings.player_count,
        settings.is_deathmatch,
        &settings.map,
        &vault.data_dir.join(&profile.name),
    );
    Warning::report_spawn(grimoire, result)
}
