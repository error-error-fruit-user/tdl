//! How the pride splash looks, from `pride.toml`: the flags whose stripes the logo can wear, one
//! picked at random each time, and the fire behind them. The built-in one is the file written out
//! by the settings ([`TEMPLATE`]): the transgender flag, with every other flag in it left in a
//! comment, over a fire that burns through the rainbow.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use super::{
    dressing::{self, Dressing, Wardrobe},
    splash::{self, Flame},
};

/// What a new `pride.toml` starts as: the built-in pride, with the other flags to put in.
pub const TEMPLATE: &str = include_str!("../../assets/pride.toml");

/// The most flags a `pride.toml` may have.
const MAX_FLAGS: usize = 256;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Pride {
    /// Behind every flag without a fire of its own.
    pub fire: PrideFire,
    /// One is picked at random each time; with none, the usual logo keeps its colors.
    #[serde(rename = "flag")]
    pub flags: Vec<Flag>,
}

impl Default for Pride {
    fn default() -> Self {
        let rgb = |hex: u32| Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
        Self {
            fire: PrideFire::default(),
            flags: vec![Flag {
                name: "transgender".into(),
                stripes: [0x5BCEFA, 0xF5A9B8, 0xFFFFFF, 0xF5A9B8, 0x5BCEFA]
                    .map(rgb)
                    .to_vec(),
                fire: None,
            }],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PrideFire {
    pub flames: Vec<Flame>,
    /// Flames a second; 0 stays on the first.
    pub speed: f32,
}

impl Default for PrideFire {
    fn default() -> Self {
        Self {
            flames: [
                (255, 0, 0),
                (255, 128, 0),
                (255, 255, 0),
                (0, 255, 0),
                (0, 128, 255),
                (128, 0, 255),
            ]
            .map(|(r, g, b)| Flame::Glow(Color::Rgb(r, g, b)))
            .to_vec(),
            speed: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Flag {
    /// Only for whoever reads the file.
    #[serde(default)]
    pub name: String,
    /// Top to bottom, spread evenly over the logo.
    pub stripes: Vec<Color>,
    /// In place of the pride fire, for this flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire: Option<PrideFire>,
}

impl Pride {
    /// The flag to wear this time, picked by `seed`, or `None` with no flags.
    pub fn pick(&self, seed: u32) -> Option<&Flag> {
        (!self.flags.is_empty()).then(|| &self.flags[seed as usize % self.flags.len()])
    }
}

impl Dressing for Pride {
    const WHAT: &str = "pride flags";
    const FILE: &str = "pride.toml";
    const SUFFIX: &str = "_pride.toml";
    const UPGRADES: &[fn(&mut toml::Table)] = &[];

    fn template() -> String {
        TEMPLATE.to_string()
    }

    fn check(&self) -> Result<(), String> {
        splash::check_flames(&self.fire.flames, self.fire.speed)?;
        if self.flags.len() > MAX_FLAGS {
            return Err(format!("there can be at most {MAX_FLAGS} flags"));
        }
        for flag in &self.flags {
            if flag.stripes.is_empty() || !dressing::is_printable(&flag.name) {
                return Err(format!(
                    "the flag '{}' needs at least one stripe, and a name with no control \
                     characters",
                    flag.name.escape_default()
                ));
            }
            if let Some(fire) = &flag.fire {
                splash::check_flames(&fire.flames, fire.speed)?;
            }
        }
        Ok(())
    }
}

static WARDROBE: Wardrobe<Pride> = Wardrobe::new();

/// The pride being worn.
pub fn worn() -> &'static Pride {
    WARDROBE.worn()
}

/// Wears `pride` for the rest of the run, or until another is worn.
pub fn wear(pride: Pride) {
    WARDROBE.wear(pride);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_written_pride_is_the_built_in_one() {
        assert_eq!(Pride::parse(TEMPLATE), Ok(Pride::default()));
        assert_eq!(Pride::parse(""), Ok(Pride::default()));
    }

    #[test]
    fn every_flag_left_in_a_comment_can_be_put_in() {
        // take the # off every commented flag line
        let all: String = TEMPLATE
            .lines()
            .map(|line| match line.strip_prefix("# ") {
                Some(rest)
                    if rest.starts_with("[[flag]]")
                        || rest.starts_with("name =")
                        || rest.starts_with("stripes =") =>
                {
                    rest
                }
                _ => line,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let pride = Pride::parse(&all).unwrap();
        assert!(pride.flags.len() > 10, "{}", pride.flags.len());
        assert!(pride.flags.iter().any(|flag| flag.name == "rainbow"));
        assert_eq!(pride.flags[1].stripes.len(), 6);
    }

    #[test]
    fn a_flag_is_picked_by_the_seed_and_may_bring_its_own_fire() {
        let pride = Pride::parse(
            "[[flag]]\nstripes = [\"red\"]\n\
             [[flag]]\nname = \"mine\"\nstripes = [\"blue\", \"white\"]\n\
             fire = { flames = [[\"black\", \"white\"]], speed = 0 }",
        )
        .unwrap();
        assert_eq!(pride.pick(0).unwrap().stripes, [Color::Red]);
        let mine = pride.pick(3).unwrap();
        assert_eq!(mine.name, "mine");
        assert_eq!(mine.fire.as_ref().unwrap().speed, 0.0);
        assert_eq!(Pride::parse("flag = []").unwrap().pick(7), None);
    }

    #[test]
    fn a_pride_with_a_mistake_says_what_it_is() {
        for wrong in [
            "[[flag]]\nstripes = []",
            "[[flag]]\nname = \"no stripes\"",
            "[[flag]]\nstripes = [\"red\"]\ncolor = \"red\"",
            "[fire]\nflames = []",
            "[[flag]]\nstripes = [\"red\"]\nfire = { flames = [] }",
            "edition = 2",
        ] {
            assert!(Pride::parse(wrong).is_err(), "{wrong}");
        }
    }
}
