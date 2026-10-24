#![cfg_attr(
    feature = "spooky",
    feature(
        associated_type_defaults,
        auto_traits,
        f128,
        negative_impls,
        trait_alias,
        yeet_expr
    )
)]

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
mod gamepad;
mod keyboard;
mod layout;
mod loadout;
mod music;
mod net;
mod picker;
mod popups;
mod quips;
mod ritual;
mod screens;
mod setup;
mod slayer;
mod tag;
mod tally;
mod theme;
mod uzdoom;
mod widgets;

fn main() {
    // 🎨 the help wears the kept theme too, if it is one that can be worn
    if let Some(theme) = Launcher::kept_theme().and_then(Result::ok) {
        theme::wear(theme);
    }
    // 📖 clap answers --help and --version itself, and says what is wrong with anything else
    let mut cli = Cli::parse(std::env::args().skip(1)).unwrap_or_else(|error| error.exit());
    // 🧰 tdl setup: the TOML's things, as if each came with its --keep- flag
    let setup = cli
        .setup
        .take()
        .map(|file| match setup::read(file, &mut cli) {
            Ok(read) => read,
            Err(problem) => {
                eprintln!("{problem}");
                std::process::exit(2);
            }
        });
    if cli.controller_test {
        if let Err(error) = gamepad::controller_test(&mut std::io::stdout()) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
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
    // 👗 a theme or a splash may be a name instead (`--theme voidless`), found in the standard config
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
    let play = cli.play;
    let rite = cli.rite;
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
    // 🧰 tdl setup keeps what its TOML gives, with no windows, and closes
    if let Some((path, profiles)) = setup {
        if let Err(message) = Launcher::setup_plain(start, &profiles) {
            eprintln!("{message}");
            std::process::exit(1);
        }
        ritual::murmur(format!("Kept what {} says", path.display()));
        return;
    }
    // 📜 tdl cli: one of the launcher's rites, from the command line, said through the séance
    if let Some(rite) = rite {
        if let Err(problem) = Launcher::rite(start, rite) {
            ritual::farewell(false, &problem);
            std::process::exit(1);
        }
        return;
    }
    // 👻 tdl headless: the possession, in ritual.rs, from the first press to the exorcism
    if let Some(play) = play {
        if !ritual::haunt(play.script.as_deref(), play.log.as_deref(), start) {
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
    let outcome = ritual::open_grimoire()
        .map_err(Error::from)
        .and_then(|mut grimoire| Launcher::summon(&mut grimoire, start));
    ratatui::restore();
    if let Err(error) = outcome {
        eprintln!("{error}");
        std::io::stderr().flush().unwrap();
        // 🛑 Ctrl+C ends tdl the way an interrupted program ends, the same as with --headless
        if matches!(error, Error::Interrupted) {
            std::process::exit(INTERRUPTED_EXIT);
        }
    }
}
