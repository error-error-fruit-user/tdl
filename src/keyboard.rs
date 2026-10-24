//! The controller's on-screen keyboard. It is never brought up by itself: in a text field a line
//! says Select brings it up, and then it is drawn over the bottom of the field being typed in, and
//! its buttons become the keys that field already knows (characters, `Backspace`, `←`/`→`,
//! `Enter`...), so no field needs to know it is there: the ritual loop asks a ritual
//! whether it is [`typing`](crate::ritual::Ritual::typing) and, if a controller spoke last, shows
//! this and feeds what it makes to the ritual instead of the plain keys.
//!
//! The d-pad (or the left stick) moves over the keys. Alone, the buttons are: A types the key under
//! the cursor, X erases, Y is a space, B types an underscore, L3 goes to the symbols and back to
//! the letters last up, R3 shifts between lower and upper case, LB/RB move the text cursor, Start
//! lists them all (the keyboard shows only the keys, and a line saying so; `OK`, on the keys, and
//! RT with A are `Enter`), and Select hides the keyboard (while it is hidden every button is what
//! it is on any other screen, so a form can be left, and Select brings the keyboard back). Held
//! with a trigger they combine: LT with A types the key as a capital, with LB/RB goes to the
//! start/end of the text, with X erases a word, with B clears the field; RT with A is `Enter`, with
//! B `Esc`, with LB/RB goes to the start/end of the text too, with the d-pad up/down to the
//! previous/next field.

use std::cell::Cell;

use gilrs::Button;
use ratatui::{
    crossterm::event::KeyCode,
    layout::{Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::{
    gamepad::{Family, Prompt},
    layout,
    ritual::{self, Page},
    theme,
    widgets::text,
};

/// Which keys are on offer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Layer {
    #[default]
    Lower,
    Upper,
    Symbols,
}

impl Layer {
    /// The four rows of ten characters.
    fn rows(self) -> [&'static str; 4] {
        match self {
            Self::Lower => ["1234567890", "qwertyuiop", "asdfghjkl'", "zxcvbnm._-"],
            Self::Upper => ["1234567890", "QWERTYUIOP", "ASDFGHJKL\"", "ZXCVBNM;:_"],
            Self::Symbols => ["!@#$%^&*()", "-_=+[]{}|\\", ";:'\"<>/?~`", ",.()[]*&^%"],
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Lower => "abc",
            Self::Upper => "ABC",
            Self::Symbols => "#+=",
        }
    }
}

/// The keys of the last row, which are not characters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Control {
    Shift,
    Symbols,
    Space,
    Erase,
    Ok,
}

const CONTROLS: [Control; 5] = [
    Control::Shift,
    Control::Symbols,
    Control::Space,
    Control::Erase,
    Control::Ok,
];

/// How many keys a row has, and how wide each key is drawn.
const COLUMNS: usize = 10;
const KEY_WIDTH: usize = 3;
/// A control is as wide as two character keys.
const CONTROL_WIDTH: usize = 2 * KEY_WIDTH;
/// The last row's place.
const BOTTOM: usize = 4;

/// What the buttons do while the keyboard is up, for the legend that `Start` opens. Every key is
/// named by its button, since this is only shown to a controller.
pub const LEGEND: &[[&str; 2]] = &[
    ["↑⟨up⟩/↓⟨down⟩/←⟨left⟩/→⟨right⟩", "Move Over the Keys"],
    ["A⟨a⟩", "Type the Key"],
    ["X⟨x⟩", "Erase"],
    ["Y⟨y⟩", "Space"],
    ["B⟨b⟩", "Underscore"],
    ["LB⟨lb⟩/RB⟨rb⟩", "Move the Text Cursor"],
    ["L3⟨l3⟩", "Symbols/the Letters Last Up"],
    ["R3⟨r3⟩", "Capitals/Lower Case"],
    ["Select⟨select⟩", "Put the Keyboard Away/Back"],
    ["Start⟨start⟩", "This List"],
    ["⟨lt+a⟩", "Type the Key as a Capital"],
    ["⟨lt+lb⟩/⟨lt+rb⟩", "Start/End of the Text"],
    ["⟨lt+x⟩", "Erase a Word"],
    ["⟨lt+b⟩", "Clear the Field"],
    ["⟨rt+a⟩", "OK (Enter)"],
    ["⟨rt+b⟩", "Cancel (Esc)"],
    ["⟨rt+lb⟩/⟨rt+rb⟩", "Start/End of the Text Too"],
    ["⟨rt+up⟩/⟨rt+down⟩", "Previous/Next Field"],
];

thread_local! {
    /// Where the text cursor was put in the page being drawn, so the keyboard stays clear of it.
    static CARET: Cell<Option<Position>> = const { Cell::new(None) };
}

/// Puts the page's cursor at `position`, and remembers where, for the keyboard not to cover it.
/// Every field that shows a cursor does it through this.
pub fn place_caret(page: &mut Page, position: Position) {
    page.set_cursor_position(position);
    CARET.set(Some(position));
}

/// Forgets where the cursor was, before a page is drawn again.
pub fn forget_caret() {
    CARET.set(None);
}

/// What a press of a button came to.
#[derive(Debug, Eq, PartialEq)]
pub enum Output {
    /// The button means nothing here.
    Nothing,
    /// The keyboard itself changed (the cursor moved, the layer changed, it was hidden or shown),
    /// so it needs drawing again.
    Redraw,
    /// These keys, one after the other, for the ritual underneath.
    Keys(Vec<KeyCode>),
    /// Show [`LEGEND`], the list of what the buttons do.
    Help,
}

#[derive(Debug)]
pub struct PadKeyboard {
    layer: Layer,
    /// The letters the keyboard had last, lower or upper case, for the symbols to go back to.
    letters: Layer,
    row: usize,
    col: usize,
    /// Put away with Select: the buttons are then the plain ones. It starts put away, so it is
    /// never brought up but by Select.
    pub hidden: bool,
}

impl Default for PadKeyboard {
    fn default() -> Self {
        Self {
            layer: Layer::default(),
            letters: Layer::default(),
            row: 0,
            col: 0,
            hidden: true,
        }
    }
}

impl PadKeyboard {
    /// What pressing `button` on a controller of `family` does. `held` says whether a button is
    /// held down, for the combinations.
    pub fn press(
        &mut self,
        button: Button,
        family: Family,
        held: impl Fn(Button) -> bool,
    ) -> Output {
        let Some(prompt) = Prompt::of(button, family) else {
            return Output::Nothing;
        };
        if self.hidden {
            return match prompt {
                Prompt::Wads => {
                    self.hidden = false;
                    Output::Redraw
                }
                // 🙈 only the buttons that finish or leave the form: a letter's button would type it
                Prompt::Confirm
                | Prompt::Back
                | Prompt::Previous
                | Prompt::Next
                | Prompt::Keys
                | Prompt::Up
                | Prompt::Down
                | Prompt::Left
                | Prompt::Right => Output::Keys(vec![prompt.chant()]),
                _ => Output::Nothing,
            };
        }
        // 🎚️ a trigger is only a modifier, for whatever is pressed while it is held
        if matches!(prompt, Prompt::Join | Prompt::Host) {
            return Output::Nothing;
        }
        let keys = |keys: &[KeyCode]| Output::Keys(keys.to_vec());
        let left_trigger = held(Button::LeftTrigger2);
        let right_trigger = held(Button::RightTrigger2);
        match (left_trigger, right_trigger, prompt) {
            (true, _, Prompt::Previous) => keys(&[KeyCode::Home]),
            (true, _, Prompt::Next) => keys(&[KeyCode::End]),
            (true, _, Prompt::Confirm) => self.type_key(true),
            (true, _, Prompt::Search) => keys(&[text::DELETE_WORD]),
            (true, _, Prompt::Back) => keys(&[text::CLEAR]),
            (_, true, Prompt::Confirm) => keys(&[KeyCode::Enter]),
            (_, true, Prompt::Back) => keys(&[KeyCode::Esc]),
            (_, true, Prompt::Previous) => keys(&[KeyCode::Home]),
            (_, true, Prompt::Next) => keys(&[KeyCode::End]),
            (_, true, Prompt::Up) => keys(&[KeyCode::BackTab]),
            (_, true, Prompt::Down) => keys(&[KeyCode::Tab]),
            // 🚫 a combination that isn't one is nothing, rather than the button alone
            (true, _, _) | (_, true, _) => Output::Nothing,
            (_, _, Prompt::Confirm) => self.type_key(false),
            (_, _, Prompt::Search) => keys(&[KeyCode::Backspace]),
            (_, _, Prompt::Cut) => keys(&[KeyCode::Char(' ')]),
            (_, _, Prompt::Back) => keys(&[KeyCode::Char('_')]),
            (_, _, Prompt::Previous) => keys(&[KeyCode::Left]),
            (_, _, Prompt::Next) => keys(&[KeyCode::Right]),
            (_, _, Prompt::Copy) => {
                self.toggle_symbols();
                Output::Redraw
            }
            (_, _, Prompt::Paste) => {
                self.shift();
                Output::Redraw
            }
            (_, _, Prompt::Keys) => Output::Help,
            (_, _, Prompt::Wads) => {
                self.hidden = true;
                Output::Redraw
            }
            (_, _, Prompt::Up | Prompt::Down | Prompt::Left | Prompt::Right) => {
                Output::from(self.step(prompt))
            }
            _ => Output::Nothing,
        }
    }

    /// Goes to the symbols, or back to whichever letters, lower or upper case, were up last.
    fn toggle_symbols(&mut self) {
        if self.layer == Layer::Symbols {
            self.layer = self.letters;
        } else {
            self.letters = self.layer;
            self.layer = Layer::Symbols;
        }
    }

    /// Goes to the other case: upper if it was lower, lower if it was upper or the symbols.
    fn shift(&mut self) {
        let other = match (self.layer, self.letters) {
            (Layer::Lower, _) | (Layer::Symbols, Layer::Lower) => Layer::Upper,
            _ => Layer::Lower,
        };
        self.letters = other;
        self.layer = other;
    }

    /// Types the key under the cursor, as a capital if `capital` and it is a letter.
    fn type_key(&mut self, capital: bool) -> Output {
        if self.row < BOTTOM {
            let key = self.layer.rows()[self.row].chars().nth(self.col);
            let key = key.map(|c| match capital {
                true => c.to_uppercase().next().unwrap_or(c),
                false => c,
            });
            return key.map_or(Output::Nothing, |c| Output::Keys(vec![KeyCode::Char(c)]));
        }
        match CONTROLS[self.col.min(CONTROLS.len() - 1)] {
            Control::Shift => {
                self.shift();
                Output::Redraw
            }
            Control::Symbols => {
                self.toggle_symbols();
                Output::Redraw
            }
            Control::Space => Output::Keys(vec![KeyCode::Char(' ')]),
            Control::Erase => Output::Keys(vec![KeyCode::Backspace]),
            Control::Ok => Output::Keys(vec![KeyCode::Enter]),
        }
    }

    /// Moves the cursor one key, wrapping around. The last row has half as many keys, so the cursor
    /// stays under (or over) the same part of the keyboard when it crosses between them.
    fn step(&mut self, arrow: Prompt) -> bool {
        let before = (self.row, self.col);
        let width = if self.row == BOTTOM {
            CONTROLS.len()
        } else {
            COLUMNS
        };
        match arrow {
            Prompt::Left => self.col = (self.col + width - 1) % width,
            Prompt::Right => self.col = (self.col + 1) % width,
            Prompt::Up | Prompt::Down => {
                let to = if arrow == Prompt::Up {
                    (self.row + BOTTOM) % (BOTTOM + 1)
                } else {
                    (self.row + 1) % (BOTTOM + 1)
                };
                if to == BOTTOM {
                    self.col /= 2;
                } else if self.row == BOTTOM {
                    self.col *= 2;
                }
                self.row = to;
            }
            _ => {}
        }
        (self.row, self.col) != before
    }

    /// The rows the keyboard takes without its hint: the borders and the five rows of keys.
    const GRID: u16 = 2 + 5;

    /// Where the keyboard goes on the page, and how many lines of help it has room for (at most
    /// `lines`). With the text cursor of the field being typed in known, the keyboard is wholly
    /// above or below it, in whichever has room for all of it, or else the larger, with the help
    /// shortened (or gone, leaving only the keys) to fit, and a blank row between it and the
    /// field. With no cursor to go by, it goes wherever less is on the page.
    fn window(page: &mut Page, width: u16, lines: usize) -> (Rect, usize) {
        let area = page.area();
        let width = width.min(area.width);
        let x = area.x + (area.width - width) / 2;
        let rect = |top: bool, lines: usize| {
            let height = (Self::GRID + lines as u16).min(area.height);
            let y = if top { area.y } else { area.bottom() - height };
            Rect::new(x, y, width, height)
        };
        if let Some(caret) = CARET.get() {
            // 🧮 the rows free on each side of the field, less one of air
            let above = caret.y.saturating_sub(area.y + 1);
            let below = area.bottom().saturating_sub(caret.y + 2);
            let wanted = Self::GRID + lines as u16;
            let top = if below >= wanted {
                false
            } else if above >= wanted {
                true
            } else {
                above > below
            };
            let room = if top { above } else { below };
            let lines = (room.saturating_sub(Self::GRID) as usize).min(lines);
            return (rect(top, lines), lines);
        }
        let (top, bottom) = (rect(true, lines), rect(false, lines));
        let buffer = page.buffer_mut();
        let busy = |region: Rect| {
            region
                .positions()
                .filter(|&position| buffer[position].symbol().trim() != "")
                .count()
        };
        if busy(top) < busy(bottom) {
            (top, lines)
        } else {
            (bottom, lines)
        }
    }

    /// Draws the keyboard over the page, or when it is hidden, a line saying how to bring it back.
    /// `held` says which buttons are held down: with a trigger, the help shows its combinations.
    pub fn draw(&self, page: &mut Page, family: Family) -> Rect {
        let area = page.area();
        if self.hidden {
            let hint = format!("{} keyboard", Prompt::Wads.label(family));
            let row = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
            let style = Style::new().fg(theme::worn().colors.muted);
            page.render_widget(Line::styled(hint, style).centered(), row);
            return row;
        }
        let key_style = Style::new().fg(theme::worn().colors.text);
        let cursor = theme::worn().styles.cursor;
        let mut keys: Vec<Line> = Vec::new();
        for (row, chars) in self.layer.rows().iter().enumerate() {
            let spans = chars.chars().enumerate().map(|(col, c)| {
                let style = if (row, col) == (self.row, self.col) {
                    cursor
                } else {
                    key_style
                };
                Span::styled(format!(" {c} "), style)
            });
            keys.push(Line::from(spans.collect::<Vec<_>>()));
        }
        let labels = [
            self.layer.name().to_string(),
            if self.layer == Layer::Symbols {
                "abc".into()
            } else {
                "123".into()
            },
            "space".into(),
            "del".into(),
            "OK".into(),
        ];
        let controls = labels.iter().enumerate().map(|(col, label)| {
            let style = if (BOTTOM, col) == (self.row, self.col) {
                cursor
            } else {
                Style::new().fg(theme::worn().colors.key)
            };
            Span::styled(format!("{label:^CONTROL_WIDTH$}"), style)
        });
        keys.push(Line::from(controls.collect::<Vec<_>>()));
        let muted = Style::new().fg(theme::worn().colors.muted);
        let hint = format!("{} buttons", Prompt::Keys.label(family));
        let width = (COLUMNS * KEY_WIDTH) as u16 + 2 + 2 * layout::PADDING;
        let (window, lines) = Self::window(page, width, 1);
        let help: Vec<Line> = (lines > 0)
            .then(|| Line::styled(hint, muted).centered())
            .into_iter()
            .collect();
        let veil = ritual::conjure_veil("keyboard", theme::worn().colors.screen);
        let inner = veil.inner(window);
        page.render_widget(Clear, window);
        page.render_widget(veil, window);
        // ⌨️ the keys are as wide as the characters' rows, centered in the window
        let keys_width = (COLUMNS * KEY_WIDTH) as u16;
        let keys_x = inner.x + inner.width.saturating_sub(keys_width) / 2;
        for (i, line) in keys.into_iter().enumerate() {
            let row = Rect::new(keys_x, inner.y + i as u16, keys_width, 1).intersection(inner);
            page.render_widget(Paragraph::new(line), row);
        }
        let rows = BOTTOM as u16 + 1;
        let help_area = Rect {
            y: inner.y + rows,
            height: inner.height.saturating_sub(rows),
            ..inner
        };
        page.render_widget(Paragraph::new(help), help_area);
        window
    }
}

impl From<bool> for Output {
    fn from(moved: bool) -> Self {
        if moved {
            Output::Redraw
        } else {
            Output::Nothing
        }
    }
}
