use std::{io::Stdout, path::Path};

use ratatui::{
    Terminal, crossterm::event, layout::Position, prelude::CrosstermBackend, style::Color,
};

use crate::{
    error::Result,
    execute, legend,
    profile::Profile,
    stage,
    window::{self, Phase},
};

const TITLE: &'static str = "Join";
const WIDTH: u16 = 32;
const HEIGHT: u16 = 5;
const IP_LABEL: &'static str = "Ip Address";
const LEGEND: &[[&'static str; 2]] = &[["Enter", "Launch"], ["Esc", "Cancel"], ["0-9/.", "Input"]];

pub fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    uzdoom: &Path,
    profile: &Profile,
) -> Result<()> {
    let mut ip = String::new();
    stage![
        terminal,
        |frame| {
            let area = frame.area();
            let (window, mut window_area, indicator, indicator_area) =
                window::new(WIDTH, HEIGHT, TITLE, Color::LightMagenta, area);
            frame.render_widget(window, window_area);
            frame.render_widget(&indicator, indicator_area);

            window_area.y += 2;
            let line = window::new_input(IP_LABEL, &ip);
            let ip_position = Position::new(
                area.width / 2 + (IP_LABEL.len() + ip.len()) as u16 / 2 + 1,
                window_area.y,
            );
            frame.render_widget(line, window_area);
            frame.set_cursor_position(ip_position);
        },
        &mut |terminal, key_code| {
            match key_code {
                event::KeyCode::Enter => {
                    execute::join(uzdoom, profile, &ip)?;
                    Ok(Phase::Prompt)
                }
                event::KeyCode::Esc => Ok(Phase::Return(())),
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
