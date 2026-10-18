//! Every color, border and glyph the interface uses, in one [`Theme`]. The screens only ever ask
//! the theme being worn for a role ("a screen border", "something broken"), never for a color.
//!
//! The built-in theme is below. Another can be worn from a `theme.toml` (see [`Dressing`]), kept
//! in `uac.rite` or given with `--theme`; anything it leaves out is the built-in theme's. The
//! splash screen dresses up apart, in [`splash`] (`splash.toml`) and [`pride`] (`pride.toml`).
//! Every field of all three is explained in `THEME.md` and `SPLASH.md`.

pub mod bundled;
pub mod dressing;
pub mod pride;
pub mod splash;

use std::{
    str::FromStr,
    sync::atomic::{AtomicBool, Ordering},
};

use ratatui::{
    style::{Color, Modifier, Style},
    widgets::BorderType,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use self::dressing::Dressing;
use self::dressing::Wardrobe;

// The built-in palette: hot metal, old blood and toxic waste. Every role of the built-in theme
// (and the built-in splash) is picked from these.

/// Almost black, with a little blood in it.
pub const VOID: Color = Color::Rgb(14, 8, 8);
pub const BLOOD: Color = Color::Rgb(186, 26, 18);
/// Dried blood: for fills that shouldn't shout.
pub const GORE: Color = Color::Rgb(92, 14, 10);
pub const HELLFIRE: Color = Color::Rgb(255, 112, 16);
/// The hottest part of the flame.
pub const FLAME: Color = Color::Rgb(255, 200, 70);
pub const BONE: Color = Color::Rgb(228, 216, 190);
pub const ASH: Color = Color::Rgb(128, 116, 106);
/// Cold, dark metal: for lines that only divide things.
pub const IRON: Color = Color::Rgb(70, 60, 58);
/// The green slime of Doom's toxic floors.
pub const NUKAGE: Color = Color::Rgb(124, 200, 46);
/// The blue of a soul sphere.
pub const SOUL: Color = Color::Rgb(96, 156, 255);
/// The file a new theme is written to, in the standard config dir, from the settings.
pub const FILE: &str = "theme.toml";

/// How the whole interface looks. Every part has a default (the built-in theme's), so a
/// `theme.toml` only needs what it changes.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub colors: Colors,
    pub styles: Styles,
    pub borders: Borders,
    pub glyphs: Glyphs,
    pub help: Help,
}

/// What each part of the interface is drawn with.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Colors {
    /// Painted behind everything. `"reset"` keeps the terminal's own background instead.
    pub background: Color,
    pub text: Color,
    /// Labels, hints, and anything that is unset rather than wrong.
    pub muted: Color,
    pub title: Color,
    /// Keys the user can press, wherever they are shown.
    pub key: Color,
    /// The border of the main screens: the launcher and the editor.
    pub screen: Color,
    /// The border of the multiplayer screens: hosting, joining, picking and naming profiles.
    pub multiplayer: Color,
    /// The border of the windows shown while hosting, joining and downloading.
    pub netplay: Color,
    /// The border of yes/no questions and other small popups.
    pub dialog: Color,
    pub warning: Color,
    pub legend: Color,
    /// The border of panels that only report, like the one under the launcher.
    pub panel: Color,
    /// The border of windows that show something in progress.
    pub busy: Color,
    /// The border and bar of the window that shows wads being hashed, kept apart from the download
    /// window's so the two are never mistaken for each other.
    pub hashing: Color,
    /// The border and bar of the window that shows a scroll being unsealed.
    pub unsealing: Color,
    /// A quip in a window that is busy.
    pub quip: Color,
    /// Something that works.
    pub good: Color,
    /// Something that stops a profile from running.
    pub bad: Color,
    /// Something that still runs but deserves a look.
    pub changed: Color,
    /// A volatile wad: expected to change.
    pub volatile: Color,
    /// Lines that only divide things, like the one under the editor's base wad.
    pub divider: Color,
    /// The empty part of a progress bar.
    pub gauge: Color,
    /// The words on a progress bar.
    pub gauge_label: Color,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            background: VOID,
            text: BONE,
            muted: ASH,
            title: FLAME,
            key: FLAME,
            screen: BLOOD,
            multiplayer: HELLFIRE,
            netplay: Color::LightMagenta,
            dialog: BONE,
            warning: BLOOD,
            legend: ASH,
            panel: IRON,
            busy: HELLFIRE,
            hashing: NUKAGE,
            unsealing: SOUL,
            quip: FLAME,
            good: NUKAGE,
            bad: BLOOD,
            changed: HELLFIRE,
            volatile: SOUL,
            divider: IRON,
            gauge: IRON,
            gauge_label: BONE,
        }
    }
}

/// Rows and badges, each a foreground, a background and whether it is bold.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Styles {
    /// The cursor row of a list.
    #[serde(with = "look")]
    pub cursor: Style,
    /// The cursor row of a list when what it points at is broken.
    #[serde(with = "look")]
    pub broken_cursor: Style,
    /// A row picked to be copied, with `Shift+V` or `M`.
    #[serde(with = "look")]
    pub selected: Style,
    /// A row that is broken, but not under the cursor.
    #[serde(with = "look")]
    pub broken_row: Style,
    /// Column headings.
    #[serde(with = "look")]
    pub header: Style,
    /// The marks on list rows, drawn as solid badges so they stand out even under the cursor.
    #[serde(with = "look")]
    pub broken_badge: Style,
    #[serde(with = "look")]
    pub changed_badge: Style,
    #[serde(with = "look")]
    pub volatile_badge: Style,
}

impl Default for Styles {
    fn default() -> Self {
        let bold = |fg, bg| Style::new().fg(fg).bg(bg).add_modifier(Modifier::BOLD);
        Self {
            cursor: bold(VOID, HELLFIRE),
            broken_cursor: bold(BONE, BLOOD),
            selected: Style::new().fg(BONE).bg(IRON),
            broken_row: Style::new().fg(BONE).bg(GORE),
            header: Style::new().fg(ASH).add_modifier(Modifier::BOLD),
            broken_badge: bold(BONE, BLOOD),
            changed_badge: bold(VOID, HELLFIRE),
            volatile_badge: bold(VOID, SOUL),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Borders {
    /// The border of full screens and panels.
    #[serde(with = "border")]
    pub window: BorderType,
    /// The border of small popups, kept plain so they stay out of the way.
    #[serde(with = "border")]
    pub popup: BorderType,
}

impl Default for Borders {
    fn default() -> Self {
        Self {
            window: BorderType::Thick,
            popup: BorderType::Plain,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Glyphs {
    /// The mark in front of a status line: its color says how things are.
    pub status: String,
    /// The mark in front of a status line for something that isn't set.
    pub unset: String,
    /// The marks on list rows: broken, changed, volatile.
    pub broken_mark: String,
    pub changed_mark: String,
    pub volatile_mark: String,
}

impl Default for Glyphs {
    fn default() -> Self {
        Self {
            status: "■".into(),
            unset: "□".into(),
            broken_mark: "!".into(),
            changed_mark: "~".into(),
            volatile_mark: "*".into(),
        }
    }
}

/// `tdl --help`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Help {
    /// Headings and the usage line.
    pub heading: Color,
    /// What is typed as it is: the flags and commands.
    pub literal: Color,
    /// What to fill in.
    pub placeholder: Color,
    /// What went wrong.
    pub error: Color,
    /// The part of a mistake that was right, and the part that wasn't.
    pub valid: Color,
    pub invalid: Color,
}

impl Default for Help {
    fn default() -> Self {
        Self {
            heading: HELLFIRE,
            literal: FLAME,
            placeholder: BONE,
            error: BLOOD,
            valid: NUKAGE,
            invalid: FLAME,
        }
    }
}

impl Dressing for Theme {
    const WHAT: &str = "theme";
    const FILE: &str = FILE;
    const SUFFIX: &str = "_theme.toml";
    const BUNDLED: &[(&str, &str)] = bundled::THEMES;
    /// Edition 1 is the theme of tdl 1.0.0, which also dressed up the splash screen.
    const UPGRADES: &[fn(&mut toml::Table)] = &[v1_to_v2];

    fn template() -> String {
        let body = toml::to_string(&Self::default()).expect("a theme is always plain TOML");
        format!("{HEADER}{}\n{body}", dressing::edition_line::<Self>())
    }
}

/// Edition 2 leaves the splash screen to `splash.toml` and `pride.toml`, so the `[splash]` and
/// `[pride]` an edition 1 theme may have are dropped.
fn v1_to_v2(theme: &mut toml::Table) {
    theme.remove("splash");
    theme.remove("pride");
}

const HEADER: &str = "\
# tdl's theme. Change anything below, or leave it out for the built-in theme's.
# Every field is explained in THEME.md. The splash screen is dressed up in splash.toml instead.
# A color is \"#rrggbb\", a terminal color's name (\"red\", \"lightmagenta\", \"darkgray\", ...),
# a terminal palette number (\"0\" to \"255\"), or \"reset\" for the terminal's own.
# A style is { fg = color, bg = color, bold = true }, each part optional.
# A border is plain, rounded, double, thick, quadrant_inside or quadrant_outside.

";

/// For `#[serde(with)]` on a [`Style`]: saved as `{ fg, bg, bold }`, each left out when unset.
mod look {
    use super::*;

    #[derive(Default, Deserialize, Serialize)]
    #[serde(default, deny_unknown_fields)]
    struct Look {
        #[serde(skip_serializing_if = "Option::is_none")]
        fg: Option<Color>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bg: Option<Color>,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        bold: bool,
    }

    pub fn serialize<S: Serializer>(style: &Style, serializer: S) -> Result<S::Ok, S::Error> {
        Look {
            fg: style.fg,
            bg: style.bg,
            bold: style.add_modifier.contains(Modifier::BOLD),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Style, D::Error> {
        let look = Look::deserialize(deserializer)?;
        let mut style = Style::new();
        if let Some(fg) = look.fg {
            style = style.fg(fg);
        }
        if let Some(bg) = look.bg {
            style = style.bg(bg);
        }
        if look.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        Ok(style)
    }
}

/// For `#[serde(with)]` on a [`BorderType`]: saved in snake case, `thick` or `quadrant_inside`.
mod border {
    use super::*;

    pub fn serialize<S: Serializer>(border: &BorderType, serializer: S) -> Result<S::Ok, S::Error> {
        let mut snake = String::new();
        for (i, character) in border.to_string().chars().enumerate() {
            if character.is_uppercase() && i > 0 {
                snake.push('_');
            }
            snake.push(character.to_ascii_lowercase());
        }
        serializer.serialize_str(&snake)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BorderType, D::Error> {
        let name = String::deserialize(deserializer)?;
        let pascal: String = name
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
                })
            })
            .collect();
        BorderType::from_str(&pascal)
            .map_err(|_| serde::de::Error::custom(format!("'{name}' isn't a border tdl knows")))
    }
}

static WARDROBE: Wardrobe<Theme> = Wardrobe::new();

/// The theme being worn.
pub fn worn() -> &'static Theme {
    WARDROBE.worn()
}

/// Wears `theme` for the rest of the run, or until another is worn.
pub fn wear(theme: Theme) {
    WARDROBE.wear(theme);
}

static PRIDE: AtomicBool = AtomicBool::new(false);

/// Wears pride for the rest of the run.
pub fn wear_pride() {
    PRIDE.store(true, Ordering::Relaxed);
}

/// Whether this run wears pride.
pub fn pride() -> bool {
    PRIDE.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn the_built_in_theme_writes_out_and_reads_back_the_same() {
        let text = Theme::template();
        assert!(text.starts_with("# tdl's theme."));
        assert!(text.contains("\nedition = 2\n"));
        assert!(text.contains("background = \"#0E0808\""));
        assert!(text.contains("window = \"thick\""));
        assert_eq!(Theme::parse(&text), Ok(Theme::default()));
    }

    #[test]
    fn a_theme_only_needs_what_it_changes() {
        let theme = Theme::parse(
            "edition = 2\n[colors]\nscreen = \"#00ff00\"\nbackground = \"reset\"\n\
             [styles]\ncursor = { fg = \"black\", bg = \"lightcyan\" }\n\
             [borders]\npopup = \"rounded\"\n[glyphs]\nbroken_mark = \"x\"",
        )
        .unwrap();
        assert_eq!(theme.colors.screen, Color::Rgb(0, 255, 0));
        assert_eq!(theme.colors.background, Color::Reset);
        assert_eq!(
            theme.styles.cursor,
            Style::new().fg(Color::Black).bg(Color::LightCyan)
        );
        assert_eq!(theme.borders.popup, BorderType::Rounded);
        assert_eq!(theme.glyphs.broken_mark, "x");
        // everything else is the built-in theme's
        assert_eq!(theme.colors.text, Colors::default().text);
        assert_eq!(theme.borders.window, BorderType::Thick);
        assert_eq!(Theme::parse(""), Ok(Theme::default()));
    }

    #[test]
    fn a_theme_with_a_mistake_says_what_it_is() {
        for wrong in [
            "[colors]\nscreen = \"not a color\"",
            "[colours]\nscreen = \"red\"",
            "[colors]\nscren = \"red\"",
            "[borders]\nwindow = \"wavy\"",
            "[styles]\ncursor = { fg = \"red\", italic = true }",
            "edition = 2\n[splash]\nshadow = \"red\"",
            "edition = 0",
            "edition = \"two\"",
            "this isn't toml",
        ] {
            assert!(Theme::parse(wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn a_theme_is_found_by_name_the_users_own_first() {
        let altar = crate::loadout::scribe::tests::Altar::new("theme-names");
        let voidless = Theme::load("voidless".as_ref(), &altar.0).unwrap();
        assert_eq!(voidless.colors.screen, Color::Rgb(0x8B, 0x5C, 0xF6));
        assert_eq!(
            Theme::load("hellfire".as_ref(), &altar.0),
            Ok(Theme::default())
        );
        // the user's own voidless_theme.toml wins over the one that comes with tdl
        fs::write(
            altar.file("voidless_theme.toml"),
            "[colors]\ntext = \"white\"",
        )
        .unwrap();
        let mine = Theme::load("voidless".as_ref(), &altar.0).unwrap();
        assert_eq!(mine.colors.text, Color::White);
        assert_eq!(mine.colors.screen, Colors::default().screen);
        fs::write(altar.file("mine_theme.toml"), "").unwrap();
        assert_eq!(Theme::load("mine".as_ref(), &altar.0), Ok(Theme::default()));
        let missing = Theme::load("nope".as_ref(), &altar.0).unwrap_err();
        assert!(missing.contains("hellfire, voidless, monokai"), "{missing}");
        // and a path is a path
        let path = altar.file("mine_theme.toml");
        assert_eq!(
            Theme::load(path.as_os_str(), &altar.0),
            Ok(Theme::default())
        );
    }

    #[test]
    fn a_theme_from_a_newer_tdl_is_refused_and_says_so() {
        let problem = Theme::parse("edition = 3\n[colors]\nsparkle = \"red\"").unwrap_err();
        assert!(problem.contains("newer tdl"), "{problem}");
    }

    #[test]
    fn a_theme_from_tdl_1_0_still_reads_without_its_splash() {
        // edition 1 had no edition line, and dressed up the splash screen too
        let theme = Theme::parse(
            "[colors]\ntext = \"white\"\n\
             [splash]\nlogo = [\"red\", \"red\", \"red\", \"red\", \"red\"]\nshadow = \"red\"\n\
             fire = [\"black\", \"white\"]\n[pride]\nlogo = [\"blue\", \"pink\", \"white\", \"pink\", \"blue\"]",
        )
        .unwrap();
        assert_eq!(theme.colors.text, Color::White);
        assert_eq!(theme.help, Help::default());
    }

    #[test]
    fn the_built_in_theme_is_written_once_and_a_changed_one_is_kept() {
        let altar = crate::loadout::scribe::tests::Altar::new("theme-write");
        let path = altar.file("dir/theme.toml");
        Theme::write_new(&path).unwrap();
        assert_eq!(Theme::read(&path), Ok(Theme::default()));
        fs::write(&path, "[colors]\ntext = \"white\"").unwrap();
        Theme::write_new(&path).unwrap();
        assert_eq!(Theme::read(&path).unwrap().colors.text, Color::White);
        assert!(Theme::read(&altar.file("missing.toml")).is_err());
    }
}
