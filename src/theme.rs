//! Every color, border and glyph the interface uses, in one [`Theme`]. The screens only ever ask
//! the theme being worn for a role ("a screen border", "something broken"), never for a color.
//!
//! The built-in theme is below. Another can be worn from a `theme.toml` (see [`Theme::read`]),
//! kept in `uac.rite` or given with `--theme`; anything it leaves out is the built-in theme's.

use std::{
    fs, io,
    path::Path,
    str::FromStr,
    sync::{
        LazyLock, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use ratatui::{
    style::{Color, Modifier, Style},
    widgets::BorderType,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// The built-in palette: hot metal, old blood and toxic waste. Every role of the built-in theme is
// picked from these.

/// Almost black, with a little blood in it.
const VOID: Color = Color::Rgb(14, 8, 8);
const BLOOD: Color = Color::Rgb(186, 26, 18);
/// Dried blood: for fills that shouldn't shout.
const GORE: Color = Color::Rgb(92, 14, 10);
const HELLFIRE: Color = Color::Rgb(255, 112, 16);
/// The hottest part of the flame.
const FLAME: Color = Color::Rgb(255, 200, 70);
const BONE: Color = Color::Rgb(228, 216, 190);
const ASH: Color = Color::Rgb(128, 116, 106);
/// Cold, dark metal: for lines that only divide things.
const IRON: Color = Color::Rgb(70, 60, 58);
/// The green slime of Doom's toxic floors.
const NUKAGE: Color = Color::Rgb(124, 200, 46);
/// The blue of a soul sphere.
const SOUL: Color = Color::Rgb(96, 156, 255);
/// The trans flag's stripes, which the pride splash's logo wears.
const TRANS_BLUE: Color = Color::Rgb(91, 206, 250);
const TRANS_PINK: Color = Color::Rgb(245, 169, 184);
const TRANS_WHITE: Color = Color::Rgb(255, 255, 255);

/// The fire's colors from the PlayStation port of Doom, from cold to white hot.
const FIRE: [(u8, u8, u8); 37] = [
    (0x07, 0x07, 0x07),
    (0x1F, 0x07, 0x07),
    (0x2F, 0x0F, 0x07),
    (0x47, 0x0F, 0x07),
    (0x57, 0x17, 0x07),
    (0x67, 0x1F, 0x07),
    (0x77, 0x1F, 0x07),
    (0x8F, 0x27, 0x07),
    (0x9F, 0x2F, 0x07),
    (0xAF, 0x3F, 0x07),
    (0xBF, 0x47, 0x07),
    (0xC7, 0x47, 0x07),
    (0xDF, 0x4F, 0x07),
    (0xDF, 0x57, 0x07),
    (0xDF, 0x57, 0x07),
    (0xD7, 0x5F, 0x07),
    (0xD7, 0x5F, 0x07),
    (0xD7, 0x67, 0x0F),
    (0xCF, 0x6F, 0x0F),
    (0xCF, 0x77, 0x0F),
    (0xCF, 0x7F, 0x0F),
    (0xCF, 0x87, 0x17),
    (0xC7, 0x87, 0x17),
    (0xC7, 0x8F, 0x17),
    (0xC7, 0x97, 0x1F),
    (0xBF, 0x9F, 0x1F),
    (0xBF, 0x9F, 0x1F),
    (0xBF, 0xA7, 0x27),
    (0xBF, 0xA7, 0x27),
    (0xBF, 0xAF, 0x2F),
    (0xB7, 0xAF, 0x2F),
    (0xB7, 0xB7, 0x2F),
    (0xB7, 0xB7, 0x37),
    (0xCF, 0xCF, 0x6F),
    (0xDF, 0xDF, 0x9F),
    (0xEF, 0xEF, 0xC7),
    (0xFF, 0xFF, 0xFF),
];

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
    pub splash: Splash,
    pub pride: Pride,
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

/// The splash screen.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Splash {
    /// One color for each row of the logo, top to bottom.
    pub logo: [Color; 5],
    /// The logo's box-drawing strokes.
    pub shadow: Color,
    pub tagline: Color,
    /// The hint breathes from the first to the second and back.
    pub hint: [Color; 2],
    /// The quip flares from the first to the second and back.
    pub quip: [Color; 2],
    /// The fire, from cold to white hot: as many as you like, at least two.
    pub fire: Vec<Color>,
}

impl Default for Splash {
    fn default() -> Self {
        Self {
            // white hot at the top, cooling a step a row down to blood at the bottom
            logo: [
                FLAME,
                halfway(FLAME, HELLFIRE),
                HELLFIRE,
                halfway(HELLFIRE, BLOOD),
                BLOOD,
            ],
            shadow: GORE,
            tagline: BONE,
            hint: [ASH, BONE],
            quip: [HELLFIRE, FLAME],
            fire: FIRE.iter().map(|&(r, g, b)| Color::Rgb(r, g, b)).collect(),
        }
    }
}

/// Pride, for `--with-pride` or the **pride** setting: the splash's fire burns through the rainbow,
/// under the logo in these colors.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Pride {
    /// The whole trans flag, a stripe a row: blue, pink, white, pink, blue.
    pub logo: [Color; 5],
}

impl Default for Pride {
    fn default() -> Self {
        Self {
            logo: [TRANS_BLUE, TRANS_PINK, TRANS_WHITE, TRANS_PINK, TRANS_BLUE],
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

/// The color halfway between `a` and `b`, or `a` if either isn't RGB.
pub const fn halfway(a: Color, b: Color) -> Color {
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            Color::Rgb(mid(r1, r2), mid(g1, g2), mid(b1, b2))
        }
        _ => a,
    }
}

/// The channel halfway between `x` and `y`.
const fn mid(x: u8, y: u8) -> u8 {
    ((x as u16 + y as u16) / 2) as u8
}

impl Theme {
    /// The theme in a `theme.toml`. `Err` says why it can't be worn, for the user.
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("Couldn't read the theme {}: {error}", path.display()))?;
        Self::parse(&text).map_err(|problem| format!("The theme {} {problem}", path.display()))
    }

    /// The theme written in `text`, as in a `theme.toml`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let theme: Self =
            toml::from_str(text).map_err(|error| format!("isn't one tdl can wear: {error}"))?;
        if theme.splash.fire.len() < 2 {
            return Err("needs at least two fire colors".to_string());
        }
        Ok(theme)
    }

    /// The theme as a `theme.toml`, with a note at the top on how to write the colors.
    pub fn to_toml(&self) -> String {
        let body = toml::to_string(self).expect("a theme is always plain TOML");
        format!("{HEADER}{body}")
    }

    /// Writes the built-in theme to `path`, for the user to change, unless a file is there
    /// already, which is kept as it is.
    pub fn write_built_in(path: &Path) -> io::Result<()> {
        if path.exists() {
            return Ok(());
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, Self::default().to_toml())
    }
}

const HEADER: &str = "\
# tdl's theme. Change anything below, or leave it out for the built-in theme's.
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

static BUILT_IN: LazyLock<Theme> = LazyLock::new(Theme::default);
/// The theme being worn, once one other than the built-in one has been put on. Each is leaked, so
/// every screen can hold on to it for as long as it draws; a theme is only changed by hand.
static WORN: RwLock<Option<&'static Theme>> = RwLock::new(None);

/// The theme being worn.
pub fn worn() -> &'static Theme {
    WORN.read().ok().and_then(|worn| *worn).unwrap_or(&BUILT_IN)
}

/// Wears `theme` for the rest of the run, or until another is worn.
pub fn wear(theme: Theme) {
    let theme = if theme == *BUILT_IN {
        None
    } else {
        Some(&*Box::leak(Box::new(theme)))
    };
    if let Ok(mut worn) = WORN.write() {
        *worn = theme;
    }
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
    use super::*;

    #[test]
    fn the_built_in_theme_writes_out_and_reads_back_the_same() {
        let text = Theme::default().to_toml();
        assert!(text.starts_with("# tdl's theme."));
        assert!(text.contains("background = \"#0E0808\""));
        assert!(text.contains("window = \"thick\""));
        assert_eq!(Theme::parse(&text), Ok(Theme::default()));
    }

    #[test]
    fn a_theme_only_needs_what_it_changes() {
        let theme = Theme::parse(
            "[colors]\nscreen = \"#00ff00\"\nbackground = \"reset\"\n\
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
        assert_eq!(theme.splash, Splash::default());
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
            "[splash]\nfire = [\"red\"]",
            "[splash]\nlogo = [\"red\"]",
            "this isn't toml",
        ] {
            assert!(Theme::parse(wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn the_built_in_theme_is_written_once_and_a_changed_one_is_kept() {
        let altar = crate::vault::scribe::tests::Altar::new("theme-write");
        let path = altar.file("dir/theme.toml");
        Theme::write_built_in(&path).unwrap();
        assert_eq!(Theme::read(&path), Ok(Theme::default()));
        fs::write(&path, "[colors]\ntext = \"white\"").unwrap();
        Theme::write_built_in(&path).unwrap();
        assert_eq!(Theme::read(&path).unwrap().colors.text, Color::White);
        assert!(Theme::read(&altar.file("missing.toml")).is_err());
    }

    #[test]
    fn halfway_is_between_two_colors() {
        assert_eq!(
            halfway(Color::Rgb(0, 100, 255), Color::Rgb(100, 0, 255)),
            Color::Rgb(50, 50, 255)
        );
        assert_eq!(halfway(Color::Red, Color::Rgb(0, 0, 0)), Color::Red);
    }
}
