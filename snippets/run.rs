use std::io::Stdout;

use ratatui::{Terminal, prelude::CrosstermBackend};

use crate::{error::Result, stage, window::Step};

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    stage![|frame| {}, &mut |_, _| Ok(Step::Return(()))];
}
