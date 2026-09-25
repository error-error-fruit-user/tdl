use std::io::Stdout;

use ratatui::{
    Terminal,
    crossterm::event,
    layout::Alignment,
    prelude::CrosstermBackend,
    style::{Color, Style},
    text::{Line, Span},
};

use crate::{
    error::Result,
    legend, stage,
    window::{self, Phase},
};

const WIDTH: u16 = 32;
const HEIGHT: u16 = 5;
const LEGEND: &[[&'static str; 2]] = &[["Enter", "Launch"], ["Esc", "Cancel"], ["2-8", "Input"]];

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<Option<u8>> {
    let mut player_count = String::new();
    stage![
        terminal,
        |frame| {
            let (window, mut window_area, indicator, indicator_area) =
                window::new(WIDTH, HEIGHT, "Host", Color::LightMagenta, frame.area());
            frame.render_widget(window, window_area);
            frame.render_widget(&indicator, indicator_area);

            window_area.y += 2;
            let line = Line::from(vec![
                Span::from("Player Count: ").style(Style::new().fg(Color::LightYellow)),
                Span::from(&player_count),
            ])
            .alignment(Alignment::Center);
            frame.render_widget(line, window_area);
        },
        &mut |terminal, key_code| {
            match key_code {
                event::KeyCode::Enter => Ok(Phase::Return(Some(2))),
                event::KeyCode::Esc => Ok(Phase::Return(Some(2))),
                event::KeyCode::Char('?') => {
                    legend::run(terminal, LEGEND)?;
                    Ok(Phase::Render)
                }
                event::KeyCode::Char(c @ '2'..='8') if player_count.is_empty() => {
                    player_count.push(c);
                    Ok(Phase::Render)
                }
                event::KeyCode::Backspace => {
                    player_count.pop();
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            }
        }
    ];
}
