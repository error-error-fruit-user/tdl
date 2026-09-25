use std::io::Stdout;

use ratatui::{Terminal, prelude::CrosstermBackend};

use crate::{error::Result, stage, window::Phase};

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    stage![terminal, |frame| {}, &mut |_, _| Ok(Phase::Return(()))];
}
