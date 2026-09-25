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
const LEGEND: &[[&'static str; 2]] = &[["Enter", "Launch"], ["Esc", "Cancel"], ["0-9/.", "Input"]];

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<Option<String>> {
    let mut ip = String::new();
    stage![
        terminal,
        |frame| {
            let (window, mut window_area, indicator, indicator_area) =
                window::new(WIDTH, HEIGHT, "Join", Color::LightMagenta, frame.area());
            frame.render_widget(window, window_area);
            frame.render_widget(&indicator, indicator_area);

            window_area.y += 2;
            let line = Line::from(vec![
                Span::from("Ip Address: ").style(Style::default().fg(Color::Yellow)),
                Span::from(&ip),
            ])
            .alignment(Alignment::Center);
            frame.render_widget(line, window_area);
        },
        &mut |terminal, key_code| {
            match key_code {
                event::KeyCode::Enter => Ok(Phase::Return(Some(ip.clone()))),
                event::KeyCode::Esc => Ok(Phase::Return(None)),
                event::KeyCode::Char('?') => {
                    legend::run(terminal, LEGEND)?;
                    Ok(Phase::Render)
                }
                event::KeyCode::Char(c @ '0'..='9' | c @ '.') => {
                    ip.push(c);
                    Ok(Phase::Render)
                }
                event::KeyCode::Backspace => {
                    ip.pop();
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            }
        }
    ];
}
