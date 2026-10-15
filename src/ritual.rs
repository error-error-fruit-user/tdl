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
use std::io::Stdout as Quill;
// 👗 FnOnce is a rental: wear it once, return it before the night is over
use std::ops::FnOnce as Rental;
// 👵 Ok, boomer? no ma'am: Yass, queen
use std::result::Result::Ok as Yass;

// 🐀 ratatui: a rat in a tutu lends us the costume and the chant, plus the whole set dressing
pub use ratatui::Frame as Page;
use ratatui::{
    Frame as Costume,
    backend::CrosstermBackend as Ink,
    crossterm::event::{
        self as portent, Event as Portent, KeyCode as Chant, KeyModifiers as Charm,
    },
    layout::{Alignment as Stance, Position as Spot, Rect as Plot},
    // 🎨 a Color is a Hex: sometimes a hex code, sometimes a curse, always a vibe
    style::{Color as Hex, Style as Rouge, Stylize as Dress},
    text::{Line as Verse, Span as Syllable, ToSpan as Utter},
    // 🕸️ a border is a ward, and a Block is the veil it holds up
    widgets::{Block as Veil, BorderType as Weave, Borders as Wards},
};

use crate::{
    // 🔮 every Result is an omen: Ok is a good sign, Err is a bad one, and both are fate
    error::{Error as Curse, Result as Omen},
    popups::resize,
};

// 📖 a terminal is a grimoire: dark, dusty, and written in Ink with a Quill (stdout) 🪶
pub type Grimoire = ratatui::Terminal<Ink<Quill>>;

// 🌙 it's not a Phase, mom, it's a Vibe
// 🃏 a three-card spread:
// Strut (Render) is The Magician, conjuring a view from nothing;
// Lurk (Prompt) is The Hermit, waiting alone in the dark;
// Spill (Return) is The World, the cycle complete
use self::Phase::{self as Vibe, Prompt as Lurk, Render as Strut, Return as Spill};

// 👻 the unit type: haunts every function that has nothing to return 🎃
type Ghost = ();

// # Safety
// see _WEIGHT_OF_A_SOUL below, a ghost has no bytes so there is nothing to get wrong
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
/// # Safety
/// the implementor swears the house is actually haunted 🕯️
pub unsafe trait Haunted {
    // 👗 the outfit is a closure, so the costume is borrowed and the sparkle is on loan! 💅
    fn bewitch(&mut self, outfit: impl Rental(&mut Costume)) -> Omen<Ghost>;

    // 🔭 look, don't touch: the costume comes back unworn, just to see how much room there is
    fn consult(&mut self) -> Page<'_>;
}

// 🏚️ we don't own the house, but the ghost is ours, and that's legal: the orphan rule says the trait or the type has to be local
// # Safety
// the house was already haunted when we moved in, we only did the paperwork
unsafe impl Haunted for Grimoire {
    #[inline]
    fn bewitch(&mut self, outfit: impl Rental(&mut Costume)) -> Omen<Ghost> {
        // 🕯️ just because I'm drawing circles doesn't mean I don't have to look fabulous 💅
        self.draw(outfit)?;
        Yass(BOO)
    }

    #[inline]
    fn consult(&mut self) -> Page<'_> {
        self.get_frame()
    }
}

/// 🏳️‍⚧️ no supertraits, no bounds, no gatekeeping: every type is welcome at the ritual, no initiation required. trans rights are human rights 🧛
pub trait Ritual {
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

// 🕸️ everything below is what a ritual needs on the night: a mood, a question, and a circle of salt

/// 🔮 what a ritual wants after reading the room: redraw, keep waiting, or give up the tea
#[must_use]
#[derive(Eq, PartialEq)]
pub enum Phase<T> {
    Render,
    Prompt,
    Return(T),
}

// 🔑 beckon: ask nicely. consent is key_code 💜
/// Waits for one Portent. Ctrl+C becomes [`Curse::Exit`], a curse that unwinds out of every ritual.
fn beckon<T>(
    grimoire: &mut Grimoire,
    question: &mut impl FnMut(&mut Grimoire, Chant) -> Omen<Vibe<T>>,
) -> Omen<Vibe<T>> {
    let portent = portent::read()?;
    // 📏 a grimoire that's too small is a cramped coven: let the user fix that first
    if let Portent::Resize(width, height) = portent {
        if resize::should_resize(width, height) {
            resize::summon(grimoire)?;
        }
        Yass(Strut)
    } else if let Portent::Key(key) = portent
        && key.is_press()
    {
        // 🛑 Ctrl+C is the safe word: everything stops, no questions asked
        if key.code == Chant::Char('c') && key.modifiers.contains(Charm::CONTROL) {
            Err(Curse::Exit)
        } else {
            question(grimoire, key.code)
        }
    } else {
        Yass(Lurk)
    }
}

// 🕯️ yellow, because every ritual needs candlelight on its title
const CANDLELIGHT: Hex = Hex::LightYellow;

/// returns the veil (the bordered window), its plot, and the `[?]` sigil drawn just below it.
pub fn conjure_window<'a>(
    title: &'a str,
    width: u16,
    height: u16,
    border_color: Hex,
    area: Plot,
) -> (Veil<'a>, Plot, Verse<'a>, Plot) {
    let title = [
        Syllable::styled("[", Rouge::new().fg(border_color)),
        Syllable::styled(format!(" {title} "), Rouge::new().fg(CANDLELIGHT)),
        Syllable::styled("]", Rouge::new().fg(border_color)),
    ]
    .into_iter()
    .collect::<Verse>();

    // 🛡️ thick wards, because boundaries are healthy
    let window = Veil::new()
        .borders(Wards::all())
        .border_type(Weave::Thick)
        .border_style(Rouge::new().fg(border_color))
        .title(title)
        .title_alignment(Stance::Center);
    let window_area = salt_circle(width, height, area);
    let sigil = ["[".to_span(), "?".to_span().light_yellow(), "]".to_span()]
        .into_iter()
        .collect::<Verse>()
        .centered();
    let sigil_area = Plot::new(
        window_area.x,
        window_area.bottom() + 1,
        window_area.width,
        1,
    );
    (window, window_area, sigil, sigil_area)
}

/// 🧂 a circle of salt, centered on the page and shrunk to fit if the grimoire is smaller
fn salt_circle(width: u16, height: u16, area: Plot) -> Plot {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Plot::new(
        (area.width - width) / 2,
        (area.height - height) / 2,
        width,
        height,
    )
}

/// A `label: input` verse, plus the spot where the cursor belongs: at the end of the input.
pub fn conjure_query<'a>(
    label: &'a str,
    input: &'a str,
    page_width: u16,
    y: u16,
) -> (Verse<'a>, Spot) {
    let verse = [
        Syllable::from(label).light_yellow(),
        ": ".to_span(),
        Syllable::from(input),
    ]
    .into_iter()
    .collect::<Verse>()
    .centered();

    let spot = Spot::new(
        (page_width + (label.len() + input.len()) as u16).div_ceil(2),
        y,
    );

    (verse, spot)
}
