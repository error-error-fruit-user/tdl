use std::io::Stdout;

use ratatui::{
    Terminal,
    buffer::Buffer,
    crossterm::event::{self, KeyModifiers},
    layout::{Alignment, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style},
    text::ToLine,
    widgets::{Block, BorderType, Borders, Widget},
};

use crate::{
    error::{Error, Result},
    resize, window,
};

pub fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    message: &'static str,
) -> Result<bool> {
    let mut dialogue = Dialogue::new(message);
    loop {
        terminal.draw(|frame| frame.render_widget(&dialogue, frame.area()))?;
        if let Some(answer) = dialogue.prompt(terminal)? {
            return Ok(answer);
        }
    }
}

struct Dialogue {
    message: &'static str,
}

impl Dialogue {
    const LEGEND: &'static str = "[Y]es [N]o";

    fn new(message: &'static str) -> Self {
        Self { message }
    }

    fn prompt(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<Option<bool>> {
        loop {
            let event = event::read()?;
            if let event::Event::Resize(width, height) = event {
                if resize::should_resize(width, height) {
                    resize::run(terminal)?;
                    return Ok(None);
                }
                return Ok(None);
            } else if let event::Event::Key(key) = event
                && key.is_press()
            {
                match key.code {
                    event::KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Err(Error::Exit);
                    }
                    event::KeyCode::Char('y') => return Ok(Some(true)),
                    event::KeyCode::Char('n') => return Ok(Some(false)),
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
