use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    net::{self, Address},
    popups::{legend::Legend, warning::Warning},
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
    widgets::form::{self, Field},
};

/// Prompts for where the host is: its IP address, the port its wads are served on, and the join
/// code it was given.
pub struct JoinAddressForm {
    ip: String,
    port: String,
    code: String,
    focus: usize,
}

impl JoinAddressForm {
    const TITLE: &str = "join";
    const BORDER_COLOR: Color = theme::MULTIPLAYER;
    const FIELDS: usize = 3;
    const IP_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/Backtab", "Traverse"],
        ["0-9/.", "Input"],
        ["Esc", "Cancel"],
    ];
    const PORT_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/Backtab", "Traverse"],
        ["0-9", "Input"],
        ["Esc", "Cancel"],
    ];
    const CODE_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/Backtab", "Traverse"],
        ["a-z/0-9", "Input"],
        ["Esc", "Cancel"],
    ];

    pub fn new() -> Self {
        Self {
            ip: String::new(),
            port: String::new(),
            code: String::new(),
            focus: 0,
        }
    }

    /// `None` if cancelled.
    pub fn summon(grimoire: &mut Grimoire) -> Result<Option<Address>> {
        Self::new().invoke(grimoire)
    }

    /// The field in focus, to type into.
    fn focused(&mut self) -> &mut String {
        match self.focus {
            0 => &mut self.ip,
            1 => &mut self.port,
            _ => &mut self.code,
        }
    }

    /// What the form says, or what is missing from it. An empty port is the default one.
    fn address(&self) -> std::result::Result<Address, &'static str> {
        if self.ip.is_empty() {
            return Err("Enter the host's IP address");
        }
        let port = if self.port.is_empty() {
            net::DEFAULT_PORT
        } else {
            match self.port.parse() {
                Ok(port) if port > 0 => port,
                _ => return Err("The port must be between 1 and 65535"),
            }
        };
        if !net::is_valid_code(&self.code) {
            return Err("Enter the join code the host was given");
        }
        Ok(Address {
            host: self.ip.clone(),
            port,
            code: self.code.clone(),
        })
    }
}

impl Ritual for JoinAddressForm {
    type Return = Option<Address>;

    fn render(&mut self, page: &mut Page) {
        let default_port = net::DEFAULT_PORT.to_string();
        let fields = [
            Field {
                label: "host",
                value: &self.ip,
                placeholder: "ip address",
                room: 15,
            },
            Field {
                label: "port",
                value: &self.port,
                placeholder: &default_port,
                room: 5,
            },
            Field {
                label: "code",
                value: &self.code,
                placeholder: "from the host",
                room: net::MAX_CODE_LENGTH as u16,
            },
        ];
        form::draw(page, Self::TITLE, Self::BORDER_COLOR, &fields, self.focus);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        match key_code {
            KeyCode::Enter => match self.address() {
                Ok(address) => Ok(Phase::Return(Some(address))),
                Err(problem) => {
                    Warning::summon(grimoire, problem)?;
                    Ok(Phase::Render)
                }
            },
            KeyCode::Esc => Ok(Phase::Return(None)),
            KeyCode::Tab => {
                self.focus = (self.focus + 1) % Self::FIELDS;
                Ok(Phase::Render)
            }
            KeyCode::BackTab => {
                self.focus = (self.focus + Self::FIELDS - 1) % Self::FIELDS;
                Ok(Phase::Render)
            }
            KeyCode::Char(c @ ('0'..='9' | '.')) if self.focus == 0 => {
                self.ip.push(c);
                Ok(Phase::Render)
            }
            KeyCode::Char(c @ '0'..='9') if self.focus == 1 && self.port.len() < 5 => {
                self.port.push(c);
                Ok(Phase::Render)
            }
            KeyCode::Char(c @ ('a'..='z' | 'A'..='Z' | '0'..='9'))
                if self.focus == 2 && self.code.len() < net::MAX_CODE_LENGTH =>
            {
                self.code.push(c.to_ascii_lowercase());
                Ok(Phase::Render)
            }
            KeyCode::Backspace => Ok(Phase::render_if(self.focused().pop().is_some())),
            KeyCode::Char('?') => {
                Legend::summon(
                    grimoire,
                    match self.focus {
                        0 => Self::IP_LEGEND,
                        1 => Self::PORT_LEGEND,
                        _ => Self::CODE_LEGEND,
                    },
                )?;
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
        assert_snapshot!(testing::draw(50, 12, |page| form.render(page)));
    }

    #[test]
    fn an_empty_port_is_the_default_one() {
        let mut form = JoinAddressForm::new();
        form.ip = "10.0.0.2".into();
        form.code = "abc234".into();
        assert_eq!(
            form.address(),
            Ok(Address {
                host: "10.0.0.2".into(),
                port: net::DEFAULT_PORT,
                code: "abc234".into(),
            })
        );
        form.port = "9000".into();
        assert_eq!(form.address().map(|a| a.port), Ok(9000));
    }

    #[test]
    fn a_form_missing_something_says_what() {
        let mut form = JoinAddressForm::new();
        assert!(form.address().is_err());
        form.ip = "10.0.0.2".into();
        assert_eq!(
            form.address(),
            Err("Enter the join code the host was given")
        );
        form.code = "abc234".into();
        form.port = "70000".into();
        assert!(form.address().is_err());
        form.port = "0".into();
        assert!(form.address().is_err());
    }
}
