use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};

use crate::{
    error::{Error, Result},
    gamepad,
};

/// Lets the user back out of work that doesn't wait for keys itself, like a download.
///
/// It knows nothing about what is being cancelled: call [`CancelHandler::check`] between steps,
/// and show [`CancelHandler::HINT`] wherever the work is drawn.
pub struct CancelHandler;

impl CancelHandler {
    pub const HINT: &str = "[Esc] Cancel";

    /// Reads every key pressed since the last check, without waiting. Esc (or a controller's back
    /// button) is [`Error::Cancelled`] and Ctrl+C is [`Error::Interrupted`]; any other key is
    /// thrown away, so it isn't replayed on the next screen.
    pub fn check() -> Result<()> {
        // 👻 a spirit's keys are for the screens, never thrown away here
        if crate::ritual::possessed() {
            return Ok(());
        }
        while let Some(key_code) = gamepad::chant() {
            if key_code == KeyCode::Esc {
                return Err(Error::Cancelled);
            }
        }
        while event::poll(Duration::ZERO)? {
            if let Event::Key(key) = event::read()?
                && key.is_press()
            {
                if key.code == KeyCode::Esc {
                    return Err(Error::Cancelled);
                }
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Err(Error::Interrupted);
                }
            }
        }
        Ok(())
    }
}
