use std::io::Stdout;

use ratatui::{
    Terminal,
    buffer::Buffer,
    crossterm::event::{self},
    layout::{Alignment, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style},
    text::ToLine,
    widgets::{Block, BorderType, Borders, Widget},
};

use crate::{
    error::Result,
    window::{self, Status},
};

pub fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    message: &'static str,
) -> Result<bool> {
    let mut dialogue = Dialogue::new(message);
    loop {
        terminal.draw(|frame| frame.render_widget(&dialogue, frame.area()))?;
        match dialogue.prompt(terminal)? {
            Status::Render => (),
            Status::Return(answer) => return Ok(answer),
            Status::Prompt => unreachable!(),
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
    ) -> Result<Status<bool>> {
        loop {
            let status = window::read_event(
                &mut |key_code, _| match key_code {
                    event::KeyCode::Char('y') => Ok(Status::Return(true)),
                    event::KeyCode::Char('n') => Ok(Status::Return(false)),
                    _ => Ok(Status::Prompt),
                },
                terminal,
            )?;
            if status != Status::Prompt {
                return Ok(status);
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
