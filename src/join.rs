use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    legend::Legend,
    ritual::{self, Grimoire, Page, Phase, Ritual},
};

/// Prompts for the IP address of the host to join. Only digits and dots are accepted.
pub struct Join {
    ip: String,
}

impl Join {
    const TITLE: &str = "Join";
    const BORDER_COLOR: Color = Color::LightMagenta;
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 5;
    const IP_LABEL: &str = "Ip Address";
    const LEGEND: &[[&str; 2]] = &[["Enter", "Launch"], ["Esc", "Cancel"], ["0-9/.", "Input"]];

    pub fn new() -> Self {
        Self { ip: String::new() }
    }

    /// `None` if cancelled.
    pub fn summon(grimoire: &mut Grimoire) -> Result<Option<String>> {
        Self::new().invoke(grimoire)
    }
}

impl Ritual for Join {
    type Return = Option<String>;

    fn render(&mut self, page: &mut Page) {
        let area = page.area();
        let (window, mut window_area, indicator, indicator_area) = ritual::conjure_window(
            Self::TITLE,
            Self::WIDTH,
            Self::HEIGHT,
            Self::BORDER_COLOR,
            area,
        );
        page.render_widget(window, window_area);
        page.render_widget(&indicator, indicator_area);

        window_area.y += 2;
        let (line, ip_position) =
            ritual::conjure_query(Self::IP_LABEL, &self.ip, area.width, window_area.y);
        page.render_widget(line, window_area);
        page.set_cursor_position(ip_position);
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
