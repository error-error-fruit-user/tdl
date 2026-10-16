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
/// by any key.
pub struct Legend<'a> {
    legend: &'a [[&'a str; 2]],
}

impl<'a> Legend<'a> {
    const TITLE: &str = "keys";

    pub fn new(legend: &'a [[&'a str; 2]]) -> Self {
        Self { legend }
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
                        Style::new().fg(theme::KEY),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!("{text:<width$}", width = text_width as usize),
                        Style::new().fg(theme::TEXT),
                    ),
                ])
            })
            .collect()
    }
}

impl Ritual for Legend<'_> {
    type Return = ();

    fn render(&mut self, page: &mut Page) {
        popups::draw(page, Self::TITLE, theme::LEGEND, &self.lines(), None);
    }

    fn prompt(&mut self, _: &mut Grimoire, _: KeyCode) -> Result<Phase<Self::Tea>> {
        Ok(Phase::Return(()))
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
}
