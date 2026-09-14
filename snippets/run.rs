use std::io::Stdout;

use ratatui::{
    Terminal,
    crossterm::event::{self, KeyModifiers},
    prelude::CrosstermBackend,
};

use crate::{
    error::{Error, Result},
    resize,
    window::{self, Status},
};

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    loop {
        terminal.draw(|frame| {})?;
        loop {
            let status = window::read_event(&mut |_, _| Ok(Status::Return(())), terminal)?;
            match status {
                Status::Render => break,
                Status::Return(_) => return Ok(()),
                Status::Prompt => (),
            }
        }
    }
}
