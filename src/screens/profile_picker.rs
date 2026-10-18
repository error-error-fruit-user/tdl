use ratatui::{
    crossterm::event::KeyCode,
    layout::Constraint,
    style::Color,
    widgets::{Row, Table, TableState},
};

use crate::{
    error::Result,
    layout,
    loadout::profile::Profile,
    popups::legend::Legend,
    ritual::{self, Grimoire, Page, Phase, Ritual},
    screens::wrap_around,
    theme,
};

/// Lists the compatible profiles so the user can choose which to join with.
pub struct ProfilePicker<'a> {
    profiles: &'a [&'a Profile],
    table_state: TableState,
}

impl<'a> ProfilePicker<'a> {
    const TITLE: &str = "pick a profile";
    fn border_color() -> Color {
        theme::worn().colors.multiplayer
    }
    const LEGEND: &[[&str; 2]] = &[
        ["Enter", "Select Profile"],
        ["J/K", "Traverse"],
        ["Esc", "Cancel"],
    ];

    pub fn summon(grimoire: &mut Grimoire, profiles: &'a [&'a Profile]) -> Result<Option<Profile>> {
        Self::new(profiles).invoke(grimoire)
    }

    fn new(profiles: &'a [&'a Profile]) -> Self {
        Self {
            profiles,
            table_state: TableState::new().with_selected(0),
        }
    }
}

impl Ritual for ProfilePicker<'_> {
    type Return = Option<Profile>;

    fn render(&mut self, page: &mut Page) {
        let width = self
            .profiles
            .iter()
            .map(|p| layout::width(&p.name))
            .chain([layout::width(Self::TITLE)])
            .max()
            .unwrap_or(0);
        let (width, height) = layout::window(width, self.profiles.len() as u16);
        let (window, window_area, indicator, indicator_area) = ritual::conjure_window(
            Self::TITLE,
            width,
            height,
            Self::border_color(),
            page.area(),
        );
        let rows = self.profiles.iter().map(|p| Row::new([p.name.as_str()]));
        let table = Table::new(rows, [Constraint::Fill(1)])
            .block(window)
            .row_highlight_style(theme::worn().styles.cursor);
        page.render_stateful_widget(table, window_area, &mut self.table_state);
        page.render_widget(indicator, indicator_area);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        let index = self.table_state.selected_mut().as_mut().unwrap();
        match key_code {
            KeyCode::Enter => Ok(Phase::Return(Some(self.profiles[*index].clone()))),
            KeyCode::Char('j') => {
                *index = wrap_around::next(*index, self.profiles.len());
                Ok(Phase::Render)
            }
            KeyCode::Char('k') => {
                *index = wrap_around::previous(*index, self.profiles.len());
                Ok(Phase::Render)
            }
            KeyCode::Esc => Ok(Phase::Return(None)),
            KeyCode::Char('?') | KeyCode::F(1) => {
                Legend::summon(grimoire, Self::LEGEND)?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}
