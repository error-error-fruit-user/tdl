use std::io::Write;

mod dialogue;
mod editor;
mod error;
mod host;
mod index;
mod join;
mod launcher;
mod legend;
mod profile;
mod resize;
mod window;

fn main() {
    if let Err(error) = ratatui::run(launcher::run) {
        eprintln!("{error}");
        std::io::stderr().flush().unwrap();
    }
}
