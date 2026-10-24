//! Finding a row by name. `/` opens a search line; typing changes only the line, never the cursor.
//! Enter moves the cursor to the best match and Esc calls the search off. Afterwards `;` and `,`
//! step through the other matches, best first. On the search line `↑` and `↓` bring back earlier
//! searches, from this session, in the launcher and the editor alike.
//!
//! What a row's name is depends on the screen: in the launcher it is the profile's name, or with a
//! `*` in front of the query the file names of its wads; in the editor it is the wad's file name.
//! Dirs are never looked at, so `doom` doesn't match every wad kept under `~/doom/`.
//!
//! The launcher's command line (`` ` ``) is a search line too, drawn with a `~` instead of a `/`
//! and with a history of its own: what is typed on it is a `tdl cli` command, not a name.

use std::cell::RefCell;

use ratatui::{
    crossterm::event::KeyCode,
    layout::{Position, Rect},
    style::Style,
    text::{Line, Span},
};

use crate::{layout, ritual::Page, theme, widgets::text};

thread_local! {
    /// The searches kept this session, oldest first, each once. The screens all run on one
    /// thread, so that is where it lives.
    static HISTORY: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// The commands typed on the command line this session, the same way.
    static SPOKEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// The history a line with `sigil` keeps: the commands' for `~`, the searches' otherwise.
fn history(sigil: char) -> &'static std::thread::LocalKey<RefCell<Vec<String>>> {
    if sigil == COMMAND { &SPOKEN } else { &HISTORY }
}

/// What a search line starts with, and what the command line starts with.
const SEARCH: char = '/';
const COMMAND: char = '~';
/// How many searches are remembered.
const HISTORY_LENGTH: usize = 32;

/// Remembers `query` as the latest of its line's, moving it to the end if it was there already.
fn remember(sigil: char, query: &str) {
    if query.is_empty() {
        return;
    }
    history(sigil).with_borrow_mut(|history| {
        history.retain(|kept| kept != query);
        history.push(query.to_string());
        let extra = history.len().saturating_sub(HISTORY_LENGTH);
        history.drain(..extra);
    });
}

/// What was typed on a line with `sigil` `back` places before the latest (0 is the latest), if
/// anything was.
fn recalled(sigil: char, back: usize) -> Option<String> {
    history(sigil).with_borrow(|history| history.iter().rev().nth(back).cloned())
}

/// How well a name matches a query; lower is better.
pub type Score = u8;

/// The name is the query (ignoring case, and for a file also ignoring its extension).
const EXACT: Score = 0;
/// The name starts with the query.
const PREFIX: Score = 1;
/// A word in the name starts with the query: after `_`, `-`, `.` or a space.
const WORD: Score = 2;
/// The query is somewhere in the name.
const INSIDE: Score = 3;

/// How well `name` matches `query`, ignoring case, or `None` if it doesn't. An empty query matches
/// nothing.
pub fn score(name: &str, query: &str) -> Option<Score> {
    if query.is_empty() {
        return None;
    }
    let name = name.to_lowercase();
    let query = query.to_lowercase();
    let stem = name
        .rsplit_once('.')
        .map_or(name.as_str(), |(stem, _)| stem);
    if name == query || stem == query {
        Some(EXACT)
    } else if name.starts_with(&query) {
        Some(PREFIX)
    } else if name
        .match_indices(&query)
        .any(|(at, _)| name[..at].ends_with(['_', '-', '.', ' ']))
    {
        Some(WORD)
    } else if name.contains(&query) {
        Some(INSIDE)
    } else {
        None
    }
}

/// Every one of `count` rows that matches, best first. Rows that match equally well go in the order
/// they come after `cursor`, wrapping around, so the cursor's own row is the last of its kind. A
/// cursor past the last row (on an add row) counts as being on the last row, so the top comes next.
pub fn rank(cursor: usize, count: usize, score: impl Fn(usize) -> Option<Score>) -> Vec<usize> {
    let mut ranked: Vec<(Score, usize, usize)> = (0..count)
        .filter_map(|row| {
            let cursor = cursor.min(count - 1);
            let after = (row + count - cursor - 1) % count;
            score(row).map(|score| (score, after, row))
        })
        .collect();
    ranked.sort_unstable();
    ranked.into_iter().map(|(_, _, row)| row).collect()
}

/// A search being typed, or a command on the command line.
pub struct Search {
    pub query: String,
    /// `/` for a search, `~` for a command.
    sigil: char,
    /// Where the cursor is in the query.
    caret: usize,
    /// How far back in the history `↑` has gone (0 is the latest search), while it has.
    back: Option<usize>,
    /// What was typed before going back, for `↓` to return to.
    draft: String,
}

/// What a key did to a search.
#[derive(Debug, PartialEq)]
pub enum Typed {
    /// The query changed; only the search line needs redrawing.
    Typing,
    /// Enter, with the query to look for.
    Done(String),
    /// Esc: the search is over and nothing moves.
    Cancelled,
    /// A key a search doesn't use.
    Ignored,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            query: String::new(),
            sigil: SEARCH,
            caret: 0,
            back: None,
            draft: String::new(),
        }
    }
}

impl Search {
    pub const LEGEND: [&str; 2] = ["/ ;⟨rb+right⟩ ,⟨rb+left⟩", "Search, Next, Previous"];

    /// The command line: a search line with a `~`, whose query is a `tdl cli` command.
    pub fn command() -> Self {
        Self {
            sigil: COMMAND,
            ..Self::default()
        }
    }

    /// The longest a command may be typed.
    const MAX_COMMAND: usize = 240;

    /// The longest a query may be typed.
    const MAX_LENGTH: usize = 64;

    pub fn key(&mut self, key_code: KeyCode) -> Typed {
        match key_code {
            KeyCode::Enter => {
                remember(self.sigil, &self.query);
                Typed::Done(std::mem::take(&mut self.query))
            }
            KeyCode::Esc => Typed::Cancelled,
            KeyCode::Up => {
                let back = self.back.map_or(0, |back| back + 1);
                let Some(earlier) = recalled(self.sigil, back) else {
                    return Typed::Ignored;
                };
                if self.back.is_none() {
                    self.draft = self.query.clone();
                }
                self.back = Some(back);
                self.recall(earlier)
            }
            KeyCode::Down => match self.back {
                Some(0) => {
                    self.back = None;
                    let draft = std::mem::take(&mut self.draft);
                    self.recall(draft)
                }
                Some(back) => match recalled(self.sigil, back - 1) {
                    Some(later) => {
                        self.back = Some(back - 1);
                        self.recall(later)
                    }
                    None => Typed::Ignored,
                },
                None => Typed::Ignored,
            },
            key_code => match text::edit(
                &mut self.query,
                &mut self.caret,
                key_code,
                if self.sigil == COMMAND {
                    Self::MAX_COMMAND
                } else {
                    Self::MAX_LENGTH
                },
                Some,
            ) {
                Some(true) => Typed::Typing,
                _ => Typed::Ignored,
            },
        }
    }

    /// Puts `query` on the line, with the cursor at its end.
    fn recall(&mut self, query: String) -> Typed {
        self.caret = text::end(&query);
        self.query = query;
        Typed::Typing
    }

    /// The search line: `/query`, and how many rows match or, before anything is typed, what to
    /// type (`placeholder`). Drawn in `area`, the row under a screen's last window, with the
    /// cursor at the end of the query.
    pub fn render(&self, matches: usize, placeholder: &str, area: Rect, page: &mut Page) {
        let tally = match (self.query.is_empty(), matches) {
            // 🗣️ a command matches nothing; it is only done
            (false, _) if self.sigil == COMMAND => Span::raw(""),
            (true, _) => Span::styled(
                format!("  {placeholder}"),
                Style::new().fg(theme::worn().colors.muted),
            ),
            (false, 0) => Span::styled("  no match", Style::new().fg(theme::worn().colors.bad)),
            (false, 1) => Span::styled("  1 match", Style::new().fg(theme::worn().colors.muted)),
            (false, n) => Span::styled(
                format!("  {n} matches"),
                Style::new().fg(theme::worn().colors.muted),
            ),
        };
        let line = Line::from(vec![
            Span::styled(
                self.sigil.to_string(),
                Style::new().fg(theme::worn().colors.key),
            ),
            Span::styled(
                self.query.as_str(),
                Style::new().fg(theme::worn().colors.text),
            ),
            tally,
        ]);
        page.render_widget(line, area);
        let column = area.x + 1 + layout::width(text::before(&self.query, self.caret));
        if area.height > 0 {
            crate::keyboard::place_caret(
                page,
                Position::new(column.min(area.right().saturating_sub(1)), area.y),
            );
        }
    }
}

/// The matches of the last search kept, best first, and which of them the cursor went to, for
/// `;` and `,` to step through. Thrown away whenever the list changes, since the rows would move.
#[derive(Default)]
pub struct Found {
    rows: Vec<usize>,
    at: usize,
}

impl Found {
    /// The matches `ranked` best first, with the cursor going to the best.
    pub fn new(ranked: Vec<usize>) -> Self {
        Self {
            rows: ranked,
            at: 0,
        }
    }

    /// The row the cursor goes to now, if anything matched.
    pub fn current(&self) -> Option<usize> {
        self.rows.get(self.at).copied()
    }

    /// The next match, wrapping around to the best.
    pub fn next(&mut self) -> Option<usize> {
        if self.rows.is_empty() {
            return None;
        }
        self.at = (self.at + 1) % self.rows.len();
        self.current()
    }

    /// The match before this one, wrapping around to the worst.
    pub fn previous(&mut self) -> Option<usize> {
        if self.rows.is_empty() {
            return None;
        }
        self.at = (self.at + self.rows.len() - 1) % self.rows.len();
        self.current()
    }
}
