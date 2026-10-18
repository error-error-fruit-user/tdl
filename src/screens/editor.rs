use std::path::{Path, PathBuf};

use ratatui::{
    crossterm::event::KeyCode,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Row, Table, TableState},
};

use crate::{
    clipboard::Clipboard,
    error::Result,
    layout, picker,
    popups::legend::Legend,
    popups::warning::Warning,
    popups::{self, dialog::Dialog, hashing, status, three_way_dialog::ThreeWayDialog},
    ritual::{self, Grimoire, Page, Phase, Ritual},
    screens::{
        config_selector::{ConfigSelector, IniChoice},
        library::Library,
        profile_namer::name_character,
        search::{self, Found, Search, Typed},
        selection::{self, Selection},
        text_prompt::TextPrompt,
        wad_namer::WadNamer,
        wrap_around,
    },
    slayer::watch,
    tag, theme,
    vault::Vault,
    vault::profile::Profile,
    vault::shelf::Shelf,
    vault::wad::{Fault, Wad},
    widgets::{
        details::Details,
        form::{self, Field},
        mark::{self, Mark},
        notes, text,
    },
};

/// Creates or edits one profile. `Enter` saves and finishes; `Esc` finishes too, first asking
/// whether to save if anything changed. A new profile left empty is discarded.
///
/// `profiles` is every existing profile, used to reject duplicate names; when editing, the
/// profile's own entry is excluded via `skipped_index`.
pub struct Editor<'a> {
    profile: Profile,
    /// The profile as the editor was opened with (an empty one for a new profile), to tell
    /// whether Esc has anything to ask about.
    original: Profile,
    profiles: &'a [Profile],
    /// Where profile data lives, for the ini file a new profile can be given.
    data_dir: &'a Path,
    skipped_index: Option<usize>,
    editing_wads: bool,
    /// Where the cursor is in the name.
    name_caret: usize,
    table_state: TableState,
    /// What is wrong with each wad in the list, checked when the list changes and not on every redraw.
    faults: Vec<Option<Fault>>,
    /// The search being typed, if one is.
    search: Option<Search>,
    /// The last search's matches, for `;` and `,` to step through.
    found: Found,
    /// The wads picked to be copied together.
    selection: Selection,
    /// The vault's clipboard, handed back when the editor closes.
    clipboard: Clipboard,
    /// What the wads screen (`L`) works on, handed back to the vault when the editor closes, so it
    /// keeps the wad dirs and follows any files moved.
    shelf: Shelf,
    /// The profile whose name this one was saved under, after the user agreed to replace it.
    replacing: Option<usize>,
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
    /// The longest a profile's name may be typed.
    const MAX_NAME: usize = 64;
    /// The longest a profile's own UZDoom arguments may be typed.
    const MAX_ARGS: usize = 120;
    const NAMING_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Save and Quit"],
        ["Esc", "Quit (Asks to Save Changes)"],
        ["Tab/↓", "Edit Wads"],
        ["A-Z/0-9/_", "Input"],
        ["←/→/Home/End", "Move the Cursor"],
        ["/", "Search Wads"],
    ];
    const EDITING_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Save and Quit"],
        ["Esc", "Unselect, or Quit (Asks to Save)"],
        ["Tab", "Edit Name (or ↑ from the top)"],
        ["J↓/K↑/Home/End", "Traverse"],
        Search::LEGEND,
        ["}/{", "Swap (Selected, If Any)"],
        ["Space", "Inspect Wad"],
        ["R", "Replace Wad"],
        ["V", "Toggle Volatility"],
        Selection::LEGEND,
        ["Y/Shift+Y", "Copy Selected/All Wads"],
        ["X/P", "Cut/Paste Wads"],
        ["A", "Add"],
        ["F", "Import from Dir"],
        ["L", "Wads (Every Dir)"],
        ["T", "Watch a Dir/Stop Watching"],
        ["D", "Delete (Selected, If Any)"],
        ["U/W/I/G", "Select UZDoom/Base Wad/Ini/Args"],
    ];

    /// Creates a profile and puts it in at `index` (see [`Vault::insert`]), or in place of the
    /// profile it replaces, where `index` then points. Nothing if cancelled or left empty.
    pub fn summon_blank(
        grimoire: &mut Grimoire,
        vault: &mut Vault,
        index: &mut usize,
    ) -> Result<()> {
        Self::summon_with(grimoire, vault, index, Vec::new())
    }

    /// Like [`Editor::summon_blank`], with a profile that starts out with `wads`, such as the ones
    /// on the clipboard.
    pub fn summon_with(
        grimoire: &mut Grimoire,
        vault: &mut Vault,
        index: &mut usize,
        wads: Vec<Wad>,
    ) -> Result<()> {
        let mut editor = Editor::new(&vault.profiles, &vault.data_dir);
        if !wads.is_empty() {
            editor.faults = check_wads(&wads);
            editor.profile.wads = wads;
            editor.editing_wads = true;
        }
        editor.clipboard = vault.clipboard.clone();
        editor.shelf = vault.shelf();
        let profile = editor.invoke(grimoire)?;
        let Editor {
            clipboard,
            shelf,
            replacing,
            ..
        } = editor;
        vault.clipboard = clipboard;
        let saved = match (profile, replacing) {
            (Some(profile), Some(replaced)) => vault
                .supplant(replaced, None, profile)
                .map(|landed| *index = landed),
            (Some(profile), None) => vault.insert(index, profile),
            (None, _) => Ok(()),
        };
        if saved.is_err() {
            Warning::save_failed(grimoire)?;
        }
        if vault.restock(shelf).is_err() {
            Warning::save_failed(grimoire)?;
        }
        Ok(())
    }

    /// Edits `vault.profiles[index]` and saves the result. Cancelling keeps the original. A profile
    /// saved under the name of another, which it replaces, can end up one place up, as `index` says.
    pub fn summon_existing(
        grimoire: &mut Grimoire,
        vault: &mut Vault,
        index: &mut usize,
    ) -> Result<()> {
        let edited = *index;
        // 👀 a watched dir's new wads are in before the edit starts
        watch::refresh_saved(grimoire, vault, edited)?;
        let mut editor = Editor::edit(&vault.profiles, &vault.data_dir, edited);
        editor.clipboard = vault.clipboard.clone();
        editor.shelf = vault.shelf();
        let profile = editor.invoke(grimoire)?;
        let Editor {
            clipboard,
            shelf,
            original,
            replacing,
            ..
        } = editor;
        vault.clipboard = clipboard;
        if let Some(mut profile) = profile {
            // 🙈 a wad taken out of a watched dir's profile stays out
            profile.note_dismissed(&original);
            let saved = match replacing {
                Some(replaced) => vault
                    .supplant(replaced, Some(edited), profile)
                    .map(|landed| *index = landed),
                None => vault.replace(edited, profile),
            };
            if saved.is_err() {
                Warning::save_failed(grimoire)?;
            }
        }
        // 🚚 after the edit is in, so a profile left unsaved still follows any file it moved
        if vault.restock(shelf).is_err() {
            Warning::save_failed(grimoire)?;
        }
        Ok(())
    }

    /// Starts on the name field.
    fn new(profiles: &'a [Profile], data_dir: &'a Path) -> Self {
        Self {
            profile: Profile::default(),
            original: Profile::default(),
            profiles,
            data_dir,
            skipped_index: None,
            editing_wads: false,
            name_caret: 0,
            table_state: TableState::new().with_selected(0),
            faults: Vec::new(),
            search: None,
            found: Found::default(),
            selection: Selection::default(),
            clipboard: Clipboard::default(),
            shelf: Shelf::default(),
            replacing: None,
        }
    }

    /// Starts on the wad list, with the existing profile loaded.
    fn edit(profiles: &'a [Profile], data_dir: &'a Path, index: usize) -> Self {
        let profile = profiles[index].clone();
        Self {
            faults: check_wads(&profile.wads),
            name_caret: text::end(&profile.name),
            original: profile.clone(),
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
        self.taken_by().is_some()
    }

    /// The other profile that already has this name, if one does. The profile being edited
    /// doesn't count.
    fn taken_by(&self) -> Option<usize> {
        self.profiles
            .iter()
            .enumerate()
            .position(|(i, p)| Some(i) != self.skipped_index && p.name == self.profile.name)
    }

    /// Gives the profile an ini of its own in its data dir, which needs a name that is free.
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

    /// Puts `wads` in at the cursor, ahead of the wad there. On the add row they go at the end, and
    /// the cursor moves down with the add row.
    fn insert_wads(&mut self, wads: Vec<Wad>) {
        let index = self.table_state.selected().unwrap_or(0);
        let length = wads.len();
        let at_end = index == self.profile.wads.len();
        self.profile.wads.splice(index..index, wads);
        self.faults = check_wads(&self.profile.wads);
        if at_end {
            self.table_state.select(Some(index + length));
        }
    }

    /// Copies the picked wads (see [`Selection`]), or with `all` every wad, onto the clipboard, and
    /// lets go of the selection. Returns whether there was anything to copy.
    fn copy_wads(&mut self, all: bool) -> bool {
        let count = self.profile.wads.len();
        let rows = if all {
            (0..count).collect()
        } else {
            self.selection.rows(self.cursor(), count)
        };
        self.selection.clear();
        if rows.is_empty() {
            return false;
        }
        self.clipboard = Clipboard::Wads(
            rows.iter()
                .map(|&row| self.profile.wads[row].clone())
                .collect(),
        );
        true
    }

    /// Watches a dir the user picks, taking in its wads now, or stops watching one, after asking.
    /// The wads stay in the profile either way.
    fn toggle_watch(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        if let Some(dir) = &self.profile.watched {
            let question = format!(
                "Stop watching {}? Its wads stay in the profile.",
                Path::new(dir).display()
            );
            if Dialog::summon(grimoire, &question)? {
                self.profile.watched = None;
                self.profile.dismissed.clear();
            }
            return Ok(());
        }
        let Some(dir) = picker::dir(grimoire, None)? else {
            return Ok(());
        };
        let dir = std::path::absolute(&dir).unwrap_or(dir);
        self.profile.watched = Some(dir.into_os_string());
        if watch::refresh(grimoire, &mut self.profile)? {
            self.faults = check_wads(&self.profile.wads);
            self.selection.clear();
            self.found = Found::default();
        }
        Ok(())
    }

    /// Opens the wads screen. The profile (and how it was opened, so moving its files isn't a
    /// change to ask about) follows any file moved there.
    fn open_library(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let before = self.shelf.moves.len();
        Library::summon(grimoire, &mut self.shelf, &mut self.clipboard)?;
        for (from, to) in &self.shelf.moves[before..] {
            self.profile.follow(from, to);
            self.original.follow(from, to);
        }
        self.faults = check_wads(&self.profile.wads);
        self.selection.clear();
        self.found = Found::default();
        Ok(())
    }

    /// Pastes the clipboard's wads in front of the cursor, asking about any whose file name the
    /// profile already has (see [`paste_asking`]).
    fn paste(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        // 🧮 files from the wads screen are hashed before they go in
        let wads = match &self.clipboard {
            Clipboard::Files { paths, .. } => match hashing::wads(grimoire, paths)? {
                Some(wads) => wads,
                None => return Ok(()),
            },
            clipboard => clipboard.wads(),
        };
        let pasting = paste_asking(grimoire, &self.profile.wads, wads)?;
        if !pasting.is_empty() {
            self.insert_wads(pasting);
        }
        Ok(())
    }

    /// Takes the wads at `rows` (in list order) out of the list, and lets go of the selection. The
    /// cursor goes to where the first of them was.
    fn remove_wads(&mut self, rows: &[usize]) {
        for &row in rows.iter().rev() {
            self.profile.wads.remove(row);
            self.faults.remove(row);
        }
        self.selection.clear();
        self.found = Found::default();
        if let Some(&first) = rows.first() {
            self.table_state
                .select(Some(first.min(self.profile.wads.len())));
        }
    }

    /// Cuts the picked wads, or the one under the cursor: copied onto the clipboard, then taken out
    /// of the list. Returns whether there was anything to cut.
    fn cut_wads(&mut self) -> bool {
        let rows = self.selection.rows(self.cursor(), self.profile.wads.len());
        if !self.copy_wads(false) {
            return false;
        }
        self.remove_wads(&rows);
        true
    }

    /// Moves every selected wad one place down, or up, keeping them selected; nothing moves if one
    /// is already at the end. The cursor goes along if it is on one of them.
    fn shift_selection(&mut self, down: bool) -> Phase<Option<Profile>> {
        let (cursor, count) = (self.cursor(), self.profile.wads.len());
        let rows = self.selection.rows(cursor, count);
        let Some(moved) = selection::shift(&mut self.profile.wads, &rows, down) else {
            return Phase::Prompt;
        };
        selection::shift(&mut self.faults, &rows, down);
        if rows.contains(&cursor) {
            self.table_state
                .select(Some(if down { cursor + 1 } else { cursor - 1 }));
        }
        self.selection.select_rows(moved);
        self.found = Found::default();
        Phase::Render
    }

    /// Whether the profile is different from what the editor was opened with.
    fn is_changed(&self) -> bool {
        self.profile != self.original
    }

    /// Closes the editor with the profile, if it has a name; otherwise says so and stays. A name
    /// another profile has asks whether to replace that one, or go back to the editor. A new
    /// profile left completely empty is simply dropped.
    fn save(&mut self, grimoire: &mut Grimoire) -> Result<Phase<Option<Profile>>> {
        if self.profile.name.is_empty() {
            if self.profile.wads.is_empty()
                && self.profile.uzdoom.is_none()
                && self.profile.base_wad.is_none()
                && self.profile.uzdoom_args.is_none()
            {
                return Ok(Phase::Return(None));
            }
            Warning::empty_profile_name(grimoire)?;
            return Ok(Phase::Render);
        }
        if let Some(taken) = self.taken_by() {
            let question = format!(
                "'{}' already exists. Replace it? (its saves are deleted)",
                self.profile.name
            );
            if !Dialog::summon(grimoire, &question)? {
                return Ok(Phase::Render);
            }
            self.replacing = Some(taken);
        }
        Ok(Phase::Return(Some(self.profile.clone())))
    }

    /// The row the cursor is on.
    fn cursor(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    /// How well the wad at `row` matches `query`, by its file name. A `*` in front, which picks
    /// wads in the launcher, means nothing more here, since the editor only has wads.
    fn wad_score(&self, row: usize, query: &str) -> Option<search::Score> {
        search::score(
            &self.profile.wads[row].file_name(),
            query.trim_start_matches('*'),
        )
    }

    /// While the name has focus it takes every key that edits text, and `↓` goes down to the wads.
    /// `None` for any other key, or while the wads have focus.
    fn name_key(&mut self, key_code: KeyCode) -> Option<Phase<Option<Profile>>> {
        if self.editing_wads {
            return None;
        }
        if key_code == KeyCode::Down {
            self.editing_wads = true;
            return Some(Phase::Render);
        }
        text::edit(
            &mut self.profile.name,
            &mut self.name_caret,
            key_code,
            Self::MAX_NAME,
            name_character,
        )
        .map(Phase::render_if)
    }

    /// Opens the search line. A search is always of the wads, so it takes the cursor there.
    fn start_search(&mut self) {
        self.editing_wads = true;
        self.search = Some(Search::default());
    }

    /// Takes a key while a search is being typed. Only Enter moves the cursor: to the best match.
    fn search_key(&mut self, key_code: KeyCode) -> Phase<Option<Profile>> {
        let Some(search) = &mut self.search else {
            return Phase::Prompt;
        };
        match search.key(key_code) {
            Typed::Typing => Phase::Render,
            Typed::Ignored => Phase::Prompt,
            Typed::Cancelled => {
                self.search = None;
                Phase::Render
            }
            Typed::Done(query) => {
                self.search = None;
                let ranked = search::rank(self.cursor(), self.profile.wads.len(), |row| {
                    self.wad_score(row, &query)
                });
                self.found = Found::new(ranked);
                if let Some(row) = self.found.current() {
                    self.table_state.select(Some(row));
                }
                Phase::Render
            }
        }
    }

    /// Moves the cursor to the next of the last search's matches, or with `back` the one before.
    fn search_again(&mut self, back: bool) -> bool {
        let row = if back {
            self.found.previous()
        } else {
            self.found.next()
        };
        row.is_some_and(|row| {
            self.table_state.select(Some(row));
            true
        })
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
            caret: Some(self.name_caret),
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
        let (cursor, count) = (self.cursor(), self.profile.wads.len());
        let rows = self
            .profile
            .wads
            .iter()
            .zip(&self.faults)
            .enumerate()
            .map(|(i, (wad, fault))| {
                // 🕳️ a wad that has gone missing leaves a hole in the list, not a gap in the story
                let style = match Mark::of_wad(wad, fault.as_ref()) {
                    // 📋 picked to be copied: the badge still says what else is going on
                    _ if self.selection.contains(i, cursor, count) => theme::SELECTED,
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
        // 📏 the search and the clipboard's note get the row as wide as the main window
        let hint_row = Rect {
            x: editor_area.x,
            width: editor_area.width,
            ..sigil_area
        };
        match &self.search {
            Some(search) => {
                let matches = (0..count)
                    .filter(|&row| self.wad_score(row, &search.query).is_some())
                    .count();
                search.render(matches, "type a wad's file name", hint_row, page);
            }
            None => {
                page.render_widget(sigil, sigil_area);
                let picked = self.selection.picked(cursor, count);
                notes::render(
                    selection::note(picked),
                    self.clipboard.note(),
                    hint_row,
                    page,
                );
            }
        }
    }

    fn prompt(&mut self, grimoire: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Self::Tea>> {
        // 🔎 a search being typed takes every key until it is kept or called off
        if self.search.is_some() {
            return Ok(self.search_key(key_code));
        }
        // 📋 Esc lets go of a selection before it leaves the editor
        if key_code == KeyCode::Esc && self.selection.is_active() {
            self.selection.clear();
            return Ok(Phase::Render);
        }
        if let Some(phase) = self.name_key(key_code) {
            return Ok(phase);
        }
        // ✂️ cut takes the selection, or the wad under the cursor, before anything lets it go
        if key_code == KeyCode::Char('x') {
            return Ok(Phase::render_if(self.cut_wads()));
        }
        // 📋 with wads selected, moving and deleting act on every one of them
        if self.selection.is_active() {
            match key_code {
                KeyCode::Char('}') => return Ok(self.shift_selection(true)),
                KeyCode::Char('{') => return Ok(self.shift_selection(false)),
                KeyCode::Char('d') => {
                    let rows = self.selection.rows(self.cursor(), self.profile.wads.len());
                    let question = format!(
                        "Are you sure you want to take the {} selected wads out?",
                        rows.len()
                    );
                    if Dialog::summon(grimoire, &question)? {
                        self.remove_wads(&rows);
                    }
                    return Ok(Phase::Render);
                }
                _ => {}
            }
        }
        // 🧹 anything that might change the list loses the selection and the search's matches,
        // since the rows they point at could move
        if !matches!(
            key_code,
            KeyCode::Char('j' | 'k' | '?' | '/' | ';' | ',' | 'V' | 'm' | 'y' | 'Y' | ' ')
                | KeyCode::F(1)
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::Tab
        ) {
            self.selection.clear();
            self.found = Found::default();
        }
        let count = self.profile.wads.len();
        let changed = self.is_changed();
        let index = self.table_state.selected_mut().as_mut().unwrap();
        match key_code {
            KeyCode::Enter => self.save(grimoire),
            // 💾 leaving with changes asks what to do with them; with none it just leaves
            KeyCode::Esc if changed => {
                match ThreeWayDialog::summon(
                    grimoire,
                    "Save the changes to this profile?",
                    ["Save", "Discard", "Back"],
                )? {
                    Some(0) => self.save(grimoire),
                    Some(1) => Ok(Phase::Return(None)),
                    _ => Ok(Phase::Render),
                }
            }
            KeyCode::Esc => Ok(Phase::Return(None)),
            KeyCode::Tab => {
                self.editing_wads = !self.editing_wads;
                Ok(Phase::Render)
            }
            // ⬆️ the arrow climbs out of the list into the name, where K wraps around instead
            KeyCode::Up if *index == 0 => {
                self.editing_wads = false;
                Ok(Phase::Render)
            }
            KeyCode::Home => Ok(Phase::render_if_then(*index != 0, || *index = 0)),
            KeyCode::End => Ok(Phase::render_if_then(*index != count, || *index = count)),
            KeyCode::Char('/') => {
                self.start_search();
                Ok(Phase::Render)
            }
            KeyCode::Char(';') if self.editing_wads => {
                Ok(Phase::render_if(self.search_again(false)))
            }
            KeyCode::Char(',') if self.editing_wads => {
                Ok(Phase::render_if(self.search_again(true)))
            }
            KeyCode::Char('V') if self.editing_wads => {
                self.selection.toggle_range(*index, count);
                Ok(Phase::Render)
            }
            KeyCode::Char('m') if self.editing_wads => {
                self.selection.toggle_mark(*index, count);
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
                    let wad = &self.profile.wads[index];
                    let found = found_elsewhere(wad, fault, &picker::wad_dirs());
                    let lines = inspect_lines(wad, fault, found.as_deref());
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
                let wad = &self.profile.wads[index];
                // 🔎 a missing wad with a file of its name in a wad dir is offered that file
                let found = self.faults[index]
                    .as_ref()
                    .and_then(|fault| found_elsewhere(wad, fault, &picker::wad_dirs()));
                let use_found = match &found {
                    Some(found) => Dialog::summon(
                        grimoire,
                        &format!("Use {}?", layout::tail(&found.to_string_lossy(), 40)),
                    )?,
                    None => false,
                };
                let path = match found {
                    Some(found) if use_found => found,
                    _ => {
                        // 📂 start where the wad used to live, since it probably only moved a little
                        let start = Path::new(&self.profile.wads[index].path).parent();
                        let Some(path) =
                            picker::file(grimoire, Some(("wads", Wad::EXTENSIONS)), start)?
                        else {
                            return Ok(Phase::Render);
                        };
                        path
                    }
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
                self.insert_wads(wads);
                Ok(Phase::Render)
            }
            KeyCode::Char('l') => {
                self.open_library(grimoire)?;
                Ok(Phase::Render)
            }
            KeyCode::Char('t') => {
                self.toggle_watch(grimoire)?;
                Ok(Phase::Render)
            }
            KeyCode::Char('f') => {
                if let Some(dir) = picker::dir(grimoire, None)? {
                    let mut wad_paths = Vec::new();
                    for entry in std::fs::read_dir(dir)? {
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
                    self.insert_wads(wads);
                    Ok(Phase::Render)
                } else {
                    Ok(Phase::Prompt)
                }
            }
            KeyCode::Char('y') => Ok(Phase::render_if(self.copy_wads(false))),
            KeyCode::Char('Y') => Ok(Phase::render_if(self.copy_wads(true))),
            KeyCode::Char('p') => {
                if self.clipboard.is_empty() {
                    return Ok(Phase::Prompt);
                }
                self.paste(grimoire)?;
                Ok(Phase::Render)
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
            KeyCode::Char('g') => {
                let current = self.profile.uzdoom_args.clone().unwrap_or_default();
                if let Some(args) = TextPrompt::summon(
                    grimoire,
                    "uzdoom args",
                    "args",
                    "the global ones",
                    &current,
                    Self::MAX_ARGS,
                )? {
                    // 🌐 nothing typed means the global ones, rather than none at all
                    let args = args.trim();
                    self.profile.uzdoom_args = (!args.is_empty()).then(|| args.to_string());
                }
                Ok(Phase::Render)
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
            KeyCode::Char('?') | KeyCode::F(1) => {
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

/// What pasting wads came to: the wads to put in, and what went wrong with any left out.
#[derive(Debug, Default)]
pub struct Pasted {
    pub wads: Vec<Wad>,
    pub problems: Vec<String>,
}

/// The `pasting` wads, ready to go into a list that already holds `existing`. A wad whose file
/// name is already there (or comes earlier in the same paste) is pasted as a copy of its file in
/// the same dir, under the name `choose` gives, or else (`None`) left out.
///
/// `choose` is told the name in use, the tagged one it could have (`brutal_2.pk3`), and which
/// names are free: none a file in the dir or a wad in the list already has.
pub fn paste_alongside(
    existing: &[Wad],
    pasting: Vec<Wad>,
    mut choose: impl FnMut(&str, &str, &dyn Fn(&str) -> bool) -> Result<Option<String>>,
) -> Result<Pasted> {
    let mut pasted = Pasted::default();
    for wad in pasting {
        let name = wad.file_name().into_owned();
        let in_use = |name: &str| {
            existing
                .iter()
                .chain(&pasted.wads)
                .any(|wad| wad.file_name() == name)
        };
        if !in_use(&name) {
            pasted.wads.push(wad);
            continue;
        }
        let path = Path::new(&wad.path);
        let Some(dir) = path.parent().filter(|_| path.is_file()) else {
            pasted
                .problems
                .push(format!("'{name}' is missing, so it can't be copied"));
            continue;
        };
        let is_free = |name: &str| !in_use(name) && !dir.join(name).exists();
        let tagged = tag::tagged_file(&name, is_free);
        let Some(new_name) = choose(&name, &tagged, &is_free)? else {
            continue;
        };
        if !is_free(&new_name) {
            pasted.problems.push(format!(
                "'{new_name}' is already taken, so '{name}' was left out"
            ));
            continue;
        }
        match wad.copied_to(&dir.join(&new_name)) {
            Ok(copy) => pasted.wads.push(copy),
            Err(error) => pasted
                .problems
                .push(format!("Could not copy '{name}' to '{new_name}': {error}")),
        }
    }
    Ok(pasted)
}

/// [`paste_alongside`], asking the user about each name in use: Tag, Name or Skip. Whatever went
/// wrong is said afterwards.
pub fn paste_asking(
    grimoire: &mut Grimoire,
    existing: &[Wad],
    pasting: Vec<Wad>,
) -> Result<Vec<Wad>> {
    let pasted = paste_alongside(existing, pasting, |name, tagged, is_free| {
        let question = format!(
            "'{name}' is already here. Paste a copy of the file as '{tagged}', or name it?"
        );
        let new_name = match ThreeWayDialog::summon(grimoire, &question, ["Tag", "Name", "Skip"])? {
            Some(0) => tagged.to_string(),
            Some(1) => match WadNamer::summon(grimoire, tagged, |name: &str| is_free(name))? {
                Some(new_name) => new_name,
                None => return Ok(None),
            },
            _ => return Ok(None),
        };
        // ⏳ a big pk3 takes a moment to copy, so say what is going on
        status::summon(
            grimoire,
            "copying",
            &format!("Copying to {new_name}..."),
            theme::BUSY,
        )?;
        Ok(Some(new_name))
    })?;
    for problem in &pasted.problems {
        Warning::summon(grimoire, problem)?;
    }
    Ok(pasted.wads)
}

/// Makes an empty `uzdoom.ini` in the profile's data dir and returns where it is. UZDoom fills
/// it in the first time it runs. An ini that is already there is left exactly as it is.
fn new_ini(data_dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let dir = data_dir.join(name);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("uzdoom.ini");
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

/// A file with a missing wad's name in one of `wad_dirs`, the first one there is. It's only a
/// direct join of the name onto each dir, never a search, and the file isn't read: whether it
/// really is the wad is checked by its hash once it's picked.
fn found_elsewhere(wad: &Wad, fault: &Fault, wad_dirs: &[PathBuf]) -> Option<PathBuf> {
    if *fault != Fault::Missing {
        return None;
    }
    let name = Path::new(&wad.path).file_name()?;
    wad_dirs
        .iter()
        .map(|dir| dir.join(name))
        .find(|path| path.is_file() && path.as_os_str() != wad.path)
}

/// What to tell the user about a wad with a fault: its name, where it should be, what is wrong,
/// and what to do about it. `found` is a file of its name in a wad dir, to offer instead.
fn inspect_lines(wad: &Wad, fault: &Fault, found: Option<&Path>) -> Vec<Line<'static>> {
    let (color, advice) = if fault.is_blocking() {
        (theme::BAD, "press R to pick the file again")
    } else {
        (
            theme::CHANGED,
            "its hash may be stale: press R to pick it again",
        )
    };
    let path = wad.path.to_string_lossy();
    let mut lines = vec![
        Line::styled(
            wad.file_name().into_owned(),
            Style::new().fg(theme::TEXT).add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            layout::tail(&path, popups::MAX_TEXT_WIDTH as usize),
            Style::new().fg(theme::MUTED),
        ),
        Line::styled(format!("It {fault}."), Style::new().fg(color)),
    ];
    match found {
        Some(found) => {
            let found = found.to_string_lossy();
            lines.push(Line::styled(
                format!(
                    "found at {}",
                    layout::tail(&found, popups::MAX_TEXT_WIDTH as usize - 9)
                ),
                Style::new().fg(theme::GOOD),
            ));
            lines.push(Line::styled(
                "press R to use it",
                Style::new().fg(theme::MUTED),
            ));
        }
        None => lines.push(Line::styled(advice, Style::new().fg(theme::MUTED))),
    }
    lines
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
        let lines: Vec<_> = inspect_lines(&wad, &Fault::Missing, None)
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
    fn a_new_ini_is_made_empty_in_the_profiles_data_dir() {
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
        let mut warning = Warning::styled(inspect_lines(&wad, &Fault::Missing, None));
        insta::assert_snapshot!(crate::testing::draw(70, 14, |page| warning.render(page)));
    }

    // 🔎 a missing wad that is sitting in a wad dir under its own name

    #[test]
    fn a_missing_wad_is_found_by_name_in_the_first_wad_dir_that_has_it() {
        let altar = Altar::new("editor-found-elsewhere");
        let (empty, first, second) = (
            altar.file("empty"),
            altar.file("first"),
            altar.file("second"),
        );
        for dir in [&empty, &first, &second] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(first.join("moved.wad"), b"moved").unwrap();
        fs::write(second.join("moved.wad"), b"moved").unwrap();
        let wad = Wad::new(altar.file("old/moved.wad").into(), "ab".repeat(32), 5);
        let dirs = [empty, first.clone(), second];
        assert_eq!(
            found_elsewhere(&wad, &Fault::Missing, &dirs),
            Some(first.join("moved.wad"))
        );
        // only a missing wad is offered another file
        assert_eq!(found_elsewhere(&wad, &Fault::NotAFile, &dirs), None);
        // and never the very path it is already missing from
        let here = Wad::new(first.join("moved.wad").into(), "ab".repeat(32), 5);
        assert_eq!(found_elsewhere(&here, &Fault::Missing, &dirs[..2]), None);
    }

    // 📋 copying and pasting wads, and searching them

    /// An editor holding wads with these names, none of them on disk.
    fn editor_with<'a>(names: &[&str]) -> Editor<'a> {
        let mut editor = Editor::new(&[], Path::new(""));
        editor.profile.wads = names
            .iter()
            .map(|name| Wad::new(format!("/wads/{name}").into(), "ab".repeat(32), 1))
            .collect();
        editor.faults = check_wads(&editor.profile.wads);
        editor.editing_wads = true;
        editor
    }

    fn names(wads: &[Wad]) -> Vec<String> {
        wads.iter()
            .map(|wad| wad.file_name().into_owned())
            .collect()
    }

    fn copied(editor: &Editor) -> Vec<String> {
        match &editor.clipboard {
            Clipboard::Wads(wads) => names(wads),
            other => panic!("expected wads on the clipboard, found {other:?}"),
        }
    }

    #[test]
    fn a_copied_wad_pastes_ahead_of_the_cursor_or_at_the_end_from_the_add_row() {
        let mut editor = editor_with(&["a.wad", "b.wad", "c.wad"]);
        editor.table_state.select(Some(2));
        assert!(editor.copy_wads(false));
        assert_eq!(copied(&editor), ["c.wad"]);

        editor.table_state.select(Some(0));
        editor.insert_wads(editor.clipboard.wads());
        assert_eq!(
            names(&editor.profile.wads),
            ["c.wad", "a.wad", "b.wad", "c.wad"]
        );
        assert_eq!(editor.table_state.selected(), Some(0));
        assert_eq!(editor.faults.len(), 4);

        // the add row: pasted at the end, and the cursor stays on the add row
        editor.table_state.select(Some(4));
        editor.insert_wads(editor.clipboard.wads());
        assert_eq!(
            names(&editor.profile.wads).last().map(String::as_str),
            Some("c.wad")
        );
        assert_eq!(editor.table_state.selected(), Some(5));
    }

    #[test]
    fn the_name_has_a_cursor_and_the_arrows_go_between_it_and_the_wads() {
        let mut editor = editor_with(&["a.wad", "b.wad"]);
        editor.editing_wads = false;
        for c in "brtal".chars() {
            assert!(editor.name_key(KeyCode::Char(c)) == Some(Phase::Render));
        }
        for _ in 0..3 {
            let _ = editor.name_key(KeyCode::Left);
        }
        let _ = editor.name_key(KeyCode::Char('u'));
        assert_eq!(editor.profile.name, "brutal");
        // keys that aren't for the name are left to the editor
        assert!(editor.name_key(KeyCode::Char('/')).is_none());
        assert!(editor.name_key(KeyCode::Down) == Some(Phase::Render));
        assert!(editor.editing_wads);
        assert!(
            editor.name_key(KeyCode::Char('x')).is_none(),
            "the wads have focus now"
        );
    }

    // 📋 pasting a wad whose name the profile already has

    /// Two wads on disk, `a.wad` and `b.wad`, as a profile's list.
    fn two_wads(altar: &Altar) -> Vec<Wad> {
        vec![
            wad_with(altar, "a.wad", b"aaaa"),
            wad_with(altar, "b.wad", b"bbbb"),
        ]
    }

    #[test]
    fn a_new_name_is_pasted_as_it_is_without_asking() {
        let altar = Altar::new("paste-new-name");
        let existing = two_wads(&altar);
        let other = wad_with(&altar, "c.wad", b"cccc");
        let pasted = paste_alongside(&existing, vec![other.clone()], |_, _, _| {
            panic!("nothing to ask about")
        })
        .unwrap();
        assert_eq!(pasted.wads, [other]);
        assert!(pasted.problems.is_empty());
    }

    #[test]
    fn a_tagged_paste_copies_the_file_under_the_next_free_number() {
        let altar = Altar::new("paste-tagged");
        let existing = two_wads(&altar);
        // a file already sits at the first tag, so the copy takes the next one
        fs::write(altar.file("a_2.wad"), b"someone else's").unwrap();
        let mut asked = Vec::new();
        let pasted = paste_alongside(&existing, vec![existing[0].clone()], |name, tagged, _| {
            asked.push((name.to_string(), tagged.to_string()));
            Ok(Some(tagged.to_string()))
        })
        .unwrap();
        assert_eq!(asked, [("a.wad".to_string(), "a_3.wad".to_string())]);
        assert_eq!(pasted.wads.len(), 1);
        assert_eq!(pasted.wads[0].file_name(), "a_3.wad");
        assert_eq!(fs::read(altar.file("a_3.wad")).unwrap(), b"aaaa");
        assert_eq!(pasted.wads[0].hash, existing[0].hash);
        // the file in the way is untouched
        assert_eq!(fs::read(altar.file("a_2.wad")).unwrap(), b"someone else's");
    }

    #[test]
    fn a_named_paste_uses_the_name_and_a_skip_leaves_it_out() {
        let altar = Altar::new("paste-named");
        let existing = two_wads(&altar);
        let mut answers = vec![None, Some("a dev.wad".to_string())].into_iter();
        let pasted = paste_alongside(
            &existing,
            vec![existing[0].clone(), existing[1].clone()],
            |_, _, _| Ok(answers.next().unwrap()),
        )
        .unwrap();
        // a skipped, b named; the name came from the second answer
        assert_eq!(pasted.wads.len(), 1);
        assert_eq!(pasted.wads[0].file_name(), "a dev.wad");
        assert!(altar.file("a dev.wad").is_file());
    }

    #[test]
    fn the_same_wad_twice_in_one_paste_is_caught_too() {
        let altar = Altar::new("paste-twice");
        let a = wad_with(&altar, "a.wad", b"aaaa");
        let pasted = paste_alongside(&[], vec![a.clone(), a.clone()], |_, tagged, _| {
            Ok(Some(tagged.into()))
        })
        .unwrap();
        let names: Vec<_> = pasted
            .wads
            .iter()
            .map(|w| w.file_name().into_owned())
            .collect();
        assert_eq!(names, ["a.wad", "a_2.wad"]);
    }

    #[test]
    fn a_missing_file_or_a_taken_name_is_a_problem_not_a_copy() {
        let altar = Altar::new("paste-problems");
        let existing = two_wads(&altar);
        let gone = Wad::new(altar.file("gone/a.wad").into(), "ab".repeat(32), 4);
        let pasted = paste_alongside(&existing, vec![gone], |_, _, _| {
            panic!("a missing file isn't asked about")
        })
        .unwrap();
        assert!(pasted.wads.is_empty());
        assert!(pasted.problems[0].contains("missing"));

        // a name the chooser gives that is in use after all is turned down, and nothing is written over
        let pasted = paste_alongside(&existing, vec![existing[0].clone()], |_, _, is_free| {
            assert!(!is_free("b.wad"));
            Ok(Some("b.wad".into()))
        })
        .unwrap();
        assert!(pasted.wads.is_empty());
        assert!(pasted.problems[0].contains("already taken"));
        assert_eq!(fs::read(altar.file("b.wad")).unwrap(), b"bbbb");
    }

    #[test]
    fn cutting_copies_the_wads_and_takes_them_out() {
        let mut editor = editor_with(&["a.wad", "b.wad", "c.wad", "d.wad"]);
        editor.selection.toggle_mark(1, 4);
        editor.selection.toggle_mark(3, 4);
        assert!(editor.cut_wads());
        assert_eq!(copied(&editor), ["b.wad", "d.wad"]);
        assert_eq!(names(&editor.profile.wads), ["a.wad", "c.wad"]);
        assert_eq!(editor.faults.len(), 2);
        assert_eq!(editor.table_state.selected(), Some(1));
        // with nothing picked, the wad under the cursor; the add row has nothing to cut
        editor.table_state.select(Some(0));
        assert!(editor.cut_wads());
        assert_eq!(copied(&editor), ["a.wad"]);
        editor.table_state.select(Some(1));
        assert!(!editor.cut_wads());
        assert_eq!(names(&editor.profile.wads), ["c.wad"]);
    }

    #[test]
    fn a_selection_moves_together_with_its_faults_and_the_cursor() {
        let mut editor = editor_with(&["a.wad", "b.wad", "c.wad", "d.wad"]);
        editor.faults[1] = None;
        editor.table_state.select(Some(1));
        editor.selection.toggle_range(1, 4);
        editor.table_state.select(Some(2));
        assert!(editor.shift_selection(true) == Phase::Render);
        assert_eq!(
            names(&editor.profile.wads),
            ["a.wad", "d.wad", "b.wad", "c.wad"]
        );
        assert_eq!(editor.faults[2], None, "b's fault went with it");
        assert_eq!(editor.table_state.selected(), Some(3));
        assert_eq!(editor.selection.rows(3, 4), [2, 3]);
        assert!(
            editor.shift_selection(true) == Phase::Prompt,
            "c is already last"
        );
        editor.remove_wads(&[2, 3]);
        assert_eq!(names(&editor.profile.wads), ["a.wad", "d.wad"]);
        assert!(!editor.selection.is_active());
    }

    #[test]
    fn only_a_profile_that_differs_from_how_it_was_opened_is_changed() {
        let altar = Altar::new("editor-changed");
        let root = crate::testing::example_vault(&altar);
        let vault = Vault::new(root.join("config"), root.join("data"));
        let mut editor = Editor::edit(&vault.profiles, &vault.data_dir, 0);
        assert!(!editor.is_changed());
        editor.toggle_volatility(0);
        assert!(editor.is_changed());
        editor.toggle_volatility(0);
        assert!(!editor.is_changed(), "toggled back, so as it was");
        // a new profile is changed as soon as it has anything in it, pasted wads included
        let mut blank = Editor::new(&[], Path::new(""));
        assert!(!blank.is_changed());
        blank.profile.name.push('x');
        assert!(blank.is_changed());
    }

    #[test]
    fn a_range_and_marks_copy_together_in_load_order() {
        let mut editor = editor_with(&["a.wad", "b.wad", "c.wad", "d.wad", "e.wad"]);
        // mark e first, then stretch a range from d back up to b
        editor.selection.toggle_mark(4, 5);
        editor.selection.toggle_range(3, 5);
        editor.table_state.select(Some(1));
        assert!(editor.copy_wads(false));
        assert_eq!(copied(&editor), ["b.wad", "c.wad", "d.wad", "e.wad"]);
        // copying lets go of the selection
        assert!(!editor.selection.is_active());
    }

    #[test]
    fn copying_all_takes_every_wad_and_the_add_row_alone_copies_nothing() {
        let mut editor = editor_with(&["a.wad", "b.wad"]);
        editor.table_state.select(Some(2));
        assert!(!editor.copy_wads(false));
        assert_eq!(editor.clipboard, Clipboard::Empty);
        assert!(editor.copy_wads(true));
        assert_eq!(
            editor.clipboard,
            Clipboard::Wads(editor.profile.wads.clone())
        );
        // a copy keeps what makes a wad itself: volatile stays volatile
        editor.profile.wads[0].volatile = true;
        editor.table_state.select(Some(0));
        editor.copy_wads(false);
        assert!(editor.clipboard.wads()[0].volatile);
    }

    #[test]
    fn copied_profiles_paste_into_the_editor_as_their_wads() {
        let mut editor = editor_with(&["a.wad"]);
        let other = editor_with(&["x.wad", "y.wad"]).profile;
        editor.clipboard = Clipboard::Profiles(vec![other]);
        editor.insert_wads(editor.clipboard.wads());
        assert_eq!(names(&editor.profile.wads), ["x.wad", "y.wad", "a.wad"]);
    }

    #[test]
    fn typing_a_search_moves_nothing_until_enter_goes_to_the_best_match() {
        let mut editor = editor_with(&[
            "superbrutal.pk3",
            "doom2.wad",
            "brutal_addon.pk3",
            "brutal.pk3",
        ]);
        editor.editing_wads = false;
        editor.table_state.select(Some(1));
        editor.start_search();
        assert!(editor.editing_wads, "searching moves over to the wads");
        for c in "brutal".chars() {
            let _ = editor.search_key(KeyCode::Char(c));
            assert_eq!(
                editor.table_state.selected(),
                Some(1),
                "nothing moves while typing"
            );
        }
        let _ = editor.search_key(KeyCode::Enter);
        assert!(editor.search.is_none());
        // the exact name wins over the earlier ones that only start with it or hold it
        assert_eq!(editor.table_state.selected(), Some(3));

        // then the next best, and around
        assert!(editor.search_again(false));
        assert_eq!(editor.table_state.selected(), Some(2));
        assert!(editor.search_again(false));
        assert_eq!(editor.table_state.selected(), Some(0));
        assert!(editor.search_again(true));
        assert_eq!(editor.table_state.selected(), Some(2));
    }

    #[test]
    fn a_search_matches_the_file_name_and_never_the_dir() {
        let mut editor = editor_with(&["a.wad"]);
        editor.profile.wads[0].path = "/brutal/a.wad".into();
        assert_eq!(editor.wad_score(0, "brutal"), None);
        assert!(editor.wad_score(0, "A.WAD").is_some());
        // a `*` for wads means nothing more in the editor
        assert!(editor.wad_score(0, "*a.wad").is_some());
    }

    #[test]
    fn calling_off_a_search_moves_nothing_and_a_search_with_no_match_neither() {
        let mut editor = editor_with(&["brutal.pk3", "doom2.wad"]);
        editor.table_state.select(Some(1));
        editor.start_search();
        let _ = editor.search_key(KeyCode::Char('b'));
        let _ = editor.search_key(KeyCode::Esc);
        assert_eq!(editor.table_state.selected(), Some(1));
        assert!(editor.search.is_none());
        assert!(!editor.search_again(false));

        editor.start_search();
        let _ = editor.search_key(KeyCode::Char('z'));
        let _ = editor.search_key(KeyCode::Enter);
        assert_eq!(editor.table_state.selected(), Some(1));
    }

    #[test]
    fn snapshot_a_search_being_typed_with_wads_copied() {
        let mut editor = editor_with(&["brutal.pk3", "doom2.wad", "brutal_addon.pk3"]);
        editor.clipboard = Clipboard::Wads(editor.profile.wads.clone());
        editor.start_search();
        for c in "brut".chars() {
            let _ = editor.search_key(KeyCode::Char(c));
        }
        insta::assert_snapshot!(crate::testing::draw(70, 22, |page| editor.render(page)));
        // once the search is over, the clipboard's note is back
        editor.search = None;
        insta::assert_snapshot!(crate::testing::draw(70, 22, |page| editor.render(page)));
    }

    #[test]
    fn snapshot_a_selection_of_two_wads() {
        let mut editor = editor_with(&["brutal.pk3", "doom2.wad", "brutal_addon.pk3"]);
        editor.selection.toggle_mark(0, 3);
        editor.selection.toggle_mark(2, 3);
        editor.table_state.select(Some(3));
        insta::assert_snapshot!(crate::testing::draw(70, 22, |page| editor.render(page)));
    }

    #[test]
    fn the_inspect_popup_says_where_the_wad_was_found() {
        let wad = Wad::new("/gone/moved.wad".into(), "ab".repeat(32), 5);
        let text: Vec<String> =
            inspect_lines(&wad, &Fault::Missing, Some(Path::new("/wads/moved.wad")))
                .iter()
                .map(|line| line.to_string())
                .collect();
        assert!(text.contains(&"found at /wads/moved.wad".to_string()));
        assert!(text.contains(&"press R to use it".to_string()));
    }
}
