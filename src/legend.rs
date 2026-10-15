use ratatui::{
    crossterm::event::KeyCode,
    style::{Color, Stylize},
    text::{Line, Span, ToSpan},
};

use crate::{
    error::Result,
    ritual::{self, Grimoire, Page, Phase, Ritual},
};

/// A popup listing `[key, description]` pairs, dismissed by any key.
pub struct Legend<'a> {
    legend: &'a [[&'a str; 2]],
    height: u16,
}

impl<'a> Legend<'a> {
    const TITLE: &str = "Legend";
    const WIDTH: u16 = 32;
    const BORDER_COLOR: Color = Color::LightGreen;

    /// Two rows per entry, so the window is tall for long legends.
    pub fn new(legend: &'a [[&'a str; 2]]) -> Self {
        Self {
            legend,
            height: legend.len() as u16 * 2 + 3,
        }
    }
}

impl Legend<'_> {
    pub fn summon(grimoire: &mut Grimoire, legend: &[[&str; 2]]) -> Result<()> {
        Legend::new(legend).invoke(grimoire)
    }
}

impl<'a> Ritual for Legend<'a> {
    type Return = ();

    fn render(&mut self, page: &mut Page) {
        let (window, mut window_area, ..) = ritual::conjure_window(
            Self::TITLE,
            Self::WIDTH,
            self.height,
            Self::BORDER_COLOR,
            page.area(),
        );
        page.render_widget(&window, window_area);
        window_area.x += 1;
        window_area.width -= 2;
        window_area.y += 2;
        for [key, explanation] in self.legend {
            let line = [
                "[".to_span(),
                Span::from(*key).light_yellow(),
                Span::from(format!("]: {explanation}")),
            ]
            .into_iter()
            .collect::<Line>()
            .centered();
            page.render_widget(line, window_area);
            window_area.y += 2;
        }
    }

    fn prompt(&mut self, _: &mut Grimoire, _: KeyCode) -> Result<Phase<Self::Tea>> {
        Ok(Phase::Return(()))
    }
}
