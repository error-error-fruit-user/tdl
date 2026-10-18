use ratatui::{
    crossterm::event::KeyCode,
    style::Style,
    text::{Line, Span},
};

use crate::{
    error::Result,
    layout, popups,
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
};

/// A popup listing `[key, description]` pairs, one per row with the keys right-aligned, dismissed
/// by any key. One too tall for the terminal is split into columns if they fit, or else into
/// pages, turned with `J`/`↓` and `K`/`↑`.
pub struct Legend<'a> {
    legend: &'a [[&'a str; 2]],
    /// The page on screen, and how many there were when it was last drawn.
    page: usize,
    pages: usize,
}

impl<'a> Legend<'a> {
    const TITLE: &str = "keys";

    pub fn new(legend: &'a [[&'a str; 2]]) -> Self {
        Self {
            legend,
            page: 0,
            pages: 1,
        }
    }

    pub fn summon(grimoire: &mut Grimoire, legend: &[[&str; 2]]) -> Result<()> {
        Legend::new(legend).invoke(grimoire)
    }

    /// Every row padded to the same width, so the columns line up once the rows are centered.
    fn lines(&self) -> Vec<Line<'a>> {
        let key_width = self
            .legend
            .iter()
            .map(|[key, _]| layout::width(key))
            .max()
            .unwrap_or(0);
        let text_width = self
            .legend
            .iter()
            .map(|[_, text]| layout::width(text))
            .max()
            .unwrap_or(0);
        self.legend
            .iter()
            .map(|[key, text]| {
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
            format!("{}/{} · J/K for more", self.page + 1, self.pages),
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

    fn prompt(&mut self, _: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Self::Tea>> {
        Ok(self.turn(key_code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_is_the_same_width_so_the_columns_line_up() {
        let legend = [["Enter", "Launch"], ["J↓/K↑", "Traverse"], ["?", "Keys"]];
        let lines = Legend::new(&legend).lines();
        let widths: Vec<_> = lines.iter().map(Line::width).collect();
        assert!(widths.iter().all(|&w| w == widths[0]));
        assert_eq!(lines[2].spans[0].content, "    ?");
    }

    #[test]
    fn a_legend_too_tall_for_the_terminal_goes_into_columns() {
        let legend: Vec<[&str; 2]> = (0..14).map(|_| ["K", "Key"]).collect();
        let lines = Legend::new(&legend).lines();
        let width = lines[0].width();
        let split = columns(lines.clone(), 9);
        assert_eq!(split.len(), 7, "two even columns of seven");
        assert_eq!(split[0].width(), width * 2 + COLUMN_GAP.len());
        // a short one is left alone
        assert_eq!(columns(lines.clone(), 20).len(), 14);
        // and an odd count leaves the last column short
        assert_eq!(columns(lines[..13].to_vec(), 9).len(), 7);
    }

    #[test]
    fn pages_turn_with_j_and_k_and_any_other_key_closes() {
        let legend = [["K", "Key"]; 3];
        let mut popup = Legend::new(&legend);
        // one page: even J closes it
        assert!(popup.turn(KeyCode::Char('j')) == Phase::Return(()));
        popup.pages = 3;
        assert!(popup.turn(KeyCode::Char('k')) == Phase::Prompt);
        assert!(popup.turn(KeyCode::Down) == Phase::Render);
        assert!(popup.turn(KeyCode::Char('j')) == Phase::Render);
        assert!(
            popup.turn(KeyCode::Char('j')) == Phase::Prompt,
            "no page after the last"
        );
        assert_eq!(popup.page, 2);
        assert!(popup.turn(KeyCode::Up) == Phase::Render);
        assert!(popup.turn(KeyCode::Esc) == Phase::Return(()));
    }

    #[test]
    fn snapshot_a_long_legend_on_a_wide_but_short_terminal() {
        let legend: Vec<[&str; 2]> = (0..14).map(|_| ["Esc", "Save and Quit"]).collect();
        let mut popup = Legend::new(&legend);
        insta::assert_snapshot!(crate::testing::draw(80, 20, |page| popup.render(page)));
    }

    #[test]
    fn snapshot_a_long_legend_on_the_smallest_terminal() {
        let legend: Vec<[&str; 2]> = [
            ["Enter", "Launch"],
            ["H", "Host"],
            ["N", "Join"],
            ["J/K", "Traverse"],
            ["/", "Search"],
            ["}/{", "Swap"],
            ["Y/P", "Copy/Paste"],
            ["A/E/D", "Add/Edit/Delete"],
            ["X/I", "Scrolls"],
            ["U/W", "UZDoom/Wad"],
            ["O", "Settings"],
            ["Esc", "Quit"],
            ["V", "Range"],
            ["M", "Mark"],
        ]
        .to_vec();
        let mut popup = Legend::new(&legend);
        insta::assert_snapshot!(crate::testing::draw(40, 20, |page| popup.render(page)));
    }

    #[test]
    fn snapshot_the_launcher_keys() {
        let legend = [
            ["Enter", "Launch"],
            ["H", "Host"],
            ["N", "Join"],
            ["J↓/K↑", "Traverse"],
            ["Esc", "Save and Quit"],
        ];
        let mut popup = Legend::new(&legend);
        insta::assert_snapshot!(crate::testing::draw(50, 14, |page| popup.render(page)));
    }
}
