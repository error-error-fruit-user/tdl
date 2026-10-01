use ratatui::{style::Color, text::ToLine};

use crate::{
    error::Result,
    stage,
    utility::{self, Phase, Terminal},
};

const TITLE: &'static str = "Warning";
const BORDER_COLOR: Color = Color::LightYellow;

pub fn run(terminal: &mut Terminal, message: &'static str) -> Result<()> {
    let width = message.len() as u16 + 4;
    let height = 5;
    stage![
        terminal,
        |frame| {
            let (window, mut window_area, ..) =
                utility::new_window(width, height, TITLE, BORDER_COLOR, frame.area());
            frame.render_widget(window, window_area);
            let message = message.to_line().centered();
            window_area.y += 2;
            frame.render_widget(message, window_area);
        },
        &mut |_, _| Ok(Phase::Return(()))
    ];
}

pub fn uzdoom_not_set(terminal: &mut Terminal) -> Result<()> {
    run(terminal, "UZDoom path not set!")
}
