//! Every color, border and glyph the interface uses, in one place. Tune the look here; the screens
//! only ever ask for a role ("a screen border", "something broken"), never for a color.

use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::{
    style::{Color, Modifier, Style},
    widgets::BorderType,
};

// The palette: hot metal, old blood and toxic waste. Every role below is picked from these.

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

// Roles: what each part of the interface is drawn with.

/// Painted behind everything. `Color::Reset` keeps the terminal's own background instead.
pub const BACKGROUND: Color = VOID;
pub const TEXT: Color = BONE;
/// Labels, hints, and anything that is unset rather than wrong.
pub const MUTED: Color = ASH;
pub const TITLE: Color = FLAME;
/// Keys the user can press, wherever they are shown.
pub const KEY: Color = FLAME;

/// The border of the main screens: the launcher and the editor.
pub const SCREEN: Color = BLOOD;
/// The border of the multiplayer screens: hosting, joining, picking and naming profiles.
pub const MULTIPLAYER: Color = HELLFIRE;
/// The border of yes/no questions and other small popups.
pub const DIALOG: Color = BONE;
pub const WARNING: Color = BLOOD;
pub const LEGEND: Color = ASH;
/// The border of panels that only report, like the one under the launcher.
pub const PANEL: Color = IRON;
/// The border of windows that show something in progress.
pub const BUSY: Color = HELLFIRE;
/// The border and bar of the window that shows wads being hashed, kept apart from the download
/// window's so the two are never mistaken for each other.
pub const HASHING: Color = NUKAGE;
/// The border and bar of the window that shows a scroll being unsealed.
pub const UNSEALING: Color = SOUL;
/// A quip in a window that is busy.
pub const QUIP: Color = FLAME;

/// Something that works.
pub const GOOD: Color = NUKAGE;
/// Something that stops a profile from running.
pub const BAD: Color = BLOOD;
/// Something that still runs but deserves a look.
pub const CHANGED: Color = HELLFIRE;
/// A volatile wad: expected to change.
pub const VOLATILE: Color = SOUL;

/// The cursor row of a list.
pub const CURSOR: Style = Style::new()
    .fg(VOID)
    .bg(HELLFIRE)
    .add_modifier(Modifier::BOLD);
/// The cursor row of a list when what it points at is broken.
pub const BROKEN_CURSOR: Style = Style::new().fg(BONE).bg(BLOOD).add_modifier(Modifier::BOLD);
/// A row picked to be copied, with `Shift+V` or `M`.
pub const SELECTED: Style = Style::new().fg(BONE).bg(IRON);
/// A row that is broken, but not under the cursor.
pub const BROKEN_ROW: Style = Style::new().fg(BONE).bg(GORE);
/// Column headings.
pub const HEADER: Style = Style::new().fg(ASH).add_modifier(Modifier::BOLD);

/// The border of full screens and panels.
pub const WINDOW_BORDER: BorderType = BorderType::Thick;
/// The border of small popups, kept plain so they stay out of the way.
pub const POPUP_BORDER: BorderType = BorderType::Plain;

// Glyphs.

/// The mark in front of a status line: its color says how things are.
pub const STATUS: &str = "■";
/// The mark in front of a status line for something that isn't set.
pub const UNSET: &str = "□";
/// The marks on list rows, drawn as solid badges so they stand out even under the cursor:
/// broken, changed, volatile.
pub const BROKEN_MARK: &str = "!";
pub const CHANGED_MARK: &str = "~";
pub const VOLATILE_MARK: &str = "*";
pub const BROKEN_BADGE: Style = Style::new().fg(BONE).bg(BLOOD).add_modifier(Modifier::BOLD);
pub const CHANGED_BADGE: Style = Style::new()
    .fg(VOID)
    .bg(HELLFIRE)
    .add_modifier(Modifier::BOLD);
pub const VOLATILE_BADGE: Style = Style::new().fg(VOID).bg(SOUL).add_modifier(Modifier::BOLD);

// Pride, for `--with-pride` or the **pride** setting: the splash's fire burns through the rainbow, under the logo in the
// trans flag's colors.

static PRIDE: AtomicBool = AtomicBool::new(false);

/// The trans flag's stripes, which the splash logo wears.
pub const TRANS_BLUE: Color = Color::Rgb(91, 206, 250);
pub const TRANS_PINK: Color = Color::Rgb(245, 169, 184);
pub const TRANS_WHITE: Color = Color::Rgb(255, 255, 255);

/// Wears pride for the rest of the run.
pub fn wear_pride() {
    PRIDE.store(true, Ordering::Relaxed);
}

/// Whether this run wears pride.
pub fn pride() -> bool {
    PRIDE.load(Ordering::Relaxed)
}
