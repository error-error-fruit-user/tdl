//! Editing a line of text with a cursor in it, the same in every field: typing goes in at the
//! cursor, `←`/`→` move it, `Home`/`End` jump to either end, `Backspace` deletes the character
//! before it and `Delete` the one under it. The cursor counts characters, not bytes.

use ratatui::crossterm::event::KeyCode;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn any(c: char) -> Option<char> {
        Some(c)
    }

    /// Runs `keys` on `text` with the cursor starting at its end.
    fn typed(text: &str, keys: &[KeyCode]) -> (String, usize) {
        let mut text = text.to_string();
        let mut caret = end(&text);
        for &key in keys {
            edit(&mut text, &mut caret, key, 10, any);
        }
        (text, caret)
    }

    #[test]
    fn typing_goes_in_at_the_cursor() {
        let keys = [KeyCode::Left, KeyCode::Left, KeyCode::Char('X')];
        assert_eq!(typed("doom", &keys), ("doXom".into(), 3));
        assert_eq!(
            typed("doom", &[KeyCode::Home, KeyCode::Char('>')]),
            (">doom".into(), 1)
        );
    }

    #[test]
    fn backspace_and_delete_take_either_side_of_the_cursor() {
        let keys = [KeyCode::Left, KeyCode::Backspace];
        assert_eq!(typed("doom", &keys), ("dom".into(), 2));
        let keys = [KeyCode::Home, KeyCode::Delete];
        assert_eq!(typed("doom", &keys), ("oom".into(), 0));
        // nothing to take at the ends, and nothing changes
        let mut text = "doom".to_string();
        let mut caret = 0;
        assert_eq!(
            edit(&mut text, &mut caret, KeyCode::Backspace, 10, any),
            Some(false)
        );
        caret = 4;
        assert_eq!(
            edit(&mut text, &mut caret, KeyCode::Delete, 10, any),
            Some(false)
        );
        assert_eq!(text, "doom");
    }

    #[test]
    fn the_cursor_stops_at_the_ends() {
        let mut text = "ab".to_string();
        let mut caret = 2;
        assert_eq!(
            edit(&mut text, &mut caret, KeyCode::Right, 10, any),
            Some(false)
        );
        assert_eq!(
            edit(&mut text, &mut caret, KeyCode::Home, 10, any),
            Some(true)
        );
        assert_eq!(
            edit(&mut text, &mut caret, KeyCode::Left, 10, any),
            Some(false)
        );
        assert_eq!(
            edit(&mut text, &mut caret, KeyCode::End, 10, any),
            Some(true)
        );
        assert_eq!(caret, 2);
    }

    #[test]
    fn characters_are_counted_not_bytes() {
        let keys = [KeyCode::Left, KeyCode::Backspace, KeyCode::Char('e')];
        assert_eq!(typed("héé", &keys), ("heé".into(), 2));
        assert_eq!(before("héllo", 2), "hé");
        assert_eq!(before("héllo", 99), "héllo");
    }

    #[test]
    fn turned_down_and_other_keys_are_left_to_the_caller() {
        let mut text = String::new();
        let mut caret = 0;
        let digits = |c: char| c.is_ascii_digit().then_some(c);
        assert_eq!(
            edit(&mut text, &mut caret, KeyCode::Char('x'), 10, digits),
            None
        );
        assert_eq!(edit(&mut text, &mut caret, KeyCode::Tab, 10, digits), None);
        assert_eq!(
            edit(&mut text, &mut caret, KeyCode::Char('7'), 10, digits),
            Some(true)
        );
        // a character can be changed on the way in
        let lower = |c: char| Some(c.to_ascii_lowercase());
        assert_eq!(
            edit(&mut text, &mut caret, KeyCode::Char('Q'), 10, lower),
            Some(true)
        );
        assert_eq!(text, "7q");
    }

    #[test]
    fn a_full_field_takes_no_more_but_still_says_the_key_was_its() {
        let mut text = "12345".to_string();
        let mut caret = 5;
        assert_eq!(
            edit(&mut text, &mut caret, KeyCode::Char('6'), 5, any),
            Some(false)
        );
        assert_eq!(text, "12345");
    }

    #[test]
    fn a_cursor_past_the_end_is_brought_back_first() {
        let mut text = "ab".to_string();
        let mut caret = 9;
        assert_eq!(
            edit(&mut text, &mut caret, KeyCode::Char('c'), 10, any),
            Some(true)
        );
        assert_eq!((text.as_str(), caret), ("abc", 3));
    }
}
