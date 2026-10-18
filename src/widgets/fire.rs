//! The fire from the PlayStation port of Doom: a grid of heat that rises, drifts and cools,
//! drawn two pixels to a cell with half blocks.

use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

use crate::theme::{
    self,
    dressing::{mix, rgb},
    splash::Flame,
};

/// The hottest heat, white hot in the original's colors. Heat goes from 0 (none, drawn as the
/// screen's own background so there is no seam where the fire meets the rest of the screen) up to
/// this, with one color for each in the original: a shaded flame's colors are spread over them.
const HOTTEST: u8 = 36;

pub struct Fire {
    width: usize,
    height: usize,
    /// Row by row from the top; the bottom row is the source.
    heat: Vec<u8>,
    seed: u32,
    /// Heat lost per pixel risen, on average, in 256ths.
    cooling: u32,
}

impl Fire {
    /// A fire filling `area`, two pixels high per cell, burning at full heat from the bottom, its
    /// flames reaching `reach` percent of the way up. The original was made for a screen far
    /// taller than a terminal, so the built-in splash has them cool faster to keep its shape.
    pub fn new(area: Rect, seed: u32, reach: u8) -> Self {
        let (width, height) = (area.width as usize, area.height as usize * 2);
        let mut heat = vec![0; width * height];
        heat[width * height.saturating_sub(1)..].fill(HOTTEST);
        let reach = (height as u32 * u32::from(reach.clamp(1, 100)) / 100).max(1);
        Self {
            width,
            height,
            heat,
            seed: seed.max(1),
            cooling: HOTTEST as u32 * 256 / reach,
        }
    }

    /// A fire filling `area` that has already burned long enough to reach its full height, so it
    /// is at its peak from the first frame instead of climbing up from the bottom. Heat rises one
    /// pixel per [`Fire::step`], so a step for every pixel of height gets the flames to the top.
    pub fn kindled(area: Rect, seed: u32, reach: u8) -> Self {
        let mut fire = Self::new(area, seed, reach);
        for _ in 0..fire.height {
            fire.step();
        }
        fire
    }

    /// Whether this fire was made for an area of this size.
    pub fn fits(&self, area: Rect) -> bool {
        (self.width, self.height) == (area.width as usize, area.height as usize * 2)
    }

    /// xorshift: plenty random for flames, and no crate needed.
    fn random(&mut self) -> u32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed
    }

    /// How much heat a pixel loses as it rises: the average cooling, with the fraction left to chance.
    fn cool(&mut self) -> u8 {
        let chance = (self.random() & 255) < (self.cooling & 255);
        (self.cooling / 256 + u32::from(chance)).min(HOTTEST as u32) as u8
    }

    /// One tick: every pixel passes its heat to the one above, drifting sideways and cooling.
    pub fn step(&mut self) {
        let width = self.width;
        for source in width..self.heat.len() {
            let heat = self.heat[source];
            let (row, column) = (source / width, source % width);
            // 🌬️ the wind blows a little to the left, but never off the edge or into another row
            let drift = (self.random() & 3) as usize;
            let column = (column + 1).saturating_sub(drift).min(width - 1);
            let above = (row - 1) * width + column;
            self.heat[above] = if heat == 0 {
                0
            } else {
                heat.saturating_sub(self.cool())
            };
        }
    }

    /// Draws into `area` with `flames`, `elapsed` into a fire cycling through them at `speed`
    /// flames a second (see [`palette`]).
    pub fn render(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        flames: &[Flame],
        speed: f32,
        elapsed: Duration,
    ) {
        let palette = palette(flames, speed, elapsed);
        self.render_with(area, buffer, |heat| palette[usize::from(heat.min(HOTTEST))]);
    }

    /// Draws into `area`: each cell is an upper half block, its top pixel the foreground color
    /// and its bottom pixel the background, each heat colored by `color`, but no heat at all
    /// always the screen's background.
    fn render_with(&self, area: Rect, buffer: &mut Buffer, color: impl Fn(u8) -> Color) {
        let color = |heat: u8| {
            if heat == 0 {
                theme::worn().colors.background
            } else {
                color(heat)
            }
        };
        for row in 0..area.height.min((self.height / 2) as u16) {
            for column in 0..area.width.min(self.width as u16) {
                let top = self.heat[row as usize * 2 * self.width + column as usize];
                let bottom = self.heat[(row as usize * 2 + 1) * self.width + column as usize];
                buffer[(area.x + column, area.y + row)]
                    .set_symbol("▀")
                    .set_style(Style::new().fg(color(top)).bg(color(bottom)));
            }
        }
    }
}

/// The color of every heat, coldest first, `elapsed` into a fire that cycles through `flames` at
/// `speed` flames a second: the flame it is on, blended into the next as the next comes round.
fn palette(flames: &[Flame], speed: f32, elapsed: Duration) -> Vec<Color> {
    // 🌑 a background the terminal picks (`Color::Reset`) can't be blended from, so black it is
    let dark = rgb(theme::worn().colors.background).unwrap_or((0, 0, 0));
    let Some(first) = flames.first() else {
        return vec![theme::worn().colors.background; usize::from(HOTTEST) + 1];
    };
    let position = elapsed.as_secs_f32() * speed;
    let (now, next, share) = match flames.len() {
        1 => (first, first, 0.0),
        count => {
            let at = position.floor() as usize % count;
            (&flames[at], &flames[(at + 1) % count], position.fract())
        }
    };
    (0..=HOTTEST)
        .map(|heat| {
            let color = flame_color(now, heat, dark);
            let color = if share > 0.0 {
                mix(color, flame_color(next, heat, dark), share)
            } else {
                color
            };
            Color::Rgb(color.0, color.1, color.2)
        })
        .collect()
}

/// How hot the body of a glowing flame is, as a share of the hottest: below it the flame darkens
/// toward the background, above it it whitens.
const BODY: f32 = 0.75;
/// How much of the way to white the hottest pixels of a glowing flame get.
const WHITE_HOT: f32 = 0.7;

/// The color of `heat` in `flame`, over a background of `dark`. A glowing flame rises out of the
/// background, so its coolest wisps melt into it, to its full color, then toward white; a shaded
/// one goes through its colors, spread evenly from cold to hot.
fn flame_color(flame: &Flame, heat: u8, dark: (u8, u8, u8)) -> (u8, u8, u8) {
    let warmth = f32::from(heat.min(HOTTEST)) / f32::from(HOTTEST);
    match flame {
        Flame::Glow(color) => {
            let full = rgb(*color).unwrap_or(dark);
            if warmth < BODY {
                mix(dark, full, warmth / BODY)
            } else {
                mix(
                    full,
                    (255, 255, 255),
                    (warmth - BODY) / (1.0 - BODY) * WHITE_HOT,
                )
            }
        }
        Flame::Shades(shades) => {
            let last = shades.len().saturating_sub(1);
            let position = usize::from(heat.min(HOTTEST)) * last;
            let (at, rest) = (
                position / usize::from(HOTTEST),
                position % usize::from(HOTTEST),
            );
            let shade = |at: usize| shades.get(at).and_then(|&color| rgb(color)).unwrap_or(dark);
            if rest == 0 {
                shade(at)
            } else {
                mix(shade(at), shade(at + 1), rest as f32 / f32::from(HOTTEST))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(0, 0, 20, 8);

    #[test]
    fn a_kindled_fire_is_already_as_tall_as_it_gets() {
        let area = Rect::new(0, 0, 40, 12);
        let top_half_heat = |fire: &Fire| -> u32 {
            let half = fire.width * fire.height / 2;
            fire.heat[..half].iter().map(|&h| u32::from(h)).sum()
        };
        assert_eq!(top_half_heat(&Fire::new(area, 7, 80)), 0);
        let kindled = Fire::kindled(area, 7, 80);
        assert!(
            top_half_heat(&kindled) > 0,
            "the flames reach the top half already"
        );
        // and it has settled: burning on doesn't make it any taller
        let mut later = Fire::kindled(area, 7, 80);
        for _ in 0..later.height * 4 {
            later.step();
        }
        let reach = |fire: &Fire| fire.heat.iter().position(|&h| h > 0).unwrap() / fire.width;
        assert!(reach(&kindled) <= reach(&later) + 2);
    }

    fn playstation() -> Vec<Flame> {
        theme::splash::Splash::default().fire.flames
    }

    #[test]
    fn a_shaded_flame_goes_through_its_colors_from_cold_to_hot() {
        let dark = (0, 0, 0);
        let gray = Flame::Shades(vec![Color::Black, Color::White]);
        assert_eq!(flame_color(&gray, 0, dark), (0, 0, 0));
        assert_eq!(flame_color(&gray, HOTTEST, dark), (255, 255, 255));
        assert_eq!(flame_color(&gray, HOTTEST / 2, dark), (128, 128, 128));
        // the original's colors are one each, exactly as they were
        let Flame::Shades(original) = &playstation()[0] else {
            unreachable!()
        };
        for heat in 0..=HOTTEST {
            assert_eq!(
                Some(flame_color(&playstation()[0], heat, dark)),
                rgb(original[usize::from(heat)])
            );
        }
    }

    #[test]
    fn a_glowing_flame_rises_out_of_the_dark_and_goes_white_hot() {
        let red = Flame::Glow(Color::Rgb(255, 0, 0));
        assert_eq!(flame_color(&red, 0, (10, 10, 10)), (10, 10, 10));
        let (r, g, b) = flame_color(&red, HOTTEST, (0, 0, 0));
        assert_eq!(r, 255);
        assert!(g > 150 && g == b, "red going white: {g} {b}");
    }

    #[test]
    fn several_flames_take_turns_blending_into_each_other() {
        let flames = [
            Flame::Shades(vec![Color::Rgb(0, 0, 0)]),
            Flame::Shades(vec![Color::Rgb(200, 0, 0)]),
        ];
        let at = |seconds: f32| palette(&flames, 1.0, Duration::from_secs_f32(seconds))[5];
        assert_eq!(at(0.0), Color::Rgb(0, 0, 0));
        assert_eq!(at(0.5), Color::Rgb(100, 0, 0));
        assert_eq!(at(1.0), Color::Rgb(200, 0, 0));
        assert_eq!(at(2.0), Color::Rgb(0, 0, 0), "and round again");
        // standing still, or with only one, it never changes
        assert_eq!(palette(&flames, 0.0, Duration::from_secs(9))[5], at(0.0));
        assert_eq!(
            palette(&flames[1..], 3.0, Duration::from_millis(1234))[5],
            at(1.0)
        );
    }

    #[test]
    fn where_the_fire_is_cold_is_the_same_as_the_rest_of_the_screen() {
        let fire = Fire::new(AREA, 7, 80);
        let rainbow = theme::pride::Pride::default().fire.flames;
        for flames in [playstation(), rainbow] {
            let mut buffer = Buffer::empty(AREA);
            fire.render(AREA, &mut buffer, &flames, 1.0, Duration::from_millis(300));
            let top = &buffer[(0, 0)];
            let background = theme::worn().colors.background;
            assert_eq!((top.fg, top.bg), (background, background));
        }
    }

    #[test]
    fn a_new_fire_burns_only_at_the_bottom() {
        let fire = Fire::new(AREA, 7, 80);
        let width = AREA.width as usize;
        assert!(fire.heat[..fire.heat.len() - width].iter().all(|&h| h == 0));
        assert!(
            fire.heat[fire.heat.len() - width..]
                .iter()
                .all(|&h| h == HOTTEST)
        );
    }

    #[test]
    fn heat_rises_but_never_gets_hotter_than_the_palette() {
        let mut fire = Fire::new(AREA, 7, 80);
        for _ in 0..30 {
            fire.step();
        }
        let width = AREA.width as usize;
        let above_the_source = &fire.heat[..fire.heat.len() - width];
        assert!(above_the_source.iter().any(|&h| h > 0));
        assert!(fire.heat.iter().all(|&h| h <= HOTTEST));
    }

    #[test]
    fn drawing_fills_the_area_with_half_blocks() {
        let mut fire = Fire::new(AREA, 7, 80);
        fire.step();
        let mut buffer = Buffer::empty(AREA);
        fire.render(AREA, &mut buffer, &playstation(), 0.5, Duration::ZERO);
        assert!(buffer.content().iter().all(|cell| cell.symbol() == "▀"));
    }

    #[test]
    fn a_fire_knows_what_size_it_was_made_for() {
        let fire = Fire::new(AREA, 7, 80);
        assert!(fire.fits(AREA));
        assert!(!fire.fits(Rect::new(0, 0, 21, 8)));
    }

    #[test]
    fn the_flames_die_out_before_the_top_of_the_fire() {
        let area = Rect::new(0, 0, 60, 15);
        let mut fire = Fire::new(area, 11, 80);
        for _ in 0..200 {
            fire.step();
        }
        let width = area.width as usize;
        let top_row = &fire.heat[..width];
        let average = top_row.iter().map(|&h| h as u32).sum::<u32>() / width as u32;
        assert!(average < 6, "the top should be smoke, not flame: {average}");
        // and the fire still burns lower down
        let middle = &fire.heat[fire.heat.len() / 2..fire.heat.len() / 2 + width];
        assert!(middle.iter().any(|&h| h > 10));
    }

    #[test]
    fn a_tiny_area_does_not_panic() {
        let mut fire = Fire::new(Rect::new(0, 0, 1, 1), 0, 80);
        fire.step();
        fire.render(
            Rect::new(0, 0, 1, 1),
            &mut Buffer::empty(Rect::new(0, 0, 1, 1)),
            &[],
            1.0,
            Duration::ZERO,
        );
    }
}
