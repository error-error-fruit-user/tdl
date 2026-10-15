#![feature(associated_type_defaults)]

use std::io::Write;

use crate::{cli::Cli, screens::launcher::Launcher};

mod cli;
mod error;
mod net;
mod popups;
mod ritual;
mod screens;
mod slayer;
mod uzdoom;
mod vault;

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
    if let Err(error) = ratatui::run(|grimoire| Launcher::summon(grimoire, cli.vault)) {
        eprintln!("{error}");
        std::io::stderr().flush().unwrap();
    }
}
