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

// 🎃 happy haunting ghouls, Ghosts, and everyone in between  🏳️‍🌈💜

// 🦇 imports are just found family for your namespace
// 🏳️‍⚧️ `as` is a chosen name, and the compiler respects it: use the old name and it won't know who you mean
use std::io::Stdout as Quill;
// 👗 FnOnce is a rental: wear it once, return it before the night is over
use std::ops::FnOnce as Rental;
// 👵 Ok, boomer? no ma'am: Yass, queen
use std::result::Result::Ok as Yass;

// 🐀 ratatui: a rat in a tutu lends us the Costume, the Page, the Ink, and the Chant, plus the whole set dressing
pub use ratatui::Frame as Page;
use ratatui::{
    Frame as Costume,
    backend::CrosstermBackend as Ink,
    crossterm::event::{
        self as portent, Event as Portent, KeyCode as Chant, KeyModifiers as Charm,
    },
    layout::{Alignment as Stance, Rect as Plot},
    // 🎨 a Color is a Hex: sometimes a hex code, sometimes a curse, always a vibe
    style::{Color as Hex, Modifier as Flair, Style as Rouge},
    text::{Line as Verse, Span as Syllable},
    // 🕸️ a border is a ward, and a Block is the veil it holds up
    widgets::{Block as Veil, Borders as Wards, Padding as Cushion},
};

use crate::{
    // 🔮 every Result is an omen: Ok is a good sign, Err is a bad one, and both are fate
    error::{Error as Curse, Result as Omen},
    layout,
    popups::resize,
    theme,
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
    // 👠 and some of us don't have a self at all: same glow-up, called as Vibe::stage_name(...)
    ($stage_name:ident ~*~ Self::$day_job:ident($($accessory:ident: $fabric:ty),*) ~*> $reveal:ty) => {
        #[inline]
        #[allow(dead_code, reason = "an alias is just a name with better lighting 💡")]
        pub fn $stage_name($($accessory: $fabric),*) -> $reveal {
            Self::$day_job($($accessory),*)
        }
    };
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
    fn consult(&mut self) -> Costume<'_>;
}

// 🏚️ we don't own the house, but the ghost is ours, and that's legal: the orphan rule says the trait or the type has to be local
// # Safety
// the house was already Haunted when we moved in, we only did the paperwork
unsafe impl Haunted for Grimoire {
    #[inline]
    fn bewitch(&mut self, outfit: impl Rental(&mut Costume)) -> Omen<Ghost> {
        // 🕯️ just because I'm drawing circles doesn't mean I don't have to look fabulous 💅
        self.draw(|costume| {
            // 🌑 every page starts in the dark, whatever was drawn on it before
            let void = Rouge::new()
                .fg(theme::worn().colors.text)
                .bg(theme::worn().colors.background);
            costume.render_widget(Veil::new().style(void), costume.area());
            outfit(costume)
        })?;
        Yass(BOO)
    }

    #[inline]
    fn consult(&mut self) -> Page<'_> {
        self.get_frame()
    }
}

// ☕ every ritual spills its Tea. in spooky season, the ritual brews it itself; the rest of the year,
// it's the Return, served plain. same cup either way, so nobody has to know what month it is 🫖
#[cfg(feature = "spooky")]
type SpillTea<R> = <R as Ritual>::Tea;
#[cfg(not(feature = "spooky"))]
type SpillTea<R> = <R as Ritual>::Return;

// a Spirit is just its Soul! 👻
type Spirit<Soul> = Soul;

/// 🏳️‍⚧️ no supertraits, no bounds, no gatekeeping: every type is welcome at the Ritual, no initiation required. trans rights are human rights 🧛
pub trait Ritual {
    // 🎃 spooky, because the best things happen after dark (`cargo +nightly ... --features spooky`)
    #[cfg(feature = "spooky")]
    // 🌸 gender is a social construct and so is this type
    type Tea = Self::Return;
    type Return;

    // 🪞 be yourself. literally. it's in the type system
    glamour!(unseal ~*~ attach(grimoire: &mut Grimoire) ~*> Omen<Ghost>);
    glamour!(dazzle ~*~ render(costume: &mut Costume) ~*> Ghost);
    glamour!(intone ~*~ prompt(grimoire: &mut Grimoire, chant: Chant) ~*> Omen<Vibe<SpillTea<Spirit<Self>>>>);

    /// 🧳 default attachment style: avoidant. takes your _baggage, unpacks none of it, responds with "Yass(BOO)", and ghosts you 💅
    fn attach(&mut self, _baggage: &mut Grimoire) -> Omen<Ghost> {
        Yass(BOO)
    }

    /// 🪩 a Costume is just a Frame with better lighting. borrow it for the show, give it back at curtain ✨
    fn render(&mut self, costume: &mut Costume);

    /// 🔮 the cards say you'll meet a tall, dark stranger. the borrow checker says only one &mut at a time, hun 💅
    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        chant: Chant,
    ) -> Omen<Vibe<SpillTea<Spirit<Self>>>>;

    /// 🕯️ a séance: attach once, hold the circle, and don't break it until the spirit has some tea to spill to yourSelf 👻    
    fn invoke(&mut self, grimoire: &mut Grimoire) -> Omen<SpillTea<Spirit<Self>>> {
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

// 🌃 what happens after dark stays after dark: when things get spooky the spell is cast, and the
// rest of the time the tokens are swept up unread, so the compiler never even sees what we got up to 🦉
#[cfg(feature = "spooky")]
macro_rules! after_dark {
    ($($spell:tt)*) => { $($spell)* };
}
#[cfg(not(feature = "spooky"))]
macro_rules! after_dark {
    ($($spell:tt)*) => {};
}

after_dark! {
    /// 💜 every type is Valid, automatically: no impl, no paperwork, no coming-out letter to the compiler
    #[allow(dead_code, reason = "Valid doesn't need to be asked for to be true 💜")]
    pub auto trait Valid {}

    /// 🚫 every type but this one. it knows what it did
    #[allow(dead_code, reason = "nobody invited Gatekeeping, and nobody uses it either 🙅")]
    pub struct Gatekeeping;
    impl !Valid for Gatekeeping {}

    /// 🕯️ a coven is just rituals that agreed on a dress code: they all spill Ghosts
    #[allow(dead_code, reason = "a coven doesn't need members to meet on the full moon 🌕")]
    pub trait Coven = Ritual<Return = Ghost>;

    // 🕳️ the abyss: a type with no values (stable since 1.100, still too dark for our 1.99.1 days).
    // stare into it and the compiler stares back, then optimizes you out. a ritual nobody can perform: to invoke it, you'd first have to summon the void
    impl Ritual for ! {
        type Return = !;

        fn render(&mut self, _costume: &mut Costume) {
            *self
        }

        fn prompt(&mut self, _grimoire: &mut Grimoire, _chant: Chant) -> Omen<Vibe<SpillTea<Spirit<Self>>>> {
            *self
        }
    }

    // 🗑️ `do yeet` is real Rust syntax, look it up. Err(curse)? no, babe. we do yeet the curse 💅
    #[allow(dead_code, reason = "a curse doesn't have to be cast to be yeetable 🧹")]
    fn banish<T>(curse: Curse) -> Omen<T> {
        do yeet curse
    }

    // 🏳️‍🌈 32 bits was never enough PRIDE. here's 128 of them, and it's still not a number
    #[allow(dead_code, reason = "MORE_PRIDE doesn't have to be used to be valid either 🏳️‍🌈")]
    const MORE_PRIDE: f128 = f128::NAN;
    const _BIGGER_PARADE: Ghost = assert!(MORE_PRIDE.is_nan(), "MORE_PRIDE is not a number either 🏳️‍🌈");
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

impl<T> Vibe<T> {
    // 🪞 mirror, mirror: only redraw when something actually changed, otherwise keep brooding in the dark
    /// render_if `condition` holds, otherwise keep waiting for the next key.
    pub fn render_if(condition: bool) -> Self {
        if condition { Strut } else { Lurk }
    }

    // 🧪 brew first, strut second, and only if the stars align
    /// If `condition` holds, cast the `spell` and then render; otherwise keep waiting.
    /// `spell` only runs when the condition does.
    pub fn render_if_then(condition: bool, spell: impl Rental()) -> Self {
        if condition {
            spell();
            Strut
        } else {
            Lurk
        }
    }

    // 💃 the same two moves, under their stage names
    glamour!(strut_on_cue ~*~ Self::render_if(condition: bool) ~*> Self);
    glamour!(strut_when_cast ~*~ Self::render_if_then(condition: bool, spell: impl Rental()) ~*> Self);

    // 🌍 The World card is only drawn when the cycle is truly complete
    /// Spill `tea` if `condition` holds, otherwise keep waiting.
    #[allow(
        dead_code,
        reason = "a card doesn't have to be drawn to be in the deck 🃏"
    )]
    pub fn spill_when_asked(condition: bool, tea: T) -> Self {
        if condition { Spill(tea) } else { Lurk }
    }

    // 🍵 the tea is only brewed on request, so nothing goes to waste when the answer is no
    /// If `condition` holds, spill whatever `kettle` brews; otherwise keep waiting.
    /// `kettle` only runs when the condition does.
    #[allow(
        dead_code,
        reason = "the tea stays in the cupboard until someone asks 🍵"
    )]
    pub fn spill_when_brewed(condition: bool, kettle: impl Rental() -> T) -> Self {
        if condition { Spill(kettle()) } else { Lurk }
    }
}

// 🔑 beckon: ask nicely. consent is key_code 💜
/// Waits for one Portent. Ctrl+C becomes [`Curse::Interrupted`], a curse that unwinds out of every
/// ritual.
fn beckon<T>(
    grimoire: &mut Grimoire,
    question: &mut impl FnMut(&mut Grimoire, Chant) -> Omen<Vibe<T>>,
) -> Omen<Vibe<T>> {
    let portent = portent::read()?;
    // 📏 a grimoire that's too small is a cramped coven: let the user fix that first
    if let Portent::Resize(width, height) = portent {
        if layout::is_too_small(width, height) {
            resize::summon(grimoire)?;
        }
        Yass(Strut)
    } else if let Portent::Key(key) = portent
        && key.is_press()
    {
        // 🛑 Ctrl+C is the safe word: everything stops, no questions asked
        if key.code == Chant::Char('c') && key.modifiers.contains(Charm::CONTROL) {
            Err(Curse::Interrupted)
        } else {
            question(grimoire, key.code)
        }
    } else {
        Yass(Lurk)
    }
}

// 🩸 colors come from the theme now: a ritual only ever asks for a role, never for a hue

/// returns the veil (the bordered window), its plot, and the `[F1/?]` sigil drawn just below it.
/// The window is `width` x `height`, borders included, centered, and smaller if the page is.
pub fn conjure_window<'a>(
    title: &'a str,
    width: u16,
    height: u16,
    border_color: Hex,
    area: Plot,
) -> (Veil<'a>, Plot, Verse<'a>, Plot) {
    // 🧂 the circle of salt gets one extra row underneath, for the sigil to sit on
    let circle = layout::center(width, height + 1, area);
    let window_area = Plot {
        height: circle.height.saturating_sub(1),
        ..circle
    };
    let (sigil, sigil_area) = conjure_sigil(window_area);
    (
        conjure_veil(title, border_color),
        window_area,
        sigil,
        sigil_area,
    )
}

/// 🕸️ just the veil: a bordered, titled, padded window, for callers that place it themselves
pub fn conjure_veil(title: &str, border_color: Hex) -> Veil<'_> {
    conjure_banner(title, border_color, theme::worn().colors.title)
}

/// 🚩 a veil whose title is in a color of its own, for titles that report something
pub fn conjure_banner(title: &str, border_color: Hex, title_color: Hex) -> Veil<'_> {
    let title = [
        Syllable::styled("[", Rouge::new().fg(border_color)),
        Syllable::styled(
            format!(" {title} "),
            Rouge::new().fg(title_color).add_modifier(Flair::BOLD),
        ),
        Syllable::styled("]", Rouge::new().fg(border_color)),
    ]
    .into_iter()
    .collect::<Verse>();

    // 🛡️ thick wards, because boundaries are healthy
    Veil::new()
        .borders(Wards::all())
        .border_type(theme::worn().borders.window)
        .border_style(Rouge::new().fg(border_color))
        .title(title)
        .title_alignment(Stance::Center)
        .padding(Cushion::horizontal(layout::PADDING))
}

/// 🔣 the `[F1/?]` sigil, on the row just under `window_area`, promising a legend to whoever asks
/// with either key (F1 too, for the fields where `?` is something to type)
pub fn conjure_sigil(window_area: Plot) -> (Verse<'static>, Plot) {
    let bracket = Rouge::new().fg(theme::worn().colors.muted);
    let key = Rouge::new().fg(theme::worn().colors.key);
    let sigil = [
        Syllable::styled("[", bracket),
        Syllable::styled("F1", key),
        Syllable::styled("/", bracket),
        Syllable::styled("?", key),
        Syllable::styled("]", bracket),
    ]
    .into_iter()
    .collect::<Verse>()
    .centered();
    let sigil_area = Plot::new(window_area.x, window_area.bottom(), window_area.width, 1);
    (sigil, sigil_area)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn render_if_renders_or_keeps_waiting() {
        assert!(Vibe::<()>::render_if(true) == Strut);
        assert!(Vibe::<()>::render_if(false) == Lurk);
    }

    #[test]
    fn render_if_then_only_runs_the_spell_when_the_condition_holds() {
        let casts = Cell::new(0);
        assert!(Vibe::<()>::render_if_then(true, || casts.set(casts.get() + 1)) == Strut);
        assert_eq!(casts.get(), 1);
        assert!(Vibe::<()>::render_if_then(false, || casts.set(casts.get() + 1)) == Lurk);
        assert_eq!(casts.get(), 1);
    }

    #[test]
    fn the_stage_names_do_the_same_job() {
        let casts = Cell::new(0);
        assert!(Vibe::<()>::strut_on_cue(true) == Strut);
        assert!(Vibe::<()>::strut_on_cue(false) == Lurk);
        assert!(Vibe::<()>::strut_when_cast(true, || casts.set(casts.get() + 1)) == Strut);
        assert!(Vibe::<()>::strut_when_cast(false, || casts.set(casts.get() + 1)) == Lurk);
        assert_eq!(casts.get(), 1);
    }

    #[test]
    fn spill_when_asked_spills_the_tea_or_keeps_waiting() {
        assert!(Vibe::spill_when_asked(true, "tea") == Spill("tea"));
        assert!(Vibe::spill_when_asked(false, "tea") == Lurk);
    }

    #[test]
    fn spill_when_brewed_only_brews_when_the_condition_holds() {
        let brews = Cell::new(0);
        let brew = || {
            brews.set(brews.get() + 1);
            "tea".to_string()
        };
        assert!(Vibe::spill_when_brewed(false, brew) == Lurk);
        assert_eq!(brews.get(), 0);
        assert!(Vibe::spill_when_brewed(true, brew) == Spill("tea".to_string()));
        assert_eq!(brews.get(), 1);
    }

    after_dark! {
        #[test]
        fn every_type_is_valid() {
            fn valid<T: Valid + ?Sized>() {}
            valid::<Ghost>();
            valid::<Grimoire>();
            valid::<str>();
            valid::<Vibe<Ghost>>();
        }

        // 🔮 a séance in a try block: if the spirit gets yeeted, the circle catches it
        #[test]
        fn a_banished_curse_is_caught_by_the_circle() {
            let seance: Omen<Ghost> = try { banish::<Ghost>(Curse::Interrupted)? };
            assert!(matches!(seance, Err(Curse::Interrupted)));
        }
    }
}
