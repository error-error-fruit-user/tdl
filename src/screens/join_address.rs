use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    popups::legend::Legend,
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
    widgets::form::{self, Field},
};

/// Prompts for the IP address of the host to join. Only digits and dots are accepted.
pub struct JoinAddressForm {
    ip: String,
}

impl JoinAddressForm {
    const TITLE: &str = "join";
    const BORDER_COLOR: Color = theme::MULTIPLAYER;
    const LEGEND: &[[&str; 2]] = &[["Enter", "Launch"], ["Esc", "Cancel"], ["0-9/.", "Input"]];

    pub fn new() -> Self {
        Self { ip: String::new() }
    }

    /// `None` if cancelled.
    pub fn summon(grimoire: &mut Grimoire) -> Result<Option<String>> {
        Self::new().invoke(grimoire)
    }
}

impl Ritual for JoinAddressForm {
    type Return = Option<String>;

    fn render(&mut self, page: &mut Page) {
        let fields = [Field {
            label: "host",
            value: &self.ip,
            placeholder: "ip address",
            room: 15,
        }];
        form::draw(page, Self::TITLE, Self::BORDER_COLOR, &fields, 0);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        match key_code {
            KeyCode::Enter => Ok(Phase::Return(Some(self.ip.clone()))),
            KeyCode::Esc => Ok(Phase::Return(None)),
            KeyCode::Char(c @ '0'..='9' | c @ '.') => {
                self.ip.push(c);
                Ok(Phase::Render)
            }
            KeyCode::Backspace => {
                self.ip.pop();
                Ok(Phase::Render)
            }
            KeyCode::Char('?') => {
                Legend::summon(grimoire, Self::LEGEND)?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;
    use crate::testing;

    #[test]
    fn an_empty_form_shows_what_goes_there() {
        let mut form = JoinAddressForm::new();
        assert_snapshot!(testing::draw(50, 10, |page| form.render(page)));
    }
}
