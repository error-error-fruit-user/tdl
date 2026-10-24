use ratatui::{
    crossterm::event::KeyCode,
    style::Style,
    text::{Line, Span},
};

use crate::{
    error::Result,
    gamepad::{self, Family},
    layout, popups,
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
};

/// A popup listing `[key, description]` pairs, one per row with the keys right-aligned, dismissed
/// by any key. One too tall for the terminal is split into columns if they fit, or else into
/// pages, turned with `J`/`↓` and `K`/`↑`. Once a controller is used, every key it has a button
/// for is shown as that button.
pub struct Legend<'a> {
    legend: &'a [[&'a str; 2]],
    /// The controller used last, if it was used after the keyboard.
    family: Option<Family>,
    /// The page on screen, and how many there were when it was last drawn.
    page: usize,
    pages: usize,
}

impl<'a> Legend<'a> {
    const TITLE: &'static str = "keys";

    pub fn new(legend: &'a [[&'a str; 2]]) -> Self {
        Self {
            legend,
            family: gamepad::shown(),
            page: 0,
            pages: 1,
        }
    }

    pub fn summon(grimoire: &mut Grimoire, legend: &[[&'static str; 2]]) -> Result<()> {
        Legend::new(legend).invoke(grimoire)
    }

    /// `key` as the user would press it: the controller's button for it, if one was used last.
    fn relabel(&self, key: &str) -> String {
        match self.family {
            Some(family) => gamepad::relabel(key, family),
            None => gamepad::plain(key).into_owned(),
        }
    }

    /// Every row padded to the same width, so the columns line up once the rows are centered.
    fn lines(&self) -> Vec<Line<'a>> {
        let keys: Vec<String> = self
            .legend
            .iter()
            .map(|[key, _]| self.relabel(key))
            .collect();
        let key_width = keys.iter().map(|key| layout::width(key)).max().unwrap_or(0);
        let text_width = self
            .legend
            .iter()
            .map(|[_, text]| layout::width(text))
            .max()
            .unwrap_or(0);
        keys.iter()
            .zip(self.legend)
            .map(|(key, [_, text])| {
                Line::from(vec![
                    Span::styled(
                        format!("{key:>width$}", width = key_width as usize),
                        Style::new().fg(theme::worn().colors.key),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!("{text:<width$}", width = text_width as usize),
                        Style::new().fg(theme::worn().colors.text),
                    ),
                ])
            })
            .collect()
    }
}

impl Legend<'_> {
    /// `J`/`↓` and `K`/`↑` turn the pages of a legend that has them; any other key closes it.
    fn turn(&mut self, key_code: KeyCode) -> Phase<()> {
        match key_code {
            KeyCode::Char('j') | KeyCode::Down if self.pages > 1 => {
                Phase::render_if_then(self.page + 1 < self.pages, || self.page += 1)
            }
            KeyCode::Char('k') | KeyCode::Up if self.pages > 1 => {
                Phase::render_if_then(self.page > 0, || self.page -= 1)
            }
            _ => Phase::Return(()),
        }
    }
}

/// The gap between columns, when a legend is too tall for the terminal and is split.
const COLUMN_GAP: &str = "    ";

/// `lines`, every one the same width, in as few columns as keep each no more than `rows` tall:
/// down the first column, then the next.
fn columns<'a>(lines: Vec<Line<'a>>, rows: usize) -> Vec<Line<'a>> {
    let rows = rows.max(1);
    if lines.len() <= rows {
        return lines;
    }
    let tall = lines.len().div_ceil(lines.len().div_ceil(rows));
    let mut joined: Vec<Line> = Vec::new();
    for (i, line) in lines.into_iter().enumerate() {
        let row = i % tall;
        if row < joined.len() {
            joined[row].spans.push(Span::raw(COLUMN_GAP));
            joined[row].spans.extend(line.spans);
        } else {
            joined.push(line);
        }
    }
    joined
}

impl Ritual for Legend<'_> {
    type Return = ();

    /// One column if it fits the terminal's height, otherwise as many as fit its width, otherwise
    /// a page at a time.
    fn render(&mut self, page: &mut Page) {
        let area = page.area();
        // 📏 the margins, the sigil's row, the borders and a blank row above and below the keys
        let rows = area.height.saturating_sub(2 * layout::MARGIN + 1 + 2 + 2) as usize;
        let room =
            area.width
                .saturating_sub(2 * layout::MARGIN + 2 + 2 * layout::PADDING) as usize;
        let lines = self.lines();
        let width = lines.first().map_or(0, Line::width);
        let count = lines.len().div_ceil(rows.max(1));
        if count == 1 || count * width + (count - 1) * COLUMN_GAP.len() <= room {
            self.pages = 1;
            let lines = columns(lines, rows);
            return popups::draw(page, Self::TITLE, theme::worn().colors.legend, &lines, None);
        }
        // 📖 two rows go to the footer that says there is more
        let per_page = rows.saturating_sub(2).max(1);
        self.pages = lines.len().div_ceil(per_page);
        self.page = self.page.min(self.pages - 1);
        let shown: Vec<_> = lines
            .into_iter()
            .skip(self.page * per_page)
            .take(per_page)
            .collect();
        let footer = Line::styled(
            format!(
                "{}/{} · {} for more",
                self.page + 1,
                self.pages,
                self.relabel("J/K")
            ),
            Style::new().fg(theme::worn().colors.muted),
        );
        popups::draw(
            page,
            Self::TITLE,
            theme::worn().colors.legend,
            &shown,
            Some(footer),
        );
    }

    fn prompt(&mut self, _: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Self::Return>> {
        Ok(self.turn(key_code))
    }
}
