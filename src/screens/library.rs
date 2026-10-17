use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use ratatui::{
    crossterm::event::KeyCode,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Row, Table, TableState},
};

use crate::{
    clipboard::Clipboard,
    error::Result,
    layout, picker,
    popups::{
        dialog::Dialog, hashing, legend::Legend, status, three_way_dialog::ThreeWayDialog,
        warning::Warning,
    },
    ritual::{self, Grimoire, Page, Phase, Ritual},
    screens::{
        search::{self, Found, Search, Typed},
        selection::{self, Selection},
        wrap_around,
    },
    theme,
    vault::shelf::{self, Clash, Destination, Keep, Shelf},
    widgets::notes,
};

/// The wads screen, opened with `L` from the launcher or the editor: every wad in every dir wads
/// are looked for in (the wad dirs, then the downloads), each dir with its wads under it, or with
/// `Tab` every wad in one list, those with the same name told apart by their dirs. Wads are copied
/// or cut here and pasted into a dir, which copies or moves the files (and any profile using a
/// moved one follows it), or pasted into a profile. Dirs are added and taken out here too.
pub struct Library<'a> {
    shelf: &'a mut Shelf,
    clipboard: &'a mut Clipboard,
    /// Every dir wads are looked in, as [`Shelf::sources`] gives them, and the wads found in each.
    sources: Vec<PathBuf>,
    stock: Vec<Vec<Shelved>>,
    /// The dirs whose wads are hidden in the dirs view.
    closed: HashSet<PathBuf>,
    /// Every wad in one list, instead of each dir with its own.
    every: bool,
    /// What each row shows, and what is written on it.
    rows: Vec<Item>,
    labels: Vec<(String, Option<String>)>,
    table_state: TableState,
    /// The search being typed, if one is.
    search: Option<Search>,
    /// The last search's matches, for `;` and `,` to step through.
    found: Found,
    /// The wads picked to be copied or cut together.
    selection: Selection,
}

/// One wad found in a dir.
#[derive(Clone, Debug, PartialEq)]
struct Shelved {
    path: PathBuf,
    size: u64,
}

/// What a row of the list is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    /// The dir at this position in the sources, with its wads under it unless it is closed.
    Source(usize),
    /// The wad at this position in that source's stock.
    Wad(usize, usize),
    /// The last row of the dirs view, which adds a dir.
    Add,
}

impl<'a> Library<'a> {
    const TITLE: &str = "wads";
    const EVERY_TITLE: &str = "every wad";
    const BORDER_COLOR: Color = theme::SCREEN;
    const NEW_ROW: &str = "+ add a wad dir";
    /// The size column: `999.9 MB`.
    const SIZE_WIDTH: u16 = 8;
    const COLUMN_SPACING: u16 = 1;
    const LEGEND: &[[&str; 2]] = &[
        ["Esc", "Back (unselects first)"],
        ["Tab", "Every Wad/Each Dir"],
        ["J↓/K↑/Home/End", "Traverse"],
        Search::LEGEND,
        ["Enter", "Open/Close Dir"],
        Selection::LEGEND,
        ["Y/X", "Copy/Cut (Selected, or Whole Dir)"],
        ["P", "Paste Into Dir (Copies or Moves Files)"],
        ["A", "Add Dir"],
        ["D", "Erase Wads or Dismiss Their Dir"],
    ];

    /// Opens the screen on `shelf`, which it changes as dirs are added and taken out and files
    /// moved, and `clipboard`, the vault's, which it copies to and pastes from.
    pub fn summon(
        grimoire: &mut Grimoire,
        shelf: &'a mut Shelf,
        clipboard: &'a mut Clipboard,
    ) -> Result<()> {
        Self::new(shelf, clipboard).invoke(grimoire)
    }

    fn new(shelf: &'a mut Shelf, clipboard: &'a mut Clipboard) -> Self {
        let mut library = Self {
            shelf,
            clipboard,
            sources: Vec::new(),
            stock: Vec::new(),
            closed: HashSet::new(),
            every: false,
            rows: Vec::new(),
            labels: Vec::new(),
            table_state: TableState::new().with_selected(0),
            search: None,
            found: Found::default(),
            selection: Selection::default(),
        };
        library.restack();
        library
    }

    /// Looks in every dir again, after anything that could have changed what is in them.
    fn restack(&mut self) {
        // 📌 what the cursor is on is read from the rows as they were, before they are redone
        let before = self.current().and_then(|item| self.identity(item));
        self.sources = self.shelf.sources();
        self.stock = self
            .sources
            .iter()
            .map(|dir| {
                shelf::scan(dir)
                    .into_iter()
                    .map(|path| Shelved {
                        size: path.metadata().map_or(0, |metadata| metadata.len()),
                        path,
                    })
                    .collect()
            })
            .collect();
        self.arrange_keeping(before);
    }

    /// The row the cursor is on.
    fn cursor(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    /// What the cursor is on, if anything.
    fn current(&self) -> Option<Item> {
        self.rows.get(self.cursor()).copied()
    }

    fn wad(&self, source: usize, index: usize) -> &Shelved {
        &self.stock[source][index]
    }

    /// The rows for what is found, in the view that is on, and what they say. The cursor stays on
    /// the dir or wad it was on, if that is still shown.
    fn arrange(&mut self) {
        let before = self.current().and_then(|item| self.identity(item));
        self.arrange_keeping(before);
    }

    /// [`Library::arrange`], with the cursor going back to `before`, the dir or wad it was on.
    fn arrange_keeping(&mut self, before: Option<PathBuf>) {
        self.rows.clear();
        let downloads = self.shelf.downloads.clone();
        if self.every {
            let mut all: Vec<(usize, usize)> = Vec::new();
            let mut seen = HashSet::new();
            for (source, wads) in self.stock.iter().enumerate() {
                for (index, wad) in wads.iter().enumerate() {
                    // 📂 a dir inside another is in both: its wads are only listed once
                    if seen.insert(wad.path.clone()) {
                        all.push((source, index));
                    }
                }
            }
            all.sort_by_cached_key(|&(source, index)| {
                let path = &self.stock[source][index].path;
                (file_name(path).to_lowercase(), path.clone())
            });
            let paths: Vec<PathBuf> = all
                .iter()
                .map(|&(source, index)| self.wad(source, index).path.clone())
                .collect();
            let hints = shelf::hints(&paths, &downloads);
            self.labels = paths
                .iter()
                .map(|path| file_name(path))
                .zip(hints)
                .collect();
            self.rows = all
                .into_iter()
                .map(|(source, index)| Item::Wad(source, index))
                .collect();
        } else {
            self.labels.clear();
            for (source, dir) in self.sources.iter().enumerate() {
                self.rows.push(Item::Source(source));
                self.labels.push((dir.display().to_string(), None));
                if self.closed.contains(dir) {
                    continue;
                }
                let paths: Vec<PathBuf> = self.stock[source]
                    .iter()
                    .map(|wad| wad.path.clone())
                    .collect();
                // 🏷️ in the downloads, two of a name are told apart by their hashes
                let hints = if *dir == downloads {
                    shelf::hints(&paths, &downloads)
                } else {
                    vec![None; paths.len()]
                };
                for (index, (path, hint)) in paths.iter().zip(hints).enumerate() {
                    let shown = match path.strip_prefix(dir) {
                        Ok(inside) if *dir != downloads => inside.display().to_string(),
                        _ => file_name(path),
                    };
                    self.rows.push(Item::Wad(source, index));
                    self.labels.push((shown, hint));
                }
            }
            self.rows.push(Item::Add);
            self.labels.push((Self::NEW_ROW.to_string(), None));
        }
        let at = before
            .and_then(|before| {
                self.rows
                    .iter()
                    .position(|&item| self.identity(item).as_ref() == Some(&before))
            })
            .unwrap_or_else(|| self.cursor().min(self.rows.len().saturating_sub(1)));
        self.table_state.select(Some(at));
        self.selection.clear();
        self.found = Found::default();
    }

    /// What a row stands for, to find it again once the rows are arranged anew.
    fn identity(&self, item: Item) -> Option<PathBuf> {
        match item {
            Item::Source(source) => self.sources.get(source).cloned(),
            Item::Wad(source, index) => self
                .stock
                .get(source)?
                .get(index)
                .map(|wad| wad.path.clone()),
            Item::Add => None,
        }
    }

    /// The wads at the picked rows, or the one under the cursor; on a dir with nothing picked,
    /// every wad in it.
    fn picked(&self) -> Vec<PathBuf> {
        let count = self.rows.len();
        if !self.selection.is_active()
            && let Some(Item::Source(source)) = self.current()
        {
            return self.stock[source]
                .iter()
                .map(|wad| wad.path.clone())
                .collect();
        }
        self.selection
            .rows(self.cursor(), count)
            .into_iter()
            .filter_map(|row| match self.rows[row] {
                Item::Wad(source, index) => Some(self.wad(source, index).path.clone()),
                _ => None,
            })
            .collect()
    }

    /// Puts the picked wads on the clipboard, to be copied or (if `cut`) moved where they are
    /// pasted. Nothing moves until then. Returns whether there was anything to take.
    fn take(&mut self, cut: bool) -> bool {
        let paths = self.picked();
        self.selection.clear();
        if paths.is_empty() {
            return false;
        }
        *self.clipboard = Clipboard::Files { paths, cut };
        true
    }

    /// Where a paste at the cursor goes: the dir the wad under it is in, or the dir itself on a
    /// dir's row. `None` in the every-wad view, and on the add row.
    fn paste_target(&self) -> Option<(usize, PathBuf)> {
        if self.every {
            return None;
        }
        match self.current()? {
            Item::Source(source) => Some((source, self.sources[source].clone())),
            Item::Wad(source, index) => {
                let dir = self.wad(source, index).path.parent()?.to_path_buf();
                Some((source, dir))
            }
            Item::Add => None,
        }
    }

    /// Pastes the clipboard's wads into the dir at the cursor: files copied or cut here are copied
    /// or moved, and wads and profiles copied elsewhere have their files copied. Into the downloads,
    /// each is hashed first and goes under its hash. A cut pastes once.
    fn paste(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let Some((source, dir)) = self.paste_target() else {
            return Warning::summon(
                grimoire,
                "Press Tab for the dirs, and paste into one of them",
            );
        };
        let (paths, cut) = match &*self.clipboard {
            Clipboard::Files { paths, cut } => (paths.clone(), *cut),
            clipboard => (
                clipboard
                    .wads()
                    .into_iter()
                    .map(|wad| PathBuf::from(wad.path))
                    .collect(),
                false,
            ),
        };
        if paths.is_empty() {
            return Ok(());
        }
        let downloads = self.shelf.downloads.clone();
        let hashes: Vec<String>;
        let into = if self.sources[source] == downloads {
            let Some(wads) = hashing::wads(grimoire, &paths)? else {
                return Ok(());
            };
            hashes = wads.into_iter().map(|wad| wad.hash).collect();
            Destination::Downloads(&hashes)
        } else {
            Destination::Dir(&dir)
        };
        let doing = if cut { "Moving" } else { "Copying" };
        status::summon(
            grimoire,
            "pasting",
            &format!("{doing} into {}...", dir.display()),
            theme::BUSY,
        )?;
        let pasted = shelf::paste(&paths, cut, into, &downloads, |name, dir| {
            let question = format!(
                "'{name}' is already in {}. Replace it, keep both, or skip it?",
                dir.display()
            );
            let answer =
                ThreeWayDialog::summon(grimoire, &question, ["Replace", "Keep Both", "Skip"])?;
            Ok(match answer {
                Some(0) => Clash::Replace,
                Some(1) => Clash::KeepBoth,
                _ => Clash::Skip,
            })
        })?;
        if cut {
            *self.clipboard = Clipboard::Empty;
        }
        self.shelf.moves.extend(pasted.moves);
        self.restack();
        if let Some(first) = pasted.placed.first()
            && let Some(row) = self
                .rows
                .iter()
                .position(|&item| self.identity(item).as_ref() == Some(first))
        {
            self.table_state.select(Some(row));
        }
        for problem in &pasted.problems {
            Warning::summon(grimoire, problem)?;
        }
        Ok(())
    }

    /// Picks a dir to look for wads in, and asks where to keep it, as with `--wads`.
    fn add_source(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let Some(dir) = picker::dir(grimoire, None)? else {
            return Ok(());
        };
        let dir = std::path::absolute(&dir).unwrap_or(dir);
        if self.shelf.has(&dir) {
            return Warning::summon(
                grimoire,
                &format!("Wads are already looked for in {}", dir.display()),
            );
        }
        let question = format!(
            "Look for wads in {} for this session, globally, or for this vault?",
            dir.display()
        );
        let keep =
            match ThreeWayDialog::summon(grimoire, &question, ["Session", "Global", "Vault"])? {
                Some(0) => Keep::Session,
                Some(1) if self.shelf.global_writable => Keep::Global,
                Some(1) => {
                    Warning::summon(
                        grimoire,
                        "The global settings are from a newer tdl, so it is for this session only",
                    )?;
                    Keep::Session
                }
                Some(2) => Keep::Vault,
                _ => return Ok(()),
            };
        self.shelf.add(dir.clone(), keep);
        self.restack();
        if let Some(row) = self
            .rows
            .iter()
            .position(|&item| self.identity(item).as_ref() == Some(&dir))
        {
            self.table_state.select(Some(row));
        }
        Ok(())
    }

    /// `D`: on a dir's row, stops looking in it. On a wad (or with wads picked), asks whether to
    /// delete the files, or to dismiss the dir the wad is in, which stops looking in it.
    fn delete_or_dismiss(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let source = match self.current() {
            Some(Item::Source(source)) if !self.selection.is_active() => {
                return self.dismiss(grimoire, source);
            }
            Some(Item::Source(source) | Item::Wad(source, _)) => source,
            _ => return Ok(()),
        };
        let paths = self.picked();
        let these = match paths.as_slice() {
            [] => return Ok(()),
            [path] => format!("'{}'", file_name(path)),
            many => format!("the {} selected wads", many.len()),
        };
        let in_downloads = self.sources[source] == self.shelf.downloads;
        // 📦 picked wads may be in several dirs, and the downloads are never dismissed
        let delete = if self.selection.is_active() || in_downloads {
            Dialog::summon(grimoire, &format!("Erase {these} from the disk?"))?
        } else {
            let question = format!(
                "Erase {these} from the disk, or dismiss {}, to stop looking for wads in it?",
                self.sources[source].display()
            );
            match ThreeWayDialog::summon(grimoire, &question, ["Erase", "Dismiss Dir", "Back"])? {
                Some(0) => true,
                Some(1) => {
                    self.selection.clear();
                    return self.remove_source(source);
                }
                _ => false,
            }
        };
        if !delete {
            return Ok(());
        }
        let downloads = self.shelf.downloads.clone();
        let mut problems = Vec::new();
        for path in &paths {
            if let Err(error) = shelf::delete(path, &downloads) {
                problems.push(format!("Could not delete {}: {error}", file_name(path)));
            }
        }
        // ✂️ nothing deleted can be pasted any more
        if let Clipboard::Files { paths: taken, .. } = &mut *self.clipboard {
            taken.retain(|path| path.is_file());
        }
        self.restack();
        for problem in &problems {
            Warning::summon(grimoire, problem)?;
        }
        Ok(())
    }

    /// Asks whether to stop looking in the dir at `source`, and does if so.
    fn dismiss(&mut self, grimoire: &mut Grimoire, source: usize) -> Result<()> {
        let dir = &self.sources[source];
        if *dir == self.shelf.downloads {
            return Warning::summon(grimoire, "The downloads are always looked in");
        }
        let question = format!(
            "Stop looking for wads in {}? The files stay where they are.",
            dir.display()
        );
        if Dialog::summon(grimoire, &question)? {
            self.remove_source(source)?;
        }
        Ok(())
    }

    /// Stops looking in the dir at `source`, now and in later sessions. Its files stay.
    fn remove_source(&mut self, source: usize) -> Result<()> {
        let dir = self.sources[source].clone();
        self.shelf.remove(&dir);
        self.closed.remove(&dir);
        self.restack();
        Ok(())
    }

    /// Opens or closes the dir under the cursor.
    fn toggle(&mut self, source: usize) {
        let dir = &self.sources[source];
        if !self.closed.remove(dir) {
            self.closed.insert(dir.clone());
        }
        self.arrange();
    }

    /// The file name a search looks at, for the row `row`; dirs and the add row have none.
    fn score(&self, row: usize, query: &str) -> Option<search::Score> {
        match self.rows[row] {
            Item::Wad(source, index) => {
                search::score(&file_name(&self.wad(source, index).path), query)
            }
            _ => None,
        }
    }

    /// Takes a key while a search is being typed. Only Enter moves the cursor: to the best match.
    fn search_key(&mut self, key_code: KeyCode) -> Phase<()> {
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
                let ranked = search::rank(self.cursor(), self.rows.len(), |row| {
                    self.score(row, &query)
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
}

/// The file name of `path`, as text.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl Ritual for Library<'_> {
    type Return = ();

    fn render(&mut self, page: &mut Page) {
        let title = if self.every {
            Self::EVERY_TITLE
        } else {
            Self::TITLE
        };
        let indent = if self.every { 0 } else { 2 };
        let label_width = self
            .rows
            .iter()
            .zip(&self.labels)
            .map(|(item, (label, hint))| {
                let indent = if matches!(item, Item::Wad(..)) {
                    indent
                } else {
                    2
                };
                indent
                    + layout::width(label)
                    + hint.as_ref().map_or(0, |hint| 2 + layout::width(hint))
            })
            .chain([layout::width(title)])
            .max()
            .unwrap_or(0);
        let list_width = label_width + Self::SIZE_WIDTH + Self::COLUMN_SPACING;
        let (width, height) = layout::window(list_width, self.rows.len() as u16);
        let list = layout::grow(width, height, 0, page.area());
        let [list_area] = layout::stack(&[list], page.area())[..] else {
            return;
        };
        let veil = ritual::conjure_veil(title, Self::BORDER_COLOR);
        let inner = veil.inner(list_area);
        let visible = inner.height as usize;
        let offset = layout::scroll(self.table_state.offset(), self.cursor(), visible);
        *self.table_state.offset_mut() = offset;

        let (cursor, count) = (self.cursor(), self.rows.len());
        // ✂️ what doesn't fit loses its start, so a long path still ends in the part that matters
        let room = inner
            .width
            .saturating_sub(Self::SIZE_WIDTH + Self::COLUMN_SPACING + 2)
            as usize;
        let muted = Style::new().fg(theme::MUTED);
        let rows =
            self.rows
                .iter()
                .zip(&self.labels)
                .enumerate()
                .map(|(row, (&item, (label, hint)))| {
                    let hint = hint
                        .as_ref()
                        .map(|hint| Span::styled(format!("  {hint}"), muted));
                    let (name, size) = match item {
                        Item::Source(source) => {
                            let dir = &self.sources[source];
                            let arrow = if self.closed.contains(dir) {
                                "▸ "
                            } else {
                                "▾ "
                            };
                            let count = self.stock[source].len();
                            // 📭 the downloads aren't made until something is downloaded
                            let tally = if dir.is_dir() || *dir == self.shelf.downloads {
                                match count {
                                    1 => "1 wad".to_string(),
                                    n => format!("{n} wads"),
                                }
                            } else {
                                "missing".to_string()
                            };
                            (
                                Line::from(vec![
                                    Span::styled(arrow, Style::new().fg(theme::KEY)),
                                    Span::styled(
                                        layout::tail(label, room),
                                        Style::new().fg(theme::TITLE).add_modifier(Modifier::BOLD),
                                    ),
                                ]),
                                Line::styled(tally, muted).right_aligned(),
                            )
                        }
                        Item::Wad(source, index) => {
                            let spans = [
                                Span::raw(" ".repeat(indent as usize)),
                                Span::raw(layout::tail(label, room)),
                            ]
                            .into_iter()
                            .chain(hint)
                            .collect::<Vec<_>>();
                            let size = layout::show_size(self.wad(source, index).size);
                            (Line::from(spans), Line::styled(size, muted).right_aligned())
                        }
                        Item::Add => (
                            Line::from(vec![Span::raw("  "), Span::styled(label.as_str(), muted)]),
                            Line::default(),
                        ),
                    };
                    let shown = Row::new([name, size]);
                    if self.selection.contains(row, cursor, count) {
                        shown.style(theme::SELECTED)
                    } else {
                        shown
                    }
                });
        let table = Table::new(
            rows,
            [Constraint::Fill(1), Constraint::Length(Self::SIZE_WIDTH)],
        )
        .column_spacing(Self::COLUMN_SPACING)
        .block(veil)
        .row_highlight_style(theme::CURSOR);
        page.render_stateful_widget(table, list_area, &mut self.table_state);

        let (sigil, sigil_area) = ritual::conjure_sigil(list_area);
        let hint_row = Rect {
            x: list_area.x,
            width: list_area.width,
            ..sigil_area
        };
        match &self.search {
            Some(search) => {
                let matches = (0..self.rows.len())
                    .filter(|&row| self.score(row, &search.query).is_some())
                    .count();
                search.render(matches, "a wad's file name", hint_row, page);
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

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        if self.search.is_some() {
            return Ok(self.search_key(key_code));
        }
        let count = self.rows.len();
        match key_code {
            KeyCode::Esc if self.selection.is_active() => {
                self.selection.clear();
                Ok(Phase::Render)
            }
            KeyCode::Esc => Ok(Phase::Return(())),
            KeyCode::Tab => {
                self.every = !self.every;
                self.arrange();
                Ok(Phase::Render)
            }
            KeyCode::Enter => match self.current() {
                Some(Item::Source(source)) => {
                    self.toggle(source);
                    Ok(Phase::Render)
                }
                Some(Item::Add) => {
                    self.add_source(grimoire)?;
                    Ok(Phase::Render)
                }
                _ => Ok(Phase::Prompt),
            },
            KeyCode::Char('j') | KeyCode::Down => Ok(Phase::render_if(wrap_around::select_next(
                self.table_state.selected_mut(),
                count,
            ))),
            KeyCode::Char('k') | KeyCode::Up => Ok(Phase::render_if(wrap_around::select_previous(
                self.table_state.selected_mut(),
                count,
            ))),
            KeyCode::Home => Ok(Phase::render_if_then(self.cursor() != 0, || {
                self.table_state.select(Some(0));
            })),
            KeyCode::End => Ok(Phase::render_if_then(self.cursor() + 1 < count, || {
                self.table_state.select(Some(count.saturating_sub(1)))
            })),
            KeyCode::Char('/') => {
                self.search = Some(Search::default());
                Ok(Phase::Render)
            }
            KeyCode::Char(';') => Ok(Phase::render_if(self.search_again(false))),
            KeyCode::Char(',') => Ok(Phase::render_if(self.search_again(true))),
            KeyCode::Char('V') => {
                self.selection.toggle_range(self.cursor(), count);
                Ok(Phase::Render)
            }
            KeyCode::Char('m') => {
                self.selection.toggle_mark(self.cursor(), count);
                Ok(Phase::Render)
            }
            KeyCode::Char('y') => Ok(Phase::render_if(self.take(false))),
            KeyCode::Char('x') => Ok(Phase::render_if(self.take(true))),
            KeyCode::Char('p') => {
                if self.clipboard.is_empty() {
                    return Ok(Phase::Prompt);
                }
                self.paste(grimoire)?;
                Ok(Phase::Render)
            }
            KeyCode::Char('a') => {
                self.add_source(grimoire)?;
                Ok(Phase::Render)
            }
            KeyCode::Char('d') => {
                self.delete_or_dismiss(grimoire)?;
                Ok(Phase::Render)
            }
            KeyCode::Char('?') | KeyCode::F(1) => {
                Legend::summon(grimoire, Self::LEGEND)?;
                Ok(Phase::Render)
            }
            _ => Ok(Phase::Prompt),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::vault::scribe::tests::Altar;

    /// Two wad dirs and the downloads in `altar`, with `brutal.pk3` in all three.
    fn stocked(altar: &Altar) -> Shelf {
        let files = [
            "wads/brutal.pk3",
            "wads/maps/sigil.wad",
            "mods/brutal.pk3",
            "data/.downloads/0123456789abcdef/brutal.pk3",
        ];
        for name in files {
            let path = altar.file(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, name.as_bytes()).unwrap();
        }
        Shelf {
            dirs: vec![altar.file("wads"), altar.file("mods")],
            downloads: altar.file("data/.downloads"),
            ..Shelf::default()
        }
    }

    fn shown(library: &Library) -> Vec<String> {
        library
            .labels
            .iter()
            .map(|(label, hint)| match hint {
                Some(hint) => format!("{label} [{hint}]"),
                None => label.clone(),
            })
            .collect()
    }

    #[test]
    fn each_dir_lists_its_wads_and_every_wad_tells_the_same_names_apart() {
        let altar = Altar::new("library-views");
        let mut shelf = stocked(&altar);
        let mut clipboard = Clipboard::Empty;
        let mut library = Library::new(&mut shelf, &mut clipboard);
        let dir = |name: &str| altar.file(name).display().to_string();
        assert_eq!(
            shown(&library),
            [
                dir("wads"),
                "brutal.pk3".into(),
                "maps/sigil.wad".into(),
                dir("mods"),
                "brutal.pk3".into(),
                dir("data/.downloads"),
                "brutal.pk3".into(),
                Library::NEW_ROW.into(),
            ]
        );

        library.every = true;
        library.arrange();
        assert_eq!(
            shown(&library),
            [
                "brutal.pk3 [.downloads 01234567]",
                "brutal.pk3 [mods]",
                "brutal.pk3 [wads]",
                "sigil.wad",
            ]
        );
    }

    #[test]
    fn a_closed_dir_hides_its_wads_and_the_cursor_stays_on_it() {
        let altar = Altar::new("library-closed");
        let mut shelf = stocked(&altar);
        let mut clipboard = Clipboard::Empty;
        let mut library = Library::new(&mut shelf, &mut clipboard);
        library.table_state.select(Some(0));
        library.toggle(0);
        assert_eq!(library.rows[..2], [Item::Source(0), Item::Source(1)]);
        assert_eq!(library.cursor(), 0);
        // a wad's place is kept across the views
        library.toggle(0);
        library.table_state.select(Some(2));
        library.every = true;
        library.arrange();
        assert_eq!(library.current(), Some(Item::Wad(0, 1)));
    }

    #[test]
    fn taking_out_a_dir_before_the_cursor_keeps_it_on_the_same_wad() {
        let altar = Altar::new("library-taken-out");
        let mut shelf = stocked(&altar);
        let mut clipboard = Clipboard::Empty;
        let mut library = Library::new(&mut shelf, &mut clipboard);
        // the download, in the last of three sources, which is about to be the second of two
        library.table_state.select(Some(6));
        assert_eq!(library.current(), Some(Item::Wad(2, 0)));
        library.shelf.remove(&altar.file("mods"));
        library.restack();
        assert_eq!(library.current(), Some(Item::Wad(1, 0)));
        // and with the cursor on what was taken out, it stays in range
        library.table_state.select(Some(3));
        library.shelf.remove(&altar.file("data/.downloads"));
        library.shelf.remove(&altar.file("wads"));
        library.restack();
        assert!(library.cursor() < library.rows.len());
    }

    #[test]
    fn copying_takes_the_picked_wads_or_a_whole_dir_and_cutting_says_so() {
        let altar = Altar::new("library-take");
        let mut shelf = stocked(&altar);
        let mut clipboard = Clipboard::Empty;
        let mut library = Library::new(&mut shelf, &mut clipboard);
        // on a dir's row, every wad in it
        library.table_state.select(Some(0));
        assert!(library.take(false));
        assert_eq!(
            *library.clipboard,
            Clipboard::Files {
                paths: vec![
                    altar.file("wads/brutal.pk3"),
                    altar.file("wads/maps/sigil.wad")
                ],
                cut: false,
            }
        );
        // picked rows, dirs among them left out
        library.selection.toggle_mark(0, library.rows.len());
        library.selection.toggle_mark(4, library.rows.len());
        assert!(library.take(true));
        assert_eq!(
            *library.clipboard,
            Clipboard::Files {
                paths: vec![altar.file("mods/brutal.pk3")],
                cut: true,
            }
        );
        // nothing on the add row
        library.table_state.select(Some(library.rows.len() - 1));
        assert!(!library.take(false));
    }

    #[test]
    fn a_paste_goes_into_the_dir_of_the_wad_under_the_cursor() {
        let altar = Altar::new("library-target");
        let mut shelf = stocked(&altar);
        let mut clipboard = Clipboard::Empty;
        let mut library = Library::new(&mut shelf, &mut clipboard);
        library.table_state.select(Some(2));
        assert_eq!(library.paste_target(), Some((0, altar.file("wads/maps"))));
        library.table_state.select(Some(3));
        assert_eq!(library.paste_target(), Some((1, altar.file("mods"))));
        library.every = true;
        library.arrange();
        assert_eq!(library.paste_target(), None, "not in the every-wad view");
    }

    #[test]
    fn snapshot_the_wads_screen() {
        let altar = Altar::new("snap-library");
        let mut shelf = stocked(&altar);
        let mut clipboard = Clipboard::Empty;
        let mut library = Library::new(&mut shelf, &mut clipboard);
        library.table_state.select(Some(1));
        // 🧹 the altar's dir changes every run, so the dirs are shown from inside it
        let root = altar.file("");
        for (item, (label, _)) in library.rows.iter().zip(&mut library.labels) {
            if let Item::Source(source) = item {
                *label = format!(
                    "<altar>/{}",
                    library.sources[*source]
                        .strip_prefix(&root)
                        .unwrap()
                        .display()
                );
            }
        }
        insta::assert_snapshot!(crate::testing::draw(80, 20, |page| library.render(page)));
    }
}
