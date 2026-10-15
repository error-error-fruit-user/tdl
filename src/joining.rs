use std::ops::Deref;

use ratatui::{
    crossterm::event::KeyCode,
    style::{Color, Style},
    text::ToLine,
    widgets::{Row, Table, TableState},
};

use crate::{
    dialog::Dialog,
    error::Result,
    execute, index,
    legend::Legend,
    library::Library,
    network::{self, Manifest},
    profile::Profile,
    ritual::{self, Grimoire, Page, Phase, Ritual},
    status,
    wad::Wad,
    warning::Warning,
};

const BORDER_COLOR: Color = Color::LightMagenta;

/// Fetches the host's manifest and picks a profile to join with: the only compatible one, a choice
/// among several, or a new profile downloaded from the host when none match.
pub fn summon(
    grimoire: &mut Grimoire,
    library: &mut Library,
    index: &mut usize,
    ip: &str,
) -> Result<()> {
    status::summon(grimoire, "Joining", "Joining...", BORDER_COLOR)?;
    let Ok(manifest) = Manifest::fetch(ip) else {
        return Warning::host_unreachable(grimoire);
    };
    let Some(profile) = resolve_profile(grimoire, library, index, ip, &manifest)? else {
        return Ok(());
    };
    let Some(uzdoom) = profile.uzdoom(&library.config) else {
        return Warning::uzdoom_not_selected(grimoire);
    };
    let base_wad = profile.base_wad(&library.config);
    if manifest.base_wad_matches(base_wad)
        || Dialog::summon(
            grimoire,
            "Selected base wad does not match, do you want to continue?",
        )?
    {
        execute::join(
            uzdoom,
            base_wad.map(Wad::deref),
            &profile,
            ip,
            &library.data_dir.join(&profile.name),
        )
    } else {
        Ok(())
    }
}

/// The profile to join with: the single compatible one, a pick among several, or a fresh download.
/// `None` if the user backs out.
fn resolve_profile(
    grimoire: &mut Grimoire,
    library: &mut Library,
    index: &mut usize,
    ip: &str,
    manifest: &Manifest,
) -> Result<Option<Profile>> {
    let compatible_profiles: Vec<_> = library
        .profiles
        .iter()
        .filter(|p| manifest.is_compatible_with(p))
        .collect();
    match compatible_profiles.len() {
        0 => download_profile(grimoire, library, index, ip, manifest),
        1 => Ok(Some(compatible_profiles[0].clone())),
        _ => ProfilePicker::summon(grimoire, &compatible_profiles),
    }
}

/// Offers to download the host's wads into a new profile, named by the user.
fn download_profile(
    grimoire: &mut Grimoire,
    library: &mut Library,
    index: &mut usize,
    ip: &str,
    manifest: &Manifest,
) -> Result<Option<Profile>> {
    if library.config.uzdoom.is_none() {
        Warning::uzdoom_not_selected(grimoire)?;
        return Ok(None);
    }
    if !Dialog::summon(
        grimoire,
        "You don't have the profile, would you like to download?",
    )? {
        return Ok(None);
    }
    let Some(profile_name) = ProfileNamer::summon(grimoire, &library.profiles)? else {
        return Ok(None);
    };
    status::summon(grimoire, "Downloading", "Downloading...", BORDER_COLOR)?;
    let Ok(wads) = network::download(ip, &manifest.entries, &library.downloads_dir()) else {
        Warning::download_failed(grimoire)?;
        return Ok(None);
    };
    let profile = Profile::new(profile_name, wads);
    library.insert(index, profile.clone());
    Ok(Some(profile))
}

/// Lists the compatible profiles so the user can choose which to join with.
struct ProfilePicker<'a> {
    profiles: &'a [&'a Profile],
    rows: Vec<Row<'a>>,
    table_state: TableState,
}

impl<'a> ProfilePicker<'a> {
    const TITLE: &str = "Select Profile";
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 12;
    const LEGEND: &[[&str; 2]] = &[
        ["Enter", "Select Profile"],
        ["J/K", "Traverse"],
        ["Esc", "Cancel"],
    ];

    fn summon(grimoire: &mut Grimoire, profiles: &'a [&'a Profile]) -> Result<Option<Profile>> {
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
            BORDER_COLOR,
            page.area(),
        );
        let table = Table::new(self.rows.clone(), [Self::WIDTH])
            .block(window)
            .row_highlight_style(Style::new().black().bg(BORDER_COLOR));
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
                *index = index::next(*index, self.profiles.len());
                Ok(Phase::Render)
            }
            KeyCode::Char('k') => {
                *index = index::previous(*index, self.profiles.len());
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

/// Prompts for a name for a downloaded profile, rejecting empty and taken names.
struct ProfileNamer<'a> {
    profile_name: String,
    used_profiles: &'a [Profile],
}

impl<'a> ProfileNamer<'a> {
    const TITLE: &str = "Name Profile";
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 5;
    const LEGEND: &[[&str; 2]] = &[
        ["Enter", "Save Profile"],
        ["A-Z/0-9/_", "Input"],
        ["Esc", "Cancel"],
    ];

    fn summon(grimoire: &mut Grimoire, used_profiles: &'a [Profile]) -> Result<Option<String>> {
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
        let (window, mut window_area, indicator, indicator_area) =
            ritual::conjure_window(Self::TITLE, Self::WIDTH, Self::HEIGHT, BORDER_COLOR, area);
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
