#![feature(associated_type_defaults)]

use std::io::Write;

use crate::launcher::Launcher;

mod config;
mod dialog;
mod edition;
mod editor;
mod error;
mod execute;
mod hash;
mod host;
mod hosting;
mod index;
mod join;
mod joining;
mod launcher;
mod legend;
mod library;
mod network;
mod profile;
mod progress;
mod resize;
mod ritual;
mod scribe;
mod status;
mod wad;
mod warning;

/// An optional first argument replaces the platform config/data directories with
/// `<arg>/config` and `<arg>/data`.
fn main() {
    let passed_directory = std::env::args().nth(1);
    if let Err(error) = ratatui::run(|grimoire| Launcher::summon(grimoire, passed_directory)) {
        eprintln!("{error}");
        std::io::stderr().flush().unwrap();
    }
}
