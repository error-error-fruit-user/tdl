mod dialogue;
mod editor;
mod index;
mod launcher;
mod legend;
mod profile;
mod resize;
mod window;

fn main() {
    ratatui::run(launcher::run);
}
