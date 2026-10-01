use ratatui::{crossterm::event::KeyCode, style::Color, text::ToLine};

use crate::{
    error::Result,
    stage,
    utility::{self, Phase, Terminal},
};

const TITLE: &'static str = "Dialogue";
const BORDER_COLOR: Color = Color::LightYellow;
const LEGEND: &'static str = "[Y]es [N]o";

pub fn run(terminal: &mut Terminal, message: &'static str) -> Result<bool> {
    let width = message.len().max(LEGEND.len()) as u16 + 4;
    let height = 7;
    stage![
        terminal,
        |frame| {
            let (window, mut window_area, ..) =
                utility::new_window(width, height, TITLE, BORDER_COLOR, frame.area());
            frame.render_widget(window, window_area);
            let message = message.to_line().centered();
            let legend = LEGEND.to_line().centered();
            window_area.y += 2;
            frame.render_widget(message, window_area);
            window_area.y += 2;
            frame.render_widget(legend, window_area);
        },
        &mut |_, key_code| match key_code {
            KeyCode::Char('y') => Ok(Phase::Return(true)),
            KeyCode::Char('n') => Ok(Phase::Return(false)),
            _ => Ok(Phase::Prompt),
        }
    ];
}

pub fn should_delete(terminal: &mut Terminal) -> Result<bool> {
    run(terminal, "Are you sure you want to delete?")
}

pub fn should_quit(terminal: &mut Terminal) -> Result<bool> {
    run(terminal, "Are you sure you want to quit?")
}
