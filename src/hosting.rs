use std::ops::Deref;

use ratatui::style::Color;

use crate::{
    error::Result, execute, host::Settings, library::Library, network, ritual::Grimoire, status,
    wad::Wad, warning::Warning,
};

const TITLE: &str = "Hosting";
const BORDER_COLOR: Color = Color::LightMagenta;

/// Serves the profile's wads on [`network::DEFAULT_PORT`], then runs the game; the server stops when
/// the game exits.
pub fn summon(
    grimoire: &mut Grimoire,
    library: &Library,
    index: usize,
    settings: Settings,
) -> Result<()> {
    let profile = &library.profiles[index];
    let Some(uzdoom) = profile.uzdoom(&library.config) else {
        return Warning::uzdoom_not_selected(grimoire);
    };
    let base_wad = profile.base_wad(&library.config);
    status::summon(grimoire, TITLE, "Hosting...", BORDER_COLOR)?;

    let Ok(manifest) = network::Manifest::build(
        &profile.wads,
        base_wad.map(|w| w.hash.clone()).unwrap_or_default(),
    ) else {
        return Warning::wad_unreadable(grimoire);
    };
    let Ok(_server) = manifest.serve(
        network::DEFAULT_PORT,
        profile.wads.iter().map(|w| w.path.clone()).collect(),
    ) else {
        return Warning::host_failed(grimoire);
    };

    execute::host(
        uzdoom,
        base_wad.map(Wad::deref),
        profile,
        &settings.player_count,
        settings.is_deathmatch,
        &settings.map,
        &library.data_dir.join(&profile.name),
    )
}
