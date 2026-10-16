use std::path::{Path, PathBuf};

use ratatui::{
    crossterm::event::KeyCode,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Row, Table, TableState},
};

use crate::{
    error::Result,
    layout, picker,
    popups::legend::Legend,
    popups::warning::Warning,
    popups::{self, dialog::Dialog, hashing},
    ritual::{self, Grimoire, Page, Phase, Ritual},
    screens::{
        config_selector::{ConfigSelector, IniChoice},
        wrap_around,
    },
    theme,
    vault::Vault,
    vault::profile::Profile,
    vault::wad::{Fault, Wad},
    widgets::{
        details::Details,
        form::{self, Field},
        mark::{self, Mark},
    },
};

/// Creates or edits one profile; `Enter`/`Esc` both finish, and an empty untouched profile is discarded.
///
/// `profiles` is every existing profile, used to reject duplicate names; when editing, the
/// profile's own entry is excluded via `skipped_index`.
pub struct Editor<'a> {
    profile: Profile,
    profiles: &'a [Profile],
    /// Where profile data lives, for the ini file a new profile can be given.
    data_dir: &'a Path,
    skipped_index: Option<usize>,
    editing_wads: bool,
    table_state: TableState,
    /// What is wrong with each wad in the list, checked when the list changes and not on every redraw.
    faults: Vec<Option<Fault>>,
}

impl<'a> Editor<'a> {
    const BORDER_COLOR: Color = theme::SCREEN;
    /// Labels the rule between the name and the wad list.
    const DIVIDER: &str = "wads";
    /// The last row, which adds wads.
    const NEW_ROW: &str = "+ add wads";
    /// The order column: up to three digits and a dot.
    const ORDER_WIDTH: u16 = 4;
    const COLUMN_SPACING: u16 = 1;
    const NAMING_LEGEND: &[[&str; 2]] = &[
        ["Enter/Esc", "Save and Quit"],
        ["Tab", "Edit Wads"],
        ["A-Z/0-9/_", "Input"],
    ];
    const EDITING_LEGEND: &[[&str; 2]] = &[
        ["Enter/Esc", "Save and Quit"],
        ["Tab", "Edit Name"],
        ["J↓/K↑", "Traverse"],
        ["}/{", "Swap"],
        ["Space", "Inspect Wad"],
        ["R", "Replace Wad"],
        ["V", "Toggle Volatility"],
        ["A", "Add"],
        ["F", "Import from Folder"],
        ["D", "Delete"],
        ["U", "Select UZDoom"],
        ["W", "Select Base Wad"],
        ["I", "Choose Ini"],
    ];

    /// Creates a profile. `None` if cancelled or left empty.
    pub fn summon_blank(grimoire: &mut Grimoire, vault: &'a Vault) -> Result<Option<Profile>> {
        Self::new(&vault.profiles, &vault.data_dir).invoke(grimoire)
    }

    /// Edits `vault.profiles[index]` and saves the result. Cancelling keeps the original.
    pub fn summon_existing(grimoire: &mut Grimoire, vault: &mut Vault, index: usize) -> Result<()> {
        if let Some(profile) =
            Editor::edit(&vault.profiles, &vault.data_dir, index).invoke(grimoire)?
            && vault.replace(index, profile).is_err()
        {
            Warning::save_failed(grimoire)?;
        }
        Ok(())
    }

    /// Starts on the name field.
    fn new(profiles: &'a [Profile], data_dir: &'a Path) -> Self {
        Self {
            profile: Profile::default(),
            profiles,
            data_dir,
            skipped_index: None,
            editing_wads: false,
            table_state: TableState::new().with_selected(0),
            faults: Vec::new(),
        }
    }

    /// Starts on the wad list, with the existing profile loaded.
    fn edit(profiles: &'a [Profile], data_dir: &'a Path, index: usize) -> Self {
        let profile = profiles[index].clone();
        Self {
            faults: check_wads(&profile.wads),
            profile,
            skipped_index: Some(index),
            editing_wads: true,
            ..Self::new(profiles, data_dir)
        }
    }

    /// Swaps the wad at `index` for `replacement`. If its hash differs from the one the profile
    /// expected, `confirm` decides whether to go ahead. Returns whether the wad was replaced.
    fn replace_wad(
        &mut self,
        index: usize,
        replacement: Wad,
        confirm: impl FnOnce() -> Result<bool>,
    ) -> Result<bool> {
        let old = &self.profile.wads[index];
        if !old.volatile && !old.hash.is_empty() && old.hash != replacement.hash && !confirm()? {
            return Ok(false);
        }
        let replacement = Wad {
            volatile: old.volatile,
            ..replacement
        };
        self.faults[index] = replacement.check();
        self.profile.wads[index] = replacement;
        Ok(true)
    }

    /// Whether another profile already has this name. The profile being edited doesn't count.
    fn name_is_taken(&self) -> bool {
        self.profiles
            .iter()
            .enumerate()
            .any(|(i, p)| Some(i) != self.skipped_index && p.name == self.profile.name)
    }

    /// Gives the profile an ini of its own in its data folder, which needs a name that is free.
    fn create_ini(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        if self.profile.name.is_empty() {
            return Warning::empty_profile_name(grimoire);
        }
        if self.name_is_taken() {
            return Warning::profile_already_exists(grimoire);
        }
        match new_ini(self.data_dir, &self.profile.name) {
            Ok(path) => self.profile.ini = Some(path.into()),
            Err(_) => Warning::save_failed(grimoire)?,
        }
        Ok(())
    }

    /// Flips whether the wad at `index` is volatile, and checks it again, since volatile wads
    /// aren't flagged for changing size.
    fn toggle_volatility(&mut self, index: usize) {
        let wad = &mut self.profile.wads[index];
        wad.volatile = !wad.volatile;
        self.faults[index] = wad.check();
    }
}

impl Ritual for Editor<'_> {
    type Return = Option<Profile>;

    /// The name and the wad list in one window, and under it what the profile overrides.
    fn render(&mut self, page: &mut Page) {
        let name = [Field {
            label: "name",
            value: &self.profile.name,
            placeholder: "type a name",
            room: 12,
        }];
        let details = Details::overrides(&self.profile);

        let wad_width = self
            .profile
            .wads
            .iter()
            .map(|w| layout::width(&w.file_name()))
            .chain([layout::width(Self::NEW_ROW)])
            .max()
            .unwrap_or(0);
        let list_width = mark::WIDTH + Self::ORDER_WIDTH + wad_width + 2 * Self::COLUMN_SPACING;
        let (name_width, _) = form::size(&name);
        let (details_width, details_height) = details.size();
        let panel = layout::window(details_width, details_height);
        // the name, a blank row, the divider, the wads and the add row
        let (width, height) = layout::window(
            list_width.max(name_width),
            self.profile.wads.len() as u16 + 4,
        );
        let editor = layout::grow(width, height, panel.1, page.area());
        let [editor_area, details_area] = layout::stack(&[editor, panel], page.area())[..] else {
            return;
        };

        let title = if self.skipped_index.is_some() {
            "edit profile"
        } else {
            "new profile"
        };
        let veil = ritual::conjure_veil(title, Self::BORDER_COLOR);
        let inner = veil.inner(editor_area);
        page.render_widget(veil, editor_area);

        let focus = (!self.editing_wads).then_some(0);
        let name_row = Rect {
            width: name_width.min(inner.width),
            height: 1,
            ..inner
        };
        if let Some(cursor) = form::render(&name, focus, name_row, page.buffer_mut()) {
            page.set_cursor_position(cursor);
        }

        let selected_fault = self
            .table_state
            .selected()
            .and_then(|i| self.faults.get(i))
            .and_then(Option::as_ref);
        let divider_row = Rect {
            y: inner.y + 2,
            height: 1,
            ..inner
        };
        page.render_widget(
            divider(Self::DIVIDER, self.profile.wads.len(), inner.width),
            divider_row.intersection(inner),
        );
        let rows = self
            .profile
            .wads
            .iter()
            .zip(&self.faults)
            .enumerate()
            .map(|(i, (wad, fault))| {
                // 🕳️ a wad that has gone missing leaves a hole in the list, not a gap in the story
                let style = match Mark::of_wad(wad, fault.as_ref()) {
                    Some(Mark::Broken) => theme::BROKEN_ROW,
                    Some(Mark::Changed) => Style::new().fg(theme::CHANGED),
                    // 🌀 volatile wads are expected to shift under your feet
                    Some(Mark::Volatile) => Style::new().fg(theme::VOLATILE),
                    None => Style::new(),
                };
                Row::new([
                    Line::default(),
                    Line::from(format!("{}.", i + 1)).right_aligned(),
                    Line::from(wad.file_name().into_owned()),
                ])
                .style(style)
            })
            .chain([Row::new([
                Line::default(),
                Line::default(),
                Line::styled(Self::NEW_ROW, Style::new().fg(theme::MUTED)),
            ])]);
        let table = Table::new(
            rows,
            [
                Constraint::Length(mark::WIDTH),
                Constraint::Length(Self::ORDER_WIDTH),
                Constraint::Fill(1),
            ],
        )
        .column_spacing(Self::COLUMN_SPACING);
        let table = match (self.editing_wads, selected_fault) {
            (false, _) => table,
            (true, Some(fault)) if fault.is_blocking() => {
                table.row_highlight_style(theme::BROKEN_CURSOR)
            }
            (true, _) => table.row_highlight_style(theme::CURSOR),
        };
        let list_area = Rect {
            y: inner.y + 3,
            height: inner.height.saturating_sub(3),
            ..inner
        };
        page.render_stateful_widget(table, list_area, &mut self.table_state);
        // 🩸 the badges go on last, so even the cursor's highlight can't hide them
        let offset = self.table_state.offset();
        let shown = offset..(offset + list_area.height as usize).min(self.profile.wads.len());
        for i in shown {
            if let Some(mark) = Mark::of_wad(&self.profile.wads[i], self.faults[i].as_ref()) {
                let y = list_area.y + (i - offset) as u16;
                mark.paint(list_area.x, y, list_area, page.buffer_mut());
            }
        }

        let panel = ritual::conjure_banner(details.title, theme::PANEL, details.title_color);
        let fitting = details.lines_fitting(panel.inner(details_area).width);
        page.render_widget(Paragraph::new(fitting).block(panel), details_area);

        let (sigil, sigil_area) = ritual::conjure_sigil(details_area);
        page.render_widget(sigil, sigil_area);
    }

    fn prompt(&mut self, grimoire: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Self::Tea>> {
        let index = self.table_state.selected_mut().as_mut().unwrap();
        match key_code {
            KeyCode::Esc | KeyCode::Enter => {
                if self.profile.name.is_empty() {
                    if self.profile.wads.is_empty()
                        && self.profile.uzdoom.is_none()
                        && self.profile.base_wad.is_none()
                    {
                        Ok(Phase::Return(None))
                    } else {
                        Warning::empty_profile_name(grimoire)?;
                        Ok(Phase::Render)
                    }
                } else if self.name_is_taken() {
                    Warning::profile_already_exists(grimoire)?;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Return(Some(self.profile.clone())))
                }
            }
            KeyCode::Tab => {
                self.editing_wads = !self.editing_wads;
                Ok(Phase::Render)
            }
            KeyCode::Char(c @ 'a'..='z' | c @ 'A'..='Z' | c @ '0'..='9' | c @ '_')
                if !self.editing_wads =>
            {
                self.profile.name.push(c);
                Ok(Phase::Render)
            }
            KeyCode::Backspace if !self.editing_wads => {
                self.profile.name.pop();
                Ok(Phase::Render)
            }
            KeyCode::Char('j') | KeyCode::Down => Ok(Phase::render_if(wrap_around::select_next(
                self.table_state.selected_mut(),
                self.profile.wads.len() + 1,
            ))),
            KeyCode::Char('k') | KeyCode::Up => Ok(Phase::render_if(wrap_around::select_previous(
                self.table_state.selected_mut(),
                self.profile.wads.len() + 1,
            ))),
            KeyCode::Char('}') => Ok(Phase::render_if_then(
                self.profile.wads.len() != 1 && *index != self.profile.wads.len(),
                || {
                    let next_index = wrap_around::next(*index, self.profile.wads.len());
                    self.profile.wads.swap(*index, next_index);
                    self.faults.swap(*index, next_index);
                    *index = next_index;
                },
            )),
            KeyCode::Char('{') => Ok(Phase::render_if_then(
                self.profile.wads.len() != 1 && *index != self.profile.wads.len(),
                || {
                    let previous_index = wrap_around::previous(*index, self.profile.wads.len());
                    self.profile.wads.swap(*index, previous_index);
                    self.faults.swap(*index, previous_index);
                    *index = previous_index;
                },
            )),
            KeyCode::Char(' ') if self.editing_wads => {
                let index = *index;
                if let Some(Some(fault)) = self.faults.get(index) {
                    let lines = inspect_lines(&self.profile.wads[index], fault);
                    Warning::summon_styled(grimoire, lines)?;
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('v') if self.editing_wads => {
                let index = *index;
                Ok(Phase::render_if_then(
                    index != self.profile.wads.len(),
                    || self.toggle_volatility(index),
                ))
            }
            KeyCode::Char('r') if self.editing_wads && *index != self.profile.wads.len() => {
                let index = *index;
                // 📂 start where the wad used to live, since it probably only moved a little
                let start = Path::new(&self.profile.wads[index].path).parent();
                let Some(path) = picker::file(grimoire, Some(("wads", Wad::EXTENSIONS)), start)?
                else {
                    return Ok(Phase::Prompt);
                };
                let Some(replacement) = hashing::wad(grimoire, &path)? else {
                    return Ok(Phase::Render);
                };
                let replaced = self.replace_wad(index, replacement, || {
                    Dialog::should_use_different_file(grimoire)
                })?;
                Ok(Phase::render_if(replaced))
            }
            KeyCode::Char('a') => {
                let Some(mod_paths) =
                    picker::files(grimoire, Some(("wads", Wad::EXTENSIONS)), None)?
                else {
                    return Ok(Phase::Prompt);
                };
                if mod_paths.is_empty() {
                    return Ok(Phase::Prompt);
                }
                let Some(wads) = hashing::wads(grimoire, &mod_paths)? else {
                    return Ok(Phase::Render);
                };
                let length = wads.len();
                let should_add = *index == self.profile.wads.len();
                for (index, wad) in (*index..*index + length).zip(wads) {
                    self.profile.wads.insert(index, wad);
                }
                self.faults = check_wads(&self.profile.wads);
                if should_add {
                    *index += length
                }
                Ok(Phase::Render)
            }
            KeyCode::Char('f') => {
                if let Some(folder) = picker::folder(grimoire, None)? {
                    let mut wad_paths = Vec::new();
                    for entry in std::fs::read_dir(folder)? {
                        let path = entry?.path();
                        if path.is_file()
                            && path
                                .extension()
                                .and_then(|e| e.to_str())
                                .is_some_and(|e| Wad::EXTENSIONS.contains(&e))
                        {
                            wad_paths.push(path);
                        }
                    }
                    wad_paths.sort();
                    let Some(wads) = hashing::wads(grimoire, &wad_paths)? else {
                        return Ok(Phase::Render);
                    };
                    let length = wads.len();
                    let should_add = *index == self.profile.wads.len();
                    for (index, wad) in (*index..*index + length).zip(wads) {
                        self.profile.wads.insert(index, wad);
                    }
                    self.faults = check_wads(&self.profile.wads);
                    if should_add {
                        *index += length;
                    }
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('d') => {
                if *index != self.profile.wads.len() {
                    if Dialog::should_delete(grimoire, &self.profile.wads[*index].file_name())? {
                        self.profile.wads.remove(*index);
                        self.faults.remove(*index);
                        if *index == self.profile.wads.len() && *index != 0 {
                            *index -= 1;
                        }
                    }
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('u') => {
                if let Some(path) = picker::file(grimoire, None, None)? {
                    self.profile.uzdoom = Some(path.into());
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('w') => {
                if let Some(path) =
                    picker::file(grimoire, Some(("base wad", Wad::EXTENSIONS)), None)?
                {
                    // 🖼️ redrawn either way, since the hashing window may have covered the editor
                    if let Some(wad) = hashing::wad(grimoire, &path)? {
                        self.profile.base_wad = Some(wad);
                    }
                    return Ok(Phase::Render);
                }
                Ok(Phase::Prompt)
            }
            KeyCode::Char('i') => {
                match ConfigSelector::summon(grimoire)? {
                    Some(IniChoice::Global) => self.profile.ini = None,
                    Some(IniChoice::New) => self.create_ini(grimoire)?,
                    Some(IniChoice::Existing(path)) => self.profile.ini = Some(path.into()),
                    None => {}
                }
                Ok(Phase::Render)
            }
            KeyCode::Char('?') => {
                let legend = if self.editing_wads {
                    Self::EDITING_LEGEND
                } else {
                    Self::NAMING_LEGEND
                };
                Legend::summon(grimoire, legend)?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}

/// Makes an empty `uzdoom.ini` in the profile's data folder and returns where it is. UZDoom fills
/// it in the first time it runs. An ini that is already there is left exactly as it is.
fn new_ini(data_dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let folder = data_dir.join(name);
    std::fs::create_dir_all(&folder)?;
    let path = folder.join("uzdoom.ini");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Err(error) if error.kind() != std::io::ErrorKind::AlreadyExists => Err(error),
        _ => Ok(path),
    }
}

/// A rule across the window with a label at the start and a count at the end, like
/// `wads ───────────── 4`, dividing the name from the list.
fn divider(label: &str, count: usize, width: u16) -> Line<'_> {
    let count = count.to_string();
    let rule = (width as usize).saturating_sub(label.len() + count.len() + 2);
    Line::from(vec![
        Span::styled(label, theme::HEADER),
        Span::styled(
            format!(" {} ", "─".repeat(rule)),
            Style::new().fg(theme::IRON),
        ),
        Span::styled(count, Style::new().fg(theme::MUTED)),
    ])
}

/// Looks at every wad on disk. One check each, no hashing.
fn check_wads(wads: &[Wad]) -> Vec<Option<Fault>> {
    wads.iter().map(Wad::check).collect()
}

/// What to tell the user about a wad with a fault: its name, where it should be, what is wrong,
/// and what to do about it.
fn inspect_lines(wad: &Wad, fault: &Fault) -> Vec<Line<'static>> {
    let (color, advice) = if fault.is_blocking() {
        (theme::BAD, "press R to pick the file again")
    } else {
        (
            theme::CHANGED,
            "its hash may be stale: press R to pick it again",
        )
    };
    let path = wad.path.to_string_lossy();
    vec![
        Line::styled(
            wad.file_name().into_owned(),
            Style::new().fg(theme::TEXT).add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            layout::tail(&path, popups::MAX_TEXT_WIDTH as usize),
            Style::new().fg(theme::MUTED),
        ),
        Line::styled(format!("It {fault}."), Style::new().fg(color)),
        Line::styled(advice, Style::new().fg(theme::MUTED)),
    ]
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, fs};

    use super::*;
    use crate::vault::scribe::tests::Altar;

    fn wad_with(altar: &Altar, name: &str, contents: &[u8]) -> Wad {
        fs::write(altar.file(name), contents).unwrap();
        Wad::from_path(&altar.file(name)).unwrap()
    }

    /// An editor holding one wad, then that wad's file deleted so the entry is a placeholder.
    fn editor_with_missing_wad<'a>(altar: &Altar, profiles: &'a [Profile]) -> Editor<'a> {
        let mut editor = Editor::new(profiles, Path::new(""));
        editor.profile.wads = vec![wad_with(altar, "old.pk3", b"old")];
        fs::remove_file(altar.file("old.pk3")).unwrap();
        editor.faults = check_wads(&editor.profile.wads);
        editor
    }

    #[test]
    fn a_missing_wad_is_a_placeholder_until_it_is_replaced() {
        let altar = Altar::new("editor-placeholder");
        let mut editor = editor_with_missing_wad(&altar, &[]);
        assert_eq!(editor.faults, [Some(Fault::Missing)]);

        let replacement = wad_with(&altar, "new.pk3", b"new");
        let asked = Cell::new(false);
        let replaced = editor
            .replace_wad(0, replacement, || {
                asked.set(true);
                Ok(true)
            })
            .unwrap();
        // the hash differs, so the user was asked first
        assert!(replaced && asked.get());
        assert_eq!(editor.faults, [None]);
        assert_eq!(editor.profile.wads[0].file_name(), "new.pk3");
    }

    #[test]
    fn declining_the_different_file_keeps_the_placeholder() {
        let altar = Altar::new("editor-decline");
        let mut editor = editor_with_missing_wad(&altar, &[]);
        let replacement = wad_with(&altar, "new.pk3", b"new");
        assert!(!editor.replace_wad(0, replacement, || Ok(false)).unwrap());
        assert_eq!(editor.profile.wads[0].file_name(), "old.pk3");
        assert_eq!(editor.faults, [Some(Fault::Missing)]);
    }

    #[test]
    fn the_same_file_found_elsewhere_replaces_without_asking() {
        let altar = Altar::new("editor-same-file");
        let mut editor = editor_with_missing_wad(&altar, &[]);
        let moved = wad_with(&altar, "moved.pk3", b"old");
        let replaced = editor
            .replace_wad(0, moved, || panic!("same hash, nothing to ask"))
            .unwrap();
        assert!(replaced);
        assert_eq!(editor.faults, [None]);
    }

    #[test]
    fn a_wad_with_no_stored_hash_is_replaced_without_asking() {
        let altar = Altar::new("editor-no-hash");
        let mut editor = Editor::new(&[], Path::new(""));
        editor.profile.wads = vec![Wad::default()];
        editor.faults = check_wads(&editor.profile.wads);
        let replacement = wad_with(&altar, "new.pk3", b"new");
        assert!(
            editor
                .replace_wad(0, replacement, || panic!("nothing to compare"))
                .unwrap()
        );
    }

    #[test]
    fn the_inspect_popup_names_the_file_the_path_and_the_problem() {
        let wad = Wad::new("/wads/moved/brutal.pk3".into(), "ab".repeat(32), 4);
        let lines: Vec<_> = inspect_lines(&wad, &Fault::Missing)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(lines[0], "brutal.pk3");
        assert_eq!(lines[1], "/wads/moved/brutal.pk3");
        assert_eq!(lines[2], "It was not found.");
        assert!(lines[3].contains("press R"));
    }

    #[test]
    fn replacing_a_volatile_wad_never_asks_and_keeps_it_volatile() {
        let altar = Altar::new("editor-volatile-replace");
        let mut editor = editor_with_missing_wad(&altar, &[]);
        editor.profile.wads[0].volatile = true;
        let replacement = wad_with(&altar, "new.pk3", b"new");
        let replaced = editor
            .replace_wad(0, replacement, || panic!("volatile wads change on purpose"))
            .unwrap();
        assert!(replaced);
        assert!(editor.profile.wads[0].volatile);
    }

    #[test]
    fn toggling_volatility_hides_and_restores_the_changed_mark() {
        let altar = Altar::new("editor-toggle");
        let mut editor = Editor::new(&[], Path::new(""));
        editor.profile.wads = vec![wad_with(&altar, "a.pk3", b"1234")];
        fs::write(altar.file("a.pk3"), b"1234-grown").unwrap();
        editor.faults = check_wads(&editor.profile.wads);
        assert!(matches!(editor.faults[0], Some(Fault::Changed { .. })));

        editor.toggle_volatility(0);
        assert!(editor.profile.wads[0].volatile);
        assert_eq!(editor.faults, [None]);

        editor.toggle_volatility(0);
        assert!(!editor.profile.wads[0].volatile);
        assert!(matches!(editor.faults[0], Some(Fault::Changed { .. })));
    }

    #[test]
    fn a_new_ini_is_made_empty_in_the_profiles_data_folder() {
        let altar = Altar::new("editor-new-ini");
        let path = new_ini(&altar.0, "brutal").unwrap();
        assert_eq!(path, altar.file("brutal/uzdoom.ini"));
        assert_eq!(fs::read(&path).unwrap(), b"");
    }

    #[test]
    fn an_ini_that_is_already_there_is_not_touched() {
        let altar = Altar::new("editor-existing-ini");
        let path = new_ini(&altar.0, "brutal").unwrap();
        fs::write(&path, b"[Doom.Player]\nname=me\n").unwrap();
        assert_eq!(new_ini(&altar.0, "brutal").unwrap(), path);
        assert_eq!(fs::read(&path).unwrap(), b"[Doom.Player]\nname=me\n");
    }

    #[test]
    fn the_divider_spans_the_width_with_the_count_at_the_end() {
        let line = divider("wads", 12, 20);
        assert_eq!(line.width(), 20);
        assert_eq!(line.to_string(), format!("wads {} 12", "─".repeat(12)));
    }

    #[test]
    fn snapshot_editing_a_profile_with_every_kind_of_mark() {
        let altar = Altar::new("snap-editor");
        let root = crate::testing::example_vault(&altar);
        let vault = Vault::new(root.join("config"), root.join("data"));
        let mut editor = Editor::edit(&vault.profiles, &vault.data_dir, 1);
        editor.table_state.select(Some(1));
        insta::assert_snapshot!(crate::testing::draw(90, 28, |page| editor.render(page)));
    }

    #[test]
    fn snapshot_a_blank_profile() {
        let mut editor = Editor::new(&[], Path::new(""));
        insta::assert_snapshot!(crate::testing::draw(60, 20, |page| editor.render(page)));
    }

    #[test]
    fn snapshot_inspecting_a_missing_wad() {
        let wad = Wad::new("/wads/moved/gone.pk3".into(), "ab".repeat(32), 4);
        let mut warning = Warning::styled(inspect_lines(&wad, &Fault::Missing));
        insta::assert_snapshot!(crate::testing::draw(70, 14, |page| warning.render(page)));
    }
}
