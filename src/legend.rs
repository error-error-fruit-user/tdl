use ratatui::{
    style::{Color, Stylize},
    text::{Line, Span, ToSpan},
};

use crate::{
    error::Result,
    stage,
    utility::{self, Phase, Terminal},
};

const TITLE: &str = "Legend";
const BORDER_COLOR: Color = Color::LightGreen;

pub fn run(terminal: &mut Terminal, legend: &[[&str; 2]]) -> Result<()> {
    let width = 32;
    let height = legend.len() as u16 * 2 + 3;
    stage![
        terminal,
        |frame| {
            let (window, mut window_area, ..) =
                utility::new_window(TITLE, width, height, BORDER_COLOR, frame.area());
            frame.render_widget(&window, window_area);
            window_area.x += 1;
            window_area.width -= 2;
            window_area.y += 2;
            for [key, explanation] in legend {
                let line = [
                    "[".to_span(),
                    Span::from(format!("{key}")).light_yellow(),
                    Span::from(format!("]: {explanation}")),
                ]
                .into_iter()
                .collect::<Line>()
                .centered();
                frame.render_widget(line, window_area);
                window_area.y += 2;
            }
        },
        &mut |_, _| Ok(Phase::Return(()))
    ];
}
