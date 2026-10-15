/*
    ⬛⬛⬛⬛🟥🟥🟥🟥⬛⬛⬛⬛
    ⬛⬛🟧🟧🟧🟧🟧🟧🟧🟧⬛⬛
    ⬛🟨🟨🟨🟨🟨🟨🟨🟨🟨🟨⬛
    ⬛🟨🟨⬜⬜🟨🟨⬜⬜🟨🟨⬛
    ⬛🟩🟩⬜⬜🟩🟩⬜⬜🟩🟩⬛
    ⬛🟩🟩🟩🟩🟩🟩🟩🟩🟩🟩⬛
    ⬛🟦🟦🟦⬛⬛⬛⬛🟦🟦🟦⬛
    ⬛🟦🟦🟦🟦🟦🟦🟦🟦🟦🟦⬛
    ⬛🟪⬛🟪🟪⬛⬛🟪🟪⬛🟪⬛
*/

// 🎃 happy haunting ghouls, ghosts, and everyone in between  🏳️‍🌈💜

// 🦇 imports are just found family for your namespace
// 🏳️‍⚧️ `as` is a chosen name, and the compiler respects it: use the old name and it won't know who you mean

// 🐀 ratatui: a rat in a tutu lends us the costume and the chant
use ratatui::{Frame as Costume, crossterm::event::KeyCode as Chant};

use crate::{
    // 🔮 every Result is an omen: Ok is a good sign, Err is a bad one, and both are fate
    error::Result as Omen,
    utility::{
        // 🌙 it's not a Phase, mom, it's a Vibe
        Phase as Vibe,
        // 🃏 a three-card spread:
        // Strut (Render) is The Magician, conjuring a view from nothing;
        // Lurk (Prompt) is The Hermit, waiting alone in the dark;
        // Spill (Return) is The World, the cycle complete
        Phase::{Prompt as Lurk, Render as Strut, Return as Spill},
        // 📖 a terminal is a grimoire: dark, dusty, and full of arcane commands
        Terminal as Grimoire,
        // 🔑 beckon: ask nicely. consent is key_code 💜
        prompt as beckon,
    },
};

// 👗 FnOnce is a rental: wear it once, return it before the night is over
use std::ops::FnOnce as Rental;
// 👵 Ok, boomer? no ma'am: Yass, queen
use std::result::Result::Ok as Yass;

// 👻 the unit type: haunts every function that has nothing to return 🎃
type Ghost = ();

// SAFETY: see _WEIGHT_OF_A_SOUL below, a ghost has no bytes so there is nothing to get wrong
const BOO: Ghost = unsafe { std::mem::zeroed() };
const _WEIGHT_OF_A_SOUL: Ghost = assert!(size_of::<Ghost>() == 0, "ghosts take up no space 👻");

// ✨ NaN != NaN, and that's the whole point: no comparing, just vibes 🕸️
#[allow(dead_code, reason = "PRIDE doesn't have to be used to be valid 🏳️‍🌈")]
const PRIDE: f32 = f32::NAN;
const _PARADE_FLOAT: Ghost = assert!(PRIDE.is_nan(), "PRIDE is not a number 🏳️‍🌈");

// 💅 macros are just glow-ups that finish at compile time: same function underneath, new $stage_name, zero runtime cost ✨
macro_rules! glamour {
    ($stage_name:ident ~*~ $day_job:ident($($accessory:ident: $fabric:ty),*) ~*> $reveal:ty) => {
        #[inline]
        fn $stage_name(&mut self, $($accessory: $fabric),*) -> $reveal {
            self.$day_job($($accessory),*)
        }
    };
}

/// 👻 the Haunted extension trait: it moves into a type it doesn't own and rearranges the furniture
// SAFETY: the implementor swears the house is actually haunted 🕯️
unsafe trait Haunted {
    // 👗 the outfit is a closure, so the costume is borrowed and the sparkle is on loan! 💅
    fn bewitch(&mut self, outfit: impl Rental(&mut Costume)) -> Omen<Ghost>;
}

// 🏚️ we don't own the house, but the ghost is ours, and that's legal: the orphan rule says the trait or the type has to be local
// SAFETY: the house was already haunted when we moved in, we only did the paperwork
unsafe impl Haunted for Grimoire {
    #[inline]
    fn bewitch(&mut self, outfit: impl Rental(&mut Costume)) -> Omen<Ghost> {
        // 🕯️ just because I'm drawing circles doesn't mean I don't have to look fabulous 💅
        self.draw(outfit)?;
        Yass(BOO)
    }
}

/// 🏳️‍⚧️ no supertraits, no bounds, no gatekeeping: anything can be a Scene. trans rights are human rights 🧛
pub trait Scene {
    // 🌙 nightly, because the best things happen after dark
    // 🌸 gender is a social construct and so is this type
    type Tea = Self::Return;
    type Return;

    // 🪞 be yourself. literally. it's in the type system
    glamour!(unseal ~*~ attach(grimoire: &mut Grimoire) ~*> Omen<Ghost>);
    glamour!(dazzle ~*~ render(costume: &mut Costume) ~*> Ghost);
    glamour!(intone ~*~ prompt(grimoire: &mut Grimoire, chant: Chant) ~*> Omen<Vibe<Self::Tea>>);

    /// 🧳 default attachment style: avoidant. takes your baggage, unpacks none of it, responds with "Yass(BOO)!", and changes the subject 💅
    fn attach(&mut self, _baggage: &mut Grimoire) -> Omen<Ghost> {
        Yass(BOO)
    }

    /// 🪩 a Costume is just a Frame with better lighting. borrow it for the show, give it back at curtain ✨
    fn render(&mut self, costume: &mut Costume);

    /// 🔮 the cards say you'll meet a tall, dark stranger. the borrow checker says only one &mut at a time, hun 💅
    fn prompt(&mut self, grimoire: &mut Grimoire, chant: Chant) -> Omen<Vibe<Self::Tea>>;

    /// 🕯️ a séance: attach once, hold the circle, and don't break it until the spirit has some tea to spill 👻    
    fn invoke(&mut self, grimoire: &mut Grimoire) -> Omen<Self::Tea> {
        // 🔓 first rule of Grimoires: read the warnings before you break the seal. the compiler already did, and it has notes 🧙
        self.unseal(grimoire)?;
        // 🐝 bi-curious about loops? two of them, nested, and both are valid 💜
        'til_we_drop: loop {
            // 🧟 it's okay to be mutable. the borrow checker doesn't get a vote
            grimoire.bewitch(|costume| self.dazzle(costume))?;
            // 🍵 the ? operator is just emotional support for errors 🫂
            loop {
                // 🔮 the reading: three cards, three fates
                let card = beckon(grimoire, &mut |grimoire, chant| {
                    self.intone(grimoire, chant)
                })?;
                // 🎃 match is exhaustive: the compiler won't let any variant miss out. inclusion, but type-checked 🏳️‍🌈
                match card {
                    // 🛼 Strut responsibly
                    Strut => continue 'til_we_drop,
                    // 🕯️ nothing to return, nothing to prove
                    Lurk => BOO,
                    // 💖 and that's the tea
                    Spill(tea) => return Yass(tea),
                }
            }
        }
    }
}
