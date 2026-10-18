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
    widgets::text,
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
    /// Where the cursor is in the value, in characters, when the field has focus: see
    /// [`crate::widgets::text`]. `None` for a field that isn't typed into (a choice, a file), which
    /// shows no cursor, since there is nothing to type at.
    pub caret: Option<usize>,
}

/// A button under the fields, pressed with Enter when it has focus.
pub struct Button<'a> {
    pub label: &'a str,
    /// A disabled button is drawn dimmed, but can still take focus, so pressing it can say why
    /// nothing happens.
    pub enabled: bool,
}

/// Gaps between buttons on their row.
const BUTTON_GAP: &str = "  ";

/// The buttons as one line, each in brackets: the focused one (`focus` counts buttons only) as the
/// cursor, a disabled one dimmed.
fn button_row<'a>(buttons: &[Button<'a>], focus: Option<usize>) -> Line<'a> {
    let mut spans = Vec::new();
    for (i, button) in buttons.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(BUTTON_GAP));
        }
        let style = if focus == Some(i) {
            theme::worn().styles.cursor
        } else if button.enabled {
            Style::new().fg(theme::worn().colors.text)
        } else {
            Style::new().fg(theme::worn().colors.muted)
        };
        spans.push(Span::styled(format!("[ {} ]", button.label), style));
    }
    Line::from(spans)
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

/// Draws the fields with the one at `focus` lit up, and returns where its cursor goes. Fields that
/// don't all fit scroll, so the focused one is always in sight.
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
    let fit = usize::from((area.height + GAP) / (1 + GAP)).max(1);
    let first = focus.map_or(0, |focus| (focus + 1).saturating_sub(fit));
    for (i, field) in fields.iter().enumerate().skip(first) {
        let y = area.y + (i - first) as u16 * (1 + GAP);
        if y >= area.bottom() {
            break;
        }
        let focused = focus == Some(i);
        let label_style = if focused {
            Style::new()
                .fg(theme::worn().colors.key)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme::worn().colors.muted)
        };
        let value = if field.value.is_empty() {
            Span::styled(
                field.placeholder,
                Style::new().fg(theme::worn().colors.muted),
            )
        } else {
            Span::styled(field.value, Style::new().fg(theme::worn().colors.text))
        };
        let padding = " ".repeat((label_width - layout::width(field.label)) as usize);
        let line = Line::from(vec![
            Span::raw(padding),
            Span::styled(field.label, label_style),
            Span::raw("  "),
            value,
        ]);
        line.render(Rect::new(x, y, area.right().saturating_sub(x), 1), buffer);
        if focused && let Some(caret) = field.caret {
            let typed = text::before(field.value, caret);
            let column = x + label_width + 2 + layout::width(typed);
            cursor = Some(Position::new(column.min(area.right().saturating_sub(1)), y));
        }
    }
    cursor
}

/// A whole form window: `fields` with a blank row above and below, centered on the page, the
/// `[F1/?]` hint underneath, and the cursor placed in the focused field.
pub fn draw(page: &mut Page, title: &str, border: Color, fields: &[Field], focus: usize) {
    draw_with_buttons(page, title, border, fields, &[], focus);
}

/// [`draw`], with a row of `buttons` under the fields. `focus` counts the fields and then the
/// buttons, so focus past the last field is on a button, and no cursor is shown.
pub fn draw_with_buttons(
    page: &mut Page,
    title: &str,
    border: Color,
    fields: &[Field],
    buttons: &[Button],
    focus: usize,
) {
    let (fields_width, fields_height) = size(fields);
    let buttons_line = button_row(buttons, focus.checked_sub(fields.len()));
    // a blank row, then the buttons
    let buttons_height = if buttons.is_empty() { 0 } else { 2 };
    let width = fields_width.max(buttons_line.width() as u16);
    let (width, height) = layout::window(width, fields_height + buttons_height + 2);
    let (veil, area, sigil, sigil_area) =
        ritual::conjure_window(title, width, height, border, page.area());
    let inner = veil.inner(area);
    page.render_widget(veil, area);
    page.render_widget(sigil, sigil_area);
    let fields_area = Rect {
        y: inner.y + 1,
        height: fields_height.min(inner.height.saturating_sub(2)),
        ..inner
    };
    let field_focus = (focus < fields.len()).then_some(focus);
    if let Some(cursor) = render(fields, field_focus, fields_area, page.buffer_mut()) {
        page.set_cursor_position(cursor);
    }
    if !buttons.is_empty() {
        let row = Rect::new(inner.x, fields_area.bottom() + 1, inner.width, 1).intersection(inner);
        page.render_widget(buttons_line.centered(), row);
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
                caret: None,
            },
            Field {
                label: "map",
                value,
                placeholder: "auto",
                room: 8,
                caret: Some(value.chars().count()),
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
    fn the_cursor_goes_where_the_caret_is_and_a_field_that_isnt_typed_into_has_none() {
        let area = Rect::new(0, 0, 18, 3);
        let mut buffer = Buffer::empty(area);
        let mut middle = fields("MAP07");
        middle[1].caret = Some(3);
        assert_eq!(
            render(&middle, Some(1), area, &mut buffer),
            Some(Position::new(7 + 2 + 3, 2))
        );
        // "players" is a choice, with no caret, so focusing it shows no cursor
        assert_eq!(render(&fields("MAP07"), Some(0), area, &mut buffer), None);
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
