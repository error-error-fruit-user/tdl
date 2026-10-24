//! The soundtrack: "Inevitable Doom" by celestialghost8, embedded so the binary stays a single file,
//! or a song of the player's own from `--music` or the **song** setting. It plays on a background
//! thread of its own, which can be stopped and started again, and a machine with no sound device
//! stays silent.

use std::{
    io::Cursor,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use rodio::{Decoder, DeviceSinkBuilder, Player};

const TRACK: &[u8] = include_bytes!("../assets/inevitable_doom.wav");
/// How often the music thread checks whether it should stop or start the track over.
const POLL: Duration = Duration::from_millis(50);

/// How long the music takes to fade in, and the splash screen with it.
pub const FADE_IN: Duration = Duration::from_millis(1500);
/// How long the music takes to fade out once it is told to stop; the splash screen fades to black
/// over the same time.
pub const FADE: Duration = Duration::from_millis(2000);

/// Which player thread should be playing, by the number it was started with; 0 for none. A thread
/// whose number this no longer is fades out and ends, so a new one can start at any time while an
/// old one is still fading.
static PLAYING: AtomicU64 = AtomicU64::new(0);
/// The number the next player thread gets.
static NEXT: AtomicU64 = AtomicU64::new(1);
/// Whether the music carries on past the splash screen.
static KEEP: AtomicBool = AtomicBool::new(false);
/// Set by `--no-music` or the **music** setting's **off**: nothing plays until it is unmuted.
static MUTED: AtomicBool = AtomicBool::new(false);
/// The player's own song, read whole, in place of [`TRACK`].
static SONG: Mutex<Option<Arc<[u8]>>> = Mutex::new(None);

/// The kinds of song the pickers offer, which are the ones rodio is built to decode here.
pub const SONG_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "ogg", "WAV", "MP3", "FLAC", "OGG"];

/// Reads the song at `path` whole, checking it is one rodio can play (wav, mp3, flac or ogg
/// vorbis), so a bad file is caught before it is needed. `Err` says what is wrong, for the user.
pub fn check_song(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Could not read the song {}: {error}", path.display()))?;
    if Decoder::try_from(Cursor::new(bytes.clone())).is_err() {
        return Err(format!(
            "{} isn't a song tdl can play (wav, mp3, flac or ogg)",
            path.display()
        ));
    }
    Ok(bytes)
}

/// Plays the file at `path` instead of the built-in track from now on, once it has passed
/// [`check_song`]. Music already playing switches over (see [`restart`]).
pub fn use_song(path: &Path) -> Result<(), String> {
    let bytes = check_song(path)?;
    set_song(Some(bytes.into()));
    Ok(())
}

/// Goes back to the built-in track.
pub fn forget_song() {
    set_song(None);
}

fn set_song(song: Option<Arc<[u8]>>) {
    if let Ok(mut current) = SONG.lock() {
        *current = song;
    }
}

/// Whether a song of the player's own has been given already, so `--music` beats the **song**
/// setting, which is looked at later.
pub fn has_song() -> bool {
    SONG.lock().is_ok_and(|song| song.is_some())
}

/// Starts the music, looping, until the splash is over (see [`splash_over`]), it is stopped, or
/// the program ends. With `keep`, it carries on past the splash screen. While music is playing,
/// another call only adds `keep` to it; while muted, nothing plays.
pub fn start(keep: bool) {
    if MUTED.load(Ordering::Relaxed) {
        return;
    }
    if keep {
        KEEP.store(true, Ordering::Relaxed);
    }
    if PLAYING.load(Ordering::Relaxed) != 0 {
        return;
    }
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    PLAYING.store(number, Ordering::Relaxed);
    thread::spawn(move || play(number));
}

/// Called when the splash screen is dismissed: fades the music out unless `--keep-music` (or the
/// **music** setting) asked to keep it.
pub fn splash_over() {
    if !KEEP.load(Ordering::Relaxed) {
        stop();
    }
}

/// Fades the music out now, whether or not it was meant to carry on past the splash screen.
/// [`start`] can bring it back.
pub fn stop() {
    KEEP.store(false, Ordering::Relaxed);
    PLAYING.store(0, Ordering::Relaxed);
}

/// Starts the music over if it is playing, so a new song is heard straight away.
pub fn restart() {
    if PLAYING.load(Ordering::Relaxed) != 0 {
        let keep = KEEP.load(Ordering::Relaxed);
        stop();
        start(keep);
    }
}

/// Keeps every later [`start`] from playing anything, and fades out whatever already plays.
pub fn mute() {
    MUTED.store(true, Ordering::Relaxed);
    stop();
}

/// Lets the music play again after [`mute`]. It doesn't start by itself.
pub fn unmute() {
    MUTED.store(false, Ordering::Relaxed);
}

/// Queues the track that should play: the player's song if they gave one, else the built-in one.
/// `false` if it won't decode.
fn queue_track(player: &Player) -> bool {
    let song = SONG.lock().ok().and_then(|song| song.clone());
    match song {
        Some(song) => Decoder::try_from(Cursor::new(song)).map(|track| player.append(track)),
        None => Decoder::try_from(Cursor::new(TRACK)).map(|track| player.append(track)),
    }
    .is_ok()
}

/// Plays until this thread's `number` is no longer the one [`PLAYING`], then fades out.
fn play(number: u64) {
    // 🔇 no sound device, or a track that won't decode: nothing to do but stay quiet
    let Ok(mut sink) = DeviceSinkBuilder::open_default_sink() else {
        return;
    };
    sink.log_on_drop(false);
    let player = Player::connect_new(sink.mixer());
    player.set_volume(0.0);
    let begun = Instant::now();
    while PLAYING.load(Ordering::Relaxed) == number {
        if player.empty() && !queue_track(&player) {
            return;
        }
        // 🔊 swell in rather than blasting out
        let swell = begun.elapsed().as_secs_f32() / FADE_IN.as_secs_f32();
        if swell < 1.0 {
            player.set_volume(swell);
        } else if swell < 1.0 + POLL.as_secs_f32() / FADE_IN.as_secs_f32() {
            player.set_volume(1.0);
        }
        thread::sleep(POLL);
    }
    // 🔉 fade out rather than cutting off mid-note, from however loud it had got
    let from = player.volume();
    let steps = (FADE.as_millis() / POLL.as_millis()) as u32;
    for step in (0..steps).rev() {
        player.set_volume(from * step as f32 / steps as f32);
        thread::sleep(POLL);
    }
}
