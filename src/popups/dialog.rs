use ratatui::{crossterm::event::KeyCode, style::Style, text::Line};

use crate::{
    error::Result,
    popups,
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
};

/// A yes/no popup, answered with `y` or `n`.
pub struct Dialog<'a> {
    message: &'a str,
}

impl<'a> Dialog<'a> {
    const TITLE: &str = "dialogue";

    pub fn new(message: &'a str) -> Self {
        Self { message }
    }
}

impl Ritual for Dialog<'_> {
    type Return = bool;

    fn render(&mut self, page: &mut Page) {
        let lines: Vec<_> = popups::wrap(self.message, page.area())
            .into_iter()
            .map(|line| Line::styled(line, Style::new().fg(theme::TEXT)))
            .collect();
        let answers = popups::answers(&["Yes", "No"]);
        popups::draw(page, Self::TITLE, theme::DIALOG, &lines, Some(answers));
    }

    fn prompt(&mut self, _: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Self::Return>> {
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

    /// For volatile wads that differ from the host's copy.
    pub fn should_join_with_different_wads(
        grimoire: &mut Grimoire,
        names: &[String],
    ) -> Result<bool> {
        match names {
            [name] => Self::summon(
                grimoire,
                &format!("{name} differs from the host's, join anyway?"),
            ),
            _ => Self::summon(
                grimoire,
                &format!("{} wads differ from the host's, join anyway?", names.len()),
            ),
        }
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
