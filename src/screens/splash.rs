//! The first thing on screen: the title over the fire from the PlayStation port of Doom, with a
//! quip under it, the way Minecraft's title screen has one. It burns until a key is pressed.

use std::time::{Duration, Instant};

use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyModifiers},
    layout::Rect,
    style::{Color, Modifier, Style},
};

use crate::{
    error::{Error, Result},
    music, quips,
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
const TAGLINE: &str = "t e r m i n a l   d o o m   l a u n c h e r";
/// For terminals too narrow for the spaced-out tagline.
const NARROW_TAGLINE: &str = "terminal doom launcher";

const HINT: &str = "press any key";

/// How long the quip takes to flare and settle, quicker than the hint, to catch the eye.
const QUIP_PULSE: Duration = Duration::from_millis(700);
/// How long the hint takes to fade from bright to dim and back.
const PULSE: Duration = Duration::from_millis(1600);
/// One frame of the animation.
const FRAME: Duration = Duration::from_millis(33);
/// The fire fills the screen from this far down.
const FIRE_TOP_PERCENT: u16 = 45;
/// With pride: how long the fire takes to burn through every color of the rainbow.
const RAINBOW: Duration = Duration::from_secs(6);

/// Plays the splash, fading in from black, until a key is pressed, then fades it to black along
/// with the music. `quip` (from `--splash-quip` or the settings) replaces the usual random
/// one, which is left out when the quips are hidden.
/// Ctrl+C still quits.
pub fn summon(grimoire: &mut Grimoire, quip: Option<&str>) -> Result<()> {
    music::start(false);
    let seed = quips::seed();
    let mut fire: Option<Fire> = None;
    let start = Instant::now();
    // when a key was pressed, which starts the fade
    let mut leaving: Option<Instant> = None;
    loop {
        let elapsed = start.elapsed();
        let fade_in = elapsed.as_secs_f32() / music::FADE_IN.as_secs_f32();
        let fade_out = leaving.map_or(1.0, |at| {
            1.0 - at.elapsed().as_secs_f32() / music::FADE.as_secs_f32()
        });
        let brightness = fade_in.min(fade_out).min(1.0);
        if fade_out <= 0.0 {
            return Ok(());
        }
        grimoire.bewitch(|page| {
            let area = page.area();
            let flames = fire_area(area);
            // 🔥 a new fire whenever the terminal changes size, since the old one no longer fits,
            // kindled first so it is roaring from the first frame rather than climbing up
            let fire = match &mut fire {
                Some(fire) if fire.fits(flames) => fire,
                _ => fire.insert(Fire::kindled(flames, seed)),
            };
            fire.step();
            let quip = match quip {
                Some(quip) => quips::fit(quip, area.width),
                None if quips::shown() => quips::pick(seed, area.width).map(str::to_string),
                None => None,
            };
            paint(fire, elapsed, area, quip.as_deref(), page.buffer_mut());
            dim(page.buffer_mut(), brightness);
        })?;
        if event::poll(FRAME)? {
            match event::read()? {
                Event::Key(key)
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    return Err(Error::Interrupted);
                }
                Event::Key(key) if key.is_press() && leaving.is_none() => {
                    music::splash_over();
                    leaving = Some(Instant::now());
                }
                _ => {}
            }
        }
    }
}

/// Scales every color toward black: 1.0 leaves the frame as it is, 0.0 is fully dark.
fn dim(buffer: &mut Buffer, brightness: f32) {
    if brightness >= 1.0 {
        return;
    }
    let scale = |color: Color| match color {
        Color::Rgb(r, g, b) => {
            let by = |channel: u8| (channel as f32 * brightness) as u8;
            Color::Rgb(by(r), by(g), by(b))
        }
        other => other,
    };
    for cell in &mut buffer.content {
        (cell.fg, cell.bg) = (scale(cell.fg), scale(cell.bg));
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

/// Draws one frame: the fire, then the logo, the quip, the tagline and the hint just above the
/// flames. `elapsed` is how long the splash has been up, which sets where the hint and the quip
/// are in their pulses.
fn paint(fire: &Fire, elapsed: Duration, area: Rect, quip: Option<&str>, buffer: &mut Buffer) {
    let flames = fire_area(area);
    if theme::pride() {
        let hue = elapsed.as_secs_f32() / RAINBOW.as_secs_f32();
        fire.render_rainbow(flames, buffer, hue);
    } else {
        fire.render(flames, buffer);
    }

    let logo_width = LOGO[0].chars().count() as u16;
    let logo_x = area.x + area.width.saturating_sub(logo_width) / 2;
    // the logo, a blank row, the tagline, a blank row, the hint, then the flames
    let logo_y = flames.y.saturating_sub(LOGO.len() as u16 + 4).max(area.y);
    // 🎨 one color for each row with blocks in it, from the theme's splash, or its pride with pride
    let theme = theme::worn();
    let colors = if theme::pride() {
        theme.pride.logo
    } else {
        theme.splash.logo
    };
    let shadow = theme.splash.shadow;
    for (i, row) in LOGO.iter().enumerate() {
        // 🌑 the last row has no blocks, only shadow, so it has no color of its own
        let color = colors.get(i).copied().unwrap_or(shadow);
        for (j, character) in row.chars().enumerate() {
            let style = if character == '█' {
                Style::new().fg(color)
            } else {
                Style::new().fg(shadow)
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

    // ✨ the quip sits centered between the logo and the tagline
    if let Some(quip) = quip {
        let x = area.x + area.width.saturating_sub(quip.chars().count() as u16) / 2;
        let style = Style::new().fg(flare(elapsed)).add_modifier(Modifier::BOLD);
        for (j, character) in quip.chars().enumerate() {
            stamp(
                buffer,
                area,
                x + j as u16,
                logo_y + LOGO.len() as u16,
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
    let style = Style::new()
        .fg(theme::worn().splash.tagline)
        .add_modifier(Modifier::BOLD);
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

/// The hint's color at `elapsed`: a slow breath between the theme's two (ash and bone).
fn pulse(elapsed: Duration) -> Color {
    let [from, to] = theme::worn().splash.hint;
    breathe(elapsed, PULSE, from, to)
}

/// The quip's color at `elapsed`: a quick flare between the theme's two (hellfire and the yellow
/// of the flame tips).
fn flare(elapsed: Duration) -> Color {
    let [from, to] = theme::worn().splash.quip;
    breathe(elapsed, QUIP_PULSE, from, to)
}

/// Goes from `from` to `to` and back once every `period`.
fn breathe(elapsed: Duration, period: Duration, from: Color, to: Color) -> Color {
    let phase = (elapsed.as_millis() % period.as_millis()) as f32 / period.as_millis() as f32;
    let brightness = 1.0 - (phase * 2.0 - 1.0).abs();
    let mix = |from: u8, to: u8| (from as f32 + (to as f32 - from as f32) * brightness) as u8;
    let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (from, to) else {
        return to;
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
    fn dimming_scales_colors_toward_black() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        buffer[(0, 0)].fg = Color::Rgb(200, 100, 50);
        dim(&mut buffer, 0.5);
        assert_eq!(buffer[(0, 0)].fg, Color::Rgb(100, 50, 25));
        dim(&mut buffer, 0.0);
        assert_eq!(buffer[(0, 0)].fg, Color::Rgb(0, 0, 0));
    }

    #[test]
    fn every_row_of_the_logo_is_the_same_width() {
        let width = LOGO[0].chars().count();
        assert!(LOGO.iter().all(|row| row.chars().count() == width));
        // every row with blocks in it has a color, and the shadow row under them needs none
        let with_blocks = LOGO.iter().filter(|row| row.contains('█')).count();
        assert_eq!(with_blocks, theme::worn().splash.logo.len());
        assert_eq!(with_blocks, theme::worn().pride.logo.len());
        assert!(LOGO[..with_blocks].iter().all(|row| row.contains('█')));
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
            paint(
                &fire,
                Duration::ZERO,
                area,
                quips::pick(3, width),
                &mut buffer,
            );
        }
    }

    #[test]
    fn the_logo_sits_above_the_flames_on_the_smallest_terminal() {
        let area = Rect::new(0, 0, 40, 20);
        let fire = Fire::new(fire_area(area), 3);
        let mut buffer = Buffer::empty(area);
        paint(&fire, Duration::ZERO, area, Some("Minecraft?"), &mut buffer);
        let row = |y: u16| (0..40).map(|x| buffer[(x, y)].symbol()).collect::<String>();
        let top = (0..20).find(|&y| row(y).contains('█')).unwrap();
        assert!(top >= 1, "the logo needs a row above it");
        // the logo, the tagline and the hint all fit above the flames
        let tagline = top + LOGO.len() as u16 + 1;
        assert!(tagline + 2 < fire_area(area).y);
        assert_eq!(row(tagline + 2).trim(), HINT);
        // 40 columns is too narrow for the spaced tagline, so the plain one is used, whole
        assert_eq!(row(tagline).trim(), NARROW_TAGLINE);
        // the quip sits between the logo and the tagline
        assert_eq!(row(tagline - 1).trim(), "Minecraft?");
    }

    #[test]
    fn the_quip_flares_between_hellfire_and_flame() {
        let [hellfire, flame] = theme::worn().splash.quip;
        assert_eq!(flare(Duration::ZERO), hellfire);
        assert_eq!(flare(QUIP_PULSE / 2), flame);
    }

    #[test]
    fn a_wide_terminal_gets_the_spaced_tagline() {
        let area = Rect::new(0, 0, 80, 24);
        let fire = Fire::new(fire_area(area), 3);
        let mut buffer = Buffer::empty(area);
        paint(&fire, Duration::ZERO, area, None, &mut buffer);
        let row = |y: u16| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>();
        assert!((0..24).any(|y| row(y).trim() == TAGLINE));
    }

    #[test]
    fn the_hint_breathes_between_ash_and_bone() {
        let [ash, bone] = theme::worn().splash.hint;
        assert_eq!(pulse(Duration::ZERO), ash);
        assert_eq!(pulse(PULSE / 2), bone);
        assert_eq!(pulse(PULSE), ash);
    }

    #[test]
    fn snapshot_a_frame_of_the_fire() {
        let area = Rect::new(0, 0, 80, 24);
        let mut fire = Fire::new(fire_area(area), 7);
        for _ in 0..40 {
            fire.step();
        }
        let snapshot = crate::testing::draw(80, 24, |page| {
            paint(
                &fire,
                Duration::ZERO,
                area,
                Some("Minecraft?"),
                page.buffer_mut(),
            )
        });
        insta::assert_snapshot!(snapshot);
    }
}
