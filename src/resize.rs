use ratatui::{
    crossterm::event::{self, KeyCode, KeyModifiers},
    layout::Rect,
    text::ToLine,
};

use crate::{
    error::{Error, Result},
    ritual::{Grimoire, Haunted},
};

const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 30;

/// Whether the terminal is smaller than the UI can be drawn in.
pub fn should_resize(width: u16, height: u16) -> bool {
    width < MIN_WIDTH || height < MIN_HEIGHT
}

/// Shows "Text too big!" until the terminal is large enough again; Ctrl+C still quits.
pub fn summon(grimoire: &mut Grimoire) -> Result<()> {
    loop {
        grimoire.bewitch(|page| {
            let area = page.area();
            page.render_widget(
                "Text too big!".to_line().centered(),
                Rect::new(0, area.height / 2, area.width, 1),
            );
        })?;
        let event = event::read()?;
        if let event::Event::Resize(width, height) = event {
            if !should_resize(width, height) {
                return Ok(());
            }
        } else if let event::Event::Key(key) = event
            && key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return Err(Error::Exit);
        }
    }
}
