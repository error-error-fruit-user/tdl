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
    handle_status, resize,
};

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let mut window = Window::new();
    loop {
        terminal.draw(|frame| frame.render_widget(&window, frame.area()))?;
        loop {
            handle_status![window.prompt(terminal)?];
        }
    }
}

struct Window {}

impl Window {
    fn new() -> Self {
        Self {}
    }

    fn prompt(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<Status> {
        window::read_event(terminal, &mut |_, _| Ok(Status(())))
    }
}

impl Widget for &Window {
    fn render(self, area: Rect, buf: &mut Buffer) {}
}
