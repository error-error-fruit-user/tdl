//! How the splash screen looks, from `splash.toml`: the logo's art and colors, the tagline, the
//! hint and the quip, and the fire. The built-in one is the file written out by the settings
//! ([`TEMPLATE`]), whose every value is the one below. The pride splash's logo and fire are in
//! [`super::pride`].

use std::time::Duration;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use super::{
    ASH, BLOOD, BONE, FLAME, GORE, HELLFIRE,
    dressing::{self, Dressing, Wardrobe},
};

/// What a new `splash.toml` starts as: the built-in splash, explained.
pub const TEMPLATE: &str = include_str!("../../assets/splash.toml");
/// The `void` splash that comes with tdl.
const VOID: &str = include_str!("../../assets/void_splash.toml");

/// The fire of the PlayStation port of Doom, from cold to white hot.
const PLAYSTATION: [(u8, u8, u8); 37] = [
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

/// The most rows and columns a logo may have.
pub const MAX_ART_ROWS: usize = 16;
pub const MAX_ART_WIDTH: usize = 120;
/// The longest the tagline and the hint may be.
const MAX_WORDS: usize = 120;

/// The whole splash screen. Every part has a default (the built-in splash's), so a `splash.toml`
/// only needs what it changes.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Splash {
    pub logo: Logo,
    pub tagline: Tagline,
    pub hint: Hint,
    pub quip: Quip,
    pub fire: Fire,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Logo {
    /// A row a line: full blocks (`█`) in [`Logo::colors`], anything else in the shadow, spaces
    /// left to the fire.
    pub art: Vec<String>,
    /// Stripes from top to bottom, spread over the rows with blocks in them, two to a row.
    pub colors: Vec<Color>,
    pub shadow: Color,
}

impl Default for Logo {
    fn default() -> Self {
        Self {
            art: [
                "████████╗██████╗ ██╗     ",
                "╚══██╔══╝██╔══██╗██║     ",
                "   ██║   ██║  ██║██║     ",
                "   ██║   ██║  ██║██║     ",
                "   ██║   ██████╔╝███████╗",
                "   ╚═╝   ╚═════╝ ╚══════╝",
            ]
            .map(String::from)
            .to_vec(),
            // white hot at the top, cooling a step a row down to blood at the bottom
            colors: vec![
                FLAME,
                Color::Rgb(255, 156, 43),
                HELLFIRE,
                Color::Rgb(220, 69, 17),
                BLOOD,
            ],
            shadow: GORE,
        }
    }
}

impl Logo {
    /// How many columns wide the art is.
    pub fn width(&self) -> usize {
        self.art
            .iter()
            .map(|row| row.chars().count())
            .max()
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tagline {
    pub text: String,
    /// For a terminal too narrow for [`Tagline::text`].
    pub narrow: String,
    pub color: Color,
    pub bold: bool,
}

impl Default for Tagline {
    fn default() -> Self {
        Self {
            text: "t e r m i n a l   d o o m   l a u n c h e r".into(),
            narrow: "terminal doom launcher".into(),
            color: BONE,
            bold: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Hint {
    pub text: String,
    /// It breathes from the first to the second and back.
    pub colors: [Color; 2],
    /// Seconds for one breath.
    pub period: f32,
}

impl Default for Hint {
    fn default() -> Self {
        Self {
            text: "press any key".into(),
            colors: [ASH, BONE],
            period: 1.6,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Quip {
    /// It flares from the first to the second and back.
    pub colors: [Color; 2],
    /// Seconds for one flare.
    pub period: f32,
    pub bold: bool,
}

impl Default for Quip {
    fn default() -> Self {
        Self {
            colors: [HELLFIRE, FLAME],
            period: 0.7,
            bold: true,
        }
    }
}

/// One kind of flame the fire burns with.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Flame {
    /// Rises out of the background into this color, then whitens where it is hottest.
    Glow(Color),
    /// Goes through these colors from coldest to hottest.
    Shades(Vec<Color>),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Fire {
    /// Cycled through in order, each blending into the next.
    pub flames: Vec<Flame>,
    /// Flames a second; 0 stays on the first.
    pub speed: f32,
    /// How far up the flames reach, as a percentage of the fire's height.
    pub reach: u8,
    /// Where the fire starts, as a percentage of the way down the screen.
    pub top: u8,
}

impl Default for Fire {
    fn default() -> Self {
        Self {
            flames: vec![Flame::Shades(
                PLAYSTATION
                    .iter()
                    .map(|&(r, g, b)| Color::Rgb(r, g, b))
                    .collect(),
            )],
            speed: 0.5,
            reach: 80,
            top: 45,
        }
    }
}

/// What is wrong with flames and their speed, if anything, for `splash.toml` and `pride.toml`.
pub fn check_flames(flames: &[Flame], speed: f32) -> Result<(), String> {
    if flames.is_empty() {
        return Err("the fire needs at least one flame".into());
    }
    if flames
        .iter()
        .any(|flame| matches!(flame, Flame::Shades(shades) if shades.is_empty()))
    {
        return Err("a shaded flame needs at least one color".into());
    }
    if !speed.is_finite() || speed < 0.0 {
        return Err("the fire's speed is a number of flames a second, 0 or more".into());
    }
    Ok(())
}

/// Whether `period` is a sensible number of seconds for something to pulse in.
fn is_period(period: f32) -> bool {
    period.is_finite() && (0.05..=60.0).contains(&period)
}

impl Dressing for Splash {
    const WHAT: &str = "splash";
    const FILE: &str = "splash.toml";
    const SUFFIX: &str = "_splash.toml";
    const BUNDLED: &[&str] = &["hellfire", "void"];
    const UPGRADES: &[fn(&mut toml::Table)] = &[];

    fn template() -> String {
        TEMPLATE.to_string()
    }

    /// `hellfire` is the built-in splash, and `void` a white fire with violet and teal embers.
    fn bundled(name: &str) -> Option<String> {
        match name {
            "hellfire" => Some(TEMPLATE.to_string()),
            "void" => Some(VOID.to_string()),
            _ => None,
        }
    }

    fn check(&self) -> Result<(), String> {
        let art = &self.logo.art;
        if art.is_empty() || art.len() > MAX_ART_ROWS || self.logo.width() > MAX_ART_WIDTH {
            return Err(format!(
                "the logo's art is 1 to {MAX_ART_ROWS} rows of up to {MAX_ART_WIDTH} characters"
            ));
        }
        let words = [&self.tagline.text, &self.tagline.narrow, &self.hint.text];
        if !art
            .iter()
            .chain(words)
            .all(|text| dressing::is_printable(text))
            || words.iter().any(|text| text.chars().count() > MAX_WORDS)
        {
            return Err(format!(
                "the logo, the tagline and the hint can't hold control characters, and the \
                 tagline and the hint are at most {MAX_WORDS} characters"
            ));
        }
        if self.logo.colors.is_empty() {
            return Err("the logo needs at least one color".into());
        }
        if !is_period(self.hint.period) || !is_period(self.quip.period) {
            return Err("a period is a number of seconds from 0.05 to 60".into());
        }
        check_flames(&self.fire.flames, self.fire.speed)?;
        if !(1..=100).contains(&self.fire.reach) || self.fire.top > 90 {
            return Err("the fire's reach is 1 to 100, and its top 0 to 90".into());
        }
        Ok(())
    }
}

/// How long one breath of the hint takes.
impl Hint {
    pub fn period(&self) -> Duration {
        Duration::from_secs_f32(self.period)
    }
}

impl Quip {
    pub fn period(&self) -> Duration {
        Duration::from_secs_f32(self.period)
    }
}

static WARDROBE: Wardrobe<Splash> = Wardrobe::new();

/// The splash being worn.
pub fn worn() -> &'static Splash {
    WARDROBE.worn()
}

/// Wears `splash` for the rest of the run, or until another is worn.
pub fn wear(splash: Splash) {
    WARDROBE.wear(splash);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_written_splash_is_the_built_in_one() {
        assert_eq!(Splash::parse(TEMPLATE), Ok(Splash::default()));
        assert!(Splash::template().contains("\nedition = 1\n"));
        assert_eq!(Splash::parse(""), Ok(Splash::default()));
    }

    #[test]
    fn a_splash_only_needs_what_it_changes_and_flames_glow_or_are_shaded() {
        let splash = Splash::parse(
            "[logo]\nart = [\"██ ██\", \"▀▀ ▀▀\"]\ncolors = [\"red\"]\n\
             [fire]\nflames = [\"#FF7010\", [\"black\", \"white\"]]\nspeed = 2",
        )
        .unwrap();
        assert_eq!(splash.logo.art.len(), 2);
        assert_eq!(splash.logo.colors, [Color::Red]);
        assert_eq!(
            splash.fire.flames,
            [
                Flame::Glow(Color::Rgb(255, 112, 16)),
                Flame::Shades(vec![Color::Black, Color::White])
            ]
        );
        assert_eq!(splash.fire.speed, 2.0);
        // everything else is the built-in splash's
        assert_eq!(splash.tagline, Tagline::default());
        assert_eq!(splash.fire.reach, 80);
    }

    #[test]
    fn the_void_splash_burns_white_with_violet_and_teal() {
        let altar = crate::loadout::scribe::tests::Altar::new("splash-void");
        let void = Splash::load("void".as_ref(), &altar.0).unwrap();
        assert_eq!(void.fire.flames.len(), 2);
        for flame in &void.fire.flames {
            let Flame::Shades(shades) = flame else {
                panic!("a shaded flame")
            };
            assert_eq!(shades.last(), Some(&Color::Rgb(255, 255, 255)), "white hot");
        }
        assert_eq!(
            Splash::load("hellfire".as_ref(), &altar.0),
            Ok(Splash::default())
        );
    }

    #[test]
    fn a_splash_with_a_mistake_says_what_it_is() {
        for wrong in [
            "[logo]\nart = []",
            "[logo]\ncolors = []",
            "[logo]\nart = [\"\\u001b[2J\"]",
            "[hint]\nperiod = 0",
            "[quip]\nperiod = -1",
            "[fire]\nflames = []",
            "[fire]\nflames = [[]]",
            "[fire]\nspeed = -1",
            "[fire]\nreach = 0",
            "[fire]\ntop = 95",
            "[fire]\nflames = [\"not a color\"]",
            "[hint]\ncolors = [\"red\"]",
            "[tagline]\ntxt = \"hi\"",
            "edition = 2",
        ] {
            assert!(Splash::parse(wrong).is_err(), "{wrong}");
        }
        assert!(
            Splash::parse("edition = 2")
                .unwrap_err()
                .contains("newer tdl")
        );
    }
}
