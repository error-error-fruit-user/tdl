//! The fire from the PlayStation port of Doom: a grid of heat that rises, drifts and cools,
//! drawn two pixels to a cell with half blocks.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

use crate::theme;

/// From cold to white hot, as in the original. Heat is an index into this. No heat at all is
/// drawn as the screen's own background instead of the first entry, so there is no seam where the
/// fire meets the rest of the screen.
const PALETTE: [(u8, u8, u8); 37] = [
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
const HOTTEST: u8 = PALETTE.len() as u8 - 1;
/// How far up the flames reach, as a share of the fire's height. The original was made for a
/// screen far taller than a terminal, so the fire cools faster here to keep its shape.
const REACH_PERCENT: u32 = 80;

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
    /// A fire filling `area`, two pixels high per cell, burning at full heat from the bottom.
    pub fn new(area: Rect, seed: u32) -> Self {
        let (width, height) = (area.width as usize, area.height as usize * 2);
        let mut heat = vec![0; width * height];
        heat[width * height.saturating_sub(1)..].fill(HOTTEST);
        let reach = (height as u32 * REACH_PERCENT / 100).max(1);
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
    pub fn kindled(area: Rect, seed: u32) -> Self {
        let mut fire = Self::new(area, seed);
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

    /// Draws into `area` in the original's colors.
    pub fn render(&self, area: Rect, buffer: &mut Buffer) {
        self.render_with(area, buffer, |heat| {
            let (r, g, b) = PALETTE[heat.min(HOTTEST) as usize];
            Color::Rgb(r, g, b)
        });
    }

    /// Draws into `area` in the colors of the rainbow: `hue` (0 to 1, round the color wheel from
    /// red) for the body of the flames, the cooler wisps a little further round the wheel, and the
    /// hottest going white. Turning `hue` over time takes the fire through every color.
    pub fn render_rainbow(&self, area: Rect, buffer: &mut Buffer, hue: f32) {
        self.render_with(area, buffer, |heat| rainbow(heat, hue));
    }

    /// Draws into `area`: each cell is an upper half block, its top pixel the foreground color
    /// and its bottom pixel the background, each heat colored by `color`, but no heat at all
    /// always the screen's background.
    fn render_with(&self, area: Rect, buffer: &mut Buffer, color: impl Fn(u8) -> Color) {
        let color = |heat: u8| {
            if heat == 0 {
                theme::BACKGROUND
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

/// How hot the body of the flames is, as a share of the hottest: below it they darken toward
/// black, above it they whiten.
const BODY: f32 = 0.75;
/// How much of the way to white the hottest pixels get.
const WHITE_HOT: f32 = 0.7;
/// How far round the color wheel the coldest wisps are from the body, so the flames show a few
/// colors at once.
const WISPS: f32 = 0.15;

/// The color of `heat` in a fire of `hue`: rising out of the screen's background, so the coolest
/// wisps melt into it, to the full color, then toward white.
fn rainbow(heat: u8, hue: f32) -> Color {
    let warmth = f32::from(heat.min(HOTTEST)) / f32::from(HOTTEST);
    let (r, g, b) = full_color(hue + (1.0 - warmth) * WISPS);
    let (light, white) = if warmth < BODY {
        (warmth / BODY, 0.0)
    } else {
        (1.0, (warmth - BODY) / (1.0 - BODY) * WHITE_HOT)
    };
    // 🌑 a background the terminal picks (`Color::Reset`) can't be blended from, so black it is
    let Color::Rgb(dark_r, dark_g, dark_b) = theme::BACKGROUND else {
        return rainbow_over((0, 0, 0), (r, g, b), light, white);
    };
    rainbow_over((dark_r, dark_g, dark_b), (r, g, b), light, white)
}

/// `color` (each channel 0 to 1) risen `light` of the way out of `dark`, then `white` of the rest
/// of the way to white.
fn rainbow_over(dark: (u8, u8, u8), color: (f32, f32, f32), light: f32, white: f32) -> Color {
    let channel = |dark: u8, c: f32| {
        let lit = f32::from(dark) / 255.0 * (1.0 - light) + c * light;
        ((lit + (1.0 - lit) * white) * 255.0) as u8
    };
    Color::Rgb(
        channel(dark.0, color.0),
        channel(dark.1, color.1),
        channel(dark.2, color.2),
    )
}

/// The brightest, fullest color at `hue` on the color wheel (0 and 1 are red), each channel 0 to 1.
fn full_color(hue: f32) -> (f32, f32, f32) {
    let sixth = hue.rem_euclid(1.0) * 6.0;
    let rising = sixth.fract();
    let falling = 1.0 - rising;
    match sixth as u8 {
        0 => (1.0, rising, 0.0),
        1 => (falling, 1.0, 0.0),
        2 => (0.0, 1.0, rising),
        3 => (0.0, falling, 1.0),
        4 => (rising, 0.0, 1.0),
        _ => (1.0, 0.0, falling),
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
        assert_eq!(top_half_heat(&Fire::new(area, 7)), 0);
        let kindled = Fire::kindled(area, 7);
        assert!(
            top_half_heat(&kindled) > 0,
            "the flames reach the top half already"
        );
        // and it has settled: burning on doesn't make it any taller
        let mut later = Fire::kindled(area, 7);
        for _ in 0..later.height * 4 {
            later.step();
        }
        let reach = |fire: &Fire| fire.heat.iter().position(|&h| h > 0).unwrap() / fire.width;
        assert!(reach(&kindled) <= reach(&later) + 2);
    }

    #[test]
    fn the_rainbow_goes_round_the_wheel_and_from_dark_to_white_hot() {
        assert_eq!(full_color(0.0), (1.0, 0.0, 0.0));
        assert_eq!(full_color(1.0 / 3.0), (0.0, 1.0, 0.0));
        assert_eq!(full_color(2.0 / 3.0), (0.0, 0.0, 1.0));
        assert_eq!(full_color(1.0), full_color(0.0));
        // the coldest melts into the background, where the fire draws nothing at all
        assert_eq!(rainbow(0, 0.5), theme::BACKGROUND);
        let Color::Rgb(r, g, b) = rainbow(HOTTEST, 0.0) else {
            unreachable!()
        };
        assert_eq!(r, 255);
        assert!(g > 150 && g == b, "red going white: {g} {b}");
    }

    #[test]
    fn where_the_fire_is_cold_is_the_same_as_the_rest_of_the_screen() {
        let fire = Fire::new(AREA, 7);
        for rainbow in [false, true] {
            let mut buffer = Buffer::empty(AREA);
            if rainbow {
                fire.render_rainbow(AREA, &mut buffer, 0.3);
            } else {
                fire.render(AREA, &mut buffer);
            }
            let top = &buffer[(0, 0)];
            assert_eq!((top.fg, top.bg), (theme::BACKGROUND, theme::BACKGROUND));
        }
    }

    #[test]
    fn a_new_fire_burns_only_at_the_bottom() {
        let fire = Fire::new(AREA, 7);
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
        let mut fire = Fire::new(AREA, 7);
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
        let mut fire = Fire::new(AREA, 7);
        fire.step();
        let mut buffer = Buffer::empty(AREA);
        fire.render(AREA, &mut buffer);
        assert!(buffer.content().iter().all(|cell| cell.symbol() == "▀"));
    }

    #[test]
    fn a_fire_knows_what_size_it_was_made_for() {
        let fire = Fire::new(AREA, 7);
        assert!(fire.fits(AREA));
        assert!(!fire.fits(Rect::new(0, 0, 21, 8)));
    }

    #[test]
    fn the_flames_die_out_before_the_top_of_the_fire() {
        let area = Rect::new(0, 0, 60, 15);
        let mut fire = Fire::new(area, 11);
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
        let mut fire = Fire::new(Rect::new(0, 0, 1, 1), 0);
        fire.step();
        fire.render(
            Rect::new(0, 0, 1, 1),
            &mut Buffer::empty(Rect::new(0, 0, 1, 1)),
        );
    }
}
