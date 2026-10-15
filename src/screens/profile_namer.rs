use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    popups::{legend::Legend, warning::Warning},
    ritual::{self, Grimoire, Page, Phase, Ritual},
    vault::profile::Profile,
};

/// Prompts for a name for a downloaded profile, rejecting empty and taken names.
pub struct ProfileNamer<'a> {
    profile_name: String,
    used_profiles: &'a [Profile],
}

impl<'a> ProfileNamer<'a> {
    const TITLE: &str = "Name Profile";
    const BORDER_COLOR: Color = Color::LightMagenta;
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 5;
    const LEGEND: &[[&str; 2]] = &[
        ["Enter", "Save Profile"],
        ["A-Z/0-9/_", "Input"],
        ["Esc", "Cancel"],
    ];

    pub fn summon(grimoire: &mut Grimoire, used_profiles: &'a [Profile]) -> Result<Option<String>> {
        Self::new(used_profiles).invoke(grimoire)
    }

    fn new(used_profiles: &'a [Profile]) -> Self {
        Self {
            profile_name: String::new(),
            used_profiles,
        }
    }
}

impl Ritual for ProfileNamer<'_> {
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
        page.render_widget(indicator, indicator_area);
        window_area.y += 2;

        let (line, cursor_position) =
            ritual::conjure_query("Name", &self.profile_name, area.width, window_area.y);

        page.render_widget(line, window_area);
        page.set_cursor_position(cursor_position);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        match key_code {
            KeyCode::Enter => {
                if self.profile_name.is_empty() {
                    Warning::empty_profile_name(grimoire)?;
                    Ok(Phase::Render)
                } else if self
                    .used_profiles
                    .iter()
                    .any(|p| p.name == self.profile_name)
                {
                    Warning::profile_already_exists(grimoire)?;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Return(Some(self.profile_name.clone())))
                }
            }
            KeyCode::Char(c @ 'a'..='z' | c @ 'A'..='Z' | c @ '0'..='9' | c @ '_') => {
                self.profile_name.push(c);
                Ok(Phase::Render)
            }
            KeyCode::Backspace => {
                self.profile_name.pop();
                Ok(Phase::Render)
            }
            KeyCode::Esc => Ok(Phase::Return(None)),
            KeyCode::Char('?') => {
                Legend::summon(grimoire, Self::LEGEND)?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}
