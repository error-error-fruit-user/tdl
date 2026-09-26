use std::io::Stdout;

use ratatui::{
    Terminal,
    crossterm::event,
    layout::Alignment,
    prelude::CrosstermBackend,
    style::{Color, Style},
    text::ToLine,
    widgets::{Block, BorderType, Borders},
};

use crate::{
    error::Result,
    stage,
    window::{self, Phase},
};

const LEGEND: &'static str = "[Y]es [N]o";

pub fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    message: &'static str,
) -> Result<bool> {
    let width = message.len().max(LEGEND.len()) as u16 + 4;
    let height = 7;
    stage![
        terminal,
        |frame| {
            let area = frame.area();
            let window = Block::new()
                .borders(Borders::all())
                .border_type(BorderType::Thick)
                .border_style(Style::new().fg(Color::Yellow));
            let mut window_area = window::new_area(width, height, area);
            frame.render_widget(window, window_area);
            let message = message.to_line().alignment(Alignment::Center);
            let legend = LEGEND.to_line().alignment(Alignment::Center);
            window_area.y += 2;
            frame.render_widget(message, window_area);
            window_area.y += 2;
            frame.render_widget(legend, window_area);
        },
        &mut |_, key_code| match key_code {
            event::KeyCode::Char('y') => Ok(Phase::Return(true)),
            event::KeyCode::Char('n') => Ok(Phase::Return(false)),
            _ => Ok(Phase::Prompt),
        }
    ];
}
