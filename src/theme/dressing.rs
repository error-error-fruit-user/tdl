//! What the TOML files that dress tdl up have in common: `theme.toml`, `splash.toml` and
//! `pride.toml`. Each says which edition of its format it is written in (`edition = 2` at the top),
//! so a later tdl can still read an older file, and an older tdl knows to leave a newer one alone
//! instead of misreading it. Each is worn for the whole run, swapped only by hand (see
//! [`Wardrobe`]).
//!
//! One can be given by path, or by name (see [`Dressing::load`]): `voidless` is the user's own
//! `voidless_theme.toml` in the standard config dir if there is one, else the one that comes with tdl.
//!
//! # Making a breaking change
//! Adding a field with a default is not breaking. Renaming, retyping or removing one is: write a
//! function `vN_to_vM(&mut toml::Table)` that fixes up the old file's table, and **append** it to
//! the type's [`Dressing::UPGRADES`]. The edition is one more than the length of that list. Never
//! reorder, edit or remove an entry, and add a test that reads a file of the previous edition.

use std::{
    ffi::OsStr,
    fs, io,
    path::Path,
    sync::{LazyLock, RwLock},
};

use ratatui::style::Color;
use serde::{Serialize, de::DeserializeOwned};

/// The key every one of these files says its edition with.
const EDITION: &str = "edition";

pub trait Dressing: Serialize + DeserializeOwned + Default + PartialEq + Sized + 'static {
    /// What it is, for messages: "theme", "splash" or "pride flags".
    const WHAT: &str;
    /// The file a new one is written to, in the standard config dir.
    const FILE: &str;
    /// What the file of one given by name ends in, in the standard config dir: `voidless` is
    /// `voidless` followed by this.
    const SUFFIX: &str;
    /// The ones that come with tdl but the built-in one, by name, with their text (see
    /// [`crate::theme::bundled`]). With any, the built-in one is `hellfire`.
    const BUNDLED: &[(&str, &str)] = &[];
    /// One function per breaking change. Entry `n` turns edition `n + 1` into edition `n + 2`. A
    /// file with no edition is edition 1.
    const UPGRADES: &[fn(&mut toml::Table)];

    /// The edition this build reads and writes.
    fn current() -> i64 {
        1 + Self::UPGRADES.len() as i64
    }

    /// What is wrong with it beyond what TOML and the fields' types can say, for the user.
    fn check(&self) -> Result<(), String> {
        Ok(())
    }

    /// The file a new one starts as: its built-in values, explained.
    fn template() -> String;

    /// The names of the ones that come with tdl: `hellfire` (the built-in one) and the rest.
    fn bundled_names() -> Vec<&'static str> {
        if Self::BUNDLED.is_empty() {
            return Vec::new();
        }
        std::iter::once("hellfire")
            .chain(Self::BUNDLED.iter().map(|(name, _)| *name))
            .collect()
    }

    /// The text of the one called `name` that comes with tdl, if there is one.
    fn bundled(name: &str) -> Option<String> {
        if name == "hellfire" && !Self::BUNDLED.is_empty() {
            return Some(Self::template());
        }
        Self::BUNDLED
            .iter()
            .find(|(bundled, _)| *bundled == name)
            .map(|(_, text)| text.to_string())
    }

    /// It as written in `text`. `Err` says why it can't be worn, for the user.
    fn parse(text: &str) -> Result<Self, String> {
        let mut table: toml::Table = text
            .parse()
            .map_err(|error| format!("isn't one tdl can wear: {error}"))?;
        let edition = match table.remove(EDITION) {
            None => 1,
            Some(toml::Value::Integer(edition)) if edition >= 1 => edition,
            Some(_) => return Err("has an edition that isn't a whole number from 1 up".into()),
        };
        if edition > Self::current() {
            return Err(format!(
                "is edition {edition}, from a newer tdl (this one wears up to {}): update tdl",
                Self::current()
            ));
        }
        for upgrade in &Self::UPGRADES[(edition - 1) as usize..] {
            upgrade(&mut table);
        }
        let dressing: Self = toml::Value::Table(table)
            .try_into()
            .map_err(|error| format!("isn't one tdl can wear: {error}"))?;
        dressing
            .check()
            .map_err(|problem| format!("isn't one tdl can wear: {problem}"))?;
        Ok(dressing)
    }

    /// It as written in the file at `path`. `Err` says why it can't be worn, naming the file.
    fn read(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|error| {
            format!(
                "Couldn't read the {} {}: {error}",
                Self::WHAT,
                path.display()
            )
        })?;
        Self::parse(&text)
            .map_err(|problem| format!("The {} {} {problem}", Self::WHAT, path.display()))
    }

    /// The one `given` names: a path to its file, or a name (see [`is_name`]), which is the file
    /// `<name><SUFFIX>` in `config_dir` (the standard config dir) if there is one, else the one
    /// of that name that comes with tdl. `Err` says why it can't be worn, for the user.
    fn load(given: &OsStr, config_dir: &Path) -> Result<Self, String> {
        let Some(name) = given.to_str().filter(|given| is_name(given)) else {
            return Self::read(Path::new(given));
        };
        let file = config_dir.join(format!("{name}{}", Self::SUFFIX));
        if file.is_file() {
            return Self::read(&file);
        }
        match Self::bundled(name) {
            Some(text) => Self::parse(&text)
                .map_err(|problem| format!("The {} '{name}' {problem}", Self::WHAT)),
            None => Err(format!(
                "There is no {} called '{name}': {} isn't there, and the ones that come with tdl \
                 are {}",
                Self::WHAT,
                file.display(),
                Self::bundled_names().join(", ")
            )),
        }
    }

    /// Writes [`Dressing::template`] to `path`, for the user to change, unless a file is there
    /// already, which is kept as it is.
    fn write_new(path: &Path) -> io::Result<()> {
        if path.exists() {
            return Ok(());
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, Self::template())
    }
}

/// Whether `given` is a name rather than a path: letters, digits, `-` and `_`, up to 64 of them. A
/// file in the working dir whose name looks like one is given as `./voidless`.
pub fn is_name(given: &str) -> bool {
    !given.is_empty()
        && given.len() <= 64
        && given
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// The `edition = N` line a written file starts with, after its comments.
pub fn edition_line<T: Dressing>() -> String {
    format!("{EDITION} = {}\n", T::current())
}

/// Where one kind of [`Dressing`] is kept while it is worn: the built-in one, until another is
/// put on. Each one worn is leaked, so every screen can hold on to it for as long as it draws;
/// they are only ever changed by hand, so few ever are.
pub struct Wardrobe<T: 'static> {
    built_in: LazyLock<T>,
    worn: RwLock<Option<&'static T>>,
}

impl<T: Dressing> Wardrobe<T> {
    pub const fn new() -> Self {
        Self {
            built_in: LazyLock::new(T::default),
            worn: RwLock::new(None),
        }
    }

    /// The one being worn.
    pub fn worn(&'static self) -> &'static T {
        self.worn
            .read()
            .ok()
            .and_then(|worn| *worn)
            .unwrap_or(&self.built_in)
    }

    /// Wears `dressing` for the rest of the run, or until another is worn.
    pub fn wear(&self, dressing: T) {
        let dressing = if dressing == *self.built_in {
            None
        } else {
            Some(&*Box::leak(Box::new(dressing)))
        };
        if let Ok(mut worn) = self.worn.write() {
            *worn = dressing;
        }
    }
}

/// `color` as red, green and blue, for blending: a terminal color's name or palette number as the
/// usual xterm values. `None` for `Color::Reset`, which only the terminal knows.
pub fn rgb(color: Color) -> Option<(u8, u8, u8)> {
    const NAMED: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    let index = match color {
        Color::Reset => return None,
        Color::Rgb(r, g, b) => return Some((r, g, b)),
        Color::Indexed(index) => index,
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
    };
    Some(match index {
        0..16 => NAMED[index as usize],
        16..232 => {
            let level = |step: u8| if step == 0 { 0 } else { 55 + step * 40 };
            let cube = index - 16;
            (level(cube / 36), level(cube / 6 % 6), level(cube % 6))
        }
        _ => {
            let gray = 8 + (index - 232) * 10;
            (gray, gray, gray)
        }
    })
}

/// `share` (0 to 1) of the way from `from` to `to`.
pub fn mix(from: (u8, u8, u8), to: (u8, u8, u8), share: f32) -> (u8, u8, u8) {
    let share = share.clamp(0.0, 1.0);
    let channel = |from: u8, to: u8| {
        (f32::from(from) + (f32::from(to) - f32::from(from)) * share).round() as u8
    };
    (
        channel(from.0, to.0),
        channel(from.1, to.1),
        channel(from.2, to.2),
    )
}

/// `share` of the way from one color to the other. A color that can't be blended (the terminal's
/// own, `Color::Reset`) is snapped to halfway through instead.
pub fn blend(from: Color, to: Color, share: f32) -> Color {
    match (rgb(from), rgb(to)) {
        (Some(from), Some(to)) => {
            let (r, g, b) = mix(from, to, share);
            Color::Rgb(r, g, b)
        }
        _ if share < 0.5 => from,
        _ => to,
    }
}

/// Whether `text` has nothing in it that would garble the screen.
pub fn is_printable(text: &str) -> bool {
    !text.chars().any(char::is_control)
}
