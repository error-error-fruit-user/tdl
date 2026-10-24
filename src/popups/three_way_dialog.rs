use ratatui::{crossterm::event::KeyCode, style::Style, text::Line};

use crate::{
    error::Result,
    gamepad, popups,
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
};

/// A popup with three answers. Each is chosen by its first letter, which is shown highlighted, so
/// the three labels must start with different letters. `Esc` backs out without answering.
pub struct ThreeWayDialog<'a> {
    message: &'a str,
    options: [&'a str; 3],
    /// The answer picked with `←`/`→`, if any, which `Enter` takes.
    selected: Option<usize>,
}

impl<'a> ThreeWayDialog<'a> {
    const TITLE: &'static str = "dialogue";

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
        Self {
            message,
            options,
            selected: None,
        }
        .invoke(grimoire)
    }
}

impl Ritual for ThreeWayDialog<'_> {
    type Return = Option<usize>;

    fn render(&mut self, page: &mut Page) {
        let lines: Vec<_> = popups::wrap(self.message, page.area())
            .into_iter()
            .map(|line| Line::styled(line, Style::new().fg(theme::worn().colors.text)))
            .collect();
        let answers = popups::answers(&self.options, self.selected);
        popups::draw(
            page,
            Self::TITLE,
            theme::worn().colors.dialog,
            &lines,
            Some(answers),
        );
    }

    fn prompt(&mut self, _: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Self::Return>> {
        // 🎮 on a controller the buttons shown answer: A, X and Y, and B backs out
        if gamepad::shown().is_some() {
            return Ok(match (popups::pad_answer(3, key_code), key_code) {
                (Some(answer), _) => Phase::Return(Some(answer)),
                (None, KeyCode::Esc) => Phase::Return(None),
                _ => Phase::Prompt,
            });
        }
        if let Some(phase) = popups::pick(&mut self.selected, self.options.len(), key_code) {
            return Ok(match phase {
                Phase::Return(answer) => Phase::Return(Some(answer)),
                Phase::Render => Phase::Render,
                Phase::Prompt => Phase::Prompt,
            });
        }
        Ok(Self::answer(&self.options, key_code))
    }
}
