//! Labelled input fields, one per row: labels right-aligned in one column, values in the next.

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::{
    layout,
    ritual::{self, Page},
    theme,
};

/// Blank rows between fields.
const GAP: u16 = 1;

pub struct Field<'a> {
    pub label: &'a str,
    pub value: &'a str,
    /// Shown dimmed when the value is empty, to say what empty means.
    pub placeholder: &'a str,
    /// The narrowest the value column gets, so there is room to type.
    pub room: u16,
}

/// The space `fields` need.
pub fn size(fields: &[Field]) -> (u16, u16) {
    let label = fields
        .iter()
        .map(|f| layout::width(f.label))
        .max()
        .unwrap_or(0);
    let value = fields
        .iter()
        .map(|f| {
            layout::width(f.value)
                .max(layout::width(f.placeholder))
                .max(f.room)
        })
        .max()
        .unwrap_or(0);
    let rows = fields.len() as u16;
    (label + 2 + value + 1, rows + rows.saturating_sub(1) * GAP)
}

/// Draws the fields with the one at `focus` lit up, and returns where its cursor goes.
pub fn render(
    fields: &[Field],
    focus: Option<usize>,
    area: Rect,
    buffer: &mut Buffer,
) -> Option<Position> {
    let label_width = fields
        .iter()
        .map(|f| layout::width(f.label))
        .max()
        .unwrap_or(0);
    let (width, _) = size(fields);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let mut cursor = None;
    for (i, field) in fields.iter().enumerate() {
        let y = area.y + i as u16 * (1 + GAP);
        if y >= area.bottom() {
            break;
        }
        let focused = focus == Some(i);
        let label_style = if focused {
            Style::new().fg(theme::KEY).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme::MUTED)
        };
        let value = if field.value.is_empty() {
            Span::styled(field.placeholder, Style::new().fg(theme::MUTED))
        } else {
            Span::styled(field.value, Style::new().fg(theme::TEXT))
        };
        let padding = " ".repeat((label_width - layout::width(field.label)) as usize);
        let line = Line::from(vec![
            Span::raw(padding),
            Span::styled(field.label, label_style),
            Span::raw("  "),
            value,
        ]);
        line.render(Rect::new(x, y, area.right().saturating_sub(x), 1), buffer);
        if focused {
            let column = x + label_width + 2 + layout::width(field.value);
            cursor = Some(Position::new(column.min(area.right().saturating_sub(1)), y));
        }
    }
    cursor
}

/// A whole form window: `fields` with a blank row above and below, centered on the page, the
/// `[?]` hint underneath, and the cursor placed in the focused field.
pub fn draw(page: &mut Page, title: &str, border: Color, fields: &[Field], focus: usize) {
    let (width, height) = size(fields);
    let (width, height) = layout::window(width, height + 2);
    let (veil, area, sigil, sigil_area) =
        ritual::conjure_window(title, width, height, border, page.area());
    let inner = veil.inner(area);
    page.render_widget(veil, area);
    page.render_widget(sigil, sigil_area);
    let fields_area = Rect {
        y: inner.y + 1,
        height: inner.height.saturating_sub(2),
        ..inner
    };
    if let Some(cursor) = render(fields, Some(focus), fields_area, page.buffer_mut()) {
        page.set_cursor_position(cursor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields<'a>(value: &'a str) -> [Field<'a>; 2] {
        [
            Field {
                label: "players",
                value: "4",
                placeholder: "",
                room: 1,
            },
            Field {
                label: "map",
                value,
                placeholder: "auto",
                room: 8,
            },
        ]
    }

    #[test]
    fn the_size_fits_the_longest_label_and_value_with_a_gap_between_rows() {
        // "players" + 2 + max(1, "auto", 8) + 1 for the cursor, and two rows with one between
        assert_eq!(size(&fields("")), (7 + 2 + 8 + 1, 3));
        assert_eq!(size(&fields("MAP07_REDUX")), (7 + 2 + 11 + 1, 3));
    }

    #[test]
    fn the_cursor_goes_after_the_focused_value() {
        let area = Rect::new(0, 0, 18, 3);
        let mut buffer = Buffer::empty(area);
        let cursor = render(&fields("MAP07"), Some(1), area, &mut buffer);
        // labels are 7 wide, then two spaces, then "MAP07"
        assert_eq!(cursor, Some(Position::new(7 + 2 + 5, 2)));
        assert_eq!(render(&fields(""), None, area, &mut buffer), None);
    }

    #[test]
    fn labels_are_right_aligned() {
        let area = Rect::new(0, 0, 18, 3);
        let mut buffer = Buffer::empty(area);
        render(&fields(""), None, area, &mut buffer);
        let row = |y: u16| {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        };
        assert!(row(0).starts_with("players  4"));
        assert!(row(2).starts_with("    map  auto"));
    }
}
