#![feature(associated_type_defaults)]

use std::{io::Write, path::PathBuf};

use crate::{
    cli::Cli,
    error::{Error, INTERRUPTED_EXIT},
    screens::launcher::{Launcher, Start},
    theme::{Dressing, Theme, splash::Splash},
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
    // 🎨 the help wears the kept theme too, if it is one that can be worn
    if let Some(theme) = Launcher::kept_theme().and_then(Result::ok) {
        theme::wear(theme);
    }
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
    // 🧪 this run's UZDoom, base wad, ini, theme and splash must be there, said now rather than at
    // launch
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
    let ini = session_file("--ini", cli.ini);
    // 👗 a theme or a splash may be a name instead (`--theme void`), found in the standard config
    // dir or among the ones that come with tdl, and must be one that can be worn, from the very
    // first window
    let config_dir = Launcher::standard_config_dir().unwrap_or_default();
    let dressing = |flag: &str, given: Option<String>| match given {
        Some(name) if theme::dressing::is_name(&name) => Some(PathBuf::from(name)),
        given => session_file(flag, given),
    };
    let theme = dressing("--theme", cli.theme);
    let splash = dressing("--splash", cli.splash);
    if let Some(given) = &theme {
        match Theme::load(given.as_os_str(), &config_dir) {
            Ok(theme) => theme::wear(theme),
            Err(message) => {
                eprintln!("--theme: {message}");
                std::process::exit(2);
            }
        }
    }
    if let Some(given) = &splash {
        match Splash::load(given.as_os_str(), &config_dir) {
            Ok(splash) => theme::splash::wear(splash),
            Err(message) => {
                eprintln!("--splash: {message}");
                std::process::exit(2);
            }
        }
    }
    let headless = cli.headless;
    let seal = cli.seal;
    let start = Start {
        loadout: cli.loadout,
        keep_loadout: cli.keep_loadout,
        forget_loadout: cli.forget_loadout,
        keep_wads: cli.keep_wads,
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
        keep_uzdoom: cli.keep_uzdoom,
        base_wad,
        keep_base_wad: cli.keep_base_wad,
        ini,
        keep_ini: cli.keep_ini,
        theme,
        keep_theme: cli.keep_theme,
        splash_look: splash,
        keep_splash: cli.keep_splash,
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
