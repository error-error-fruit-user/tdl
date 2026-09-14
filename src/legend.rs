use std::io::Stdout;

use ratatui::{
    Terminal,
    layout::Alignment,
    prelude::CrosstermBackend,
    style::{Color, Style},
    text::{Line, Span},
};

use crate::{
    error::Result,
    window::{self, Status},
};

pub fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    legend: &[[&'static str; 2]],
) -> Result<()> {
    loop {
        terminal.draw(|frame| {
            let width = 31;
            let height = legend.len() as u16 * 2 + 3;
            let (window, mut window_area, ..) =
                window::new_window(width, height, "Legend", Color::LightGreen, frame.area());
            frame.render_widget(&window, window_area);
            window_area.x += 1;
            window_area.width -= 2;
            window_area.y += 2;
            for [key, explanation] in legend {
                let line = Line::from(vec![
                    Span::from(format!("[{key}]")).style(Style::new().fg(Color::LightYellow)),
                    Span::from(format!(": {explanation}")),
                ])
                .alignment(Alignment::Center);
                frame.render_widget(line, window_area);
                window_area.y += 2;
            }
        })?;
        loop {
            let status = window::read_event(&mut |_, _| Ok(Status::Return(())), terminal)?;
            match status {
                Status::Render => break,
                Status::Return(_) => return Ok(()),
                Status::Prompt => (),
            }
        }
    }
}
