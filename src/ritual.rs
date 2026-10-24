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
use std::io::{Stdout as Inkwell, Write as Scribble};
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
    // 🎮 a controller is a familiar: it can't speak, but it knows every chant by button
    gamepad::{self as familiar, Prompt as Gesture},
    keyboard::{self, Output as Typed, PadKeyboard},
    layout,
    popups::{legend::Legend, resize},
    theme,
};

// 📖 a terminal is a grimoire: dark, dusty, and written in Ink with a Quill (stdout) 🪶
pub type Grimoire = ratatui::Terminal<Ink<Quill>>;

// 🪶 a quill writes in the terminal, or into the void when nobody is watching (`tdl headless`,
// whose script reads the page instead)
pub enum Quill {
    Inkwell(Inkwell),
    Void,
    // 📜 parchment: no terminal either, but always written on: the séance draws its lines on it
    // with `bewitch`, then reads them off into the terminal, colors and all
    Parchment,
}

impl Scribble for Quill {
    fn write(&mut self, ink: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Inkwell(inkwell) => inkwell.write(ink),
            Self::Void | Self::Parchment => Yass(ink.len()),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Inkwell(inkwell) => inkwell.flush(),
            Self::Void | Self::Parchment => Yass(BOO),
        }
    }
}

// 🕯️ opening the grimoire: raw mode, the alternate screen, and a ward that closes it again if
// anything panics, the way `ratatui::run` does it
pub fn open_grimoire() -> std::io::Result<Grimoire> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        hook(info);
    }));
    ratatui::crossterm::terminal::enable_raw_mode()?;
    ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::EnterAlternateScreen
    )?;
    ratatui::Terminal::new(Ink::new(Quill::Inkwell(std::io::stdout())))
}

// 🌫️ a grimoire with no terminal behind it, a fixed size, for a script to read
pub fn void_grimoire(width: u16, height: u16) -> std::io::Result<Grimoire> {
    ratatui::Terminal::with_options(
        Ink::new(Quill::Void),
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Fixed(Plot::new(0, 0, width, height)),
        },
    )
}

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
        // 🫥 a look nobody will see is a look nobody has to serve: a possessed run's screens are
        // drawn into the void, and nobody watches the void, so that outfit stays on the hanger 👻
        let parchment = INSCRIBING.get();
        if possessed() && !parchment {
            return Yass(BOO);
        }
        // 🕯️ just because I'm drawing circles doesn't mean I don't have to look fabulous 💅
        let drawn = self.draw(|costume| {
            // 🌑 every page starts in the dark, whatever was drawn on it before (parchment stays
            // bare, so the words sit on the terminal's own background)
            if !parchment {
                let void = Rouge::new()
                    .fg(theme::worn().colors.text)
                    .bg(theme::worn().colors.background);
                costume.render_widget(Veil::new().style(void), costume.area());
            }
            outfit(costume)
        })?;
        // 📜 what was written on parchment is kept, for the séance to read off
        if parchment {
            let page = drawn.buffer.clone();
            PARCHMENT.with_borrow_mut(|kept| *kept = Some(page));
        }
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

    /// ⌨️ whether a text field is being typed in, which a controller can't do without a keyboard on
    /// screen: while it is, and a controller spoke last, the loop draws one over the ritual and
    /// hands the ritual the keys it makes, so a ritual only has to say so
    fn typing(&self) -> bool {
        false
    }

    /// 📜 the legend this ritual shows on F1 right now (none by default), which also names what a
    /// possessed run's presses do
    fn legend(&self) -> &[[&'static str; 2]] {
        &[]
    }

    /// 🗡️ what `chant` does here, in words, for a possessed run's confession: its legend's line, by
    /// default. `None` is nothing
    fn deed(&self, chant: Chant) -> Option<String> {
        deed_in(self.legend(), chant)
    }

    /// 🕯️ a séance: attach once, hold the circle, and don't break it until the spirit has some tea to spill to yourSelf 👻    
    fn invoke(&mut self, grimoire: &mut Grimoire) -> Omen<SpillTea<Spirit<Self>>> {
        // 🔓 first rule of Grimoires: read the warnings before you break the seal. the compiler already did, and it has notes 🧙
        // 👻 a possessed run hears only its spirit, never a controller, even one that is plugged in
        if !possessed() {
            familiar::start();
        }
        self.unseal(grimoire)?;
        // 🐝 bi-curious about loops? two of them, nested, and both are valid 💜
        let mut keyboard = PadKeyboard::default();
        'til_we_drop: loop {
            // 🎹 a familiar can't type, so while a field is being typed in, it gets a keyboard
            let typing = self.typing() && familiar::shown().is_some();
            crate::keyboard::forget_caret();
            // 🧟 it's okay to be mutable. the borrow checker doesn't get a vote
            grimoire.bewitch(|costume| {
                self.dazzle(costume);
                if typing && let Some(family) = familiar::shown() {
                    keyboard.draw(costume, family);
                }
            })?;
            // 🍵 the ? operator is just emotional support for errors 🫂
            loop {
                // 🔮 the reading: three cards, three fates
                let card = beckon(
                    grimoire,
                    typing.then_some(&mut keyboard),
                    &mut |grimoire, chant| {
                        // 🙏 the spirit owns up to each press before the ritual answers it
                        if possessed() {
                            confess(&spirit_name::<Self>(), self.deed(chant));
                        }
                        self.intone(grimoire, chant)
                    },
                )?;
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
/// Waits for one Portent, or a press of a controller, which is heard as the key it stands for.
/// Ctrl+C becomes [`Curse::Interrupted`], a curse that unwinds out of every ritual.
fn beckon<T>(
    grimoire: &mut Grimoire,
    mut keyboard: Option<&mut PadKeyboard>,
    question: &mut impl FnMut(&mut Grimoire, Chant) -> Omen<Vibe<T>>,
) -> Omen<Vibe<T>> {
    // 🎭 who spoke last, keyboard or familiar: the legends dress for whoever it was
    let voice = familiar::shown();
    // 🐈‍⬛ between glances at the door, check whether the familiar has anything to say
    let portent = loop {
        if let Some(keyboard) = keyboard.as_deref_mut() {
            // 🎹 with a keyboard up, the familiar's buttons are its keys and combinations
            if let Some((button, family)) = familiar::press() {
                match keyboard.press(button, family, familiar::is_held) {
                    Typed::Help => {
                        Legend::summon(grimoire, keyboard::LEGEND)?;
                        return Yass(Strut);
                    }
                    Typed::Nothing => continue,
                    Typed::Redraw => return Yass(Strut),
                    Typed::Keys(chants) => {
                        for chant in chants {
                            match question(grimoire, chant)? {
                                Lurk => {}
                                vibe => return Yass(change_costume(voice, vibe)),
                            }
                        }
                        continue;
                    }
                }
            }
        } else if !possessed()
            && let Some(chant) = familiar::chant()
        {
            let vibe = question(grimoire, chant)?;
            return Yass(change_costume(voice, vibe));
        }
        // 🧪 a test with a pretend controller has nothing to wait for once it has said its piece
        // 👻 a spirit's keys come the same way a person's do
        if listen(familiar::patience())? {
            break hear()?;
        }
    };
    // 📏 a grimoire that's too small is a cramped coven: let the user fix that first
    if let Portent::Resize(width, height) = portent {
        if layout::is_too_small(width, height) {
            resize::summon(grimoire)?;
        }
        Yass(Strut)
    } else if let Portent::Key(key) = portent
        && key.is_press()
    {
        familiar::keyboard_used();
        // 🛑 Ctrl+C is the safe word: everything stops, no questions asked
        if key.code == Chant::Char('c') && key.modifiers.contains(Charm::CONTROL) {
            confess("tdl", Some("Stop tdl".into()));
            Err(Curse::Interrupted)
        } else {
            let vibe = question(grimoire, key.code)?;
            Yass(change_costume(voice, vibe))
        }
    } else {
        Yass(Lurk)
    }
}

// 👗 a quick change between acts: whoever spoke last, the legends should be dressed for them,
// so a ritual that was only going to keep brooding struts once more in the new outfit
fn change_costume<T>(voice: Option<familiar::Family>, vibe: Vibe<T>) -> Vibe<T> {
    match vibe {
        Lurk if familiar::shown() != voice => Strut,
        vibe => vibe,
    }
}

// 🕯️🔮 THE SÉANCE 🔮🕯️
// with no window to haunt (`tdl headless`, `--headless`), tdl talks to the dead in plain lines
// instead: every word the command line says goes through here, dressed in the theme's colors when
// a terminal is listening and bare when it is only a file. the possessed confess their presses,
// and the planchette spins while everyone waits for a sign 👻

// 🤐 a séance held behind a curtain: while one is, every word is written down instead of said, so
// a rite done from the launcher's command line can be read in a window, not over the screen
std::thread_local! {
    static CURTAIN: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

// 🎭 runs `rite` behind the curtain: what it returns, and every word said meanwhile, in order
pub fn behind_the_curtain<T>(rite: impl Rental() -> T) -> (T, Vec<String>) {
    CURTAIN.with_borrow_mut(|curtain| *curtain = Some(Vec::new()));
    let returned = rite();
    let said = CURTAIN.with_borrow_mut(Option::take).unwrap_or_default();
    (returned, said)
}

// 🫣 is a séance being held behind the curtain? nobody can be asked anything there
pub fn curtained() -> bool {
    CURTAIN.with_borrow(Option::is_some)
}

// 🪄 is the planchette mid-spin on the last line? it has to stop before anyone else can speak
static SPINNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// 🌀 the planchette's path around the board, one stop per glance
const PLANCHETTE: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// 🎨 a theme's hue, translated for the spirit box (the terminal's own sixteen, or the full rainbow)
fn tint(hex: Hex) -> ratatui::crossterm::style::Color {
    use ratatui::crossterm::style::Color as Spirit;
    match hex {
        Hex::Reset => Spirit::Reset,
        Hex::Black => Spirit::Black,
        Hex::Red => Spirit::DarkRed,
        Hex::Green => Spirit::DarkGreen,
        Hex::Yellow => Spirit::DarkYellow,
        Hex::Blue => Spirit::DarkBlue,
        Hex::Magenta => Spirit::DarkMagenta,
        Hex::Cyan => Spirit::DarkCyan,
        Hex::Gray => Spirit::Grey,
        Hex::DarkGray => Spirit::DarkGrey,
        Hex::LightRed => Spirit::Red,
        Hex::LightGreen => Spirit::Green,
        Hex::LightYellow => Spirit::Yellow,
        Hex::LightBlue => Spirit::Blue,
        Hex::LightMagenta => Spirit::Magenta,
        Hex::LightCyan => Spirit::Cyan,
        Hex::White => Spirit::White,
        Hex::Rgb(r, g, b) => Spirit::Rgb { r, g, b },
        Hex::Indexed(index) => Spirit::AnsiValue(index),
    }
}

std::thread_local! {
    // ✍️ whether the page being bewitched is parchment (see `inscribe`)
    static INSCRIBING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    // 📜 the last page written on parchment, waiting to be read off
    static PARCHMENT: std::cell::RefCell<Option<ratatui::buffer::Buffer>> =
        const { std::cell::RefCell::new(None) };
}

// 🗣️ verses said to `out`: drawn on parchment with `bewitch`, styled from the theme, then read off
// cell by cell into the terminal in their colors, or bare when it is only a file. raw mode (a game
// going under `--headless`) needs its own carriage return, or the words drift off like a ghost
fn utter(mut out: impl Scribble + std::io::IsTerminal, verses: Vec<Verse<'static>>) {
    let bare: Vec<String> = verses.iter().map(ToString::to_string).collect();
    let written = CURTAIN.with_borrow_mut(|curtain| {
        curtain
            .as_mut()
            .map(|said| said.extend(bare.iter().cloned()))
    });
    if written.is_some() {
        return;
    }
    hush();
    let end = if ratatui::crossterm::terminal::is_raw_mode_enabled().unwrap_or(false) {
        "\r\n"
    } else {
        "\n"
    };
    let read_off = out.is_terminal().then(|| inscribe(verses)).flatten();
    match read_off {
        Some(page) => {
            for row in page {
                let _ = write!(out, "{row}{end}");
            }
        }
        None => {
            for line in bare {
                let _ = write!(out, "{line}{end}");
            }
        }
    }
    let _ = out.flush();
}

// ✍️ the verses written on a parchment page just their size with `bewitch`, then read back off it
// as the terminal's own colored text, a row per verse. `None` if the parchment wouldn't take ink
fn inscribe(verses: Vec<Verse<'static>>) -> Option<Vec<String>> {
    use ratatui::crossterm::style::{Attribute, SetAttribute, SetForegroundColor};
    let width = verses
        .iter()
        .map(Verse::width)
        .max()
        .unwrap_or(0)
        .clamp(1, 2000) as u16;
    let height = verses.len().clamp(1, 2000) as u16;
    let mut parchment = ratatui::Terminal::with_options(
        Ink::new(Quill::Parchment),
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Fixed(Plot::new(0, 0, width, height)),
        },
    )
    .ok()?;
    INSCRIBING.set(true);
    let inked = parchment.bewitch(|costume| {
        for (row, verse) in verses.into_iter().enumerate() {
            costume.render_widget(verse, Plot::new(0, row as u16, width, 1));
        }
    });
    INSCRIBING.set(false);
    inked.ok()?;
    let page = PARCHMENT.with_borrow_mut(Option::take)?;
    let rows = (0..height)
        .map(|y| {
            let cells: Vec<_> = (0..width).map(|x| &page[(x, y)]).collect();
            let used = cells
                .iter()
                .rposition(|cell| cell.symbol() != " ")
                .map_or(0, |last| last + 1);
            let mut row = String::new();
            let mut last: Option<Rouge> = None;
            for cell in &cells[..used] {
                let style = cell.style();
                if last != Some(style) {
                    row.push_str(&SetAttribute(Attribute::Reset).to_string());
                    if style.fg.is_some_and(|fg| fg != Hex::Reset) {
                        row.push_str(&SetForegroundColor(tint(style.fg.unwrap())).to_string());
                    }
                    if style.add_modifier.contains(Flair::BOLD) {
                        row.push_str(&SetAttribute(Attribute::Bold).to_string());
                    }
                    if style.add_modifier.contains(Flair::ITALIC) {
                        row.push_str(&SetAttribute(Attribute::Italic).to_string());
                    }
                    last = Some(style);
                }
                row.push_str(cell.symbol());
            }
            row.push_str(&SetAttribute(Attribute::Reset).to_string());
            row
        })
        .collect();
    Some(rows)
}

// 🖌️ `words` as verses, a verse per line, all in `rouge`, each after `indent`
fn verses(words: &str, rouge: Rouge, indent: &str) -> Vec<Verse<'static>> {
    words
        .lines()
        .map(|line| Verse::from(Syllable::styled(format!("{indent}{line}"), rouge)))
        .collect()
}

// 💬 a murmur: what is happening, said plainly to whoever is listening (stdout)
pub fn murmur(words: impl std::fmt::Display) {
    let rouge = Rouge::new().fg(theme::worn().colors.text);
    utter(std::io::stdout(), verses(&words.to_string(), rouge, ""));
}

// 📣 a proclamation: a heading over what follows, bold, in the theme's title color
pub fn proclaim(words: impl std::fmt::Display) {
    let rouge = Rouge::new()
        .fg(theme::worn().colors.title)
        .add_modifier(Flair::BOLD);
    utter(std::io::stdout(), verses(&words.to_string(), rouge, ""));
}

// 🌫️ an aside: something said in passing, faint and slanted, like a breath on the glass
pub fn aside(words: impl std::fmt::Display) {
    let rouge = Rouge::new()
        .fg(theme::worn().colors.muted)
        .add_modifier(Flair::ITALIC);
    utter(
        std::io::stdout(),
        verses(&words.to_string(), rouge, "     "),
    );
}

// 😱 a wail: something went wrong or was gone along with, said to the night (stderr)
pub fn wail(words: impl std::fmt::Display) {
    let rouge = Rouge::new().fg(theme::worn().colors.warning);
    utter(std::io::stderr(), verses(&words.to_string(), rouge, ""));
}

// 🪄 the planchette spins on the last line while the table waits for a sign, and says how long
// it has been waiting. only a terminal sees it: a file would just fill up with spinning
pub fn planchette(waiting_for: &str, waited: std::time::Duration) {
    use ratatui::crossterm::style::Stylize;
    let mut out = std::io::stdout();
    if curtained() || !std::io::IsTerminal::is_terminal(&out) {
        return;
    }
    let stop = (waited.as_millis() / 100) as usize % PLANCHETTE.len();
    let _ = write!(
        out,
        "\r\x1b[2K{}",
        format!(
            "     {} {waiting_for} · {:.1}s",
            PLANCHETTE[stop],
            waited.as_secs_f64()
        )
        .with(tint(theme::worn().colors.busy))
    );
    let _ = out.flush();
    SPINNING.store(true, std::sync::atomic::Ordering::Relaxed);
}

// 🤫 the planchette stops and its line is wiped, so the next words start clean
pub fn hush() {
    if SPINNING.swap(false, std::sync::atomic::Ordering::Relaxed) {
        let mut out = std::io::stdout();
        let _ = write!(out, "\r\x1b[2K");
        let _ = out.flush();
    }
}

// 🌅 the spirits rest (✓) or the circle is broken (✗): how the séance ended
pub fn farewell(peaceful: bool, words: &str) {
    let colors = &theme::worn().colors;
    let (sign, hue, out_hue) = if peaceful {
        ("✓", colors.title, colors.text)
    } else {
        ("✗", colors.warning, colors.warning)
    };
    let mut said: Vec<Verse<'static>> = Vec::new();
    for (index, line) in words.lines().enumerate() {
        let lead = if index == 0 {
            format!("   {sign} ")
        } else {
            "     ".to_string()
        };
        said.push(Verse::from(vec![
            Syllable::styled(lead, Rouge::new().fg(hue).add_modifier(Flair::BOLD)),
            Syllable::styled(line.to_string(), Rouge::new().fg(out_hue)),
        ]));
    }
    if peaceful {
        utter(std::io::stdout(), said);
    } else {
        utter(std::io::stderr(), said);
    }
}

// 🪦👻 THE POSSESSION 👻🪦
// `tdl headless`: no one sits at the keyboard, so a spirit does. it presses keys and buttons from a
// script (a file, or stdin a line at a time), and every ritual hears them exactly as it would hear
// a person's: through `listen` and `hear`, which stand in for crossterm's poll and read wherever
// tdl waits for a key. nobody watches the pages, so none are drawn (see `bewitch`). with `--log`
// the spirit prays: it confesses every press as it makes it, on the terminal and in its prayer
// (tdl.log): the key or button, the ritual that heard it and what that ritual does with it, named
// from its legend; and what UZDoom says goes into the prayer from beyond (uzdoom.log) beside it.
// a script line is presses (keys like `j`, `enter`, `shift+b`, `ctrl+c`, and buttons the way the
// legends' aliases write them, `⟨a⟩`, `⟨lb+a⟩`, `⟨rb+down⟩`), separated by spaces, or `wait 2s`;
// `#` starts a comment. a ` speaks the rest of its line as a `tdl cli` command: the rituals close
// for it (`Curse::Rite`), and open again after

/// One press of the possession: how the script wrote it, and the key it makes, or for ` the
/// `tdl cli` command spoken after it.
struct Press {
    label: String,
    portent: Result<Portent, String>,
}

/// The spirit at the keyboard, while one is.
struct Possession {
    /// The script's lines, numbered from 1, as they come.
    lines: std::sync::mpsc::Receiver<(usize, String)>,
    /// Presses read and not heard yet.
    presses: std::collections::VecDeque<Press>,
    /// When a `wait` is over, and when it began.
    until: Option<(std::time::Instant, std::time::Instant)>,
    /// How the last press heard was written, for its confession.
    last: String,
    /// The spirit's prayer (tdl.log), and when the possession began, with `--log`.
    prayer: Option<(std::fs::File, std::time::Instant)>,
}

std::thread_local! {
    static SPIRIT: std::cell::RefCell<Option<Possession>> = const { std::cell::RefCell::new(None) };
}

/// Where the prayers are kept (`--log`'s dir), once a spirit has the keyboard.
static CONFESSIONAL: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

// 🕯️ the spirit takes the keyboard: its script from `script` (stdin without one), its prayers in
// `prayers` (tdl.log and uzdoom.log) if given. from here on every key tdl hears is the spirit's
pub fn possess(
    script: Option<&std::path::Path>,
    prayers: Option<&std::path::Path>,
) -> std::io::Result<()> {
    let source: Box<dyn std::io::Read + Send> = match script {
        Some(script) => Box::new(std::fs::File::open(script).map_err(|error| {
            std::io::Error::new(error.kind(), format!("{}: {error}", script.display()))
        })?),
        None => Box::new(std::io::stdin()),
    };
    // 📜 read on a thread of its own, so a script typed line by line never holds a ritual up
    let (sender, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for (index, line) in std::io::BufReader::new(source).lines().enumerate() {
            let Yass(line) = line else { break };
            if sender.send((index + 1, line)).is_err() {
                break;
            }
        }
    });
    let prayer = match prayers {
        Some(dir) => {
            std::fs::create_dir_all(dir)?;
            std::fs::File::create(dir.join("uzdoom.log"))?;
            let _ = CONFESSIONAL.set(dir.to_path_buf());
            let mut file = std::fs::File::create(dir.join("tdl.log"))?;
            let read = script.map_or("stdin".into(), |script| script.display().to_string());
            writeln!(
                file,
                "tdl {} headless, pressing {read}",
                env!("CARGO_PKG_VERSION")
            )?;
            Some((file, std::time::Instant::now()))
        }
        None => None,
    };
    SPIRIT.with_borrow_mut(|spirit| {
        *spirit = Some(Possession {
            lines,
            presses: std::collections::VecDeque::new(),
            until: None,
            last: String::new(),
            prayer,
        })
    });
    Yass(BOO)
}

// 👻 is a spirit at the keyboard?
pub fn possessed() -> bool {
    SPIRIT.with_borrow(Option::is_some)
}

// 👂 like crossterm's poll: whether a key is ready within `patience`. possessed, the keys are the
// spirit's, and its waits are waited on the way to the next one
pub fn listen(patience: std::time::Duration) -> Omen<bool> {
    if !possessed() {
        return Yass(portent::poll(patience)?);
    }
    SPIRIT.with_borrow_mut(|spirit| spirit.as_mut().unwrap().listen(patience))
}

// 🗣️ like crossterm's read: the next key, the spirit's while it has the keyboard
pub fn hear() -> Omen<Portent> {
    if !possessed() {
        return Yass(portent::read()?);
    }
    loop {
        if listen(std::time::Duration::from_secs(1))? {
            let press = SPIRIT.with_borrow_mut(|spirit| {
                let spirit = spirit.as_mut().unwrap();
                let press = spirit.presses.pop_front()?;
                spirit.last = press.label;
                Some(press.portent)
            });
            match press {
                Some(Yass(portent)) => return Yass(portent),
                Some(Err(rite)) => return Err(Curse::Rite(rite)),
                None => {}
            }
        }
    }
}

// 🏚️ a whole haunting: the spirit takes the keyboard (`possess`), the launcher opens on a page in
// the void with no splash and no music, and the spirit presses its script into it; when it speaks
// a rite (`), the launcher closes, the rite is done (`speak`), and it opens again; the launcher
// quitting, the script running out or Ctrl+C ends it, and the spirit is exorcised. returns
// whether it ended well
pub fn haunt(
    script: Option<&std::path::Path>,
    prayers: Option<&std::path::Path>,
    start: crate::screens::launcher::Start,
) -> bool {
    use crate::screens::launcher::{Launcher, Start};
    let outcome = possess(script, prayers)
        .and_then(|()| void_grimoire(100, 30))
        .map_err(|error| error.to_string())
        .and_then(|mut grimoire| {
            let start = Start {
                splash: false,
                no_music: true,
                ..start
            };
            loop {
                match Launcher::summon(&mut grimoire, start.clone()) {
                    // 🚪 the launcher quit, the spirit's script ran out, or it pressed Ctrl+C
                    Yass(()) | Err(Curse::ScriptEnd | Curse::Interrupted) => break Yass(()),
                    // 📜 a spoken rite: done on the loadout, then the launcher opens again
                    Err(Curse::Rite(spoken)) => speak(&start, &spoken),
                    Err(curse) => break Err(curse.to_string()),
                }
            }
        });
    exorcise(&outcome);
    outcome.is_ok()
}

// 🗣️ the `tdl cli` command (or `sh` and a shell command) the spirit spoke after a `, confessed and
// done, with what it says said through the séance. one that fails is wailed about, and the
// possession carries on
fn speak(start: &crate::screens::launcher::Start, spoken: &str) {
    confess("tdl cli", Some(format!("tdl cli {spoken}")));
    // 🐚 `sh ...` runs a shell command, behind the same guard rails as the launcher's command line
    let outcome = match spoken.strip_prefix("sh ") {
        Some(command) => crate::slayer::commands::shell(command),
        None => crate::cli::Rite::from_phrase(spoken)
            .and_then(|rite| crate::screens::launcher::Launcher::rite(start.clone(), rite)),
    };
    if let Err(problem) = outcome {
        wail(problem);
    }
}

// 🙏 the spirit confesses the press just heard: which ritual heard it, and what it did there
// (`None` is nothing). only when it prays (`--log`): on the terminal and in its prayer
pub fn confess(heard_by: &str, deed: Option<String>) {
    // 🙏 written in the prayer while the spirit is held, said once it is let go: saying it draws
    // on parchment, and `bewitch` asks after the spirit
    let press = SPIRIT.with_borrow_mut(|spirit| {
        let spirit = spirit.as_mut()?;
        let (file, began) = spirit.prayer.as_mut()?;
        let line = format!(
            "{:<12} {heard_by:<18} → {}",
            spirit.last,
            deed.as_deref().unwrap_or("nothing")
        );
        let _ = writeln!(file, "[{:8.3}s] {line}", began.elapsed().as_secs_f64());
        Some(spirit.last.clone())
    });
    if let Some(press) = press {
        confession(&press, heard_by, deed.as_deref());
    }
}

// 📜 a spoken rite's words, split the way a shell would at spaces, keeping "quoted words" together
pub fn rite_words(spoken: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut started) = (false, false);
    for c in spoken.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err(format!("a quote isn't closed in `{spoken}"));
    }
    if started {
        words.push(word);
    }
    Yass(words)
}

// 🫢 something the spirit should know that no press caused (a file dialog it can't answer), told
// the same way as a confession, when it prays
pub fn whisper(words: &str) {
    let prays = SPIRIT.with_borrow_mut(|spirit| {
        let Some((file, began)) = spirit.as_mut().and_then(|spirit| spirit.prayer.as_mut()) else {
            return false;
        };
        let _ = writeln!(file, "[{:8.3}s] {words}", began.elapsed().as_secs_f64());
        true
    });
    if prays {
        aside(words);
    }
}

// ⚰️ the prayer from beyond (uzdoom.log), opened to add UZDoom's words to, if the spirit prays
pub fn prayer_from_beyond() -> Option<std::fs::File> {
    let dir = CONFESSIONAL.get()?;
    std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(dir.join("uzdoom.log"))
        .ok()
}

// 🌄 the spirit lets go: how the possession ended, said on the terminal (a failure always, the
// rest when it prays) and written at the end of its prayer
pub fn exorcise(outcome: &Result<(), String>) {
    let logged = SPIRIT.with_borrow_mut(|spirit| {
        let Some((file, began)) = spirit.as_mut().and_then(|spirit| spirit.prayer.as_mut()) else {
            return false;
        };
        let ending = match outcome {
            Yass(()) => "done".to_string(),
            Err(problem) => format!("ended: {problem}"),
        };
        let _ = writeln!(file, "[{:8.3}s] {ending}", began.elapsed().as_secs_f64());
        true
    });
    match outcome {
        Yass(()) if logged => farewell(true, "the spirit is at rest"),
        Yass(()) => {}
        Err(problem) => farewell(false, problem),
    }
}

// 🏷️ a ritual's name, the way a confession says it: `ProfileNamer` heard it as "profile namer"
pub fn spirit_name<Soul: ?Sized>() -> String {
    let full = std::any::type_name::<Soul>();
    let bare = full.split('<').next().unwrap_or(full);
    let bare = bare.rsplit("::").next().unwrap_or(bare);
    let mut name = String::new();
    for c in bare.chars() {
        if c.is_uppercase() && !name.is_empty() {
            name.push(' ');
        }
        name.push(c.to_ascii_lowercase());
    }
    name
}

// 📜 what `chant` does according to `legend`: the text of the first line whose keys take it
pub fn deed_in(legend: &[[&'static str; 2]], chant: Chant) -> Option<String> {
    legend
        .iter()
        .find(|[keys, _]| keys_take(&familiar::plain(keys), chant) || aliases_take(keys, chant))
        .map(|[_, deed]| (*deed).to_string())
        // 📖 F1 (Start on a controller) shows the legend itself, which never lists it
        .or_else(|| (chant == Chant::F(1) && !legend.is_empty()).then(|| "Show the legend".into()))
}

// 🎮 whether one of a legend's button aliases (`H⟨host⟩`, `X⟨home|lb+b⟩`) makes `chant`: a button
// of its own, like the host trigger's F2, makes a key the legend doesn't name
fn aliases_take(keys: &str, chant: Chant) -> bool {
    keys.split('⟨')
        .skip(1)
        .filter_map(|rest| rest.split_once('⟩').map(|(aliases, _)| aliases))
        .flat_map(|aliases| aliases.split('|'))
        .any(|alias| Gesture::press(alias) == Some(chant))
}

// 🔑 whether a legend's keys (`J↓/K↑`, `Y/Shift+Y`, `A-Z/0-9/_-`, `←/→/Home/End`, `/`) take `chant`
fn keys_take(keys: &str, chant: Chant) -> bool {
    if keys == "/" {
        return chant == Chant::Char('/');
    }
    keys.split(['/', ' '])
        // 🧩 `Tab/↓, Backtab/↑` separates its groups with a comma, which `,` alone is the key of
        .map(|token| {
            if token.len() > 1 {
                token.trim_end_matches(',')
            } else {
                token
            }
        })
        .filter(|token| !token.is_empty())
        .any(|token| key_takes(token, chant))
}

// 🗝️ one legend key against `chant`
fn key_takes(token: &str, chant: Chant) -> bool {
    let named = match token {
        "Enter" => Some(Chant::Enter),
        "Esc" => Some(Chant::Esc),
        "Tab" => Some(Chant::Tab),
        "Backtab" | "Shift+Tab" => Some(Chant::BackTab),
        "Space" => Some(Chant::Char(' ')),
        "Home" => Some(Chant::Home),
        "End" => Some(Chant::End),
        "PgUp" | "PageUp" => Some(Chant::PageUp),
        "PgDn" | "PageDown" => Some(Chant::PageDown),
        "Delete" | "Del" => Some(Chant::Delete),
        "Backspace" => Some(Chant::Backspace),
        _ => None,
    };
    if let Some(named) = named {
        return named == chant;
    }
    if let Some(number) = token.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
        return chant == Chant::F(number);
    }
    if let Some(shifted) = token.strip_prefix("Shift+") {
        return shifted.chars().count() == 1
            && chant == Chant::Char(shifted.chars().next().unwrap().to_ascii_uppercase());
    }
    // 🔤 a range like A-Z or 0-9, or the two characters of `_-`
    let chars: Vec<char> = token.chars().collect();
    if let [from, '-', to] = chars[..]
        && let Chant::Char(c) = chant
    {
        return (from.to_ascii_lowercase()..=to.to_ascii_lowercase())
            .contains(&c.to_ascii_lowercase());
    }
    chars.iter().any(|&c| match c {
        '↑' => chant == Chant::Up,
        '↓' => chant == Chant::Down,
        '←' => chant == Chant::Left,
        '→' => chant == Chant::Right,
        // 🔡 a legend's letter is the plain key; Shift+ marks the capital
        c if c.is_ascii_alphabetic() => chant == Chant::Char(c.to_ascii_lowercase()),
        c => chant == Chant::Char(c),
    })
}

// 🖋️ a confession on the terminal: the press in the key color, the rest in the text color
fn confession(press: &str, heard_by: &str, deed: Option<&str>) {
    let colors = &theme::worn().colors;
    let deed = match deed {
        Some(deed) => Syllable::styled(deed.to_string(), Rouge::new().fg(colors.text)),
        None => Syllable::styled(
            "nothing",
            Rouge::new().fg(colors.muted).add_modifier(Flair::ITALIC),
        ),
    };
    let verse = Verse::from(vec![
        Syllable::styled(
            format!("  {press:<12} "),
            Rouge::new().fg(colors.key).add_modifier(Flair::BOLD),
        ),
        Syllable::styled(format!("{heard_by:<18} "), Rouge::new().fg(colors.muted)),
        Syllable::styled("→ ", Rouge::new().fg(colors.muted)),
        deed,
    ]);
    utter(std::io::stdout(), vec![verse]);
}

impl Possession {
    fn listen(&mut self, patience: std::time::Duration) -> Omen<bool> {
        loop {
            // 📜 a spoken rite is handed over before anything else can hear it
            if let Some(Press {
                portent: Err(_), ..
            }) = self.presses.front()
            {
                let press = self.presses.pop_front().unwrap();
                self.last = press.label;
                return Err(Curse::Rite(press.portent.unwrap_err()));
            }
            if !self.presses.is_empty() {
                return Yass(true);
            }
            if let Some((began, until)) = self.until {
                let now = std::time::Instant::now();
                if now >= until {
                    self.until = None;
                    hush();
                    continue;
                }
                if self.prayer.is_some() {
                    planchette("waiting", began.elapsed());
                }
                std::thread::sleep(patience.min(until - now));
                return Yass(false);
            }
            let (number, text) = match self.lines.recv_timeout(patience) {
                Yass(line) => line,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => return Yass(false),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(Curse::ScriptEnd);
                }
            };
            let text = text.trim();
            if text.is_empty() || text.starts_with('#') {
                continue;
            }
            if let Some(how_long) = text.strip_prefix("wait") {
                let how_long =
                    duration(how_long).map_err(|problem| self.failed(number, &problem))?;
                let now = std::time::Instant::now();
                self.until = Some((now, now + how_long));
                continue;
            }
            // 🗣️ a ` speaks the rest of the line as a command, after the presses before it
            let (presses, rite) = match text.split_once('`') {
                Some((presses, rite)) => (presses, Some(rite.trim())),
                None => (text, None),
            };
            let mut heard = Vec::new();
            for label in presses.split_whitespace() {
                let portent = press(label).map_err(|problem| self.failed(number, &problem))?;
                heard.push(Press {
                    label: label.to_string(),
                    portent: Yass(portent),
                });
            }
            if let Some(rite) = rite {
                if rite.is_empty() {
                    return Err(self.failed(number, "` needs a command after it, like `profiles"));
                }
                heard.push(Press {
                    label: "`".into(),
                    portent: Err(rite.to_string()),
                });
            }
            self.presses.extend(heard);
        }
    }

    // ✗ line `number` couldn't be read: the curse, written in the prayer too
    fn failed(&mut self, number: usize, problem: &str) -> Curse {
        let problem = format!("line {number}: {problem}");
        if let Some((file, began)) = &mut self.prayer {
            let _ = writeln!(
                file,
                "[{:8.3}s] failed: {problem}",
                began.elapsed().as_secs_f64()
            );
        }
        Curse::Script(problem)
    }
}

// ⏳ a wait's time: `2s`, `500ms` or `2` (seconds)
fn duration(text: &str) -> Result<std::time::Duration, String> {
    let text = text.trim();
    let (number, scale) = match (text.strip_suffix("ms"), text.strip_suffix('s')) {
        (Some(number), _) => (number, 0.001),
        (None, Some(number)) => (number, 1.0),
        _ => (text, 1.0),
    };
    number
        .parse::<f64>()
        .ok()
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .map(|seconds| std::time::Duration::from_secs_f64(seconds * scale))
        .ok_or_else(|| format!("wait needs a time, like wait 2s or wait 500ms, not \"{text}\""))
}

// ⌨️ a press as the script writes it: a key (`j`, `enter`, `shift+b`, `ctrl+c`, `f1`, `space`), or
// buttons the way the legends' aliases write them (`⟨a⟩`, `⟨lb+a⟩`, `⟨lb+rb+down⟩`, `⟨host⟩`;
// `<lb+a>` for a keyboard without the angle brackets)
fn press(label: &str) -> Result<Portent, String> {
    use ratatui::crossterm::event::KeyEvent as Keystroke;
    let aliases = label
        .strip_prefix('⟨')
        .and_then(|label| label.strip_suffix('⟩'))
        .or_else(|| {
            label
                .strip_prefix('<')
                .and_then(|label| label.strip_suffix('>'))
        });
    if let Some(aliases) = aliases {
        return Gesture::press(&aliases.to_ascii_lowercase())
            .map(|chant| Portent::Key(Keystroke::new(chant, Charm::NONE)))
            .ok_or_else(|| {
                format!(
                    "{label} isn't buttons the way a legend writes them, like ⟨a⟩, ⟨lb+a⟩ or \
                     ⟨lb+rb+down⟩ (a b x y lb rb lt rt l3 r3 home start select up down left right \
                     host join)"
                )
            });
    }
    let mut parts: Vec<&str> = label.split('+').collect();
    // ➕ `+` and `shift++` are the plus key itself
    if label == "+" || label.ends_with("++") {
        parts.retain(|part| !part.is_empty());
        parts.push("+");
    }
    let last = parts.pop().unwrap_or_default();
    let mut charms = Charm::NONE;
    for part in parts {
        charms |= match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Charm::CONTROL,
            "shift" => Charm::SHIFT,
            "alt" => Charm::ALT,
            _ => return Err(format!("{part} in {label} isn't ctrl, shift or alt")),
        };
    }
    let chant = match last.to_ascii_lowercase().as_str() {
        "enter" | "return" => Chant::Enter,
        "esc" | "escape" => Chant::Esc,
        "tab" if charms.contains(Charm::SHIFT) => Chant::BackTab,
        "tab" => Chant::Tab,
        "backtab" => Chant::BackTab,
        "up" => Chant::Up,
        "down" => Chant::Down,
        "left" => Chant::Left,
        "right" => Chant::Right,
        "home" => Chant::Home,
        "end" => Chant::End,
        "pageup" | "pgup" => Chant::PageUp,
        "pagedown" | "pgdn" => Chant::PageDown,
        "delete" | "del" => Chant::Delete,
        "backspace" => Chant::Backspace,
        "insert" => Chant::Insert,
        "space" => Chant::Char(' '),
        // 🗣️ the ` key itself, which opens the launcher's command line (a bare ` speaks instead)
        "backtick" | "grave" => Chant::Char('`'),
        f if f.len() > 1
            && f.starts_with('f')
            && let Yass(number) = f[1..].parse::<u8>() =>
        {
            Chant::F(number)
        }
        _ => {
            let mut chars = last.chars();
            match (chars.next(), chars.next()) {
                // 🔠 a shifted letter comes as the capital, the way terminals send it
                (Some(c), None) if charms.contains(Charm::SHIFT) => {
                    Chant::Char(c.to_ascii_uppercase())
                }
                (Some(c), None) => Chant::Char(c),
                _ => return Err(format!("{label} isn't a key or a button")),
            }
        }
    };
    Yass(Portent::Key(Keystroke::new(chant, charms)))
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
/// with either key (F1 too, for the fields where `?` is something to type), or the controller's
/// own button for it when a controller was the last to speak
pub fn conjure_sigil(window_area: Plot) -> (Verse<'static>, Plot) {
    let bracket = Rouge::new().fg(theme::worn().colors.muted);
    let key = Rouge::new().fg(theme::worn().colors.key);
    let words = match familiar::shown() {
        // 🐈‍⬛ the familiar only knows one way to ask
        Some(family) => vec![Syllable::styled(Gesture::Keys.label(family), key)],
        None => vec![
            Syllable::styled("F1", key),
            Syllable::styled("/", bracket),
            Syllable::styled("?", key),
        ],
    };
    // 🔘 a button is named in parentheses, which say as much as brackets would
    let named = familiar::shown().is_some();
    let (open, close) = if named { ("", "") } else { ("[", "]") };
    let sigil = std::iter::once(Syllable::styled(open, bracket))
        .chain(words)
        .chain([Syllable::styled(close, bracket)])
        .collect::<Verse>()
        .centered();
    let sigil_area = Plot::new(window_area.x, window_area.bottom(), window_area.width, 1);
    (sigil, sigil_area)
}
