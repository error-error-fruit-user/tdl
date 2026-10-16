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
/// centered down it, with one row left under the last for its `[?]` hint. When the terminal is too
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

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect::new(0, 0, 80, 24);

    #[test]
    fn a_window_adds_its_borders_and_padding() {
        assert_eq!(window(10, 3), (14, 5));
    }

    #[test]
    fn a_box_is_centered_inside_the_margin() {
        assert_eq!(center(20, 10, SCREEN), Rect::new(30, 7, 20, 10));
    }

    #[test]
    fn a_box_too_big_for_the_terminal_shrinks_to_the_margin() {
        assert_eq!(center(500, 500, SCREEN), Rect::new(1, 1, 78, 22));
    }

    #[test]
    fn centering_respects_an_area_that_is_not_at_the_origin() {
        let area = Rect::new(10, 5, 40, 20);
        assert_eq!(center(10, 4, area), Rect::new(25, 13, 10, 4));
    }

    #[test]
    fn stacked_windows_sit_together_with_a_row_left_for_the_hint() {
        let rects = stack(&[(30, 10), (30, 6)], SCREEN);
        assert_eq!(rects, [Rect::new(25, 3, 30, 10), Rect::new(25, 13, 30, 6)]);
    }

    #[test]
    fn stacked_windows_of_different_widths_are_each_centered() {
        let rects = stack(&[(50, 10), (20, 6)], SCREEN);
        assert_eq!(rects, [Rect::new(15, 3, 50, 10), Rect::new(30, 13, 20, 6)]);
    }

    #[test]
    fn the_first_stacked_window_shrinks_when_the_terminal_is_short() {
        let rects = stack(&[(30, 40), (30, 6)], SCREEN);
        // 22 usable rows, one for the hint, six for the second window
        assert_eq!(rects[0].height, 15);
        assert_eq!(rects[1], Rect::new(25, 16, 30, 6));
    }

    #[test]
    fn stacking_never_draws_past_the_terminal() {
        for rect in stack(&[(30, 3), (30, 50)], Rect::new(0, 0, 40, 10)) {
            assert!(rect.bottom() <= 9);
        }
    }

    #[test]
    fn a_main_window_grows_into_the_free_space() {
        // 78 x 22 usable; 22 - 1 hint row - 6 below = 15 rows of room
        assert_eq!(grow(20, 5, 6, SCREEN), (54, 10));
        // but never smaller than what it holds
        assert_eq!(grow(70, 14, 6, SCREEN), (70, 14));
    }

    #[test]
    fn a_main_window_stops_growing_at_the_widest_width() {
        let huge = Rect::new(0, 0, 400, 100);
        assert_eq!(grow(20, 5, 6, huge).0, MAX_GROWN_WIDTH);
    }

    #[test]
    fn wrapping_breaks_at_spaces() {
        assert_eq!(
            wrap("rip and tear until it is done", 12),
            ["rip and tear", "until it is", "done"]
        );
    }

    #[test]
    fn a_word_longer_than_the_line_is_broken() {
        assert_eq!(wrap("aaaaaaaaaa b", 4), ["aaaa", "aaaa", "aa b"]);
    }

    #[test]
    fn wrapping_nothing_gives_one_empty_line() {
        assert_eq!(wrap("", 10), [""]);
    }

    #[test]
    fn long_text_keeps_its_ending() {
        assert_eq!(tail("short", 10), "short");
        assert_eq!(tail("/a/very/long/path/to/file.pk3", 12), "…to/file.pk3");
    }

    #[test]
    fn the_minimum_size_is_enforced() {
        assert!(is_too_small(MIN_WIDTH - 1, MIN_HEIGHT));
        assert!(is_too_small(MIN_WIDTH, MIN_HEIGHT - 1));
        assert!(!is_too_small(MIN_WIDTH, MIN_HEIGHT));
    }
}
