//! Small windows that ask or tell one thing, then get out of the way. They share one look: plain
//! border, a blank row around the text, and wrapped lines so they stay small on any terminal.

pub mod dialog;
pub mod legend;
pub mod progress;
pub mod resize;
pub mod status;
pub mod three_way_dialog;
pub mod warning;

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
};

use crate::{
    layout,
    ritual::{self, Page},
    theme,
};

/// The widest a popup's text gets before it wraps, so popups stay small.
pub const MAX_TEXT_WIDTH: u16 = 46;

/// `text` wrapped to fit a popup in `area`.
pub fn wrap(text: &str, area: Rect) -> Vec<String> {
    let room = area
        .width
        .saturating_sub(2 * layout::MARGIN + 2 + 2 * layout::PADDING);
    layout::wrap(text, MAX_TEXT_WIDTH.min(room) as usize)
}

/// The answers as `[Y]es  [N]o`: the first letter of each is the key that picks it.
pub fn answers(options: &[&str]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, option) in options.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let mut letters = option.chars();
        let first = letters.next().map(String::from).unwrap_or_default();
        spans.push(Span::styled("[", Style::new().fg(theme::MUTED)));
        spans.push(Span::styled(first, Style::new().fg(theme::KEY)));
        spans.push(Span::styled("]", Style::new().fg(theme::MUTED)));
        spans.push(Span::styled(
            letters.as_str().to_string(),
            Style::new().fg(theme::TEXT),
        ));
    }
    Line::from(spans)
}

/// Draws a popup: `lines` centered, a blank row above and below them, and `footer` (usually the
/// answers) one blank row further down.
pub fn draw(page: &mut Page, title: &str, border: Color, lines: &[Line], footer: Option<Line>) {
    let width = lines
        .iter()
        .chain(&footer)
        .chain([&Line::from(format!("[ {title} ]"))])
        .map(|line| line.width() as u16)
        .max()
        .unwrap_or(0);
    let height = lines.len() as u16 + 2 + footer.as_ref().map_or(0, |_| 2);
    let (width, height) = layout::window(width, height);
    let (veil, area, ..) = ritual::conjure_window(title, width, height, border, page.area());
    let veil = veil.border_type(theme::POPUP_BORDER);
    let inner = veil.inner(area);
    page.render_widget(veil, area);
    let row =
        |offset: u16| Rect::new(inner.x, inner.y + offset, inner.width, 1).intersection(inner);
    for (i, line) in lines.iter().enumerate() {
        page.render_widget(line.clone().centered(), row(1 + i as u16));
    }
    if let Some(footer) = footer {
        page.render_widget(footer.centered(), row(lines.len() as u16 + 2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_highlight_their_first_letter() {
        let line = answers(&["Yes", "No"]);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "[Y]es  [N]o");
        assert_eq!(line.spans[1].style.fg, Some(theme::KEY));
    }

    #[test]
    fn popup_text_wraps_to_stay_small() {
        let wide = Rect::new(0, 0, 200, 40);
        let lines = wrap(&"word ".repeat(40), wide);
        assert!(
            lines
                .iter()
                .all(|l| l.chars().count() <= MAX_TEXT_WIDTH as usize)
        );
        // and to fit a narrow terminal
        let narrow = Rect::new(0, 0, 30, 40);
        assert!(
            wrap(&"word ".repeat(40), narrow)
                .iter()
                .all(|l| l.chars().count() <= 24)
        );
    }
}
