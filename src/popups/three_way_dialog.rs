use ratatui::{crossterm::event::KeyCode, style::Style, text::Line};

use crate::{
    error::Result,
    popups,
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
};

/// A popup with three answers. Each is chosen by its first letter, which is shown highlighted, so
/// the three labels must start with different letters. `Esc` backs out without answering.
pub struct ThreeWayDialog<'a> {
    message: &'a str,
    options: [&'a str; 3],
}

impl<'a> ThreeWayDialog<'a> {
    const TITLE: &str = "dialogue";

    /// Which option, if any, is the user's answer, or `Prompt` if the key means nothing here.
    fn answer(options: &[&str; 3], key_code: KeyCode) -> Phase<Option<usize>> {
        match key_code {
            KeyCode::Esc => Phase::Return(None),
            KeyCode::Char(typed) => options
                .iter()
                .position(|option| {
                    option
                        .chars()
                        .next()
                        .is_some_and(|first| first.eq_ignore_ascii_case(&typed))
                })
                .map_or(Phase::Prompt, |index| Phase::Return(Some(index))),
            _ => Phase::Prompt,
        }
    }

    /// Asks the question and returns the position of the option chosen, or `None` for `Esc`.
    pub fn summon(
        grimoire: &mut Grimoire,
        message: &'a str,
        options: [&'a str; 3],
    ) -> Result<Option<usize>> {
        Self { message, options }.invoke(grimoire)
    }
}

impl Ritual for ThreeWayDialog<'_> {
    type Return = Option<usize>;

    fn render(&mut self, page: &mut Page) {
        let lines: Vec<_> = popups::wrap(self.message, page.area())
            .into_iter()
            .map(|line| Line::styled(line, Style::new().fg(theme::TEXT)))
            .collect();
        let answers = popups::answers(&self.options);
        popups::draw(page, Self::TITLE, theme::DIALOG, &lines, Some(answers));
    }

    fn prompt(&mut self, _: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Self::Return>> {
        Ok(Self::answer(&self.options, key_code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTIONS: [&str; 3] = ["Global", "New", "Select"];

    fn answer(key_code: KeyCode) -> Phase<Option<usize>> {
        ThreeWayDialog::answer(&OPTIONS, key_code)
    }

    #[test]
    fn each_first_letter_picks_its_option() {
        assert!(answer(KeyCode::Char('g')) == Phase::Return(Some(0)));
        assert!(answer(KeyCode::Char('n')) == Phase::Return(Some(1)));
        assert!(answer(KeyCode::Char('s')) == Phase::Return(Some(2)));
    }

    #[test]
    fn capitals_work_too() {
        assert!(answer(KeyCode::Char('G')) == Phase::Return(Some(0)));
    }

    #[test]
    fn escape_backs_out() {
        assert!(answer(KeyCode::Esc) == Phase::Return(None));
    }

    #[test]
    fn other_keys_keep_waiting() {
        assert!(answer(KeyCode::Char('x')) == Phase::Prompt);
        assert!(answer(KeyCode::Enter) == Phase::Prompt);
        assert!(answer(KeyCode::Tab) == Phase::Prompt);
    }
}
