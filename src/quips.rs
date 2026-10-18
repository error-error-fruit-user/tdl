//! One-liners in the spirit of Minecraft's title screen: one sits under the splash logo, and the
//! progress windows cycle through them while you wait.

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

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
    "Six, Seven!",
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
/// Off with `--no-quips` or the **quips** setting, which leaves the quips out of every window.
static SHOWN: AtomicBool = AtomicBool::new(true);

/// Shows or hides the quips everywhere from now on.
pub fn show(shown: bool) {
    SHOWN.store(shown, Ordering::Relaxed);
}

/// Whether windows should show a quip.
pub fn shown() -> bool {
    SHOWN.load(Ordering::Relaxed)
}

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

/// The built-in quip numbered `number`, counting from 1 as `--list-quips` shows them.
pub fn numbered(number: usize) -> Option<&'static str> {
    QUIPS.get(number.checked_sub(1)?).copied()
}

/// A quip of the user's own, from `--splash-quip` or the **splash quip** setting, with every
/// `#<number>` in it replaced by that built-in quip and `##` by a plain `#`. So `#67` is quip 67,
/// `#67, hun!` is quip 67 with `, hun!` after it (no space is added), and a `#` with no number after
/// it stays as it is. `Err` says which number isn't one of the quips.
pub fn expand(template: &str) -> Result<String, String> {
    let mut quip = String::new();
    let mut rest = template;
    while let Some(at) = rest.find('#') {
        quip.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        if let Some(after) = rest.strip_prefix('#') {
            quip.push('#');
            rest = after;
            continue;
        }
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            quip.push('#');
            continue;
        }
        let picked = rest[..digits]
            .parse::<usize>()
            .ok()
            .and_then(numbered)
            .ok_or_else(|| {
                format!(
                    "There is no quip #{}: pick from #1 to #{} (tdl --list-quips shows them)",
                    &rest[..digits],
                    QUIPS.len()
                )
            })?;
        quip.push_str(picked);
        rest = &rest[digits..];
    }
    quip.push_str(rest);
    Ok(quip)
}

/// A quip given on the command line, cut short with `…` if it doesn't fit in `width` with a
/// column to spare on each side. `None` if there is no room for even one letter.
pub fn fit(quip: &str, width: u16) -> Option<String> {
    let room = (width as usize).checked_sub(2).filter(|&room| room >= 2)?;
    if quip.chars().count() <= room {
        Some(quip.to_string())
    } else {
        Some(quip.chars().take(room - 1).chain(['…']).collect())
    }
}

/// Every quip on its own line, numbered from 1 the way `#<number>` in a splash quip counts, the numbers
/// right-aligned so the quips line up. For `--list-quips`.
pub fn listing() -> String {
    let width = QUIPS.len().to_string().len();
    QUIPS
        .iter()
        .enumerate()
        .map(|(i, quip)| format!("{:>width$}  {quip}\n", i + 1))
        .collect()
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
    fn quips_are_numbered_from_one() {
        assert_eq!(numbered(1), Some(QUIPS[0]));
        assert_eq!(numbered(QUIPS.len()), QUIPS.last().copied());
        assert_eq!(numbered(0), None);
        assert_eq!(numbered(QUIPS.len() + 1), None);
    }

    #[test]
    fn every_number_is_replaced_by_its_quip_and_the_rest_is_kept_as_typed() {
        let (one, two) = (QUIPS[0], QUIPS[1]);
        assert_eq!(expand("#1").unwrap(), one);
        assert_eq!(expand("#2message").unwrap(), format!("{two}message"));
        assert_eq!(expand("Hey! #1").unwrap(), format!("Hey! {one}"));
        assert_eq!(expand("#1#2").unwrap(), format!("{one}{two}"));
        assert_eq!(expand("Rip and tear!").unwrap(), "Rip and tear!");
        // a `#` without a number stays, and `##` is how to write a `#` before digits
        assert_eq!(expand("# 1 #").unwrap(), "# 1 #");
        assert_eq!(expand("##1 fan!").unwrap(), "#1 fan!");
        assert_eq!(expand("").unwrap(), "");
    }

    #[test]
    fn a_number_that_is_not_a_quip_says_so() {
        assert!(expand("#0").unwrap_err().contains("#0"));
        let past = QUIPS.len() + 1;
        assert!(
            expand(&format!("hi #{past}"))
                .unwrap_err()
                .contains(&format!("#{past}"))
        );
        assert!(expand("#99999999999999999999999").is_err());
    }

    #[test]
    fn a_quip_of_your_own_is_cut_short_only_when_it_has_to_be() {
        assert_eq!(fit("Hello, hell!", 80).as_deref(), Some("Hello, hell!"));
        assert_eq!(fit("Hello, hell!", 14).as_deref(), Some("Hello, hell!"));
        assert_eq!(fit("Hello, hell!", 10).as_deref(), Some("Hello, …"));
        // counted in characters, not bytes
        assert_eq!(fit("héllo", 7).as_deref(), Some("héllo"));
        assert_eq!(fit("Hello", 3), None);
    }

    #[test]
    fn the_listing_numbers_every_quip_from_one_in_order() {
        let listing = listing();
        let lines: Vec<_> = listing.lines().collect();
        assert_eq!(lines.len(), QUIPS.len());
        let width = QUIPS.len().to_string().len();
        assert_eq!(lines[0], format!("{:>width$}  {}", 1, QUIPS[0]));
        let last = lines.last().unwrap();
        assert_eq!(
            *last,
            format!("{}  {}", QUIPS.len(), QUIPS[QUIPS.len() - 1])
        );
    }

    #[test]
    fn a_wait_moves_on_to_the_next_quip_every_few_seconds() {
        assert_eq!(cycling(0, Duration::ZERO), QUIPS[0]);
        assert_eq!(cycling(0, EVERY - Duration::from_millis(1)), QUIPS[0]);
        assert_eq!(cycling(0, EVERY), QUIPS[1]);
        assert_eq!(cycling(QUIPS.len() as u32 - 1, EVERY), QUIPS[0]);
    }
}
