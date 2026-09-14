use std::io::Stdout;

use ratatui::{
    Terminal,
    buffer::Buffer,
    crossterm::event::{self, KeyModifiers},
    layout::Rect,
    prelude::CrosstermBackend,
    widgets::Widget,
};

use crate::{
    error::{Error, Result},
    resize,
};

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let mut window = Window::new();
    loop {
        terminal.draw(|frame| frame.render_widget(&window, frame.area()))?;
        match window.prompt(terminal)? {
            Status::Render => (),
            Status::Return(_) => return Ok(()),
            Status::Prompt => unreachable!(),
        }
    }
}

struct Window {}

impl Window {
    fn new() -> Self {
        Self {}
    }

    fn prompt(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<Status> {
        loop {
            let status = window::read_event(&mut |_, _| Ok(Status(())>), terminal)?;
            if status != Status::Prompt {
                return Ok(status)
            }
        }
    }
}

impl Widget for &Window {
    fn render(self, area: Rect, buf: &mut Buffer) {}
}
