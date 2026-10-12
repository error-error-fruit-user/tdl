use std::{ffi::OsStr, ops::Deref, path::Path};

use ratatui::{style::Color, text::ToLine};

use crate::{
    error::Result,
    execute, network,
    profile::Profile,
    utility::{self, Terminal},
    wad::Wad,
};

const TITLE: &str = "Hosting";
const BORDER_COLOR: Color = Color::LightMagenta;
const WIDTH: u16 = 32;
const HEIGHT: u16 = 5;

pub fn run(
    terminal: &mut Terminal,
    uzdoom: &OsStr,
    base_wad: Option<&Wad>,
    profile: &Profile,
    player_count: &str,
    is_deathmatch: bool,
    map: &str,
    data_subdir: &Path,
) -> Result<()> {
    terminal.draw(|frame| {
        let (window, mut window_area, ..) =
            utility::new_window(TITLE, WIDTH, HEIGHT, BORDER_COLOR, frame.area());

        frame.render_widget(window, window_area);
        window_area.y += 2;
        frame.render_widget("Hosting...".to_line().centered(), window_area);
    })?;

    let _server = {
        let manifest = network::Manifest::build(
            &profile.wads,
            base_wad.map(|w| w.hash.clone()).unwrap_or(String::new()),
        )?;
        Some(manifest.serve(
            network::DEFAULT_PORT,
            profile.wads.iter().map(|w| w.path.clone()).collect(),
        )?)
    };

    execute::host(
        uzdoom,
        base_wad.map(Wad::deref),
        profile,
        player_count,
        is_deathmatch,
        map,
        data_subdir,
    )
}
