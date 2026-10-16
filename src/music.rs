//! The soundtrack: "Inevitable Doom" by celestialghost8, embedded so the binary stays a single file.
//! It plays on a background thread of its own, and a machine with no sound device stays silent.

use std::{
    io::Cursor,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

use rodio::{Decoder, DeviceSinkBuilder, Player};

const TRACK: &[u8] = include_bytes!("../assets/inevitable_doom.wav");
/// How often the music thread checks whether it should stop or start the track over.
const POLL: Duration = Duration::from_millis(50);

/// How long the music takes to fade out once it is told to stop.
const FADE: Duration = Duration::from_millis(600);

static STOP: AtomicBool = AtomicBool::new(false);
static KEEP: AtomicBool = AtomicBool::new(false);

/// Starts the track, looping, until [`stop`] or the program ends. With `keep`, the music
/// carries on past the splash screen (see [`splash_over`]).
pub fn start(keep: bool) {
    KEEP.store(keep, Ordering::Relaxed);
    thread::spawn(play);
}

/// Called when the splash screen is dismissed: fades the music out unless `--music` asked to keep it.
pub fn splash_over() {
    if !KEEP.load(Ordering::Relaxed) {
        STOP.store(true, Ordering::Relaxed);
    }
}

fn play() {
    // 🔇 no sound device, or a track that won't decode: nothing to do but stay quiet
    let Ok(mut sink) = DeviceSinkBuilder::open_default_sink() else {
        return;
    };
    sink.log_on_drop(false);
    let player = Player::connect_new(sink.mixer());
    while !STOP.load(Ordering::Relaxed) {
        if player.empty() {
            match Decoder::try_from(Cursor::new(TRACK)) {
                Ok(track) => player.append(track),
                Err(_) => return,
            }
        }
        thread::sleep(POLL);
    }
    // 🔉 fade out rather than cutting off mid-note
    let steps = (FADE.as_millis() / POLL.as_millis()) as u32;
    for step in (0..steps).rev() {
        player.set_volume(step as f32 / steps as f32);
        thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_track_decodes() {
        assert!(Decoder::try_from(Cursor::new(TRACK)).is_ok());
    }
}
