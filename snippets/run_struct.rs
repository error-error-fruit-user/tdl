use std::io::Stdout;

use ratatui::{
    Terminal,
    buffer::Buffer,
    crossterm::event::{self, KeyModifiers},
    layout::Rect,
    prelude::CrosstermBackend,
    widgets::Widget,
};

use crate::{error::Error, handle_status, loop_prompt, render, resize};

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let mut window = Window::new();
    loop {
        render!(terminal, &window);
        loop_prompt!(terminal, window);
    }
}

struct Window {}

impl Window {
    fn new() -> Self {
        Self {}
    }

    fn prompt(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<Status> {
        window::read_event(terminal, &mut |_, _| Ok(Status::Return(())))
    }
}

impl Widget for &Window {
    fn render(self, area: Rect, buf: &mut Buffer) {}
}
