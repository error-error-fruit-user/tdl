//! Editing a line of text with a cursor in it, the same in every field: typing goes in at the
//! cursor, `←`/`→` move it, `Home`/`End` jump to either end, `Backspace` deletes the character
//! before it and `Delete` the one under it. The cursor counts characters, not bytes. The
//! controller's on-screen keyboard has two more: [`DELETE_WORD`] and [`CLEAR`], which are keys
//! no keyboard sends.

use ratatui::crossterm::event::KeyCode;

/// Takes the word before the cursor (and the spaces and marks between it and the cursor).
pub const DELETE_WORD: KeyCode = KeyCode::F(20);
/// Empties the field.
pub const CLEAR: KeyCode = KeyCode::F(21);

/// Takes one key for `text`, whose cursor is at character `caret`. A typed character goes through
/// `accept`, which turns it down (`None`) or says what to put in (to lowercase it, say), and only
/// goes in while `text` is shorter than `max` characters.
///
/// `None` if the key isn't one for editing text (or is a character `accept` turned down), so the
/// caller can give it another meaning; otherwise whether anything changed and needs drawing.
pub fn edit(
    text: &mut String,
    caret: &mut usize,
    key_code: KeyCode,
    max: usize,
    accept: impl Fn(char) -> Option<char>,
) -> Option<bool> {
    let length = text.chars().count();
    *caret = (*caret).min(length);
    match key_code {
        KeyCode::Char(c) => {
            let c = accept(c)?;
            if length >= max {
                return Some(false);
            }
            text.insert(byte_at(text, *caret), c);
            *caret += 1;
            Some(true)
        }
        KeyCode::Backspace if *caret > 0 => {
            *caret -= 1;
            text.remove(byte_at(text, *caret));
            Some(true)
        }
        KeyCode::Delete if *caret < length => {
            text.remove(byte_at(text, *caret));
            Some(true)
        }
        KeyCode::Backspace | KeyCode::Delete => Some(false),
        key if key == DELETE_WORD => {
            let chars: Vec<char> = text.chars().collect();
            let mut from = *caret;
            while from > 0 && !chars[from - 1].is_alphanumeric() {
                from -= 1;
            }
            while from > 0 && chars[from - 1].is_alphanumeric() {
                from -= 1;
            }
            let removed = *caret - from;
            text.replace_range(byte_at(text, from)..byte_at(text, *caret), "");
            *caret = from;
            Some(removed > 0)
        }
        key if key == CLEAR => {
            let changed = !text.is_empty();
            text.clear();
            *caret = 0;
            Some(changed)
        }
        KeyCode::Left => Some(move_to(caret, caret.saturating_sub(1))),
        KeyCode::Right => Some(move_to(caret, (*caret + 1).min(length))),
        KeyCode::Home => Some(move_to(caret, 0)),
        KeyCode::End => Some(move_to(caret, length)),
        _ => None,
    }
}

/// Where the cursor goes in `text` when its field is focused: at the end.
pub fn end(text: &str) -> usize {
    text.chars().count()
}

/// The part of `text` before character `caret`, to measure where the cursor is drawn.
pub fn before(text: &str, caret: usize) -> &str {
    &text[..byte_at(text, caret)]
}

/// The byte where character `index` starts, or the end of `text`.
fn byte_at(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(at, _)| at)
}

/// Moves the cursor to `to`, saying whether it moved.
fn move_to(caret: &mut usize, to: usize) -> bool {
    let moved = *caret != to;
    *caret = to;
    moved
}
