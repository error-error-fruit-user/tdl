use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    error::Result,
    present,
    utility::{self, Phase, Terminal},
};

pub fn run(terminal: &mut Terminal) -> Result<()> {
    let mut window = Window::new();
    present![terminal, window];
}

struct Window {}

impl Window {
    fn new() -> Self {
        Self {}
    }

    fn prompt(&mut self, terminal: &mut Terminal) -> Result<Phase<()>> {
        window::process(terminal, &mut |_, _| Ok(Phase::Return(())))
    }
}

impl Widget for &Window {
    fn render(self, area: Rect, buf: &mut Buffer) {}
}
