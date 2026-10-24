//! Game controllers, through gilrs. Their buttons, d-pad and left stick become the keys tdl already
//! knows ([`chant`]), so every screen and popup works with one without knowing it is there, and
//! once a controller is used the legends show its buttons ([`relabel`]) until a key is pressed.
//!
//! A controller that is plugged in when tdl starts (or later) is recognized at once, so the legends
//! show its buttons from the first screen, until a key is pressed.
//!
//! Buttons are always drawn as text in parentheses, `(A)` or `(LB)`, so that a button can't be
//! mistaken for the key of the same name.

use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, VecDeque},
    io::Write,
    sync::atomic::{AtomicU8, Ordering},
    time::{Duration, Instant},
};

use gilrs::{Axis, Button, EventType, Gilrs, MappingSource};
use ratatui::crossterm::event::KeyCode;

/// A kind of controller, which says what its buttons are called and where its confirm button is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Family {
    Xbox = 1,
    PlayStation,
    Nintendo,
    SteamDeck,
    SteamController,
    Generic,
}

impl Family {
    pub const ALL: [Family; 6] = [
        Family::Xbox,
        Family::PlayStation,
        Family::Nintendo,
        Family::SteamDeck,
        Family::SteamController,
        Family::Generic,
    ];

    /// Tells a controller apart by its USB vendor and product, or failing that by its name.
    pub fn of(name: &str, vendor: Option<u16>, product: Option<u16>) -> Self {
        match (vendor, product) {
            (Some(0x045e), _) => return Family::Xbox,
            (Some(0x054c), _) => return Family::PlayStation,
            (Some(0x057e), _) => return Family::Nintendo,
            (Some(0x28de), Some(0x1205)) => return Family::SteamDeck,
            (Some(0x28de), Some(0x1102 | 0x1142)) => return Family::SteamController,
            // 🎮 Steam Input's virtual gamepad pretends to be an Xbox 360 one
            (Some(0x28de), _) => return Family::Xbox,
            _ => {}
        }
        let name = name.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|word| name.contains(word));
        if has(&["steam deck"]) {
            Family::SteamDeck
        } else if has(&["steam controller"]) {
            Family::SteamController
        } else if has(&["xbox", "x-box", "xinput"]) {
            Family::Xbox
        } else if has(&[
            "playstation",
            "dualshock",
            "dualsense",
            "ps3",
            "ps4",
            "ps5",
            "sony",
        ]) {
            Family::PlayStation
        } else if has(&["nintendo", "switch", "joy-con", "pro controller"]) {
            Family::Nintendo
        } else {
            Family::Generic
        }
    }

    fn from_u8(byte: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|family| *family as u8 == byte)
    }
}

/// What a button does in tdl, whichever controller it is on: the thing a legend shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Prompt {
    /// `Enter`: A on Xbox and Nintendo (where it is the right button), Cross on PlayStation.
    Confirm,
    /// `Esc`: B, or Circle.
    Back,
    /// `X`, cut: the top face button.
    Cut,
    /// `/`: the left face button.
    Search,
    /// `Shift+Tab` and `Tab`: the shoulder buttons.
    Previous,
    Next,
    /// `F2` and `F3`: the triggers, which host and join a game on the launcher.
    Host,
    Join,
    /// `F1`: Start (Menu, Options, +).
    Keys,
    /// `W`: Select (View, Share, −), which opens the wads menu where there is one.
    Wads,
    /// `Y` and `P`: pressing the left and the right stick in.
    Copy,
    Paste,
    /// `O`: the Home button (Xbox, PS, Steam), which opens the settings.
    Settings,
    /// The arrows: the d-pad, or the left stick.
    Up,
    Down,
    Left,
    Right,
}

impl Prompt {
    /// The key a press of this means.
    pub fn chant(self) -> KeyCode {
        match self {
            Prompt::Confirm => KeyCode::Enter,
            Prompt::Back => KeyCode::Esc,
            Prompt::Cut => KeyCode::Char('x'),
            Prompt::Search => KeyCode::Char('/'),
            Prompt::Previous => KeyCode::BackTab,
            Prompt::Next => KeyCode::Tab,
            Prompt::Host => KeyCode::F(2),
            Prompt::Join => KeyCode::F(3),
            Prompt::Keys => KeyCode::F(1),
            Prompt::Wads => KeyCode::Char('w'),
            Prompt::Copy => KeyCode::Char('y'),
            Prompt::Paste => KeyCode::Char('p'),
            Prompt::Settings => KeyCode::Char('o'),
            Prompt::Up => KeyCode::Up,
            Prompt::Down => KeyCode::Down,
            Prompt::Left => KeyCode::Left,
            Prompt::Right => KeyCode::Right,
        }
    }

    /// What a button means on a controller of `family`. gilrs names the face buttons by where they
    /// are, so on a Nintendo one, whose A is on the right, confirm and back swap places.
    pub fn of(button: Button, family: Family) -> Option<Self> {
        let nintendo = family == Family::Nintendo;
        Some(match button {
            Button::South if nintendo => Prompt::Back,
            Button::East if nintendo => Prompt::Confirm,
            Button::South => Prompt::Confirm,
            Button::East => Prompt::Back,
            Button::West => Prompt::Search,
            Button::North => Prompt::Cut,
            Button::LeftTrigger => Prompt::Previous,
            Button::RightTrigger => Prompt::Next,
            Button::LeftTrigger2 => Prompt::Join,
            Button::RightTrigger2 => Prompt::Host,
            Button::Start => Prompt::Keys,
            Button::Select => Prompt::Wads,
            Button::LeftThumb => Prompt::Copy,
            Button::RightThumb => Prompt::Paste,
            Button::Mode => Prompt::Settings,
            Button::DPadUp => Prompt::Up,
            Button::DPadDown => Prompt::Down,
            Button::DPadLeft => Prompt::Left,
            Button::DPadRight => Prompt::Right,
            _ => return None,
        })
    }

    /// The keyboard key a legend names, as a button: `Enter`, `Esc`, `J↓`, `Tab` and so on. Keys a
    /// controller has nothing for (letters that do one thing on one screen) are `None`.
    fn named(key: &str) -> Option<Self> {
        Some(match key {
            "Enter" => Prompt::Confirm,
            "Esc" => Prompt::Back,
            "X" => Prompt::Cut,
            "/" => Prompt::Search,
            "Backtab" | "Shift+Tab" => Prompt::Previous,
            "Tab" => Prompt::Next,

            "?" | "F1" => Prompt::Keys,
            "W" => Prompt::Wads,
            "Y" => Prompt::Copy,
            "P" => Prompt::Paste,
            "K" | "K↑" | "↑" => Prompt::Up,
            "J" | "J↓" | "↓" => Prompt::Down,
            "←" => Prompt::Left,
            "→" => Prompt::Right,
            _ => return None,
        })
    }

    /// The button an alias names: `a`, `b`, `x` and `y` for the face buttons (where Xbox has them),
    /// `lb`, `rb`, `lt`, `rt`, `l3`, `r3`, `start`, `select`, the four d-pad directions, and `host`
    /// and `join` for the triggers.
    fn alias(name: &str) -> Option<Self> {
        Some(match name {
            "a" => Prompt::Confirm,
            "b" => Prompt::Back,
            "x" => Prompt::Search,
            "y" => Prompt::Cut,
            "lb" => Prompt::Previous,
            "rb" => Prompt::Next,
            "lt" | "join" => Prompt::Join,
            "rt" | "host" => Prompt::Host,
            "l3" => Prompt::Copy,
            "r3" => Prompt::Paste,
            "home" => Prompt::Settings,
            "start" => Prompt::Keys,
            "select" => Prompt::Wads,
            "up" => Prompt::Up,
            "down" => Prompt::Down,
            "left" => Prompt::Left,
            "right" => Prompt::Right,
            _ => return None,
        })
    }

    /// `H⟨host⟩`, `A⟨lb+a⟩` or `X⟨home|lb+b⟩`: a legend key that is `H` or `A` on the keyboard and is the buttons
    /// named inside the ⟨⟩ (joined by `+` for a chord) on a controller of `family`.
    fn aliased(key: &str, family: Family) -> Option<String> {
        let (_, alias) = key.split_once('⟨')?;
        // ➗ `home|lb+b`: either of two ways, which are both shown
        let ways: Option<Vec<String>> = alias
            .trim_end_matches('⟩')
            .split('|')
            .map(|way| {
                let buttons: Option<Vec<String>> = way
                    .split('+')
                    .map(|name| Prompt::alias(name).map(|prompt| prompt.label(family)))
                    .collect();
                buttons.map(|buttons| buttons.join("+"))
            })
            .collect();
        ways.map(|ways| ways.join("/"))
    }

    /// The button's name on a controller of `family`, in plain text.
    pub fn text(self, family: Family) -> &'static str {
        use Family::*;
        match (self, family) {
            (Prompt::Confirm, PlayStation) => "✕",
            (Prompt::Back, PlayStation) => "○",
            (Prompt::Search, PlayStation) => "□",
            (Prompt::Cut, PlayStation) => "△",
            (Prompt::Confirm, _) => "A",
            (Prompt::Back, _) => "B",
            (Prompt::Search, Nintendo) => "Y",
            (Prompt::Cut, Nintendo) => "X",
            (Prompt::Search, _) => "X",
            (Prompt::Cut, _) => "Y",
            (Prompt::Previous, PlayStation | SteamDeck) => "L1",
            (Prompt::Next, PlayStation | SteamDeck) => "R1",
            (Prompt::Join, PlayStation | SteamDeck) => "L2",
            (Prompt::Host, PlayStation | SteamDeck) => "R2",
            (Prompt::Previous, Nintendo) => "L",
            (Prompt::Next, Nintendo) => "R",
            (Prompt::Join, Nintendo) => "ZL",
            (Prompt::Host, Nintendo) => "ZR",
            (Prompt::Previous, _) => "LB",
            (Prompt::Next, _) => "RB",
            (Prompt::Join, _) => "LT",
            (Prompt::Host, _) => "RT",
            (Prompt::Keys, Xbox | SteamDeck) => "Menu",
            (Prompt::Keys, PlayStation) => "Options",
            (Prompt::Keys, Nintendo) => "+",
            (Prompt::Keys, _) => "Start",
            (Prompt::Wads, Xbox | SteamDeck) => "View",
            (Prompt::Wads, PlayStation) => "Share",
            (Prompt::Wads, Nintendo) => "−",
            (Prompt::Wads, _) => "Select",
            (Prompt::Settings, Xbox) => "Xbox",
            (Prompt::Settings, PlayStation) => "PS",
            (Prompt::Settings, SteamDeck) => "Steam",
            (Prompt::Settings, _) => "Home",
            (Prompt::Copy, _) => "L3",
            (Prompt::Paste, _) => "R3",
            (Prompt::Up, _) => "D↑",
            (Prompt::Down, _) => "D↓",
            (Prompt::Left, _) => "D←",
            (Prompt::Right, _) => "D→",
        }
    }

    /// The button as drawn: its name in parentheses (`(A)`), so `Enter` shown as a button can't be
    /// mistaken for the key `A`.
    pub fn label(self, family: Family) -> String {
        format!("({})", self.text(family))
    }
}

/// A legend's key, such as `Enter` or `Tab/↓, Backtab/↑`, with every key a controller has a
/// button for named as that button on `family`. The rest are left as they are, for the keyboard.
pub fn relabel(label: &str, family: Family) -> String {
    let mut out = String::new();
    let mut rest = label;
    while let Some(first) = rest.chars().next() {
        // 🔪 a lone `/` is the key itself; one between two keys only separates them
        let alone = first == '/' && (out.is_empty() || out.ends_with(' '));
        // 🔑 a `,` right before a `⟨` is the key of that alias, as in `,⟨rb+left⟩`
        let comma_alias = first == ',' && rest[1..].starts_with('⟨');
        let separator = !alone && !comma_alias && matches!(first, '/' | ',' | ' ');
        let end = if alone || separator {
            first.len_utf8()
        } else if comma_alias {
            1 + rest[1..].find(['/', ',', ' ']).unwrap_or(rest.len() - 1)
        } else {
            rest.find(['/', ',', ' ']).unwrap_or(rest.len())
        };
        let (key, after) = rest.split_at(end);
        let button = match key.contains('⟨') {
            true => Prompt::aliased(key, family),
            false => Prompt::named(key).map(|prompt| prompt.label(family)),
        };
        match button.filter(|_| !separator) {
            Some(button) => out.push_str(&button),
            None => out.push_str(key),
        }
        rest = after;
    }
    out
}

/// A legend's key as the keyboard has it: without the `⟨alias⟩` that names a controller's button.
pub fn plain(label: &str) -> Cow<'_, str> {
    if !label.contains('⟨') {
        return Cow::Borrowed(label);
    }
    let mut out = String::new();
    let mut skipping = false;
    for c in label.chars() {
        match c {
            '⟨' => skipping = true,
            '⟩' => skipping = false,
            c if !skipping => out.push(c),
            _ => {}
        }
    }
    Cow::Owned(out)
}

/// What was used last: 0 for the keyboard, or a controller's [`Family`].
static LAST: AtomicU8 = AtomicU8::new(0);

/// The family of the controller used last, if one was used after the last key was pressed.
pub fn shown() -> Option<Family> {
    Family::from_u8(LAST.load(Ordering::Relaxed))
}

/// Marks the keyboard as used last, and says whether it wasn't already.
pub fn keyboard_used() -> bool {
    LAST.swap(0, Ordering::Relaxed) != 0
}

fn controller_used(family: Family) -> bool {
    // 🧪 a test's pretend controller must not change what every other test's legends look like
    if cfg!(test) {
        return false;
    }
    LAST.swap(family as u8, Ordering::Relaxed) != family as u8
}

/// Looks for controllers now, rather than at the first key wait, so one that is plugged in is
/// known before the first screen is drawn. Does nothing in tests, so a controller on the
/// developer's desk doesn't change a snapshot.
pub fn start() {
    if !cfg!(test) {
        PADS.with_borrow_mut(|pads| {
            pads.get_or_insert_with(Pads::open);
        });
    }
}

/// How long a d-pad or stick direction is held before it starts to repeat, and how often then.
const REPEAT_DELAY: Duration = Duration::from_millis(400);
const REPEAT_EVERY: Duration = Duration::from_millis(80);
/// How far the left stick goes before it counts as an arrow.
const STICK: f32 = 0.5;

/// How long to wait for a key between looks at the controllers: often while one is plugged in, so
/// presses don't lag, now and then otherwise, to notice one being plugged in.
pub fn patience() -> Duration {
    PADS.with_borrow(|pads| match pads {
        Some(Some(pads))
            if pads
                .gilrs
                .as_ref()
                .is_some_and(|gilrs| gilrs.gamepads().next().is_some()) =>
        {
            Duration::from_millis(16)
        }
        Some(None) => Duration::from_secs(60),
        _ => Duration::from_millis(250),
    })
}

struct Pads {
    /// The controllers; none in a test, which feeds [`Pads::fake`] instead.
    gilrs: Option<Gilrs>,
    /// What the test says the controllers did, as `gilrs` would have.
    #[cfg(test)]
    fake: VecDeque<Heard>,
    inputs: Inputs,
    /// Presses made and not yet given out, with the family of the controller they were on.
    ready: VecDeque<(Button, Family)>,
    /// The family of the controller heard from last, for an arrow that repeats.
    family: Family,
    /// Buttons let go since they were last looked at, for the bumpers that wait to be let go.
    released: VecDeque<(Button, Family)>,
    chords: Chords,
}

/// What a controller did, as far as tdl cares.
enum Heard {
    Connected(Family),
    Disconnected,
    /// A signal, from a controller of this family, whose X and Y are named the wrong way round
    /// if the last is true.
    Signal(Signal, Family, bool),
}

/// What a gilrs event says, as far as tdl cares, without its device code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Signal {
    Pressed(Button),
    Released(Button),
    /// A button's value, as for an analog trigger: 0 to 1.
    Changed(Button, f32),
    Axis(Axis, f32),
    /// An axis gilrs has no name for, by its device code and value: where some Bluetooth Xbox
    /// controllers' triggers are.
    Unnamed(u32, f32),
}

/// The signal an event is, if tdl has a use for it.
fn signal(event: &EventType) -> Option<Signal> {
    Some(match *event {
        EventType::ButtonPressed(button, _) => Signal::Pressed(button),
        EventType::ButtonReleased(button, _) => Signal::Released(button),
        EventType::ButtonChanged(button, value, _) => Signal::Changed(button, value),
        EventType::AxisChanged(Axis::Unknown, value, code) => {
            Signal::Unnamed(code.into_u32(), value)
        }
        EventType::AxisChanged(axis, value, _) => Signal::Axis(axis, value),
        _ => return None,
    })
}

/// How far a trigger goes before it counts as pressed and how far back before it counts as let
/// go, as a share of its travel: a button that gilrs reports (`Changed`), and an axis (`Z`) whose
/// -1 to 1 is that share doubled and shifted.
const TRIGGER_PRESSED: f32 = 0.5;
const TRIGGER_RELEASED: f32 = 0.3;
/// The device codes (`kind << 16 | code`) of Linux's `EV_ABS` `ABS_BRAKE` and `ABS_GAS`.
const ABS_BRAKE: u32 = (3 << 16) | 10;
const ABS_GAS: u32 = (3 << 16) | 9;
const Z_PRESSED: f32 = 0.6;
const Z_RELEASED: f32 = 0.4;

/// The buttons and sticks of a controller as tdl takes them, from the signals gilrs sends. Every
/// way a trigger can arrive (a button pressed and let go, a button's value, or an axis) comes out
/// as the same presses and releases of `LeftTrigger2` and `RightTrigger2`, once each.
#[derive(Default)]
struct Inputs {
    /// Every button held down now, for combinations.
    down: Vec<Button>,
    /// The arrow held down, and when it next repeats.
    held: Option<(Button, Instant)>,
    /// Where the left stick points, which only counts when it changes.
    stick: (f32, f32),
    /// Whether the left and the right trigger are down.
    triggers: [bool; 2],
    /// Presses and releases not yet taken, in the order they came.
    presses: VecDeque<Button>,
    released: VecDeque<Button>,
}

impl Inputs {
    const TRIGGERS: [Button; 2] = [Button::LeftTrigger2, Button::RightTrigger2];

    fn feed(&mut self, signal: Signal, swapped: bool, now: Instant) {
        match signal {
            Signal::Pressed(button) => match Self::trigger_of(button) {
                Some(which) => self.trigger(which, true, now),
                None => self.press(physical(button, swapped), now),
            },
            Signal::Released(button) => match Self::trigger_of(button) {
                Some(which) => self.trigger(which, false, now),
                None => self.release(physical(button, swapped)),
            },
            Signal::Changed(button, value) => {
                if let Some(which) = Self::trigger_of(button) {
                    self.analog(which, value, TRIGGER_PRESSED, TRIGGER_RELEASED, now);
                }
            }
            Signal::Axis(axis @ (Axis::LeftZ | Axis::RightZ), value) => {
                let which = usize::from(axis == Axis::RightZ);
                let share = (value + 1.0) / 2.0;
                self.analog(which, share, Z_PRESSED, Z_RELEASED, now);
            }
            Signal::Axis(axis @ (Axis::LeftStickX | Axis::LeftStickY), value) => {
                let before = stick_arrow(self.stick);
                match axis {
                    Axis::LeftStickX => self.stick.0 = value,
                    _ => self.stick.1 = value,
                }
                let after = stick_arrow(self.stick);
                if after != before {
                    self.held = after.map(|button| (button, now + REPEAT_DELAY));
                    if let Some(button) = after {
                        self.presses.push_back(button);
                    }
                }
            }
            Signal::Axis(..) => {}
            // 🎚️ Linux's `ABS_BRAKE` and `ABS_GAS` are the left and the right trigger of a
            // Bluetooth Xbox controller, which no mapping names, and like `Z` they run -1 to 1
            Signal::Unnamed(code, value) => {
                if let Some(which) = [ABS_BRAKE, ABS_GAS].iter().position(|&known| known == code) {
                    self.analog(which, (value + 1.0) / 2.0, Z_PRESSED, Z_RELEASED, now);
                }
            }
        }
    }

    fn trigger_of(button: Button) -> Option<usize> {
        Self::TRIGGERS.iter().position(|&trigger| trigger == button)
    }

    fn press(&mut self, button: Button, now: Instant) {
        if !self.down.contains(&button) {
            self.down.push(button);
        }
        if is_arrow(button) {
            self.held = Some((button, now + REPEAT_DELAY));
        }
        self.presses.push_back(button);
    }

    fn release(&mut self, button: Button) {
        self.down.retain(|down| *down != button);
        self.released.push_back(button);
        if self.held.is_some_and(|(held, _)| held == button) {
            self.held = None;
        }
    }

    /// A trigger is now down or up; nothing if it already was.
    fn trigger(&mut self, which: usize, down: bool, now: Instant) {
        if self.triggers[which] == down {
            return;
        }
        self.triggers[which] = down;
        match down {
            true => self.press(Self::TRIGGERS[which], now),
            false => self.release(Self::TRIGGERS[which]),
        }
    }

    /// A trigger is `share` of the way down: down once past `pressed`, up again once back past
    /// `released`, and as it was in between.
    fn analog(&mut self, which: usize, share: f32, pressed: f32, released: f32, now: Instant) {
        if share >= pressed {
            self.trigger(which, true, now);
        } else if share <= released {
            self.trigger(which, false, now);
        }
    }

    /// The arrow that is held and due to repeat, if any.
    fn repeat(&mut self, now: Instant) -> Option<Button> {
        let (button, at) = self.held?;
        (now >= at).then(|| {
            self.held = Some((button, now + REPEAT_EVERY));
            button
        })
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

/// What the bumpers have been used for, so a bumper is `Tab` or `Backtab` when it is pressed and
/// let go alone, and half of a chord when anything is pressed meanwhile.
#[derive(Default)]
struct Chords {
    /// Keys made and not yet given out.
    out: VecDeque<KeyCode>,
    /// A bumper pressed alone, which is its key once it is let go, unless something was pressed
    /// with it.
    pending: Option<Button>,
    /// Whether a chord was made since the bumpers went down.
    chorded: bool,
}

thread_local! {
    /// The controllers, started on first use: `Some(None)` if gilrs couldn't start.
    static PADS: RefCell<Option<Option<Pads>>> = const { RefCell::new(None) };
}

/// The key the next controller press means, if any button was pressed (or an arrow held long
/// enough to repeat) since the last call. It doesn't wait.
pub fn chant() -> Option<KeyCode> {
    PADS.with_borrow_mut(|pads| {
        let pads = pads.get_or_insert_with(Pads::open).as_mut()?;
        loop {
            if let Some(key) = pads.chords.out.pop_front() {
                return Some(key);
            }
            let press = pads.next_press();
            // 🕰️ what was let go came before what was pressed, whichever call found it
            while let Some((button, family)) = pads.released.pop_front() {
                pads.chords.let_go(&pads.inputs.down, button, family);
            }
            match press {
                Some((button, family)) => pads.chords.pressed(&pads.inputs.down, button, family),
                None if pads.chords.out.is_empty() => return None,
                None => {}
            }
        }
    })
}

/// Checks that every `Key⟨chord⟩` in `legend` names the chord that really makes that key.
#[cfg(test)]
pub fn assert_chords_match(legend: &[[&str; 2]]) {
    for [keys, text] in legend {
        // 🧩 a `,` that is the key, not the separator, is told apart for the splitting
        let keys = keys.replace(",⟨", "§⟨");
        for token in keys
            .split(['/', ',', ' '])
            .filter(|token| token.contains('⟨'))
        {
            let (key, alias) = token.split_once('⟨').unwrap();
            for way in alias.trim_end_matches('⟩').split('|') {
                let names: Vec<&str> = way.split('+').collect();
                let (last, held) = names.split_last().unwrap();
                let prompt = Prompt::alias(last).unwrap_or_else(|| panic!("{token}"));
                let wanted = match key.strip_prefix("Shift+") {
                    Some(shifted) => shifted.chars().next(),
                    None if key == "Space" => Some(' '),
                    None if key == "§" => Some(','),
                    None => key.chars().next().map(|c| c.to_ascii_lowercase()),
                };
                // ➡️ a button of its own, like Host or Home, is no chord, but makes its key too
                if held.is_empty() {
                    if prompt != Prompt::Host && prompt != Prompt::Join {
                        let made = prompt.chant();
                        assert_eq!(made, KeyCode::Char(wanted.unwrap()), "{token} for {text}");
                    }
                    continue;
                }
                let made = chord(held.contains(&"lb"), held.contains(&"rb"), prompt);
                assert_eq!(made, wanted, "{token} for {text}");
            }
        }
    }
}

/// The key a chord makes: `LB` or `RB` (or both) held while `prompt`'s button is pressed. The
/// letters are the keys that do the same on the screens that have them, and mean nothing on the
/// ones that don't, so a chord never needs to know where it is.
///
/// `LB`: `A` add, `X` edit, `B` delete, up/down mark and range-select, left/right seal and import
/// a scroll. `RB`: `A` UZDoom, `B` base wad from the wads, `X` base wad from a
/// file, `Y` add wads from files, up/down swap, left/right the previous/next search match.
/// Both: `A`/`X` replace a wad from the wads/a file, `B` rename its file, `Y` toggle volatility,
/// up inspect, down add a mod folder, left import a dir or a watching profile, right watch a dir.
pub fn chord(left: bool, right: bool, prompt: Prompt) -> Option<char> {
    Some(match (left, right, prompt) {
        (true, false, Prompt::Confirm) => 'a',
        (true, false, Prompt::Search) => 'e',
        (true, false, Prompt::Back) => 'd',
        (true, false, Prompt::Up) => 'm',
        (true, false, Prompt::Down) => 'V',
        (true, false, Prompt::Left) => 's',
        (true, false, Prompt::Right) => 'i',
        (false, true, Prompt::Confirm) => 'u',
        (false, true, Prompt::Back) => 'b',
        (false, true, Prompt::Search) => 'B',
        (false, true, Prompt::Cut) => 'W',
        (false, true, Prompt::Up) => '{',
        (false, true, Prompt::Down) => '}',
        (false, true, Prompt::Left) => ',',
        (false, true, Prompt::Right) => ';',
        (true, true, Prompt::Confirm) => 'r',
        (true, true, Prompt::Search) => 'R',
        (true, true, Prompt::Back) => 'n',
        (true, true, Prompt::Cut) => 'v',
        (true, true, Prompt::Up) => ' ',
        (true, true, Prompt::Down) => 'F',
        (true, true, Prompt::Left) => 'f',
        (true, true, Prompt::Right) => 't',
        _ => return None,
    })
}

/// The next button pressed (the d-pad and the left stick's directions as the d-pad's buttons, and
/// repeating while held), whatever it means, and the family of the controller it is on. It doesn't
/// wait. For the on-screen keyboard, which has uses for buttons the keys have none for.
pub fn press() -> Option<(Button, Family)> {
    PADS.with_borrow_mut(|pads| {
        let pads = pads.get_or_insert_with(Pads::open).as_mut()?;
        let press = pads.next_press();
        // 🧹 nobody is waiting for the releases here
        pads.released.clear();
        press
    })
}

/// Whether `button` is held down right now, for combinations. Only what [`press`] and [`chant`]
/// have read so far counts.
pub fn is_held(button: Button) -> bool {
    PADS.with_borrow(|pads| matches!(pads, Some(Some(pads)) if pads.inputs.down.contains(&button)))
}

/// Throws away every controller press since the last look, as for keys pressed during a game.
pub fn forget() {
    PADS.with_borrow_mut(|pads| {
        if let Some(Some(pads)) = pads {
            while pads
                .gilrs
                .as_mut()
                .is_some_and(|gilrs| gilrs.next_event().is_some())
            {}
            #[cfg(test)]
            pads.fake.clear();
            pads.inputs.clear();
            pads.ready.clear();
            pads.released.clear();
            pads.chords = Chords::default();
        }
    });
}

/// The family `TDL_CONTROLLER_FAMILY` forces (`xbox`, `playstation`, `nintendo`, `steamdeck`,
/// `steamcontroller` or `generic`), for a controller that tells tdl what it is wrongly.
fn forced() -> Option<Family> {
    let name = std::env::var("TDL_CONTROLLER_FAMILY").ok()?.to_lowercase();
    Family::ALL
        .into_iter()
        .find(|family| format!("{family:?}").to_lowercase() == name)
}

/// Whether the controller's `West` and `North` are the other way round from where the buttons are.
/// Linux calls the Xbox layout's X button `BTN_X`, which is the same code as `BTN_NORTH`, so a
/// controller that no SDL mapping describes (gilrs then goes by those names) reports its X, on
/// the left, as `North`, and its Y, on the top, as `West`. `TDL_CONTROLLER_XY=swap` or `keep` says
/// so when tdl guesses wrong.
fn xy_swapped(gamepad: &gilrs::Gamepad, family: Family) -> bool {
    match std::env::var("TDL_CONTROLLER_XY").as_deref() {
        Ok("swap") => true,
        Ok("keep") => false,
        _ => {
            cfg!(target_os = "linux")
                && gamepad.mapping_source() != MappingSource::SdlMappings
                && matches!(family, Family::Xbox | Family::Generic)
        }
    }
}

/// `button` as it is on the controller: its left and top face buttons swapped back if they are
/// named the wrong way round (see [`xy_swapped`]).
fn physical(button: Button, swapped: bool) -> Button {
    match (swapped, button) {
        (true, Button::West) => Button::North,
        (true, Button::North) => Button::West,
        _ => button,
    }
}

fn family_of(gamepad: gilrs::Gamepad) -> Family {
    forced()
        .unwrap_or_else(|| Family::of(gamepad.name(), gamepad.vendor_id(), gamepad.product_id()))
}

impl Pads {
    /// Starts gilrs, and takes note of a controller that is already plugged in.
    fn open() -> Option<Self> {
        let gilrs = Gilrs::new().ok()?;
        if let Some((_, gamepad)) = gilrs.gamepads().next() {
            controller_used(family_of(gamepad));
        }
        Some(Self {
            gilrs: Some(gilrs),
            #[cfg(test)]
            fake: VecDeque::new(),
            inputs: Inputs::default(),
            ready: VecDeque::new(),
            family: Family::Generic,
            released: VecDeque::new(),
            chords: Chords::default(),
        })
    }

    fn next_press(&mut self) -> Option<(Button, Family)> {
        loop {
            if let Some((button, family)) = self.ready.pop_front() {
                controller_used(family);
                return Some((button, family));
            }
            let Some(heard) = self.heard() else {
                break;
            };
            match heard {
                // 🔌 a controller plugged in later is welcomed as the first one was; the last one
                // unplugged hands the legends back to the keyboard
                Heard::Connected(family) => {
                    self.family = family;
                    controller_used(family);
                }
                Heard::Disconnected => {
                    self.inputs.clear();
                    let any = self
                        .gilrs
                        .as_ref()
                        .is_some_and(|gilrs| gilrs.gamepads().any(|(_, pad)| pad.is_connected()));
                    if !any {
                        keyboard_used();
                    }
                }
                Heard::Signal(signal, family, swapped) => {
                    self.family = family;
                    self.inputs.feed(signal, swapped, Instant::now());
                    self.take(family);
                }
            }
        }
        let button = self.inputs.repeat(Instant::now())?;
        controller_used(self.family);
        Some((button, self.family))
    }

    /// The next thing a controller did, if anything.
    fn heard(&mut self) -> Option<Heard> {
        #[cfg(test)]
        if let Some(heard) = self.fake.pop_front() {
            return Some(heard);
        }
        let gilrs = self.gilrs.as_mut()?;
        loop {
            let event = gilrs.next_event()?;
            let gamepad = gilrs.gamepad(event.id);
            let family = family_of(gamepad);
            let swapped = xy_swapped(&gamepad, family);
            match event.event {
                EventType::Connected => return Some(Heard::Connected(family)),
                EventType::Disconnected => return Some(Heard::Disconnected),
                ref other => {
                    if let Some(signal) = signal(other) {
                        return Some(Heard::Signal(signal, family, swapped));
                    }
                }
            }
        }
    }

    /// Moves what `inputs` has made onto the queues, as made on a controller of `family`.
    fn take(&mut self, family: Family) {
        while let Some(button) = self.inputs.presses.pop_front() {
            self.ready.push_back((button, family));
        }
        while let Some(button) = self.inputs.released.pop_front() {
            self.released.push_back((button, family));
        }
    }
}

impl Chords {
    fn bumper(button: Button) -> bool {
        matches!(button, Button::LeftTrigger | Button::RightTrigger)
    }

    /// Makes the key (or the chord's key) of a button that was pressed, with `down` the buttons
    /// held now.
    fn pressed(&mut self, down: &[Button], button: Button, family: Family) {
        let Some(prompt) = Prompt::of(button, family) else {
            return;
        };
        if Self::bumper(button) {
            // 🫸 a bumper waits to see whether it is a key or half of a chord, unless the other
            // bumper is down, when it is the second half of the one
            let other = match button {
                Button::LeftTrigger => Button::RightTrigger,
                _ => Button::LeftTrigger,
            };
            self.pending = (!down.contains(&other)).then_some(button);
            return;
        }
        let left = down.contains(&Button::LeftTrigger);
        let right = down.contains(&Button::RightTrigger);
        if left || right {
            // 🔗 a chord, or nothing: never the button's own key as well. With a trigger down too
            // it is nothing: a trigger pressed by accident can't turn a chord into another
            self.chorded = true;
            let trigger = Inputs::TRIGGERS
                .iter()
                .any(|trigger| down.contains(trigger));
            if !trigger && let Some(key) = chord(left, right, prompt) {
                self.out.push_back(KeyCode::Char(key));
            }
            return;
        }
        self.out.push_back(prompt.chant());
    }

    /// A bumper let go with nothing pressed meanwhile is its own key after all.
    fn let_go(&mut self, down: &[Button], button: Button, family: Family) {
        if !Self::bumper(button) {
            return;
        }
        if self.pending == Some(button) {
            self.pending = None;
            if !self.chorded
                && let Some(prompt) = Prompt::of(button, family)
            {
                self.out.push_back(prompt.chant());
            }
        }
        if !down.contains(&Button::LeftTrigger) && !down.contains(&Button::RightTrigger) {
            self.chorded = false;
            self.pending = None;
        }
    }
}

fn is_arrow(button: Button) -> bool {
    matches!(
        button,
        Button::DPadUp | Button::DPadDown | Button::DPadLeft | Button::DPadRight
    )
}

/// The d-pad button the left stick stands for, if it is pushed far enough: whichever way it is
/// pushed further. Up is positive.
fn stick_arrow((x, y): (f32, f32)) -> Option<Button> {
    if x.abs().max(y.abs()) < STICK {
        None
    } else if y.abs() >= x.abs() {
        Some(if y > 0.0 {
            Button::DPadUp
        } else {
            Button::DPadDown
        })
    } else {
        Some(if x > 0.0 {
            Button::DPadRight
        } else {
            Button::DPadLeft
        })
    }
}

/// `tdl --controller-test`: with no windows, says what every controller sends as gilrs names it
/// (the button or axis, and its value), where on the pad tdl takes it to be, and the key it makes,
/// until Ctrl+C. For finding out what a pad does on the machine it is plugged into.
pub fn controller_test(out: &mut impl Write) -> std::io::Result<()> {
    let Ok(mut gilrs) = Gilrs::new() else {
        writeln!(
            out,
            "gilrs could not start, so no controller can be read here"
        )?;
        return Ok(());
    };
    writeln!(
        out,
        "Press buttons and move sticks and triggers; Ctrl+C stops."
    )?;
    let mut probe = Probe::default();
    let known: Vec<_> = gilrs.gamepads().map(|(id, _)| id).collect();
    if known.is_empty() {
        writeln!(out, "No controller is plugged in yet.")?;
    }
    for id in known {
        probe.introduce(out, gilrs.gamepad(id))?;
    }
    loop {
        while let Some(event) = gilrs.next_event() {
            let gamepad = gilrs.gamepad(event.id);
            let family = family_of(gamepad);
            let swapped = xy_swapped(&gamepad, family);
            match event.event {
                EventType::Connected => probe.introduce(out, gamepad)?,
                EventType::Disconnected => {
                    writeln!(out, "controller {} unplugged", event.id)?;
                    probe.inputs.clear();
                }
                ref other => {
                    if let Some(signal) = signal(other) {
                        let label = format!("{other:?}");
                        probe.signal(out, &label, signal, family, swapped)?;
                    }
                }
            }
            out.flush()?;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// What `--controller-test` has seen so far.
#[derive(Default)]
struct Probe {
    inputs: Inputs,
    chords: Chords,
    /// The last value shown for each axis or button's value, so a stick doesn't flood the screen.
    shown: HashMap<String, f32>,
}

impl Probe {
    fn introduce(&mut self, out: &mut impl Write, gamepad: gilrs::Gamepad) -> std::io::Result<()> {
        let family = family_of(gamepad);
        writeln!(
            out,
            "controller {}: {:?}, vendor {:04x?} product {:04x?}, taken for {family:?}; \
             mapping from {:?}; X and Y put right: {}",
            gamepad.id(),
            gamepad.name(),
            gamepad.vendor_id(),
            gamepad.product_id(),
            gamepad.mapping_source(),
            if xy_swapped(&gamepad, family) {
                "yes"
            } else {
                "no"
            },
        )
    }

    /// Says what `signal` (which gilrs sent as `label`) is: raw, then as tdl takes it.
    fn signal(
        &mut self,
        out: &mut impl Write,
        label: &str,
        signal: Signal,
        family: Family,
        swapped: bool,
    ) -> std::io::Result<()> {
        // 🌊 a value that has hardly moved since it was last shown is left out
        let value = match signal {
            Signal::Changed(button, value) => Some((format!("{button:?}"), value)),
            Signal::Axis(axis, value) => Some((format!("{axis:?}"), value)),
            Signal::Unnamed(code, value) => Some((format!("axis {code:#x}"), value)),
            _ => None,
        };
        if let Some((name, value)) = value {
            let last = self.shown.insert(name, value);
            if last.is_some_and(|last| (last - value).abs() < 0.15 && value.abs() < 0.99) {
                return Ok(());
            }
            writeln!(out, "{label}")?;
        } else {
            writeln!(out, "{label}")?;
        }
        self.inputs.feed(signal, swapped, Instant::now());
        while let Some(button) = self.inputs.released.pop_front() {
            self.chords.let_go(&self.inputs.down, button, family);
            writeln!(out, "    let go: {}", Self::describe(button, family))?;
            self.keys(out)?;
        }
        while let Some(button) = self.inputs.presses.pop_front() {
            self.chords.pressed(&self.inputs.down, button, family);
            writeln!(out, "    pressed: {}", Self::describe(button, family))?;
            self.keys(out)?;
        }
        Ok(())
    }

    fn keys(&mut self, out: &mut impl Write) -> std::io::Result<()> {
        while let Some(key) = self.chords.out.pop_front() {
            writeln!(out, "    sends the key {key:?}")?;
        }
        Ok(())
    }

    /// `button`, where it is on the pad, and what tdl calls it there.
    fn describe(button: Button, family: Family) -> String {
        let place = match button {
            Button::South => "the bottom face button",
            Button::East => "the right face button",
            Button::West => "the left face button",
            Button::North => "the top face button",
            Button::LeftTrigger => "the left bumper",
            Button::RightTrigger => "the right bumper",
            Button::LeftTrigger2 => "the left trigger",
            Button::RightTrigger2 => "the right trigger",
            Button::Select => "Select (View, Share, -)",
            Button::Start => "Start (Menu, Options, +)",
            Button::Mode => "Home (the logo button)",
            Button::LeftThumb => "the left stick pressed in",
            Button::RightThumb => "the right stick pressed in",
            Button::DPadUp => "d-pad up",
            Button::DPadDown => "d-pad down",
            Button::DPadLeft => "d-pad left",
            Button::DPadRight => "d-pad right",
            _ => "a button tdl has no use for",
        };
        match Prompt::of(button, family) {
            Some(prompt) => format!(
                "{button:?}, {place}: {}, key {:?}",
                prompt.label(family),
                prompt.chant()
            ),
            None => format!("{button:?}, {place}: nothing"),
        }
    }
}

/// Makes `signals` what a pretend controller does next, for the global functions here, so a test
/// can run the very code the ritual loop runs. Nothing else is read from a real controller.
#[cfg(test)]
pub fn pretend(signals: &[Signal]) {
    let pads = Pads {
        gilrs: None,
        fake: signals
            .iter()
            .map(|&signal| Heard::Signal(signal, Family::Xbox, false))
            .collect(),
        inputs: Inputs::default(),
        ready: VecDeque::new(),
        family: Family::Xbox,
        released: VecDeque::new(),
        chords: Chords::default(),
    };
    PADS.with_borrow_mut(|slot| *slot = Some(Some(pads)));
}

/// [`pretend`], with the buttons pressed (`true`) and let go (`false`) in order.
#[cfg(test)]
pub fn pretend_buttons(events: &[(Button, bool)]) {
    let signals: Vec<Signal> = events
        .iter()
        .map(|&(button, down)| match down {
            true => Signal::Pressed(button),
            false => Signal::Released(button),
        })
        .collect();
    pretend(&signals);
}

/// Whether a pretend controller has nothing more to do, so a test's loop can stop waiting.
#[cfg(test)]
pub fn drained() -> bool {
    PADS.with_borrow(|pads| match pads {
        Some(Some(pads)) => pads.gilrs.is_none() && pads.fake.is_empty() && pads.ready.is_empty(),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROMPTS: [Prompt; 17] = [
        Prompt::Confirm,
        Prompt::Back,
        Prompt::Cut,
        Prompt::Search,
        Prompt::Previous,
        Prompt::Next,
        Prompt::Host,
        Prompt::Join,
        Prompt::Keys,
        Prompt::Wads,
        Prompt::Copy,
        Prompt::Paste,
        Prompt::Settings,
        Prompt::Up,
        Prompt::Down,
        Prompt::Left,
        Prompt::Right,
    ];

    #[test]
    fn controllers_are_told_apart_by_vendor_then_by_name() {
        assert_eq!(Family::of("whatever", Some(0x045e), None), Family::Xbox);
        assert_eq!(
            Family::of("Wireless Controller", Some(0x054c), Some(0x0ce6)),
            Family::PlayStation
        );
        assert_eq!(
            Family::of("Pro Controller", Some(0x057e), Some(0x2009)),
            Family::Nintendo
        );
        assert_eq!(
            Family::of("Steam Deck", Some(0x28de), Some(0x1205)),
            Family::SteamDeck
        );
        assert_eq!(
            Family::of("Steam Controller", Some(0x28de), Some(0x1142)),
            Family::SteamController
        );
        assert_eq!(
            Family::of("Steam Virtual Gamepad", Some(0x28de), Some(0x11ff)),
            Family::Xbox
        );
        assert_eq!(Family::of("Xbox Controller", None, None), Family::Xbox);
        assert_eq!(
            Family::of("Sony DualSense", None, None),
            Family::PlayStation
        );
        assert_eq!(
            Family::of("Nintendo Switch Pro Controller", None, None),
            Family::Nintendo
        );
        assert_eq!(
            Family::of("8BitDo SN30", Some(0x2dc8), None),
            Family::Generic
        );
        for family in Family::ALL {
            assert_eq!(Family::from_u8(family as u8), Some(family));
        }
        assert_eq!(Family::from_u8(0), None);
    }

    #[test]
    fn a_is_enter_and_b_is_esc_even_where_a_is_on_the_right() {
        let key = |button, family| Prompt::of(button, family).map(Prompt::chant);
        assert_eq!(key(Button::South, Family::Xbox), Some(KeyCode::Enter));
        assert_eq!(key(Button::East, Family::PlayStation), Some(KeyCode::Esc));
        assert_eq!(key(Button::East, Family::Nintendo), Some(KeyCode::Enter));
        assert_eq!(key(Button::South, Family::Nintendo), Some(KeyCode::Esc));
        assert_eq!(key(Button::Start, Family::Generic), Some(KeyCode::F(1)));
        assert_eq!(key(Button::Select, Family::Xbox), Some(KeyCode::Char('w')));
        assert_eq!(
            key(Button::LeftThumb, Family::Xbox),
            Some(KeyCode::Char('y'))
        );
        assert_eq!(
            key(Button::RightThumb, Family::Xbox),
            Some(KeyCode::Char('p'))
        );
        // 🔍 search is the left face button, X on an Xbox, and cut the top one, whichever controller
        // it is
        for family in [Family::Xbox, Family::PlayStation, Family::Nintendo] {
            assert_eq!(key(Button::North, family), Some(KeyCode::Char('x')));
            assert_eq!(key(Button::West, family), Some(KeyCode::Char('/')));
        }
        assert_eq!(
            key(Button::DPadDown, Family::SteamDeck),
            Some(KeyCode::Down)
        );
        assert_eq!(key(Button::Mode, Family::Xbox), Some(KeyCode::Char('o')));
        assert_eq!(key(Button::C, Family::Xbox), None);
    }

    /// Runs `events` (a button pressed `(button, true)` or let go `(button, false)`) the way the
    /// controller loop does, and says what keys came out.
    fn chorded(events: &[(Button, bool)]) -> Vec<KeyCode> {
        let (mut chords, mut down) = (Chords::default(), Vec::new());
        for &(button, pressed) in events {
            if pressed {
                down.push(button);
                chords.pressed(&down, button, Family::Xbox);
            } else {
                down.retain(|held| *held != button);
                chords.let_go(&down, button, Family::Xbox);
            }
        }
        chords.out.into()
    }

    #[test]
    fn a_bumper_alone_is_tab_once_it_is_let_go_and_a_chord_never_is() {
        use Button::*;
        let tab = KeyCode::Tab;
        assert_eq!(
            chorded(&[(RightTrigger, true)]),
            [],
            "not until it is let go"
        );
        assert_eq!(
            chorded(&[(RightTrigger, true), (RightTrigger, false)]),
            [tab]
        );
        assert_eq!(
            chorded(&[(LeftTrigger, true), (LeftTrigger, false)]),
            [KeyCode::BackTab]
        );
        // 🔗 LB then A is add, and neither Backtab nor Enter come with it
        let add = [
            (LeftTrigger, true),
            (South, true),
            (South, false),
            (LeftTrigger, false),
        ];
        assert_eq!(chorded(&add), [KeyCode::Char('a')]);
        // ✋ several chords on one hold, and the bumper still isn't a key afterwards
        let two = [
            (RightTrigger, true),
            (DPadDown, true),
            (DPadDown, false),
            (East, true),
            (East, false),
            (RightTrigger, false),
        ];
        assert_eq!(chorded(&two), [KeyCode::Char('}'), KeyCode::Char('b')]);
        // 🚫 a chord that isn't one is nothing, not the button's own key
        let none = [(LeftTrigger, true), (Start, true), (LeftTrigger, false)];
        assert_eq!(chorded(&none), []);
        // 🤝 both bumpers make a third layer, and neither is a key when let go
        let both = [
            (LeftTrigger, true),
            (RightTrigger, true),
            (South, true),
            (RightTrigger, false),
            (LeftTrigger, false),
        ];
        assert_eq!(chorded(&both), [KeyCode::Char('r')]);
        let nothing = [
            (LeftTrigger, true),
            (RightTrigger, true),
            (LeftTrigger, false),
            (RightTrigger, false),
        ];
        assert_eq!(chorded(&nothing), []);
        // after the chord, a bumper alone is a key again
        let later = [
            (LeftTrigger, true),
            (South, true),
            (LeftTrigger, false),
            (RightTrigger, true),
            (RightTrigger, false),
        ];
        assert_eq!(chorded(&later), [KeyCode::Char('a'), tab]);
        // ⌨️ other buttons don't wait
        assert_eq!(chorded(&[(South, true)]), [KeyCode::Enter]);
    }

    #[test]
    fn chords_are_on_the_physical_buttons_where_the_readme_says() {
        use Button::*;
        // (what is held, the button pressed, the key it makes): Xbox's X is West, Y is North
        let table: &[(&[Button], Button, Option<char>)] = &[
            (&[LeftTrigger], South, Some('a')),
            (&[LeftTrigger], West, Some('e')),
            (&[LeftTrigger], East, Some('d')),
            (&[LeftTrigger], North, None),
            (&[LeftTrigger], DPadUp, Some('m')),
            (&[LeftTrigger], DPadDown, Some('V')),
            (&[LeftTrigger], DPadLeft, Some('s')),
            (&[LeftTrigger], DPadRight, Some('i')),
            (&[RightTrigger], South, Some('u')),
            (&[RightTrigger], East, Some('b')),
            (&[RightTrigger], West, Some('B')),
            (&[RightTrigger], North, Some('W')),
            (&[RightTrigger], DPadUp, Some('{')),
            (&[RightTrigger], DPadDown, Some('}')),
            (&[RightTrigger], DPadLeft, Some(',')),
            (&[RightTrigger], DPadRight, Some(';')),
            (&[LeftTrigger, RightTrigger], South, Some('r')),
            (&[LeftTrigger, RightTrigger], West, Some('R')),
            (&[LeftTrigger, RightTrigger], East, Some('n')),
            (&[LeftTrigger, RightTrigger], North, Some('v')),
            (&[LeftTrigger, RightTrigger], DPadUp, Some(' ')),
            (&[LeftTrigger, RightTrigger], DPadDown, Some('F')),
            (&[LeftTrigger, RightTrigger], DPadLeft, Some('f')),
            (&[LeftTrigger, RightTrigger], DPadRight, Some('t')),
        ];
        for &(held, button, key) in table {
            let mut events: Vec<(Button, bool)> = held.iter().map(|&b| (b, true)).collect();
            events.extend([(button, true), (button, false)]);
            events.extend(held.iter().map(|&b| (b, false)));
            let made = chorded(&events);
            let wanted: Vec<KeyCode> = key.map(KeyCode::Char).into_iter().collect();
            assert_eq!(made, wanted, "{held:?} + {button:?}");
        }
    }

    #[test]
    fn the_plain_face_buttons_are_where_they_are_on_an_xbox_pad() {
        // Xbox: A south, B east, X west (search), Y north (cut)
        let key = |button| Prompt::of(button, Family::Xbox).map(Prompt::chant);
        assert_eq!(key(Button::South), Some(KeyCode::Enter));
        assert_eq!(key(Button::East), Some(KeyCode::Esc));
        assert_eq!(key(Button::West), Some(KeyCode::Char('/')));
        assert_eq!(key(Button::North), Some(KeyCode::Char('x')));
        // and they are named as they are on the pad: West is X, North is Y
        assert_eq!(Prompt::Search.text(Family::Xbox), "X");
        assert_eq!(Prompt::Cut.text(Family::Xbox), "Y");
    }

    #[test]
    fn a_pad_that_names_its_x_and_y_the_wrong_way_round_is_put_right() {
        assert_eq!(physical(Button::West, true), Button::North);
        assert_eq!(physical(Button::North, true), Button::West);
        assert_eq!(physical(Button::South, true), Button::South);
        assert_eq!(physical(Button::West, false), Button::West);
    }

    /// Feeds `signals` (the way gilrs sends them) through the inputs and the chords, and says what
    /// presses came out, and what keys.
    fn through(signals: &[Signal], swapped: bool) -> (Vec<Button>, Vec<Button>, Vec<KeyCode>) {
        let (mut inputs, mut chords) = (Inputs::default(), Chords::default());
        let (mut presses, mut releases) = (Vec::new(), Vec::new());
        for &signal in signals {
            inputs.feed(signal, swapped, Instant::now());
            while let Some(button) = inputs.released.pop_front() {
                releases.push(button);
                chords.let_go(&inputs.down, button, Family::Xbox);
            }
            while let Some(button) = inputs.presses.pop_front() {
                presses.push(button);
                chords.pressed(&inputs.down, button, Family::Xbox);
            }
        }
        (presses, releases, chords.out.into())
    }

    #[test]
    fn a_trigger_that_is_a_button_is_pressed_and_let_go_once() {
        use Button::*;
        let (presses, releases, _) = through(
            &[
                Signal::Pressed(LeftTrigger2),
                Signal::Pressed(LeftTrigger2),
                Signal::Released(LeftTrigger2),
                Signal::Released(LeftTrigger2),
            ],
            false,
        );
        assert_eq!(
            (presses, releases),
            (vec![LeftTrigger2], vec![LeftTrigger2])
        );
    }

    #[test]
    fn a_trigger_that_is_a_button_with_a_value_is_down_past_half_and_up_below_a_third() {
        use Button::*;
        let changed = |value| Signal::Changed(RightTrigger2, value);
        let (presses, releases, _) = through(
            &[
                changed(0.1),
                changed(0.45),
                changed(0.6),
                changed(0.9),
                changed(0.4),
                changed(0.31),
                changed(0.2),
                changed(0.0),
            ],
            false,
        );
        assert_eq!(
            (presses, releases),
            (vec![RightTrigger2], vec![RightTrigger2])
        );
    }

    /// The same, with each way a trigger can be reported: the keyboard's `RT` + `A` is `Enter`
    /// and `LT` + `LB` is `Home` whichever it is, with the trigger's value moving on the way.
    #[test]
    fn the_keyboards_trigger_chords_work_for_every_way_a_trigger_is_reported() {
        use crate::keyboard::{Output, PadKeyboard};
        use Button::*;
        let ways: Vec<(&str, [Signal; 2], bool)> = vec![
            (
                "buttons",
                [
                    Signal::Pressed(RightTrigger2),
                    Signal::Released(RightTrigger2),
                ],
                true,
            ),
            (
                "values",
                [
                    Signal::Changed(RightTrigger2, 1.0),
                    Signal::Changed(RightTrigger2, 0.0),
                ],
                true,
            ),
            (
                "z axes",
                [
                    Signal::Axis(Axis::RightZ, 1.0),
                    Signal::Axis(Axis::RightZ, -1.0),
                ],
                true,
            ),
            (
                "unnamed axes",
                [
                    Signal::Unnamed(ABS_GAS, 1.0),
                    Signal::Unnamed(ABS_GAS, -1.0),
                ],
                true,
            ),
            (
                "unnamed axes (left)",
                [
                    Signal::Unnamed(ABS_BRAKE, 1.0),
                    Signal::Unnamed(ABS_BRAKE, -1.0),
                ],
                false,
            ),
        ];
        for (name, [down, up], right) in ways {
            let (chord_button, wanted) = match right {
                true => (South, KeyCode::Enter),
                false => (LeftTrigger, KeyCode::Home),
            };
            pretend(&[
                // the trigger, moving on the way down and held
                down,
                Signal::Pressed(chord_button),
                Signal::Released(chord_button),
                up,
            ]);
            let mut board = PadKeyboard::default();
            let mut last = None;
            while let Some((pressed, family)) = press() {
                last = Some((pressed, board.press(pressed, family, is_held)));
            }
            assert_eq!(
                last,
                Some((chord_button, Output::Keys(vec![wanted]))),
                "{name}"
            );
        }
    }

    /// The keyboard's own trigger chords, through the same functions the ritual loop calls
    /// (`press`, then `is_held` as the keyboard asks it), with the trigger held while the other
    /// button is pressed.
    #[test]
    fn the_keyboards_trigger_chords_work_through_the_functions_the_ritual_loop_calls() {
        use crate::keyboard::{Output, PadKeyboard};
        use Button::*;
        let cases: &[(Button, Button, Output)] = &[
            (RightTrigger2, South, Output::Keys(vec![KeyCode::Enter])),
            (RightTrigger2, East, Output::Keys(vec![KeyCode::Esc])),
            (
                RightTrigger2,
                LeftTrigger,
                Output::Keys(vec![KeyCode::Home]),
            ),
            (
                RightTrigger2,
                RightTrigger,
                Output::Keys(vec![KeyCode::End]),
            ),
            (LeftTrigger2, LeftTrigger, Output::Keys(vec![KeyCode::Home])),
            (LeftTrigger2, RightTrigger, Output::Keys(vec![KeyCode::End])),
            (RightTrigger2, DPadUp, Output::Keys(vec![KeyCode::BackTab])),
            (RightTrigger2, DPadDown, Output::Keys(vec![KeyCode::Tab])),
            (
                LeftTrigger2,
                West,
                Output::Keys(vec![crate::widgets::text::DELETE_WORD]),
            ),
            (
                LeftTrigger2,
                East,
                Output::Keys(vec![crate::widgets::text::CLEAR]),
            ),
        ];
        for (trigger, button, wanted) in cases {
            pretend(&[
                Signal::Pressed(*trigger),
                Signal::Pressed(*button),
                Signal::Released(*button),
                Signal::Released(*trigger),
            ]);
            let mut board = PadKeyboard::default();
            let mut made = Vec::new();
            while let Some((pressed, family)) = press() {
                made.push((pressed, board.press(pressed, family, is_held)));
            }
            let last = made.pop().expect("both presses came");
            assert_eq!(last.0, *button, "{trigger:?} + {button:?}");
            assert_eq!(&last.1, wanted, "{trigger:?} + {button:?}");
            assert!(!is_held(*trigger), "let go at the end");
        }
    }

    #[test]
    fn gilrs_sending_both_the_value_and_the_press_is_one_press() {
        use Button::*;
        // gilrs sends `ButtonChanged(1.0)` and then `ButtonPressed`, and the same when let go
        let (presses, releases, _) = through(
            &[
                Signal::Changed(LeftTrigger2, 1.0),
                Signal::Pressed(LeftTrigger2),
                Signal::Changed(LeftTrigger2, 0.0),
                Signal::Released(LeftTrigger2),
            ],
            false,
        );
        assert_eq!(
            (presses, releases),
            (vec![LeftTrigger2], vec![LeftTrigger2])
        );
    }

    #[test]
    fn a_trigger_that_is_an_axis_rests_at_minus_one_and_is_down_near_one() {
        use Button::*;
        let z = |axis, value| Signal::Axis(axis, value);
        let (presses, releases, _) = through(
            &[
                z(Axis::LeftZ, -1.0),
                z(Axis::LeftZ, -0.8),
                z(Axis::LeftZ, 0.1),
                z(Axis::LeftZ, 0.9),
                z(Axis::LeftZ, 0.0),
                z(Axis::LeftZ, -0.4),
                z(Axis::LeftZ, -1.0),
                z(Axis::RightZ, -1.0),
                z(Axis::RightZ, 1.0),
            ],
            false,
        );
        assert_eq!(presses, [LeftTrigger2, RightTrigger2]);
        assert_eq!(releases, [LeftTrigger2]);
    }

    #[test]
    fn a_bluetooth_xbox_pads_triggers_are_unnamed_axes_and_still_triggers() {
        use Button::*;
        // what `--controller-test` showed on an Xbox Wireless Controller: kind 3 codes 10 and 9
        let brake = |value| Signal::Unnamed(ABS_BRAKE, value);
        let gas = |value| Signal::Unnamed(ABS_GAS, value);
        let (presses, releases, keys) = through(
            &[
                brake(-0.9977725),
                brake(-0.19365332),
                brake(1.0),
                brake(0.39635372),
                brake(-0.659196),
                gas(1.0),
                gas(-1.0),
                // some other axis nobody has a use for
                Signal::Unnamed((3 << 16) | 40, 1.0),
            ],
            false,
        );
        assert_eq!(presses, [LeftTrigger2, RightTrigger2]);
        assert_eq!(releases, [LeftTrigger2, RightTrigger2]);
        assert_eq!(keys, [KeyCode::F(3), KeyCode::F(2)]);
    }

    #[test]
    fn a_trigger_axis_that_rests_in_the_middle_is_not_pressed_by_resting() {
        let (presses, ..) = through(&[Signal::Axis(Axis::LeftZ, 0.0); 3], false);
        assert!(presses.is_empty());
    }

    #[test]
    fn select_start_and_home_are_buttons_like_any_other() {
        use Button::*;
        let (presses, _, keys) = through(
            &[
                Signal::Pressed(Select),
                Signal::Pressed(Start),
                Signal::Pressed(Mode),
            ],
            false,
        );
        assert_eq!(presses, [Select, Start, Mode]);
        assert_eq!(
            keys,
            [KeyCode::Char('w'), KeyCode::F(1), KeyCode::Char('o')]
        );
    }

    #[test]
    fn a_trigger_makes_join_and_host_keys_and_ends_the_splash_like_a_button() {
        use Button::*;
        let (presses, _, keys) = through(
            &[
                Signal::Axis(Axis::LeftZ, 1.0),
                Signal::Changed(RightTrigger2, 1.0),
            ],
            false,
        );
        // the splash ends on any press, and these are presses
        assert_eq!(presses, [LeftTrigger2, RightTrigger2]);
        assert_eq!(keys, [KeyCode::F(3), KeyCode::F(2)]);
    }

    #[test]
    fn a_bumper_chord_with_a_trigger_down_makes_nothing_and_without_it_makes_its_key() {
        use Button::*;
        let bumper_chord = |trigger: Option<Signal>| {
            let mut signals: Vec<Signal> = trigger.into_iter().collect();
            signals.extend([
                Signal::Pressed(LeftTrigger),
                Signal::Pressed(South),
                Signal::Released(South),
                Signal::Released(LeftTrigger),
            ]);
            through(&signals, false).2
        };
        assert_eq!(bumper_chord(None), [KeyCode::Char('a')]);
        let with_trigger = bumper_chord(Some(Signal::Axis(Axis::RightZ, 1.0)));
        // the trigger's own key, and no chord, and no Enter or Backtab either
        assert_eq!(with_trigger, [KeyCode::F(2)]);
    }

    #[test]
    fn the_inputs_put_a_pads_x_and_y_right_before_anything_else_sees_them() {
        use Button::*;
        let (presses, ..) = through(&[Signal::Pressed(West), Signal::Pressed(North)], true);
        assert_eq!(presses, [North, West]);
        let (_, _, keys) = through(&[Signal::Pressed(North)], true);
        assert_eq!(
            keys,
            [KeyCode::Char('/')],
            "the pad's X, called North, is search"
        );
    }

    #[test]
    fn an_arrow_held_repeats_and_the_stick_makes_arrows() {
        use Button::*;
        let start = Instant::now();
        let mut inputs = Inputs::default();
        inputs.feed(Signal::Pressed(DPadDown), false, start);
        assert_eq!(inputs.presses.pop_front(), Some(DPadDown));
        assert_eq!(inputs.repeat(start), None);
        assert_eq!(inputs.repeat(start + REPEAT_DELAY), Some(DPadDown));
        inputs.feed(Signal::Released(DPadDown), false, start);
        assert_eq!(inputs.repeat(start + 2 * REPEAT_DELAY), None);
        inputs.feed(Signal::Axis(Axis::LeftStickX, 0.9), false, start);
        assert_eq!(inputs.presses.pop_front(), Some(DPadRight));
    }

    #[test]
    fn the_controller_test_says_what_arrived_where_tdl_takes_it_and_the_key() {
        use Button::*;
        let mut probe = Probe::default();
        let mut out = Vec::new();
        let mut say = |label: &str, signal| {
            probe
                .signal(&mut out, label, signal, Family::Xbox, true)
                .unwrap();
        };
        // a pad that calls its left X "North" and a trigger that is an axis
        say("ButtonPressed(North)", Signal::Pressed(North));
        say("AxisChanged(RightZ, 1.0)", Signal::Axis(Axis::RightZ, 1.0));
        say(
            "AxisChanged(LeftStickX, 0.02)",
            Signal::Axis(Axis::LeftStickX, 0.02),
        );
        say(
            "AxisChanged(LeftStickX, 0.03)",
            Signal::Axis(Axis::LeftStickX, 0.03),
        );
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("ButtonPressed(North)"), "{text}");
        assert!(
            text.contains("West, the left face button: (X), key Char('/')"),
            "{text}"
        );
        assert!(
            text.contains("RightTrigger2, the right trigger: (RT), key F(2)"),
            "{text}"
        );
        assert!(text.contains("sends the key F(2)"), "{text}");
        // 🌊 a stick that hardly moved is shown once
        assert_eq!(text.matches("LeftStickX").count(), 1, "{text}");
    }

    #[test]
    fn every_chord_is_its_own_key_and_none_repeats_a_plain_button() {
        let prompts = [
            Prompt::Confirm,
            Prompt::Back,
            Prompt::Search,
            Prompt::Cut,
            Prompt::Up,
            Prompt::Down,
            Prompt::Left,
            Prompt::Right,
        ];
        let mut made = Vec::new();
        for (left, right) in [(true, false), (false, true), (true, true)] {
            for prompt in prompts {
                let key = chord(left, right, prompt);
                // 🚫 LB with Y is nothing: settings are on the Home button
                if (left, right, prompt) == (true, false, Prompt::Cut) {
                    assert_eq!(key, None);
                    continue;
                }
                assert!(key.is_some(), "{left} {right} {prompt:?}");
                made.push(key.unwrap());
            }
        }
        let mut unique = made.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), made.len(), "no two chords make the same key");
        assert_eq!(chord(false, false, Prompt::Confirm), None);
        assert_eq!(chord(true, false, Prompt::Keys), None);
    }

    #[test]
    fn the_stick_points_where_it_is_pushed_furthest() {
        assert_eq!(stick_arrow((0.2, 0.3)), None);
        assert_eq!(stick_arrow((0.1, 0.9)), Some(Button::DPadUp));
        assert_eq!(stick_arrow((0.6, -0.9)), Some(Button::DPadDown));
        assert_eq!(stick_arrow((-0.8, 0.4)), Some(Button::DPadLeft));
    }

    #[test]
    fn legend_keys_become_buttons_and_the_rest_stay_keys() {
        let text = |label| relabel(label, Family::Xbox);
        // 🔘 named in text, a button is in parentheses, so it is never taken for a key
        assert_eq!(text("Enter"), "(A)");
        assert_eq!(text("Esc"), "(B)");
        assert_eq!(text("Tab/↓, Backtab/↑"), "(RB)/(D↓), (LB)/(D↑)");
        assert_eq!(text("J↓/K↑"), "(D↓)/(D↑)");
        assert_eq!(text("H⟨host⟩/N⟨join⟩"), "(RT)/(LT)");
        assert_eq!(plain("H⟨host⟩"), "H");
        assert_eq!(plain("Enter"), "Enter");
        assert_eq!(text("J/K, ←/→"), "(D↓)/(D↑), (D←)/(D→)");
        assert_eq!(text("/"), "(X)");
        assert_eq!(text("/ ; ,"), "(X) ; ,");
        assert_eq!(text("Shift+V/M"), "Shift+V/M");
        assert_eq!(text("A-Z/0-9/_"), "A-Z/0-9/_");
        assert_eq!(text("F1"), "(Menu)");
        assert_eq!(text("W/B"), "(View)/B");
        assert_eq!(text("Y/Shift+Y"), "(L3)/Shift+Y");
        assert_eq!(text("X/P"), "(Y)/(R3)");
        assert_eq!(relabel("Enter", Family::PlayStation), "(✕)");
        assert_eq!(relabel("Esc", Family::Nintendo), "(B)");
        assert_eq!(relabel("X", Family::Nintendo), "(X)");
        assert_eq!(relabel("/", Family::Nintendo), "(Y)");
        assert_eq!(text("A⟨lb+a⟩"), "(LB)+(A)");
        assert_eq!(text("U⟨rb+a⟩, M⟨lb+up⟩"), "(RB)+(A), (LB)+(D↑)");
        assert_eq!(relabel("A⟨lb+a⟩", Family::PlayStation), "(L1)+(✕)");
        assert_eq!(plain("A⟨lb+a⟩/E⟨lb+x⟩"), "A/E");
        assert_eq!(text("/ ;⟨rb+right⟩ ,⟨rb+left⟩"), "(X) (RB)+(D→) (RB)+(D←)");
    }

    #[test]
    fn every_button_is_named_in_parentheses_on_every_controller() {
        for family in Family::ALL {
            for prompt in PROMPTS {
                assert!(!prompt.text(family).is_empty());
                assert_eq!(prompt.label(family), format!("({})", prompt.text(family)));
            }
        }
    }
}
