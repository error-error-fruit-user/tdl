use ratatui::{
    style::{Color, Stylize},
    text::ToLine,
};

use crate::{
    error::Result,
    stage,
    utility::{self, Phase, Terminal},
};

const TITLE: &str = "Warning";
const BORDER_COLOR: Color = Color::LightCyan;

pub fn run(terminal: &mut Terminal, message: &str) -> Result<()> {
    let width = message.len() as u16 + 4;
    let height = 5;
    stage![
        terminal,
        |frame| {
            let (window, mut window_area, ..) =
                utility::new_window(TITLE, width, height, BORDER_COLOR, frame.area());
            frame.render_widget(window, window_area);
            let message = message.to_line().centered().light_red();
            window_area.width -= 2;
            window_area.x += 1;
            window_area.y += 2;
            frame.render_widget(message, window_area);
        },
        &mut |_, _| Ok(Phase::Return(()))
    ];
}

pub fn uzdoom_not_selected(terminal: &mut Terminal) -> Result<()> {
    run(terminal, "No UZDoom path selected!")
}
