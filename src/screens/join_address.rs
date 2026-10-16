use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    net::{self, Address},
    popups::{legend::Legend, warning::Warning},
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
    widgets::{
        form::{self, Field},
        text,
    },
};

/// Prompts for where the host is: its IP address, the port its wads are served on, and the join
/// code it was given.
pub struct JoinAddressForm {
    ip: String,
    port: String,
    code: String,
    focus: usize,
    /// Where the cursor is in the focused field.
    caret: usize,
}

impl JoinAddressForm {
    const TITLE: &str = "join";
    const BORDER_COLOR: Color = theme::MULTIPLAYER;
    const FIELDS: usize = 3;
    /// Long enough for any IPv4 address.
    const MAX_IP: usize = 15;
    const IP_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["←/→/Home/End", "Move the Cursor"],
        ["0-9/.", "Input"],
        ["Esc", "Cancel"],
    ];
    const PORT_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["←/→/Home/End", "Move the Cursor"],
        ["0-9", "Input"],
        ["Esc", "Cancel"],
    ];
    const CODE_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["←/→/Home/End", "Move the Cursor"],
        ["a-z/0-9", "Input"],
        ["Esc", "Cancel"],
    ];

    pub fn new() -> Self {
        Self {
            ip: String::new(),
            port: String::new(),
            code: String::new(),
            focus: 0,
            caret: 0,
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
                caret: (self.focus == 0).then_some(self.caret),
            },
            Field {
                label: "port",
                value: &self.port,
                placeholder: &default_port,
                room: 5,
                caret: (self.focus == 1).then_some(self.caret),
            },
            Field {
                label: "code",
                value: &self.code,
                placeholder: "from the host",
                room: net::MAX_CODE_LENGTH as u16,
                caret: (self.focus == 2).then_some(self.caret),
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
            KeyCode::Char('?') | KeyCode::F(1) => {
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
            key_code => Ok(self.edit(key_code)),
        }
    }
}

impl JoinAddressForm {
    /// Moves to the field `by` away, wrapping around, with the cursor at the end of its text.
    fn move_focus(&mut self, by: usize) -> Phase<Option<Address>> {
        self.focus = (self.focus + by) % Self::FIELDS;
        self.caret = text::end(self.focused());
        Phase::Render
    }

    /// Every key but the ones that close the form or show the legend.
    fn edit(&mut self, key_code: KeyCode) -> Phase<Option<Address>> {
        match key_code {
            KeyCode::Tab | KeyCode::Down => self.move_focus(1),
            KeyCode::BackTab | KeyCode::Up => self.move_focus(Self::FIELDS - 1),
            key_code => {
                let (max, accept): (usize, fn(char) -> Option<char>) = match self.focus {
                    0 => (Self::MAX_IP, |c| {
                        (c.is_ascii_digit() || c == '.').then_some(c)
                    }),
                    1 => (5, |c| c.is_ascii_digit().then_some(c)),
                    _ => (net::MAX_CODE_LENGTH, |c| {
                        c.is_ascii_alphanumeric().then_some(c.to_ascii_lowercase())
                    }),
                };
                let mut caret = self.caret;
                let changed = text::edit(self.focused(), &mut caret, key_code, max, accept);
                self.caret = caret;
                match changed {
                    Some(changed) => Phase::render_if(changed),
                    None => Phase::Prompt,
                }
            }
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
    fn the_arrows_move_between_fields_and_about_the_text() {
        let mut form = JoinAddressForm::new();
        for c in "10.0.2".chars() {
            let _ = form.edit(KeyCode::Char(c));
        }
        // back two, and the missing `.0` goes in the middle
        let _ = form.edit(KeyCode::Left);
        let _ = form.edit(KeyCode::Left);
        let _ = form.edit(KeyCode::Char('.'));
        let _ = form.edit(KeyCode::Char('0'));
        assert_eq!(form.ip, "10.0.0.2");
        assert!(form.edit(KeyCode::Down) == Phase::Render);
        assert_eq!(form.focus, 1);
        let _ = form.edit(KeyCode::Up);
        let _ = form.edit(KeyCode::Up);
        assert_eq!(form.focus, 2, "wrapping around to the code");
        let _ = form.edit(KeyCode::Char('A'));
        assert_eq!(form.code, "a");
        // letters don't go in the ip
        let _ = form.edit(KeyCode::Down);
        assert!(form.edit(KeyCode::Char('x')) == Phase::Prompt);
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
