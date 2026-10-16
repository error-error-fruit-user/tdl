//! The row under a screen's last window, around the `[F1/?]` in its middle: what is selected on the
//! left and what is copied on the right, so it is clear what `Y` and `P` would do.

use ratatui::{layout::Rect, style::Style, text::Line};

use crate::{layout, ritual::Page, theme};

/// Draws `left` and `right` at either end of `area`, each only if it stays clear of the `[F1/?]`.
pub fn render(left: Option<String>, right: Option<String>, area: Rect, page: &mut Page) {
    // 🧷 the `[F1/?]` takes six columns in the middle; keep a column of air on either side of it
    let room = area.width.saturating_sub(8) / 2;
    let fits = |note: &String| layout::width(note) <= room;
    if let Some(left) = left.filter(fits) {
        page.render_widget(Line::styled(left, Style::new().fg(theme::KEY)), area);
    }
    if let Some(right) = right.filter(fits) {
        page.render_widget(
            Line::styled(right, Style::new().fg(theme::MUTED)).right_aligned(),
            area,
        );
    }
}
