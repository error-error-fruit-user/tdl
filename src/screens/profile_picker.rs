use ratatui::{
    crossterm::event::KeyCode,
    style::{Color, Style},
    text::ToLine,
    widgets::{Row, Table, TableState},
};

use crate::{
    error::Result,
    popups::legend::Legend,
    ritual::{self, Grimoire, Page, Phase, Ritual},
    screens::wrap_around,
    vault::profile::Profile,
};

/// Lists the compatible profiles so the user can choose which to join with.
pub struct ProfilePicker<'a> {
    profiles: &'a [&'a Profile],
    rows: Vec<Row<'a>>,
    table_state: TableState,
}

impl<'a> ProfilePicker<'a> {
    const TITLE: &str = "Select Profile";
    const BORDER_COLOR: Color = Color::LightMagenta;
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 12;
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
            rows: profiles
                .iter()
                .map(|p| Row::new([p.name.to_line().centered()]))
                .collect(),
            table_state: TableState::new().with_selected(0),
        }
    }
}

impl Ritual for ProfilePicker<'_> {
    type Return = Option<Profile>;

    fn render(&mut self, page: &mut Page) {
        let (window, window_area, indicator, indicator_area) = ritual::conjure_window(
            Self::TITLE,
            Self::WIDTH,
            Self::HEIGHT,
            Self::BORDER_COLOR,
            page.area(),
        );
        let table = Table::new(self.rows.clone(), [Self::WIDTH])
            .block(window)
            .row_highlight_style(Style::new().black().bg(Self::BORDER_COLOR));
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
            KeyCode::Char('?') => {
                Legend::summon(grimoire, Self::LEGEND)?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}
