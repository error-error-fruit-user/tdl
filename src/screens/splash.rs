//! The first thing on screen: the title over the fire from the PlayStation port of Doom, with a
//! quip under it, the way Minecraft's title screen has one. It burns until a key is pressed.
//!
//! How it looks comes from the splash being worn (`splash.toml`, see [`theme::splash`]), and with
//! pride from a flag picked out of `pride.toml` (see [`theme::pride`]).

use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyModifiers},
    layout::Rect,
    style::{Color, Modifier, Style},
};

use crate::{
    error::{Error, Result},
    gamepad, music, quips,
    ritual::{Grimoire, Haunted},
    theme::{
        self,
        dressing::blend,
        pride::Pride,
        splash::{Flame, Splash},
    },
    widgets::fire::Fire,
};

/// What the logo's art paints in its colors; any other character is its shadow.
const BLOCK: char = '█';
/// One frame of the animation.
const FRAME: Duration = Duration::from_millis(33);

/// How one showing of the splash looks: the splash being worn, with the logo's stripes and the
/// fire from a pride flag when pride is worn.
struct Look<'a> {
    splash: &'a Splash,
    stripes: &'a [Color],
    flames: &'a [Flame],
    speed: f32,
}

impl<'a> Look<'a> {
    /// `splash`, with `pride`'s flag picked by `seed` if pride is worn (`Some`). A flag without a
    /// fire of its own burns the pride fire; with no flags at all, only the fire is pride's.
    fn of(splash: &'a Splash, pride: Option<&'a Pride>, seed: u32) -> Self {
        let mut look = Self {
            splash,
            stripes: &splash.logo.colors,
            flames: &splash.fire.flames,
            speed: splash.fire.speed,
        };
        if let Some(pride) = pride {
            // 🏳️‍🌈 the flag is picked apart from the quip, though both come from the same seed
            let flag = pride.pick(seed.rotate_left(16));
            let fire = flag
                .and_then(|flag| flag.fire.as_ref())
                .unwrap_or(&pride.fire);
            look.flames = &fire.flames;
            look.speed = fire.speed;
            if let Some(flag) = flag {
                look.stripes = &flag.stripes;
            }
        }
        look
    }
}

/// Plays the splash, fading in from black, until a key is pressed, then fades it to black along
/// with the music. `quip` (from `--splash-quip` or the settings) replaces the usual random
/// one, which is left out when the quips are hidden.
/// Ctrl+C still quits.
pub fn summon(grimoire: &mut Grimoire, quip: Option<&str>) -> Result<()> {
    music::start(false);
    gamepad::start();
    let seed = quips::seed();
    let pride = theme::pride().then(theme::pride::worn);
    let look = Look::of(theme::splash::worn(), pride, seed);
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
            let flames = fire_area(area, look.splash);
            // 🔥 a new fire whenever the terminal changes size, since the old one no longer fits,
            // kindled first so it is roaring from the first frame rather than climbing up
            let fire = match &mut fire {
                Some(fire) if fire.fits(flames) => fire,
                _ => fire.insert(Fire::kindled(flames, seed, look.splash.fire.reach)),
            };
            fire.step();
            let quip = match quip {
                Some(quip) => quips::fit(quip, area.width),
                None if quips::shown() => quips::pick(seed, area.width).map(str::to_string),
                None => None,
            };
            paint(
                &look,
                fire,
                elapsed,
                area,
                quip.as_deref(),
                page.buffer_mut(),
            );
            dim(page.buffer_mut(), brightness);
        })?;
        // 🎮 any button of a controller's ends it as a key's does, even a bumper that waits to be
        // a chord's
        if gamepad::press().is_some() && leaving.is_none() {
            music::splash_over();
            leaving = Some(Instant::now());
        }
        if event::poll(FRAME)? {
            match event::read()? {
                Event::Key(key)
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    return Err(Error::Interrupted);
                }
                Event::Key(key) if key.is_press() && leaving.is_none() => {
                    gamepad::keyboard_used();
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

/// Where the fire burns: the bottom part of the screen from the splash's `top` down, starting low
/// enough on a short terminal to leave a row above the logo, then the logo, the tagline and the
/// hint with a blank row after each.
fn fire_area(area: Rect, splash: &Splash) -> Rect {
    let headroom = splash.logo.art.len() as u16 + 6;
    let top = (area.height * u16::from(splash.fire.top) / 100)
        .max(headroom)
        .min(area.height);
    Rect::new(area.x, area.y + top, area.width, area.height - top)
}

/// Draws one frame: the fire, then the logo, the quip, the tagline and the hint just above the
/// flames. `elapsed` is how long the splash has been up, which sets where the fire is in its
/// cycle and the hint and the quip in their pulses.
fn paint(
    look: &Look,
    fire: &Fire,
    elapsed: Duration,
    area: Rect,
    quip: Option<&str>,
    buffer: &mut Buffer,
) {
    let splash = look.splash;
    let flames = fire_area(area, splash);
    fire.render(flames, buffer, look.flames, look.speed, elapsed);

    let art = &splash.logo.art;
    let logo_width = splash.logo.width() as u16;
    let logo_x = area.x + area.width.saturating_sub(logo_width) / 2;
    // the logo, a blank row, the tagline, a blank row, the hint, then the flames
    let logo_y = flames.y.saturating_sub(art.len() as u16 + 4).max(area.y);
    let shadow = Style::new().fg(splash.logo.shadow);
    let painted = Painted::of(art, look.stripes);
    for (i, row) in art.iter().enumerate() {
        for (j, character) in row.chars().enumerate() {
            let (x, y) = (logo_x + j as u16, logo_y + i as u16);
            if character != BLOCK {
                stamp(buffer, area, x, y, character, shadow);
                continue;
            }
            // 🎨 two stripes to a row: a half block with the top's color over the bottom's,
            // or a whole one where they are the same
            let (top, bottom) = painted.colors(i);
            if top == bottom {
                stamp(buffer, area, x, y, BLOCK, Style::new().fg(top));
            } else {
                stamp(buffer, area, x, y, '▀', Style::new().fg(top).bg(bottom));
            }
        }
    }

    // ✨ the quip sits centered between the logo and the tagline
    if let Some(quip) = quip {
        let x = area.x + area.width.saturating_sub(quip.chars().count() as u16) / 2;
        let mut style = Style::new().fg(flare(splash, elapsed));
        if splash.quip.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        write(buffer, area, x, logo_y + art.len() as u16, quip, style);
    }

    let tagline = &splash.tagline;
    let words = if tagline.text.chars().count() as u16 <= area.width {
        &tagline.text
    } else {
        &tagline.narrow
    };
    let tagline_x = area.x + area.width.saturating_sub(words.chars().count() as u16) / 2;
    let tagline_y = logo_y + art.len() as u16 + 1;
    let mut style = Style::new().fg(tagline.color);
    if tagline.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    write(buffer, area, tagline_x, tagline_y, words, style);

    let hint = hint_words(splash, gamepad::shown());
    let hint = hint.as_ref();
    let hint_x = area.x + area.width.saturating_sub(hint.chars().count() as u16) / 2;
    let style = Style::new().fg(pulse(splash, elapsed));
    write(buffer, area, hint_x, tagline_y + 2, hint, style);
}

/// The words under the tagline: the hint from the splash file, or when a controller is recognized
/// (`family`) `Hit a Button to Start`, since any of its buttons ends the splash.
fn hint_words(splash: &Splash, family: Option<gamepad::Family>) -> Cow<'_, str> {
    match family {
        Some(_) => Cow::Borrowed(PAD_HINT),
        None => Cow::Borrowed(&splash.hint.text),
    }
}

/// What the hint says while a controller is recognized.
const PAD_HINT: &str = "Hit a Button to Start";

/// Which stripe each half row of the logo is: the stripes spread evenly over the rows from the
/// first with a block in it to the last, two half rows to a row.
struct Painted<'a> {
    stripes: &'a [Color],
    /// The first and the last row with a block in it.
    first: usize,
    last: usize,
}

impl<'a> Painted<'a> {
    fn of(art: &[String], stripes: &'a [Color]) -> Self {
        let has_block = |row: &&String| row.contains(BLOCK);
        let first = art.iter().position(|row| has_block(&row)).unwrap_or(0);
        let last = art.iter().rposition(|row| has_block(&row)).unwrap_or(0);
        Self {
            stripes,
            first,
            last,
        }
    }

    /// The colors of the top and the bottom half of row `row`.
    fn colors(&self, row: usize) -> (Color, Color) {
        let halves = 2 * (self.last.saturating_sub(self.first) + 1);
        let half = 2 * row.saturating_sub(self.first);
        let stripe = |half: usize| {
            let at = (half.min(halves - 1) * self.stripes.len() / halves)
                .min(self.stripes.len().saturating_sub(1));
            self.stripes.get(at).copied().unwrap_or(Color::Reset)
        };
        (stripe(half), stripe(half + 1))
    }
}

/// The hint's color at `elapsed`: a slow breath between its two (ash and bone, built in).
fn pulse(splash: &Splash, elapsed: Duration) -> Color {
    let [from, to] = splash.hint.colors;
    breathe(elapsed, splash.hint.period(), from, to)
}

/// The quip's color at `elapsed`: a quick flare between its two (hellfire and the yellow of the
/// flame tips, built in).
fn flare(splash: &Splash, elapsed: Duration) -> Color {
    let [from, to] = splash.quip.colors;
    breathe(elapsed, splash.quip.period(), from, to)
}

/// Goes from `from` to `to` and back once every `period`.
fn breathe(elapsed: Duration, period: Duration, from: Color, to: Color) -> Color {
    let period = period.as_millis().max(1);
    let phase = (elapsed.as_millis() % period) as f32 / period as f32;
    let brightness = 1.0 - (phase * 2.0 - 1.0).abs();
    blend(from, to, brightness)
}

/// Writes `text` from `x` on, a character a cell.
fn write(buffer: &mut Buffer, area: Rect, x: u16, y: u16, text: &str, style: Style) {
    for (j, character) in text.chars().enumerate() {
        stamp(
            buffer,
            area,
            x.saturating_add(j as u16),
            y,
            character,
            style,
        );
    }
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

    fn built_in() -> Look<'static> {
        static SPLASH: std::sync::LazyLock<Splash> = std::sync::LazyLock::new(Splash::default);
        Look::of(&SPLASH, None, 0)
    }

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
    fn five_stripes_over_five_rows_of_blocks_are_a_stripe_a_row() {
        let look = built_in();
        let art = &look.splash.logo.art;
        let width = art[0].chars().count();
        assert!(art.iter().all(|row| row.chars().count() == width));
        let painted = Painted::of(art, look.stripes);
        for (row, &stripe) in look.stripes.iter().enumerate() {
            assert_eq!(painted.colors(row), (stripe, stripe));
        }
        assert_eq!(look.stripes.len(), 5);
    }

    #[test]
    fn more_stripes_than_rows_split_rows_in_half_and_fewer_spread_out() {
        let art: Vec<String> = ["██", "██", "██"].map(String::from).to_vec();
        let six: Vec<Color> = (0..6).map(Color::Indexed).collect();
        let painted = Painted::of(&art, &six);
        assert_eq!(painted.colors(0), (six[0], six[1]));
        assert_eq!(painted.colors(2), (six[4], six[5]));
        let two = [Color::Red, Color::Blue];
        let painted = Painted::of(&art, &two);
        assert_eq!(painted.colors(0), (Color::Red, Color::Red));
        assert_eq!(painted.colors(1), (Color::Red, Color::Blue));
        assert_eq!(painted.colors(2), (Color::Blue, Color::Blue));
    }

    #[test]
    fn a_pride_flag_brings_its_stripes_and_the_pride_fire() {
        let splash = Splash::default();
        let pride = Pride::default();
        let look = Look::of(&splash, Some(&pride), 12345);
        assert_eq!(look.stripes, &pride.flags[0].stripes[..]);
        assert_eq!(look.flames, &pride.fire.flames[..]);
        // with no flags, the logo keeps its colors and only the fire is pride's
        let flagless = Pride {
            flags: Vec::new(),
            ..Pride::default()
        };
        let look = Look::of(&splash, Some(&flagless), 1);
        assert_eq!(look.stripes, &splash.logo.colors[..]);
        assert_eq!(look.flames, &pride.fire.flames[..]);
    }

    #[test]
    fn the_fire_burns_in_the_bottom_part_of_the_screen() {
        let splash = Splash::default();
        assert_eq!(
            fire_area(Rect::new(0, 0, 80, 40), &splash),
            Rect::new(0, 18, 80, 22)
        );
        // a short terminal keeps room for the logo above the flames
        assert_eq!(
            fire_area(Rect::new(0, 0, 80, 20), &splash),
            Rect::new(0, 12, 80, 8)
        );
    }

    #[test]
    fn a_frame_draws_at_every_size_without_panicking() {
        let look = built_in();
        for (width, height) in [(40, 20), (80, 24), (200, 60), (10, 4), (1, 1)] {
            let area = Rect::new(0, 0, width, height);
            let mut fire = Fire::new(fire_area(area, look.splash), 3, 80);
            fire.step();
            let mut buffer = Buffer::empty(area);
            paint(
                &look,
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
        let look = built_in();
        let area = Rect::new(0, 0, 40, 20);
        let fire = Fire::new(fire_area(area, look.splash), 3, 80);
        let mut buffer = Buffer::empty(area);
        paint(
            &look,
            &fire,
            Duration::ZERO,
            area,
            Some("Minecraft?"),
            &mut buffer,
        );
        let row = |y: u16| (0..40).map(|x| buffer[(x, y)].symbol()).collect::<String>();
        let top = (0..20).find(|&y| row(y).contains(BLOCK)).unwrap();
        assert!(top >= 1, "the logo needs a row above it");
        // the logo, the tagline and the hint all fit above the flames
        let tagline = top + look.splash.logo.art.len() as u16 + 1;
        assert!(tagline + 2 < fire_area(area, look.splash).y);
        assert_eq!(row(tagline + 2).trim(), look.splash.hint.text);
        // 40 columns is too narrow for the spaced tagline, so the plain one is used, whole
        assert_eq!(row(tagline).trim(), look.splash.tagline.narrow);
        // the quip sits between the logo and the tagline
        assert_eq!(row(tagline - 1).trim(), "Minecraft?");
    }

    #[test]
    fn the_quip_flares_and_the_hint_breathes_between_their_colors() {
        let splash = Splash::default();
        let [hellfire, flame] = splash.quip.colors;
        assert_eq!(flare(&splash, Duration::ZERO), hellfire);
        assert_eq!(flare(&splash, splash.quip.period() / 2), flame);
        let [ash, bone] = splash.hint.colors;
        assert_eq!(pulse(&splash, Duration::ZERO), ash);
        assert_eq!(pulse(&splash, splash.hint.period() / 2), bone);
        assert_eq!(pulse(&splash, splash.hint.period()), ash);
    }

    #[test]
    fn a_wide_terminal_gets_the_spaced_tagline() {
        let look = built_in();
        let area = Rect::new(0, 0, 80, 24);
        let fire = Fire::new(fire_area(area, look.splash), 3, 80);
        let mut buffer = Buffer::empty(area);
        paint(&look, &fire, Duration::ZERO, area, None, &mut buffer);
        let row = |y: u16| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>();
        assert!((0..24).any(|y| row(y).trim() == look.splash.tagline.text));
    }

    #[test]
    fn a_recognized_controller_is_told_to_hit_a_button() {
        let look = built_in();
        use gamepad::Family::*;
        assert_eq!(hint_words(look.splash, None), look.splash.hint.text);
        for family in [
            Xbox,
            PlayStation,
            Nintendo,
            SteamDeck,
            SteamController,
            Generic,
        ] {
            assert_eq!(
                hint_words(look.splash, Some(family)),
                "Hit a Button to Start"
            );
        }
    }

    #[test]
    fn snapshot_a_frame_of_the_fire() {
        let look = built_in();
        let area = Rect::new(0, 0, 80, 24);
        let mut fire = Fire::new(fire_area(area, look.splash), 7, 80);
        for _ in 0..40 {
            fire.step();
        }
        let snapshot = crate::testing::draw(80, 24, |page| {
            paint(
                &look,
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
