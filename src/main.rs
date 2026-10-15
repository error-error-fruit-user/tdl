#![feature(associated_type_defaults)]

use std::io::Write;

use crate::launcher::Launcher;

mod config;
mod dialog;
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
mod resize;
mod scene;
mod utility;
mod wad;
mod warning;

fn main() {
    let passed_directory = std::env::args().nth(1);
    if let Err(error) = ratatui::run(|terminal| Launcher::evoke(terminal, passed_directory)) {
        eprintln!("{error}");
        std::io::stderr().flush().unwrap();
    }
}
