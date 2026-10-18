//! The badge at the start of a list row: `!` for broken, `~` for changed, `*` for volatile.
//! Badges are drawn after the list, so the cursor's highlight never paints over them.

use ratatui::{buffer::Buffer, layout::Rect, style::Style};

use crate::{
    theme,
    vault::{global::Global, profile::Profile, wad::Fault, wad::Wad},
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
    pub fn of_profile(profile: &Profile, global: &Global) -> Option<Self> {
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::vault::scribe::tests::Altar;

    fn setup(name: &str) -> (Altar, Global, Wad) {
        let altar = Altar::new(name);
        fs::write(altar.file("uzdoom"), b"").unwrap();
        fs::write(altar.file("a.pk3"), b"1234").unwrap();
        let global = Global {
            uzdoom: Some(altar.file("uzdoom").into()),
            ..Global::default()
        };
        let wad = Wad::from_path(&altar.file("a.pk3")).unwrap();
        (altar, global, wad)
    }

    #[test]
    fn a_healthy_profile_has_no_mark() {
        let (_altar, global, wad) = setup("mark-healthy");
        let profile = Profile::new("p".into(), vec![wad]);
        assert_eq!(Mark::of_profile(&profile, &global), None);
    }

    #[test]
    fn the_worst_thing_about_a_profile_wins() {
        let (altar, global, wad) = setup("mark-worst");
        let mut volatile = wad.clone();
        volatile.volatile = true;
        let profile = Profile::new("p".into(), vec![wad, volatile]);
        assert_eq!(Mark::of_profile(&profile, &global), Some(Mark::Volatile));

        fs::write(altar.file("a.pk3"), b"123456").unwrap();
        assert_eq!(Mark::of_profile(&profile, &global), Some(Mark::Changed));

        fs::remove_file(altar.file("a.pk3")).unwrap();
        assert_eq!(Mark::of_profile(&profile, &global), Some(Mark::Broken));
    }

    #[test]
    fn no_uzdoom_at_all_is_broken() {
        let (_altar, _, wad) = setup("mark-no-uzdoom");
        let profile = Profile::new("p".into(), vec![wad]);
        assert_eq!(
            Mark::of_profile(&profile, &Global::default()),
            Some(Mark::Broken)
        );
    }

    #[test]
    fn a_wad_is_marked_by_its_fault_first() {
        let mut wad = Wad::default();
        assert_eq!(
            Mark::of_wad(&wad, Some(&Fault::Missing)),
            Some(Mark::Broken)
        );
        let changed = Fault::Changed {
            expected: 1,
            found: 2,
        };
        assert_eq!(Mark::of_wad(&wad, Some(&changed)), Some(Mark::Changed));
        assert_eq!(Mark::of_wad(&wad, None), None);
        wad.volatile = true;
        assert_eq!(Mark::of_wad(&wad, None), Some(Mark::Volatile));
        assert_eq!(
            Mark::of_wad(&wad, Some(&Fault::Missing)),
            Some(Mark::Broken)
        );
    }

    #[test]
    fn a_badge_is_painted_over_whatever_is_there_and_clipped() {
        let area = Rect::new(0, 0, 4, 1);
        let mut buffer = Buffer::empty(area);
        buffer.set_style(area, theme::worn().styles.cursor);
        Mark::Broken.paint(2, 0, area, &mut buffer);
        assert_eq!(buffer[(2, 0)].symbol(), " ");
        assert_eq!(buffer[(3, 0)].symbol(), "!");
        assert_eq!(Some(buffer[(3, 0)].bg), theme::worn().styles.broken_badge.bg);
        // the cell before the badge keeps the cursor's style
        assert_eq!(Some(buffer[(1, 0)].bg), theme::worn().styles.cursor.bg);
    }

    #[test]
    fn a_badge_on_a_row_of_its_own_color_is_inverted() {
        let area = Rect::new(0, 0, 3, 1);
        let mut buffer = Buffer::empty(area);
        buffer.set_style(area, theme::worn().styles.broken_cursor);
        Mark::Broken.paint(0, 0, area, &mut buffer);
        assert_eq!(Some(buffer[(1, 0)].bg), theme::worn().styles.broken_badge.fg);
        assert_eq!(Some(buffer[(1, 0)].fg), theme::worn().styles.broken_badge.bg);
    }
}
