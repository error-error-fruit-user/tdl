#![feature(associated_type_defaults)]

use std::io::Write;

use crate::{cli::Cli, screens::launcher::Launcher};

mod cli;
mod error;
mod layout;
mod music;
mod net;
mod picker;
mod popups;
mod ritual;
mod screens;
mod slayer;
#[cfg(test)]
mod testing;
mod theme;
mod uzdoom;
mod vault;
mod widgets;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", cli::USAGE);
        return;
    }
    let cli = Cli::parse(args).unwrap_or_else(|message| {
        eprintln!("{message}\n\n{}", cli::USAGE);
        std::process::exit(2);
    });
    if cli.music {
        music::start(true);
    }
    if let Err(error) =
        ratatui::run(|grimoire| Launcher::summon(grimoire, cli.vault, !cli.no_splash))
    {
        eprintln!("{error}");
        std::io::stderr().flush().unwrap();
    }
}
