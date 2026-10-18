use ratatui::{
    crossterm::event::{self, KeyCode, KeyModifiers},
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
};

use crate::{
    error::{Error, Result},
    layout,
    ritual::{Grimoire, Haunted},
    theme,
};

/// Shows how much bigger the terminal has to be, until it is; Ctrl+C still quits.
pub fn summon(grimoire: &mut Grimoire) -> Result<()> {
    loop {
        grimoire.bewitch(|page| {
            let area = page.area();
            let middle = area.height / 2;
            let lines = [
                Line::styled(
                    "too small",
                    Style::new()
                        .fg(theme::worn().colors.bad)
                        .add_modifier(Modifier::BOLD),
                ),
                Line::styled(
                    format!("{}×{} needed", layout::MIN_WIDTH, layout::MIN_HEIGHT),
                    Style::new().fg(theme::worn().colors.muted),
                ),
                Line::styled(
                    format!("{}×{} now", area.width, area.height),
                    Style::new().fg(theme::worn().colors.muted),
                ),
            ];
            for (i, line) in lines.into_iter().enumerate() {
                let y = (middle + i as u16).saturating_sub(1);
                if y < area.height {
                    page.render_widget(line.centered(), Rect::new(0, y, area.width, 1));
                }
            }
        })?;
        let event = event::read()?;
        if let event::Event::Resize(width, height) = event {
            if !layout::is_too_small(width, height) {
                return Ok(());
            }
        } else if let event::Event::Key(key) = event
            && key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return Err(Error::Interrupted);
        }
    }
}
