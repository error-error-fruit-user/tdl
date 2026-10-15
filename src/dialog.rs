use ratatui::{
    crossterm::event::KeyCode,
    style::{Color, Stylize},
    text::{Line, ToLine, ToSpan},
};

use crate::{
    error::Result,
    ritual::{self, Grimoire, Page, Phase, Ritual},
};

/// A yes/no popup, answered with `y` or `n`.
pub struct Dialog<'a> {
    message: &'a str,
    width: u16,
}

impl<'a> Dialog<'a> {
    const TITLE: &str = "Dialogue";
    const HEIGHT: u16 = 7;
    const BORDER_COLOR: Color = Color::LightCyan;

    /// Sizes the window to fit `message`.
    pub fn new(message: &'a str) -> Self {
        Self {
            width: message.len().max(10) as u16 + 4,
            message,
        }
    }
}

impl Ritual for Dialog<'_> {
    type Return = bool;

    fn render(&mut self, page: &mut Page) {
        let (window, mut window_area, ..) = ritual::conjure_window(
            Self::TITLE,
            self.width,
            Self::HEIGHT,
            Self::BORDER_COLOR,
            page.area(),
        );
        page.render_widget(window, window_area);
        window_area.y += 2;
        page.render_widget(self.message.to_line().centered(), window_area);
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
        page.render_widget(legend, window_area);
    }

    fn prompt(&mut self, _: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Self::Return>> {
        match key_code {
            KeyCode::Char('y') => Ok(Phase::Return(true)),
            KeyCode::Char('n') => Ok(Phase::Return(false)),
            _ => Ok(Phase::Prompt),
        }
    }
}

impl Dialog<'_> {
    /// Returns whether the user answered yes.
    pub fn summon(grimoire: &mut Grimoire, message: &str) -> Result<bool> {
        Dialog::new(message).invoke(grimoire)
    }

    pub fn should_delete(grimoire: &mut Grimoire, element: &str) -> Result<bool> {
        Self::summon(
            grimoire,
            &format!("Are you sure you want to delete '{element}'?"),
        )
    }

    pub fn should_quit(grimoire: &mut Grimoire) -> Result<bool> {
        Self::summon(grimoire, "Are you sure you want to quit?")
    }
}
