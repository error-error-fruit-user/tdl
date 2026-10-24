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
