use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    popups::legend::Legend,
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
    widgets::{
        form::{self, Field},
        text,
    },
};

/// Asks for one line of text, starting from what it is now: Enter keeps it, Esc leaves it as it
/// was. Any character can be typed, `?` too, so the keys are on `F1`.
pub struct TextPrompt<'a> {
    title: &'a str,
    label: &'a str,
    placeholder: &'a str,
    text: String,
    caret: usize,
    max: usize,
}

impl<'a> TextPrompt<'a> {
    fn border_color() -> Color {
        theme::worn().colors.screen
    }
    const LEGEND: &'static [[&'static str; 2]] = &[
        ["Enter", "Keep It"],
        ["Any Key", "Input"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Leave It as It Was"],
    ];

    /// The text as it was left, or `None` if Esc left it as it was.
    pub fn summon(
        grimoire: &mut Grimoire,
        title: &'a str,
        label: &'a str,
        placeholder: &'a str,
        text: &str,
        max: usize,
    ) -> Result<Option<String>> {
        Self::new(title, label, placeholder, text, max).invoke(grimoire)
    }

    fn new(title: &'a str, label: &'a str, placeholder: &'a str, text: &str, max: usize) -> Self {
        Self {
            title,
            label,
            placeholder,
            caret: text::end(text),
            text: text.to_string(),
            max,
        }
    }

    /// Every key but the legend's, none of which needs the terminal.
    fn edit(&mut self, key_code: KeyCode) -> Phase<Option<String>> {
        match key_code {
            KeyCode::Enter => Phase::Return(Some(self.text.clone())),
            KeyCode::Esc => Phase::Return(None),
            key_code => match text::edit(&mut self.text, &mut self.caret, key_code, self.max, Some)
            {
                Some(changed) => Phase::render_if(changed),
                None => Phase::Prompt,
            },
        }
    }
}

impl Ritual for TextPrompt<'_> {
    type Return = Option<String>;

    fn typing(&self) -> bool {
        true
    }

    fn legend(&self) -> &[[&'static str; 2]] {
        Self::LEGEND
    }

    fn render(&mut self, page: &mut Page) {
        let fields = [Field {
            label: self.label,
            value: &self.text,
            placeholder: self.placeholder,
            room: 24,
            caret: Some(self.caret),
        }];
        form::draw(page, self.title, Self::border_color(), &fields, 0);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        if key_code == KeyCode::F(1) {
            Legend::summon(grimoire, Self::LEGEND)?;
            return Ok(Phase::Render);
        }
        Ok(self.edit(key_code))
    }
}
