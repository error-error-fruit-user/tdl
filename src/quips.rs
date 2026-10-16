//! One-liners in the spirit of Minecraft's title screen: one sits under the splash logo, and the
//! progress windows cycle through them while you wait.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The longest a quip may be: the narrowest terminal, less a column on each side.
pub const LONGEST: usize = 38;
/// How long a progress window keeps one quip before moving on to the next.
pub const EVERY: Duration = Duration::from_secs(10);

/// Plain ASCII, so a character is a column, and at most [`LONGEST`] long, so any of them fits
/// the narrowest terminal.
pub const QUIPS: &[&str] = &[
    "Minecraft?",
    // 🔫 knee deep in the dead
    "Rip and tear, bestie!",
    "IDDQD, darling!",
    "IDKFA: all keys, all looks!",
    "Also try Doom!",
    "Hurt me plenty (emotionally)!",
    "The cacodemon is valid!",
    "The Icon of Sin, but make it iconic!",
    "This door needs a blue key, hun!",
    "Don't leave yet, hun!",
    "E1M1 is a love song!",
    "Now at 35 tics per second!",
    "No BFG required to slay!",
    "Your soul sphere is glowing!",
    "Doomguy never skips leg day!",
    "Huge guts, huger looks!",
    "Hell on Earth, heaven in a terminal!",
    // 🦇 from the ritual
    "Yass, queen!",
    "Ok, boomer? Yass, queen!",
    "Ghosted! Boo!",
    "Ghosts take up no space!",
    "PRIDE is not a number!",
    "It's not a phase, mom!",
    "Consent is key_code!",
    "Ctrl+C is the safe word!",
    "Spill the tea, not the blood!",
    "Trans rights are human rights!",
    "Happy haunting, ghouls!",
    "The house is actually haunted!",
    "Unsafe, but make it fashion!",
    "Mutable and proud!",
    "Bi-curious about loops!",
    "Strut. Lurk. Spill.",
    "Every variant is invited!",
    "Thick wards, healthy boundaries!",
    "Hex codes and curses!",
    "Read the warnings, break the seal!",
    "Bewitched, bothered, borrow-checked!",
    "Built nightly, like all good spells!",
    "Slay, demon, slay!",
    "Terminally fabulous!",
    // 🦀 the crab joins the coven
    "Jaquon is awesome!",
    "thread 'main' panicked at 'too cute'",
    "E0666: demon does not live long enough",
    "Segmentation fault (just kidding)",
    "Fearless concurrency, fearless demons!",
    "Zero-cost abstractions, high slay!",
    "unwrap() responsibly, darling!",
    "Lifetimes are just vibes with rules!",
    "The borrow checker is your bestie!",
    "Borrowed a BFG, returned it on time",
    "Cargo cult? No, Cargo coven!",
    "cargo run --release the demons",
    "It compiles! Ship it to Hell!",
    "Option<Demon>: Some(Cacodemon)",
    "Result<Slay, Never>",
    "impl Fabulous for Doomguy {}",
    "Use of moved value: your soul",
    "Your demons are Send + Sync!",
    "No null, no fear, no mercy!",
    "Unsafe blocks, safe spaces!",
    "Clippy has notes, sweetie!",
    "Monomorphized and moisturized!",
    "Pattern matched. Demons dispatched.",
    "Big O? Big Ouch!",
    "IDCLIP through the type system!",
    "Cyberdemon, but make it async!",
    "Hello from the other side(effect)!",
    "Nightly is a lifestyle, babe!",
    // 💅 extra yass, no compiler required
    "Werk it, Doomguy, werk it!",
    "Yass, Doomslayer, yass!",
    "Shotgun? Darling, it's a statement.",
    "Mother is mothering in Hell!",
    "The demons are gagged!",
    "Ate and left no imps!",
    "Hellfire looks good on you!",
    "That chainsaw? Iconic.",
    "Strutting through E1M1 like a runway!",
    "The Spider Mastermind could never!",
    "Pink demon, pink lipstick!",
    "Lipstick red, like the red key!",
    "Gorgeous, gory, and glowing!",
    "Living for this lava floor!",
    "The runway is made of nukage!",
    "Werk the room, clear the room!",
    "Serving looks, taking skulls!",
    "Fierce, fabulous, and frag-ready!",
    "Not today, Satan! Okay, maybe today.",
    "Hell called. It wants its glam back.",
    "Too glam to give a damn!",
    "Health 100%, confidence 200%!",
    "Snatched waist, snatched souls!",
    "A soul sphere a day keeps Hell away!",
    "Tyler is a faggot?",
];
/// A seed that differs from run to run, so each start gets its own quip. It comes from the clock
/// rather than a random-number crate, since a quip doesn't need to be unpredictable.
pub fn seed() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |time| time.subsec_nanos())
}

/// The quip for `seed`, from the ones that fit in `width` with a column to spare on each side, so
/// it only changes if the terminal shrinks too far for it.
pub fn pick(seed: u32, width: u16) -> Option<&'static str> {
    let fitting: Vec<_> = QUIPS
        .iter()
        .filter(|quip| quip.len() + 2 <= width as usize)
        .collect();
    (!fitting.is_empty()).then(|| *fitting[seed as usize % fitting.len()])
}

/// The quip to show `elapsed` into a wait that started from `seed`: the next one every [`EVERY`].
pub fn cycling(seed: u32, elapsed: Duration) -> &'static str {
    let step = (elapsed.as_secs() / EVERY.as_secs()) as usize;
    QUIPS[(seed as usize).wrapping_add(step) % QUIPS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_quip_fits_the_smallest_terminal() {
        for quip in QUIPS {
            assert!(quip.is_ascii(), "{quip} must be plain ASCII");
            assert!(
                quip.len() + 2 <= crate::layout::MIN_WIDTH as usize,
                "{quip} is too long"
            );
        }
    }

    #[test]
    fn the_quip_is_picked_by_the_seed_and_only_from_ones_that_fit() {
        assert_eq!(pick(0, 80), Some(QUIPS[0]));
        assert_eq!(pick(QUIPS.len() as u32 + 1, 80), Some(QUIPS[1]));
        assert!((0..100).all(|seed| pick(seed, 14).is_some_and(|q| q.len() <= 12)));
        assert_eq!(pick(0, 3), None);
    }

    #[test]
    fn a_wait_moves_on_to_the_next_quip_every_few_seconds() {
        assert_eq!(cycling(0, Duration::ZERO), QUIPS[0]);
        assert_eq!(cycling(0, EVERY - Duration::from_millis(1)), QUIPS[0]);
        assert_eq!(cycling(0, EVERY), QUIPS[1]);
        assert_eq!(cycling(QUIPS.len() as u32 - 1, EVERY), QUIPS[0]);
    }
}
