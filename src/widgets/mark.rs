//! The badge at the start of a list row: `!` for broken, `~` for changed, `*` for volatile.
//! Badges are drawn after the list, so the cursor's highlight never paints over them.

use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::{
    loadout::{profile::Profile, uac::Uac, wad::Fault, wad::Wad},
    theme,
};

/// How wide a badge is: the glyph with a space on either side.
pub const WIDTH: u16 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mark {
    /// Can't be used: a file is missing or unreadable, or there is no UZDoom at all.
    Broken,
    /// Still works, but a file changed since it was added.
    Changed,
    /// Fine, and expected to change: a mod still being worked on.
    Volatile,
}

impl Mark {
    fn glyph(self) -> &'static str {
        match self {
            Self::Broken => &theme::worn().glyphs.broken_mark,
            Self::Changed => &theme::worn().glyphs.changed_mark,
            Self::Volatile => &theme::worn().glyphs.volatile_mark,
        }
    }

    fn style(self) -> Style {
        match self {
            Self::Broken => theme::worn().styles.broken_badge,
            Self::Changed => theme::worn().styles.changed_badge,
            Self::Volatile => theme::worn().styles.volatile_badge,
        }
    }

    /// The mark for one wad: its fault if it has one, otherwise whether it is volatile.
    pub fn of_wad(wad: &Wad, fault: Option<&Fault>) -> Option<Self> {
        match fault {
            Some(fault) if fault.is_blocking() => Some(Self::Broken),
            Some(_) => Some(Self::Changed),
            None if wad.volatile => Some(Self::Volatile),
            None => None,
        }
    }

    /// The mark for a whole profile: the worst thing about it. This looks at the disk once for each
    /// of its files, so call it only for profiles that are on screen.
    pub fn of_profile(profile: &Profile, global: &Uac) -> Option<Self> {
        let faults = profile.faults(global);
        if profile.uzdoom(global).is_none() || faults.iter().any(|(_, f)| f.is_blocking()) {
            Some(Self::Broken)
        } else if !faults.is_empty() {
            Some(Self::Changed)
        } else if profile.wads.iter().any(|wad| wad.volatile) {
            Some(Self::Volatile)
        } else {
            None
        }
    }

    /// Draws the badge with its left edge at (`x`, `y`), clipped to `area`. If the row behind it is
    /// already the badge's color (a broken row under the cursor), the badge is drawn inverted so
    /// it still stands out.
    pub fn paint(self, x: u16, y: u16, area: Rect, buffer: &mut Buffer) {
        if !(area.left()..area.right()).contains(&x) || !(area.top()..area.bottom()).contains(&y) {
            return;
        }
        let mut style = self.style();
        if style.bg == Some(buffer[(x, y)].bg) {
            style = Style {
                fg: style.bg,
                bg: style.fg,
                ..style
            };
        }
        let badge = format!(" {} ", self.glyph());
        for (i, character) in badge.chars().enumerate() {
            let x = x + i as u16;
            if x < area.right() {
                buffer[(x, y)].set_char(character).set_style(style);
            }
        }
    }
}
