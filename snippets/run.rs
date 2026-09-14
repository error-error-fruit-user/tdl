use std::io::Stdout;

use ratatui::{
    Terminal,
    crossterm::event::{self, KeyModifiers},
    prelude::CrosstermBackend,
};

use crate::{
    error::{Error, Result},
    resize,
};

pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    loop {
        terminal.draw(|frame| {})?;
        loop {
            let event = event::read()?;
            if let event::Event::Resize(width, height) = event {
                if resize::should_resize(width, height) {
                    return resize::run(terminal);
                }
                break;
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
