use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    style::{Color, Stylize},
    text::ToLine,
};

use crate::{
    error::Result,
    scene::Scene,
    utility::{self, Phase, Terminal},
};

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
    pub fn evoke(terminal: &mut Terminal, message: &str) -> Result<()> {
        Warning::new(message).invoke(terminal)
    }

    pub fn uzdoom_not_selected(terminal: &mut Terminal) -> Result<()> {
        Self::evoke(terminal, "No UZDoom path selected!")
    }

    pub fn empty_profile_name(terminal: &mut Terminal) -> Result<()> {
        Self::evoke(terminal, "Profile name cannot be empty")
    }

    pub fn profile_already_exists(terminal: &mut Terminal) -> Result<()> {
        Self::evoke(terminal, "Profile name already exists")
    }

    pub fn host_unreachable(terminal: &mut Terminal) -> Result<()> {
        Self::evoke(terminal, "Could not connect to host!")
    }

    pub fn wad_unreadable(terminal: &mut Terminal) -> Result<()> {
        Self::evoke(terminal, "A wad file could not be read!")
    }

    pub fn host_failed(terminal: &mut Terminal) -> Result<()> {
        Self::evoke(terminal, "Could not start the host server!")
    }

    pub fn download_failed(terminal: &mut Terminal) -> Result<()> {
        Self::evoke(terminal, "Download failed!")
    }
}

impl Scene for Warning<'_> {
    type Return = ();

    fn render(&mut self, frame: &mut Frame) {
        let (window, mut window_area, ..) = utility::new_window(
            Self::TITLE,
            self.width,
            Self::HEIGHT,
            Self::BORDER_COLOR,
            frame.area(),
        );
        frame.render_widget(window, window_area);
        window_area.width -= 2;
        window_area.x += 1;
        window_area.y += 2;
        frame.render_widget(self.message.to_line().centered().light_red(), window_area);
    }

    fn prompt(&mut self, _: &mut Terminal, _: KeyCode) -> Result<Phase<Self::Return>> {
        Ok(Phase::Return(()))
    }
}
