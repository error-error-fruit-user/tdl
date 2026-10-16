use ratatui::{
    style::{Color, Style},
    text::Line,
};

use crate::{
    error::Result,
    popups,
    ritual::{Grimoire, Haunted},
    theme,
};

/// Draws a one-off message window, for work that blocks (hosting, joining, downloading).
pub fn summon(
    grimoire: &mut Grimoire,
    title: &str,
    message: &str,
    border_color: Color,
) -> Result<()> {
    grimoire.bewitch(|page| {
        let line = Line::styled(message, Style::new().fg(theme::TEXT));
        popups::draw(page, title, border_color, &[line], None);
    })
}
