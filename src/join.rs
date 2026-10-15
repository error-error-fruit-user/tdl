use ratatui::{Frame, crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    legend::Legend,
    scene::Scene,
    utility::{self, Phase, Terminal},
};

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

    pub fn evoke(terminal: &mut Terminal) -> Result<Option<String>> {
        Self::new().invoke(terminal)
    }
}

impl Scene for Join {
    type Return = Option<String>;

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let (window, mut window_area, indicator, indicator_area) = utility::new_window(
            Self::TITLE,
            Self::WIDTH,
            Self::HEIGHT,
            Self::BORDER_COLOR,
            area,
        );
        frame.render_widget(window, window_area);
        frame.render_widget(&indicator, indicator_area);

        window_area.y += 2;
        let (line, ip_position) =
            utility::new_query(Self::IP_LABEL, &self.ip, area.width, window_area.y);
        frame.render_widget(line, window_area);
        frame.set_cursor_position(ip_position);
    }

    fn prompt(
        &mut self,
        terminal: &mut Terminal,
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
                Legend::evoke(terminal, Self::LEGEND)?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}
