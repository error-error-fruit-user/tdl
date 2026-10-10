use std::path::Path;

use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    config::Config,
    error::Result,
    joining, legend,
    profile::Profile,
    stage,
    utility::{self, Phase, Terminal},
};

const TITLE: &str = "Join";
const BORDER_COLOR: Color = Color::LightMagenta;
const WIDTH: u16 = 32;
const HEIGHT: u16 = 5;
const IP_LABEL: &str = "Ip Address";
const LEGEND: &[[&str; 2]] = &[["Enter", "Launch"], ["Esc", "Cancel"], ["0-9/.", "Input"]];

pub fn run(
    terminal: &mut Terminal,
    config: &mut Config,
    profiles: &mut Vec<Profile>,
    index: usize,
    config_dir: &Path,
    data_dir: &Path,
) -> Result<()> {
    let mut ip = String::new();
    stage![
        terminal,
        |frame| {
            let area = frame.area();
            let (window, mut window_area, indicator, indicator_area) =
                utility::new_window(TITLE, WIDTH, HEIGHT, BORDER_COLOR, area);
            frame.render_widget(window, window_area);
            frame.render_widget(&indicator, indicator_area);

            window_area.y += 2;
            let (line, ip_position) = utility::new_query(IP_LABEL, &ip, area.width, window_area.y);
            frame.render_widget(line, window_area);
            frame.set_cursor_position(ip_position);
        },
        &mut |terminal, key_code| {
            match key_code {
                KeyCode::Enter => {
                    joining::run(terminal, config, profiles, index, &ip, config_dir, data_dir)?;
                    Ok(Phase::Render)
                }
                KeyCode::Esc => Ok(Phase::Return(())),
                KeyCode::Char(c @ '0'..='9' | c @ '.') => {
                    ip.push(c);
                    Ok(Phase::Render)
                }
                KeyCode::Backspace => {
                    ip.pop();
                    Ok(Phase::Render)
                }
                KeyCode::Char('?') => {
                    legend::run(terminal, LEGEND)?;
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            }
        }
    ];
}
