use ratatui::{
    crossterm::event::KeyCode,
    style::{Color, Stylize},
    text::ToLine,
};

use crate::{
    error::Result,
    ritual::{self, Grimoire, Page, Phase, Ritual},
};

/// A one-line popup dismissed by any key.
pub struct Warning<'a> {
    message: &'a str,
    width: u16,
}

impl<'a> Warning<'a> {
    const TITLE: &str = "Warning";
    const HEIGHT: u16 = 5;
    const BORDER_COLOR: Color = Color::LightCyan;

    pub fn new(message: &'a str) -> Self {
        Self {
            width: message.len() as u16 + 4,
            message,
        }
    }
}

impl Warning<'_> {
    /// Shows `message` and waits for a key.
    pub fn summon(grimoire: &mut Grimoire, message: &str) -> Result<()> {
        Warning::new(message).invoke(grimoire)
    }

    pub fn uzdoom_not_selected(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "No UZDoom path selected!")
    }

    pub fn empty_profile_name(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Profile name cannot be empty")
    }

    pub fn profile_already_exists(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Profile name already exists")
    }

    pub fn host_unreachable(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Could not connect to host!")
    }

    pub fn wad_unreadable(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "A wad file could not be read!")
    }

    pub fn host_failed(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Could not start the host server!")
    }

    pub fn files_too_new(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Files are from a newer version, please update!")
    }

    pub fn upgrade_failed(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Upgrade failed, your original files were kept!")
    }

    pub fn download_failed(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Download failed!")
    }
}

impl Ritual for Warning<'_> {
    type Return = ();

    fn render(&mut self, page: &mut Page) {
        let (window, mut window_area, ..) = ritual::conjure_window(
            Self::TITLE,
            self.width,
            Self::HEIGHT,
            Self::BORDER_COLOR,
            page.area(),
        );
        page.render_widget(window, window_area);
        window_area.width -= 2;
        window_area.x += 1;
        window_area.y += 2;
        page.render_widget(self.message.to_line().centered().light_red(), window_area);
    }

    fn prompt(&mut self, _: &mut Grimoire, _: KeyCode) -> Result<Phase<Self::Return>> {
        Ok(Phase::Return(()))
    }
}
