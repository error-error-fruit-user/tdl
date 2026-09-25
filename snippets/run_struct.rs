use std::io::Stdout;

use ratatui::{Terminal, buffer::Buffer, layout::Rect, prelude::CrosstermBackend, widgets::Widget};

use crate::{error::Result, present, window::Phase};

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let mut window = Window::new();
    present![terminal, window];
}

struct Window {}

impl Window {
    fn new() -> Self {
        Self {}
    }

    fn prompt(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<Step<()>> {
        window::process(terminal, &mut |_, _| Ok(Phase::Return(())))
    }
}

impl Widget for &Window {
    fn render(self, area: Rect, buf: &mut Buffer) {}
}
