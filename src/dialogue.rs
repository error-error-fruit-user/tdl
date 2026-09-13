use ratatui::{
    Terminal,
    buffer::Buffer,
    crossterm::event::{self, KeyModifiers},
    layout::{Alignment, Rect},
    prelude::Backend,
    style::{Color, Style},
    text::ToLine,
    widgets::{Block, BorderType, Borders, Widget},
};

use crate::{resize, window};

pub fn run(terminal: &mut Terminal<impl Backend>, message: &'static str) -> Option<bool> {
    let mut dialogue = Dialogue::new(message);
    while !dialogue.should_quit {
        terminal
            .draw(|frame| frame.render_widget(&dialogue, frame.area()))
            .unwrap();
        if let Some(answer) = dialogue.prompt(terminal) {
            return Some(answer);
        }
    }
    None
}

struct Dialogue {
    message: &'static str,
    should_quit: bool,
}

impl Dialogue {
    const LEGEND: &'static str = "[Y]es [N]o";

    fn new(message: &'static str) -> Self {
        Self {
            message,
            should_quit: false,
        }
    }

    fn prompt(&mut self, terminal: &mut Terminal<impl Backend>) -> Option<bool> {
        loop {
            let event = event::read().unwrap();
            if let event::Event::Resize(width, height) = event {
                if resize::should_resize(width, height) {
                    self.should_quit = resize::run(terminal);
                }
                return None;
            } else if let event::Event::Key(key) = event
                && key.is_press()
            {
                match key.code {
                    event::KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.should_quit = true;
                        return None;
                    }
                    event::KeyCode::Char('y') => return Some(true),
                    event::KeyCode::Char('n') => return Some(false),
                    _ => (),
                }
            }
        }
    }
}

impl Widget for &Dialogue {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let window = Block::new()
            .borders(Borders::all())
            .border_type(BorderType::Thick)
            .border_style(Style::new().fg(Color::Yellow));

        let width = self.message.len().max(Dialogue::LEGEND.len()) as u16 + 4;
        let height = 7;
        let mut window_area = window::new_window_area(width, height, area);

        window.render(window_area, buf);

        let message = self.message.to_line().alignment(Alignment::Center);
        let legend = Dialogue::LEGEND.to_line().alignment(Alignment::Center);

        window_area.y += 2;
        message.render(window_area, buf);
        window_area.y += 2;
        legend.render(window_area, buf);
    }
}
