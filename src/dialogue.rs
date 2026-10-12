use ratatui::{
    crossterm::event::KeyCode,
    style::{Color, Stylize},
    text::{Line, ToLine, ToSpan},
};

use crate::{
    error::Result,
    stage,
    utility::{self, Phase, Terminal},
};

const TITLE: &str = "Dialogue";
const BORDER_COLOR: Color = Color::LightCyan;

pub fn run(terminal: &mut Terminal, message: &str) -> Result<bool> {
    let width = message.len().max(10) as u16 + 4;
    let height = 7;
    stage![
        terminal,
        |frame| {
            let (window, mut window_area, ..) =
                utility::new_window(TITLE, width, height, BORDER_COLOR, frame.area());
            let legend = [
                "[".to_span(),
                "Y".to_span().light_yellow(),
                "]es [".to_span(),
                "N".to_span().light_yellow(),
                "]o".to_span(),
            ]
            .into_iter()
            .collect::<Line>()
            .centered();
            frame.render_widget(window, window_area);
            let message = message.to_line().centered();
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

pub fn should_delete(terminal: &mut Terminal, element: &str) -> Result<bool> {
    run(
        terminal,
        &format!("Are you sure you want to delete '{element}'?"),
    )
}

pub fn should_quit(terminal: &mut Terminal) -> Result<bool> {
    run(terminal, "Are you sure you want to quit?")
}
