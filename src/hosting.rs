use std::ops::Deref;

use ratatui::style::Color;

use crate::{
    error::Result,
    execute,
    host::Settings,
    library::Library,
    network,
    utility::{self, Terminal},
    wad::Wad,
    warning::Warning,
};

const TITLE: &str = "Hosting";
const BORDER_COLOR: Color = Color::LightMagenta;

pub fn host(
    terminal: &mut Terminal,
    library: &Library,
    index: usize,
    settings: Settings,
) -> Result<()> {
    let profile = &library.profiles[index];
    let Some(uzdoom) = profile.uzdoom(&library.config) else {
        return Warning::uzdoom_not_selected(terminal);
    };
    let base_wad = profile.base_wad(&library.config);
    utility::show_status(terminal, TITLE, "Hosting...", BORDER_COLOR)?;

    let Ok(manifest) = network::Manifest::build(
        &profile.wads,
        base_wad.map(|w| w.hash.clone()).unwrap_or_default(),
    ) else {
        return Warning::wad_unreadable(terminal);
    };
    let Ok(_server) = manifest.serve(
        network::DEFAULT_PORT,
        profile.wads.iter().map(|w| w.path.clone()).collect(),
    ) else {
        return Warning::host_failed(terminal);
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
