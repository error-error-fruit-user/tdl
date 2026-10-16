//! The fire from the PlayStation port of Doom: a grid of heat that rises, drifts and cools,
//! drawn two pixels to a cell with half blocks.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};

/// From cold to white hot, as in the original. Heat is an index into this.
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

    /// Draws into `area`: each cell is an upper half block, its top pixel the foreground color
    /// and its bottom pixel the background.
    pub fn render(&self, area: Rect, buffer: &mut Buffer) {
        let color = |heat: u8| {
            let (r, g, b) = PALETTE[heat.min(HOTTEST) as usize];
            Color::Rgb(r, g, b)
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
