//! The first thing on screen: the title over the fire from the PlayStation port of Doom.
//! It burns until a key is pressed.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyModifiers},
    layout::Rect,
    style::{Color, Modifier, Style},
};

use crate::{
    error::{Error, Result},
    ritual::{Grimoire, Haunted},
    theme,
    widgets::fire::Fire,
};

/// The title, in full blocks; the box-drawing strokes are its shadow.
const LOGO: [&str; 6] = [
    "████████╗██████╗ ██╗     ",
    "╚══██╔══╝██╔══██╗██║     ",
    "   ██║   ██║  ██║██║     ",
    "   ██║   ██║  ██║██║     ",
    "   ██║   ██████╔╝███████╗",
    "   ╚═╝   ╚═════╝ ╚══════╝",
];
/// One color per row of the logo, white hot at the top down to blood at the bottom.
const LOGO_COLORS: [Color; 6] = [
    theme::FLAME,
    theme::FLAME,
    theme::HELLFIRE,
    theme::HELLFIRE,
    theme::BLOOD,
    theme::BLOOD,
];
const SHADOW: Color = theme::GORE;
const TAGLINE: &str = "t e r m i n a l   d o o m   l a u n c h e r";
/// For terminals too narrow for the spaced-out tagline.
const NARROW_TAGLINE: &str = "terminal doom launcher";

const HINT: &str = "press any key";
/// How long the hint takes to fade from bright to dim and back.
const PULSE: Duration = Duration::from_millis(1600);
/// One frame of the animation.
const FRAME: Duration = Duration::from_millis(33);
/// The fire fills the screen from this far down.
const FIRE_TOP_PERCENT: u16 = 45;

/// Plays the splash until a key is pressed. Ctrl+C still quits.
pub fn summon(grimoire: &mut Grimoire) -> Result<()> {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |time| time.subsec_nanos());
    let mut fire: Option<Fire> = None;
    let start = Instant::now();
    loop {
        let elapsed = start.elapsed();
        grimoire.bewitch(|page| {
            let area = page.area();
            let flames = fire_area(area);
            // 🔥 a new fire whenever the terminal changes size, since the old one no longer fits
            let fire = match &mut fire {
                Some(fire) if fire.fits(flames) => fire,
                _ => fire.insert(Fire::new(flames, seed)),
            };
            fire.step();
            paint(fire, elapsed, area, page.buffer_mut());
        })?;
        if event::poll(FRAME)? {
            match event::read()? {
                Event::Key(key)
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    return Err(Error::Exit);
                }
                Event::Key(key) if key.is_press() => return Ok(()),
                _ => {}
            }
        }
    }
}

/// Where the fire burns: the bottom part of the screen, starting low enough on a short terminal
/// to leave a row above the logo, then the logo, the tagline and the hint with a blank row after each.
fn fire_area(area: Rect) -> Rect {
    let headroom = LOGO.len() as u16 + 6;
    let top = (area.height * FIRE_TOP_PERCENT / 100)
        .max(headroom)
        .min(area.height);
    Rect::new(area.x, area.y + top, area.width, area.height - top)
}

/// Draws one frame: the fire, then the logo, the tagline and the hint just above the flames.
/// `elapsed` is how long the splash has been up, which sets where the hint is in its pulse.
fn paint(fire: &Fire, elapsed: Duration, area: Rect, buffer: &mut Buffer) {
    let flames = fire_area(area);
    fire.render(flames, buffer);

    let logo_width = LOGO[0].chars().count() as u16;
    let logo_x = area.x + area.width.saturating_sub(logo_width) / 2;
    // the logo, a blank row, the tagline, a blank row, the hint, then the flames
    let logo_y = flames.y.saturating_sub(LOGO.len() as u16 + 4).max(area.y);
    for (i, (row, color)) in LOGO.iter().zip(LOGO_COLORS).enumerate() {
        for (j, character) in row.chars().enumerate() {
            let style = if character == '█' {
                Style::new().fg(color)
            } else {
                Style::new().fg(SHADOW)
            };
            stamp(
                buffer,
                area,
                logo_x + j as u16,
                logo_y + i as u16,
                character,
                style,
            );
        }
    }

    let tagline = if TAGLINE.chars().count() as u16 <= area.width {
        TAGLINE
    } else {
        NARROW_TAGLINE
    };
    let tagline_width = tagline.chars().count() as u16;
    let tagline_x = area.x + area.width.saturating_sub(tagline_width) / 2;
    let tagline_y = logo_y + LOGO.len() as u16 + 1;
    let style = Style::new().fg(theme::BONE).add_modifier(Modifier::BOLD);
    for (j, character) in tagline.chars().enumerate() {
        stamp(
            buffer,
            area,
            tagline_x + j as u16,
            tagline_y,
            character,
            style,
        );
    }

    let hint_x = area.x + area.width.saturating_sub(HINT.len() as u16) / 2;
    let style = Style::new().fg(pulse(elapsed));
    for (j, character) in HINT.chars().enumerate() {
        stamp(
            buffer,
            area,
            hint_x + j as u16,
            tagline_y + 2,
            character,
            style,
        );
    }
}

/// The hint's color at `elapsed`: a slow breath between ash and bone.
fn pulse(elapsed: Duration) -> Color {
    let phase = (elapsed.as_millis() % PULSE.as_millis()) as f32 / PULSE.as_millis() as f32;
    let brightness = 1.0 - (phase * 2.0 - 1.0).abs();
    let mix = |from: u8, to: u8| (from as f32 + (to as f32 - from as f32) * brightness) as u8;
    let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (theme::ASH, theme::BONE) else {
        return theme::BONE;
    };
    Color::Rgb(mix(r1, r2), mix(g1, g2), mix(b1, b2))
}

/// Puts one character on the screen, if it's on the screen. Spaces are skipped, so whatever is
/// behind them (the fire) shows through.
fn stamp(buffer: &mut Buffer, area: Rect, x: u16, y: u16, character: char, style: Style) {
    if character != ' ' && x < area.right() && y < area.bottom() {
        buffer[(x, y)].set_char(character).set_style(style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_of_the_logo_is_the_same_width() {
        let width = LOGO[0].chars().count();
        assert!(LOGO.iter().all(|row| row.chars().count() == width));
        assert_eq!(LOGO.len(), LOGO_COLORS.len());
    }

    #[test]
    fn the_fire_burns_in_the_bottom_part_of_the_screen() {
        assert_eq!(fire_area(Rect::new(0, 0, 80, 40)), Rect::new(0, 18, 80, 22));
        // a short terminal keeps room for the logo above the flames
        assert_eq!(fire_area(Rect::new(0, 0, 80, 20)), Rect::new(0, 12, 80, 8));
    }

    #[test]
    fn a_frame_draws_at_every_size_without_panicking() {
        for (width, height) in [(40, 20), (80, 24), (200, 60), (10, 4), (1, 1)] {
            let area = Rect::new(0, 0, width, height);
            let mut fire = Fire::new(fire_area(area), 3);
            fire.step();
            let mut buffer = Buffer::empty(area);
            paint(&fire, Duration::ZERO, area, &mut buffer);
        }
    }

    #[test]
    fn the_logo_sits_above_the_flames_on_the_smallest_terminal() {
        let area = Rect::new(0, 0, 40, 20);
        let fire = Fire::new(fire_area(area), 3);
        let mut buffer = Buffer::empty(area);
        paint(&fire, Duration::ZERO, area, &mut buffer);
        let row = |y: u16| (0..40).map(|x| buffer[(x, y)].symbol()).collect::<String>();
        let top = (0..20).find(|&y| row(y).contains('█')).unwrap();
        assert!(top >= 1, "the logo needs a row above it");
        // the logo, the tagline and the hint all fit above the flames
        let tagline = top + LOGO.len() as u16 + 1;
        assert!(tagline + 2 < fire_area(area).y);
        assert_eq!(row(tagline + 2).trim(), HINT);
        // 40 columns is too narrow for the spaced tagline, so the plain one is used, whole
        assert_eq!(row(tagline).trim(), NARROW_TAGLINE);
    }

    #[test]
    fn a_wide_terminal_gets_the_spaced_tagline() {
        let area = Rect::new(0, 0, 80, 24);
        let fire = Fire::new(fire_area(area), 3);
        let mut buffer = Buffer::empty(area);
        paint(&fire, Duration::ZERO, area, &mut buffer);
        let row = |y: u16| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>();
        assert!((0..24).any(|y| row(y).trim() == TAGLINE));
    }

    #[test]
    fn the_hint_breathes_between_ash_and_bone() {
        assert_eq!(pulse(Duration::ZERO), theme::ASH);
        assert_eq!(pulse(PULSE / 2), theme::BONE);
        assert_eq!(pulse(PULSE), theme::ASH);
    }

    #[test]
    fn snapshot_a_frame_of_the_fire() {
        let area = Rect::new(0, 0, 80, 24);
        let mut fire = Fire::new(fire_area(area), 7);
        for _ in 0..40 {
            fire.step();
        }
        let snapshot = crate::testing::draw(80, 24, |page| {
            paint(&fire, Duration::ZERO, area, page.buffer_mut())
        });
        insta::assert_snapshot!(snapshot);
    }
}
