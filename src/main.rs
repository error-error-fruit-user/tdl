mod dialogue;
mod editor;
mod error;
mod index;
mod launcher;
mod legend;
mod profile;
mod resize;
mod window;

fn main() -> error::Result<()> {
    ratatui::run(launcher::run)
}
