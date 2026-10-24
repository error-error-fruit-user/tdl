use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    popups::{legend::Legend, warning::Warning},
    ritual::{Grimoire, Page, Phase, Ritual},
    tag, theme,
    widgets::{
        form::{self, Field},
        text,
    },
};

/// Prompts for the file name of a pasted wad's copy, of the joiner's own wad when it is renamed
/// to make room for the host's, or of a wad renamed with `N`. Only the part before the extension
/// is typed; the extension stays
/// the original's, so UZDoom still knows what kind of file it is. A name that `is_free` turns down
/// (a file already there, or a wad the profile already has) is refused.
pub struct WadNamer<F> {
    stem: String,
    /// Where the cursor is in the stem.
    caret: usize,
    extension: String,
    title: String,
    naming: Naming,
    is_free: F,
}

/// What the name is for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Naming {
    /// A pasted wad's copy.
    Copy,
    /// The joiner's own wad, to make room for the host's.
    Yours,
    /// A wad renamed where it is.
    Wad,
}

impl<F: Fn(&str) -> bool> WadNamer<F> {
    fn border_color() -> Color {
        theme::worn().colors.screen
    }
    /// The longest a typed name gets, so it stays something a window can show.
    const MAX_LENGTH: usize = 64;
    const LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Paste the Copy"],
        ["A-Z/0-9/_-. ", "Input"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Skip It"],
    ];
    const RENAMING_LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Rename Yours"],
        ["A-Z/0-9/_-. ", "Input"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Back"],
    ];
    const RENAMING_WAD_LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Rename It"],
        ["A-Z/0-9/_-. ", "Input"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Back"],
    ];

    /// Asks for the copy's file name, starting from `suggested` (a whole file name, such as the
    /// tagged one). The full file name, extension included, or `None` if skipped.
    pub fn summon(grimoire: &mut Grimoire, suggested: &str, is_free: F) -> Result<Option<String>> {
        Self::new(suggested, is_free).invoke(grimoire)
    }

    /// Asks for a new file name for the joiner's own wad, so the host's can go where it was,
    /// starting from `suggested`. The full file name, or `None` to go back.
    pub fn summon_renaming(
        grimoire: &mut Grimoire,
        suggested: &str,
        is_free: F,
    ) -> Result<Option<String>> {
        let mut namer = Self::new(suggested, is_free);
        namer.title = format!("rename yours ({})", extension_label(&namer.extension));
        namer.naming = Naming::Yours;
        namer.invoke(grimoire)
    }

    /// Asks for a new file name for a wad, starting from `current`, its file name now. The full
    /// file name (which may still be `current`), or `None` to go back.
    pub fn summon_renaming_wad(
        grimoire: &mut Grimoire,
        current: &str,
        is_free: F,
    ) -> Result<Option<String>> {
        let mut namer = Self::new(current, is_free);
        namer.title = format!("rename the wad ({})", extension_label(&namer.extension));
        namer.naming = Naming::Wad;
        namer.invoke(grimoire)
    }

    fn new(suggested: &str, is_free: F) -> Self {
        let (stem, extension) = tag::split_extension(suggested);
        Self {
            stem: stem.to_string(),
            caret: text::end(stem),
            extension: extension.to_string(),
            title: format!("name the copy ({})", extension_label(extension)),
            naming: Naming::Copy,
            is_free,
        }
    }

    /// The file name as it stands.
    fn file_name(&self) -> String {
        format!("{}{}", self.stem, self.extension)
    }

    /// Whether `c` can go in a file name on every OS tdl runs on.
    fn allowed(c: char) -> bool {
        allowed(c)
    }

    /// What Enter does with the name as it stands: the name, or why it can't be used.
    fn check(&self) -> std::result::Result<String, &'static str> {
        let name = self.file_name();
        if self.stem.trim().is_empty() || self.stem.starts_with('.') {
            Err(match self.naming {
                Naming::Copy => "Type a name for the copy",
                Naming::Yours => "Type a new name for yours",
                Naming::Wad => "Type a new name for the wad",
            })
        } else if !(self.is_free)(&name) {
            Err("A file or wad of that name is already there")
        } else {
            Ok(name)
        }
    }
}

/// Whether `c` can go in a file name on every OS tdl runs on.
pub fn allowed(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ' ')
}

/// How the extension is named in the title: `.pk3`, or `no extension`.
fn extension_label(extension: &str) -> &str {
    if extension.is_empty() {
        "no extension"
    } else {
        extension
    }
}

impl<F: Fn(&str) -> bool> Ritual for WadNamer<F> {
    type Return = Option<String>;

    fn typing(&self) -> bool {
        true
    }

    fn legend(&self) -> &[[&'static str; 2]] {
        match self.naming {
            Naming::Copy => Self::LEGEND,
            Naming::Yours => Self::RENAMING_LEGEND,
            Naming::Wad => Self::RENAMING_WAD_LEGEND,
        }
    }

    fn render(&mut self, page: &mut Page) {
        let fields = [Field {
            label: "name",
            value: &self.stem,
            placeholder: "type a name",
            room: 20,
            caret: Some(self.caret),
        }];
        form::draw(page, &self.title, Self::border_color(), &fields, 0);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        match key_code {
            KeyCode::Enter => match self.check() {
                Ok(name) => Ok(Phase::Return(Some(name))),
                Err(problem) => {
                    Warning::summon(grimoire, problem)?;
                    Ok(Phase::Render)
                }
            },
            KeyCode::Esc => Ok(Phase::Return(None)),
            KeyCode::Char('?') | KeyCode::F(1) => {
                let legend = match self.naming {
                    Naming::Copy => Self::LEGEND,
                    Naming::Yours => Self::RENAMING_LEGEND,
                    Naming::Wad => Self::RENAMING_WAD_LEGEND,
                };
                Legend::summon(grimoire, legend)?;
                Ok(Phase::Render)
            }
            key_code => Ok(
                match text::edit(
                    &mut self.stem,
                    &mut self.caret,
                    key_code,
                    Self::MAX_LENGTH,
                    |c| Self::allowed(c).then_some(c),
                ) {
                    Some(changed) => Phase::render_if(changed),
                    None => Phase::Prompt,
                },
            ),
        }
    }
}
