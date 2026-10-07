use std::io::Write;

mod config;
mod dialogue;
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
mod network;
mod profile;
mod resize;
mod utility;
mod warning;

fn main() {
    let args = std::env::args();
    let pass = args.skip(1).next();
    if let Err(error) = ratatui::run(|terminal| launcher::run(terminal, pass)) {
        eprintln!("{error}");
        std::io::stderr().flush().unwrap();
    }
}
