use ratatui::{crossterm::event::KeyCode, style::Style, text::Line};

use crate::{
    error::Result,
    gamepad, popups,
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
};

/// A yes/no popup, answered with `y` or `n`, or by picking an answer with `←`/`→` and `Enter`; on a
/// controller, with the buttons drawn by the answers, A for yes and B for no.
pub struct Dialog<'a> {
    message: &'a str,
    /// The answer picked with the arrows, if any: 0 for yes, 1 for no.
    selected: Option<usize>,
}

impl<'a> Dialog<'a> {
    const TITLE: &'static str = "dialogue";

    pub fn new(message: &'a str) -> Self {
        Self {
            message,
            selected: None,
        }
    }
}

impl Ritual for Dialog<'_> {
    type Return = bool;

    fn render(&mut self, page: &mut Page) {
        let lines: Vec<_> = popups::wrap(self.message, page.area())
            .into_iter()
            .map(|line| Line::styled(line, Style::new().fg(theme::worn().colors.text)))
            .collect();
        let answers = popups::answers(&["Yes", "No"], self.selected);
        popups::draw(
            page,
            Self::TITLE,
            theme::worn().colors.dialog,
            &lines,
            Some(answers),
        );
    }

    fn prompt(&mut self, _: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Self::Return>> {
        // 🎮 on a controller the buttons shown answer: A is yes and B is no
        if gamepad::shown().is_some() {
            return Ok(match popups::pad_answer(2, key_code) {
                Some(answer) => Phase::Return(answer == 0),
                None => Phase::Prompt,
            });
        }
        if let Some(phase) = popups::pick(&mut self.selected, 2, key_code) {
            return Ok(match phase {
                Phase::Return(answer) => Phase::Return(answer == 0),
                Phase::Render => Phase::Render,
                Phase::Prompt => Phase::Prompt,
            });
        }
        match key_code {
            KeyCode::Char('y') => Ok(Phase::Return(true)),
            KeyCode::Char('n') => Ok(Phase::Return(false)),
            _ => Ok(Phase::Prompt),
        }
    }
}

impl Dialog<'_> {
    /// Returns whether the user answered yes.
    pub fn summon(grimoire: &mut Grimoire, message: &str) -> Result<bool> {
        Dialog::new(message).invoke(grimoire)
    }

    pub fn should_delete(grimoire: &mut Grimoire, element: &str) -> Result<bool> {
        Self::summon(
            grimoire,
            &format!("Are you sure you want to delete '{element}'?"),
        )
    }

    pub fn should_upgrade(grimoire: &mut Grimoire) -> Result<bool> {
        Self::summon(grimoire, "Older files found, upgrade them? (backed up)")
    }

    pub fn should_delete_originals(grimoire: &mut Grimoire) -> Result<bool> {
        Self::summon(grimoire, "Upgraded! Delete the original files?")
    }

    pub fn should_use_different_file(grimoire: &mut Grimoire) -> Result<bool> {
        Self::summon(grimoire, "Different file than expected, use it anyway?")
    }

    pub fn should_quit_unsaved(grimoire: &mut Grimoire) -> Result<bool> {
        Self::summon(grimoire, "Your changes were not saved, quit anyway?")
    }

    pub fn should_quit(grimoire: &mut Grimoire) -> Result<bool> {
        Self::summon(grimoire, "Are you sure you want to quit?")
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;
    use crate::testing;

    #[test]
    fn a_question_with_its_answers() {
        let mut dialog = Dialog::new("Are you sure you want to delete 'sigil_run'?");
        assert_snapshot!(testing::draw(70, 12, |page| dialog.render(page)));
    }

    #[test]
    fn a_long_question_wraps() {
        let mut dialog = Dialog::new(
            "Older files found, upgrade them? Your originals are kept as backups until you say otherwise.",
        );
        assert_snapshot!(testing::draw(70, 14, |page| dialog.render(page)));
    }
}
