//! Where windows go and how big they are. Every window is sized from what it holds, then shrunk to
//! fit the terminal; lists that no longer fit scroll instead.

use ratatui::layout::{Margin, Rect};

/// The smallest terminal the interface is drawn in. Anything smaller shows the resize notice.
pub const MIN_WIDTH: u16 = 40;
pub const MIN_HEIGHT: u16 = 20;
/// Space kept free between windows and the edges of the terminal.
pub const MARGIN: u16 = 1;
/// Blank columns between a window's border and what is inside it.
pub const PADDING: u16 = 1;
/// Whether the terminal is smaller than the interface can be drawn in.
pub fn is_too_small(width: u16, height: u16) -> bool {
    width < MIN_WIDTH || height < MIN_HEIGHT
}

/// The outer size of a window whose contents are `width` x `height`: its borders and padding added.
pub fn window(width: u16, height: u16) -> (u16, u16) {
    (width + 2 + 2 * PADDING, height + 2)
}

/// The part of `area` windows may use: everything inside the margin.
fn usable(area: Rect) -> Rect {
    area.inner(Margin::new(MARGIN, MARGIN))
}

/// A `width` x `height` box centered in `area`, shrunk to fit if `area` is smaller.
pub fn center(width: u16, height: u16, area: Rect) -> Rect {
    let usable = usable(area);
    let width = width.min(usable.width);
    let height = height.min(usable.height);
    Rect::new(
        usable.x + (usable.width - width) / 2,
        usable.y + (usable.height - height) / 2,
        width,
        height,
    )
}

/// How far a main window (the launcher's list, the editor) grows into the free space, as a share of
/// it, and the widest it gets. A window never shrinks below what it holds.
pub const GROW_PERCENT: u16 = 70;
pub const MAX_GROWN_WIDTH: u16 = 100;

/// The size of a main window that holds `width` x `height`, grown into the room left on the page
/// once `below` rows are kept for the windows under it.
pub fn grow(width: u16, height: u16, below: u16, area: Rect) -> (u16, u16) {
    let usable = usable(area);
    let share = |room: u16| (room as u32 * GROW_PERCENT as u32 / 100) as u16;
    let room_height = usable.height.saturating_sub(1 + below);
    (
        width.max(share(usable.width).min(MAX_GROWN_WIDTH)),
        height.max(share(room_height)),
    )
}

/// Windows of the given sizes stacked top to bottom, each centered across the page and the stack
/// centered down it, with one row left under the last for its `[F1/?]` hint. When the terminal is too
/// short, the first window (the list, which can scroll) gives up rows first.
pub fn stack(sizes: &[(u16, u16)], area: Rect) -> Vec<Rect> {
    let usable = usable(area);
    let room = usable.height.saturating_sub(1);
    let rest: u16 = sizes.iter().skip(1).map(|&(_, h)| h).sum();
    let first = sizes.first().map_or(0, |&(_, first)| {
        first.min(room.saturating_sub(rest)).max(first.min(3))
    });
    let total = rest.saturating_add(first);
    let mut y = usable.y + room.saturating_sub(total) / 2;
    sizes
        .iter()
        .enumerate()
        .map(|(i, &(width, height))| {
            let width = width.min(usable.width);
            let height = if i == 0 { first } else { height };
            let height = height.min(usable.bottom().saturating_sub(y));
            let rect = Rect::new(usable.x + (usable.width - width) / 2, y, width, height);
            y += height;
            rect
        })
        .collect()
}

/// The first row to show in a list `height` rows tall, starting from `offset`, so that `selected`
/// is on screen. Scrolls as little as possible, the way the list itself does.
pub fn scroll(offset: usize, selected: usize, height: usize) -> usize {
    if selected < offset {
        selected
    } else if height > 0 && selected >= offset + height {
        selected + 1 - height
    } else {
        offset
    }
}

/// `bytes` in the largest unit there is at least one of, to one decimal.
pub fn show_size(bytes: u64) -> String {
    match bytes {
        1_000_000_000.. => format!("{:.1} GB", bytes as f64 / 1e9),
        1_000_000.. => format!("{:.1} MB", bytes as f64 / 1e6),
        1_000.. => format!("{:.1} KB", bytes as f64 / 1e3),
        _ => format!("{bytes} B"),
    }
}

/// The display width of `text`.
pub fn width(text: &str) -> u16 {
    ratatui::text::Line::from(text).width() as u16
}

/// `text` broken into lines no wider than `width`, at spaces where possible.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        while word.chars().count() > width {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            let rest = word.split_off(
                word.char_indices()
                    .nth(width)
                    .map_or(word.len(), |(i, _)| i),
            );
            lines.push(std::mem::replace(&mut word, rest));
        }
        let needed = line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
        if needed > width && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// The last `max` characters of `text`, with a `…` in front if the start was cut off, so the end
/// of a long path (the part that matters) stays visible.
pub fn tail(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        text.to_string()
    } else if max == 0 {
        String::new()
    } else {
        format!(
            "…{}",
            text.chars().skip(count - (max - 1)).collect::<String>()
        )
    }
}
