use ratatui::{
    Frame,
    crossterm::event::KeyCode,
    style::{Color, Stylize},
    text::{Line, Span, ToSpan},
};

use crate::{
    error::Result,
    scene::Scene,
    utility::{self, Phase, Terminal},
};

pub struct Legend<'a> {
    legend: &'a [[&'a str; 2]],
    height: u16,
}

impl<'a> Legend<'a> {
    const TITLE: &str = "Legend";
    const WIDTH: u16 = 32;
    const BORDER_COLOR: Color = Color::LightGreen;

    pub fn new(legend: &'a [[&'a str; 2]]) -> Self {
        Self {
            legend,
            height: legend.len() as u16 * 2 + 5,
        }
    }
}

impl Legend<'_> {
    pub fn evoke(terminal: &mut Terminal, legend: &[[&str; 2]]) -> Result<()> {
        Legend::new(legend).invoke(terminal)
    }
}

impl<'a> Scene for Legend<'a> {
    type Return = ();

    fn render(&mut self, frame: &mut Frame) {
        let (window, mut window_area, ..) = utility::new_window(
            Self::TITLE,
            Self::WIDTH,
            self.height,
            Self::BORDER_COLOR,
            frame.area(),
        );
        frame.render_widget(&window, window_area);
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
            frame.render_widget(line, window_area);
            window_area.y += 2;
        }
    }

    fn prompt(&mut self, _: &mut Terminal, _: KeyCode) -> Result<Phase<Self::Tea>> {
        Ok(Phase::Return(()))
    }
}
