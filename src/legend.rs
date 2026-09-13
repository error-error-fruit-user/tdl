use ratatui::{
    Terminal,
    crossterm::event::{self, KeyModifiers},
    layout::Alignment,
    prelude::Backend,
    style::{Color, Style},
    text::{Line, Span},
};

use crate::{resize, window};

pub fn run(terminal: &mut Terminal<impl Backend>, legend: &[[&'static str; 2]]) -> bool {
    loop {
        terminal
            .draw(|frame| {
                let width = 31;
                let height = legend.len() as u16 * 2 + 3;
                let (window, mut window_area, _) =
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
            })
            .unwrap();
        loop {
            let event = event::read().unwrap();
            if let event::Event::Resize(width, height) = event {
                if resize::should_resize(width, height) {
                    return resize::run(terminal);
                }
                break;
            } else if let event::Event::Key(key) = event
                && key.code == event::KeyCode::Char('c')
                && key.modifiers.contains(KeyModifiers::CONTROL)
            {
                return true;
            } else {
                return false;
            }
        }
    }
}
