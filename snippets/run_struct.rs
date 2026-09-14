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
        if window.prompt(terminal)? {
            return Err(Error::Exit);
        }
    }
}

struct Window {}

impl Window {
    fn new() -> Self {
        Self {}
    }

    fn prompt(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<bool> {
        loop {
            let event = event::read()?;
            if let event::Event::Resize(width, height) = event {
                if resize::should_resize(width, height) {
                    resize::run(terminal)?;
                }
                return Ok(false);
            } else if let event::Event::Key(key) = event
                && key.is_press()
            {
                match key.code {
                    event::KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Err(Error::Exit);
                    }
                    _ => (),
                }
            }
        }
    }
}

impl Widget for &Window {
    fn render(self, area: Rect, buf: &mut Buffer) {}
}
