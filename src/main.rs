#![feature(associated_type_defaults)]

use std::io::Write;

use crate::{
    cli::Cli,
    error::{Error, INTERRUPTED_EXIT},
    screens::launcher::{Launcher, Start},
};

mod cli;
mod clipboard;
mod error;
mod layout;
mod loadout;
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
mod widgets;

fn main() {
    // 📖 clap answers --help and --version itself, and says what is wrong with anything else
    let cli = Cli::parse(std::env::args().skip(1)).unwrap_or_else(|error| error.exit());
    if cli.list_quips {
        print!("{}", quips::listing());
        return;
    }
    if cli.pride {
        theme::wear_pride();
    }
    if let Some(song) = &cli.music
        && let Err(message) = music::use_song(std::path::Path::new(song))
    {
        eprintln!("{message}");
        std::process::exit(2);
    }
    // 🧪 this run's UZDoom and base wad must be there, said now rather than at launch
    let session_file = |flag: &str, file: Option<String>| {
        file.map(|file| {
            let path = std::path::absolute(&file).unwrap_or_else(|_| file.clone().into());
            if !path.is_file() {
                eprintln!("{flag}: there is no file at {}", path.display());
                std::process::exit(2);
            }
            path
        })
    };
    let uzdoom = session_file("--uzdoom", cli.uzdoom);
    let base_wad = session_file("--base-wad", cli.base_wad);
    let headless = cli.headless;
    let seal = cli.seal;
    let start = Start {
        loadout: cli.loadout,
        ask_about_loadout: !cli.loadout_once,
        forget_loadout: cli.forget_loadout,
        ask_about_wads: !cli.wads_once,
        forget_wads: cli.forget_wads,
        wads: cli.wads,
        unseal: cli.unseal,
        unseal_delete: cli.unseal_delete,
        // 🎯 a command goes straight to the game, so it skips the splash
        splash: !cli.no_splash && cli.command.is_none(),
        quip: cli.quip,
        no_quips: cli.no_quips,
        keep_music: cli.keep_music,
        no_music: cli.no_music,
        uzdoom,
        base_wad,
        command: cli.command,
    };
    // 📜 --seal draws no windows either: the loadout goes into the scroll, and tdl closes
    if let Some(seal) = seal {
        if let Err(message) = Launcher::seal_plain(start, &seal) {
            eprintln!("{message}");
            std::process::exit(1);
        }
        return;
    }
    // 🖥️ --headless draws no windows at all: plain lines, and an error is a failing exit
    if headless {
        if let Err(message) = Launcher::summon_plain(start) {
            eprintln!("{message}");
            std::process::exit(1);
        }
        return;
    }
    if let Err(error) = ratatui::run(|grimoire| Launcher::summon(grimoire, start)) {
        eprintln!("{error}");
        std::io::stderr().flush().unwrap();
        // 🛑 Ctrl+C ends tdl the way an interrupted program ends, the same as with --headless
        if matches!(error, Error::Interrupted) {
            std::process::exit(INTERRUPTED_EXIT);
        }
    }
}
