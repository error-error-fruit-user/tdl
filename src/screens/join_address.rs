use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    net::{self, Address, ini::HostIni},
    popups::{legend::Legend, warning::Warning},
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
    widgets::{
        form::{self, Field},
        text,
    },
};

/// Prompts for where the host is: its IP address, the port its wads are served on, and the join
/// code it was given, and whether to play with the host's ini (and its keys), if it shares it.
pub struct JoinAddressForm {
    ip: String,
    port: String,
    code: String,
    host_ini: HostIni,
    focus: usize,
    /// Where the cursor is in the focused field.
    caret: usize,
}

impl JoinAddressForm {
    const TITLE: &str = "join";
    fn border_color() -> Color {
        theme::worn().colors.multiplayer
    }
    const FIELDS: usize = 4;
    /// Long enough for any IPv4 address.
    const MAX_IP: usize = 15;
    const IP_LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["←/→/Home/End", "Move the Cursor"],
        ["0-9/.", "Input"],
        ["Esc", "Cancel"],
    ];
    const PORT_LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["←/→/Home/End", "Move the Cursor"],
        ["0-9", "Input"],
        ["Esc", "Cancel"],
    ];
    const CODE_LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["←/→/Home/End", "Move the Cursor"],
        ["a-z/0-9", "Input"],
        ["Esc", "Cancel"],
    ];

    const INI_LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Launch"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["J/K, ←/→", "Yours, the Host's, or It and Its Keys"],
        ["Esc", "Cancel"],
    ];

    pub fn new() -> Self {
        Self {
            ip: String::new(),
            port: String::new(),
            code: String::new(),
            host_ini: HostIni::Yours,
            focus: 0,
            caret: 0,
        }
    }

    /// `None` if cancelled.
    pub fn summon(grimoire: &mut Grimoire) -> Result<Option<Joining>> {
        Self::new().invoke(grimoire)
    }

    /// [`JoinAddressForm::summon`], filled in with what `tdl join` was given, with the focus on
    /// the first field still empty (or the IP, if none is), ready to change or submit.
    pub fn summon_with(
        grimoire: &mut Grimoire,
        host: &str,
        port: Option<u16>,
        code: Option<&str>,
        host_ini: HostIni,
    ) -> Result<Option<Joining>> {
        Self::filled(host, port, code, host_ini).invoke(grimoire)
    }

    /// A form filled in with `host`, `port`, `code` and which ini to play with.
    fn filled(host: &str, port: Option<u16>, code: Option<&str>, host_ini: HostIni) -> Self {
        let mut form = Self {
            ip: host.to_string(),
            port: port.map(|port| port.to_string()).unwrap_or_default(),
            code: code.unwrap_or_default().to_string(),
            host_ini,
            ..Self::new()
        };
        // 👆 the code is what is most likely missing, so that is where the cursor waits
        form.focus = if form.code.is_empty() { 2 } else { 0 };
        form.caret = form.focused().map_or(0, |text| text::end(text));
        form
    }

    /// The field in focus, to type into, if it is one that is typed into.
    fn focused(&mut self) -> Option<&mut String> {
        match self.focus {
            0 => Some(&mut self.ip),
            1 => Some(&mut self.port),
            2 => Some(&mut self.code),
            _ => None,
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

/// What the join form was left with: where the host is, and which ini to play with.
#[derive(Debug, PartialEq)]
pub struct Joining {
    pub address: Address,
    pub host_ini: HostIni,
}

impl Ritual for JoinAddressForm {
    type Return = Option<Joining>;

    fn typing(&self) -> bool {
        self.focus < 3
    }

    fn legend(&self) -> &[[&'static str; 2]] {
        match self.focus {
            0 => Self::IP_LEGEND,
            1 => Self::PORT_LEGEND,
            2 => Self::CODE_LEGEND,
            _ => Self::INI_LEGEND,
        }
    }

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
            Field {
                label: "ini",
                value: match self.host_ini {
                    HostIni::Yours => "yours",
                    HostIni::Settings => "the host's",
                    HostIni::WithKeys => "the host's + its keys",
                },
                placeholder: "",
                room: 21,
                caret: None,
            },
        ];
        form::draw(page, Self::TITLE, Self::border_color(), &fields, self.focus);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        match key_code {
            KeyCode::Enter => match self.address() {
                Ok(address) => Ok(Phase::Return(Some(Joining {
                    address,
                    host_ini: self.host_ini,
                }))),
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
                        2 => Self::CODE_LEGEND,
                        _ => Self::INI_LEGEND,
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
    fn move_focus(&mut self, by: usize) -> Phase<Option<Joining>> {
        self.focus = (self.focus + by) % Self::FIELDS;
        self.caret = self.focused().map_or(0, |text| text::end(text));
        Phase::Render
    }

    /// Every key but the ones that close the form or show the legend.
    fn edit(&mut self, key_code: KeyCode) -> Phase<Option<Joining>> {
        match key_code {
            KeyCode::Tab | KeyCode::Down => self.move_focus(1),
            KeyCode::BackTab | KeyCode::Up => self.move_focus(Self::FIELDS - 1),
            KeyCode::Char('j') | KeyCode::Right if self.focus == 3 => {
                self.host_ini = match self.host_ini {
                    HostIni::Yours => HostIni::Settings,
                    HostIni::Settings => HostIni::WithKeys,
                    HostIni::WithKeys => HostIni::Yours,
                };
                Phase::Render
            }
            KeyCode::Char('k') | KeyCode::Left if self.focus == 3 => {
                self.host_ini = match self.host_ini {
                    HostIni::Yours => HostIni::WithKeys,
                    HostIni::Settings => HostIni::Yours,
                    HostIni::WithKeys => HostIni::Settings,
                };
                Phase::Render
            }
            _ if self.focus == 3 => Phase::Prompt,
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
                let text = self.focused().expect("a typed field has focus");
                let changed = text::edit(text, &mut caret, key_code, max, accept);
                self.caret = caret;
                match changed {
                    Some(changed) => Phase::render_if(changed),
                    None => Phase::Prompt,
                }
            }
        }
    }
}
