use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    style::{Color, Stylize},
    text::{Line, ToLine, ToSpan},
};

use crate::{
    error::Result,
    scene::Scene,
    utility::{self, Phase, Terminal},
};

pub struct Dialog<'a> {
    message: &'a str,
    width: u16,
}

impl<'a> Dialog<'a> {
    const TITLE: &str = "Dialogue";
    const HEIGHT: u16 = 7;
    const BORDER_COLOR: Color = Color::LightCyan;

    pub fn new(message: &'a str) -> Self {
        Self {
            width: message.len().max(10) as u16 + 4,
            message,
        }
    }
}

impl Scene for Dialog<'_> {
    type Return = bool;

    fn render(&mut self, frame: &mut Frame) {
        let (window, mut window_area, ..) = utility::new_window(
            Self::TITLE,
            self.width,
            Self::HEIGHT,
            Self::BORDER_COLOR,
            frame.area(),
        );
        frame.render_widget(window, window_area);
        window_area.y += 2;
        frame.render_widget(self.message.to_line().centered(), window_area);
        let legend = [
            "[".to_span(),
            "Y".to_span().light_yellow(),
            "]es [".to_span(),
            "N".to_span().light_yellow(),
            "]o".to_span(),
        ]
        .into_iter()
        .collect::<Line>()
        .centered();
        window_area.y += 2;
        frame.render_widget(legend, window_area);
    }

    fn prompt(&mut self, _: &mut Terminal, key_code: KeyCode) -> Result<Phase<Self::Return>> {
        match key_code {
            KeyCode::Char('y') => Ok(Phase::Return(true)),
            KeyCode::Char('n') => Ok(Phase::Return(false)),
            _ => Ok(Phase::Prompt),
        }
    }
}

impl Dialog<'_> {
    pub fn evoke(terminal: &mut Terminal, message: &str) -> Result<bool> {
        Dialog::new(message).invoke(terminal)
    }

    pub fn should_delete(terminal: &mut Terminal, element: &str) -> Result<bool> {
        Self::evoke(
            terminal,
            &format!("Are you sure you want to delete '{element}'?"),
        )
    }

    pub fn should_quit(terminal: &mut Terminal) -> Result<bool> {
        Self::evoke(terminal, "Are you sure you want to quit?")
    }
}
