use std::io::Write;

mod config;
mod dialogue;
mod editor;
mod error;
mod execute;
mod host;
mod index;
mod join;
mod launcher;
mod legend;
mod profile;
mod resize;
mod utility;
mod warning;

fn main() {
    if let Err(error) = ratatui::run(launcher::run) {
        eprintln!("{error}");
        std::io::stderr().flush().unwrap();
    }
}
