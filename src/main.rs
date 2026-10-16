#![feature(associated_type_defaults)]

use std::io::Write;

use crate::{
    cli::Cli,
    screens::launcher::{Launcher, Start},
};

mod cli;
mod clipboard;
mod error;
mod layout;
mod music;
mod net;
mod picker;
mod popups;
mod quips;
mod ritual;
mod screens;
mod slayer;
mod tag;
mod tally;
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
    if args.iter().any(|arg| arg == "--list-quips") {
        print!("{}", quips::listing());
        return;
    }
    let cli = Cli::parse(args).unwrap_or_else(|message| {
        eprintln!("{message}\n\n{}", cli::USAGE);
        std::process::exit(2);
    });
    if let Some(song) = &cli.music
        && let Err(message) = music::use_song(std::path::Path::new(song))
    {
        eprintln!("{message}");
        std::process::exit(2);
    }
    if let Err(error) = ratatui::run(|grimoire| {
        let start = Start {
            vault: cli.vault,
            ask_about_vault: !cli.vault_once,
            forget_vault: cli.forget_vault,
            wads: cli.wads,
            unseal: cli.unseal,
            splash: !cli.no_splash,
            quip: cli.quip,
            no_quips: cli.no_quips,
            keep_music: cli.keep_music,
            no_music: cli.no_music,
        };
        Launcher::summon(grimoire, start)
    }) {
        eprintln!("{error}");
        std::io::stderr().flush().unwrap();
    }
}
