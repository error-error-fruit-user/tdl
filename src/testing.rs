//! Shared helpers for tests: drawing a screen to text, and a vault full of example profiles.

use std::{fs, path::PathBuf};

use ratatui::{Frame, Terminal, backend::TestBackend};

use crate::vault::{Vault, profile::Profile, scribe::tests::Altar, wad::Wad};

/// Draws `scene` on a `width` x `height` screen and returns what is on it, one line per row with
/// trailing blanks trimmed, ready for a snapshot.
pub fn draw(width: u16, height: u16, scene: impl FnOnce(&mut Frame)) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(scene).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            let row: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
            row.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A vault in `altar` with three example profiles, the same ones as in the screenshots:
/// `brutal_doom` is healthy; `sigil_run` has a missing, a changed and a volatile wad;
/// `test_build` points at a UZDoom that doesn't exist. Returns the vault's folder.
pub fn example_vault(altar: &Altar) -> PathBuf {
    let wads = altar.file("wads");
    fs::create_dir_all(&wads).unwrap();
    let names = [
        "DOOM2.WAD",
        "brutalv22.pk3",
        "brutal_extras.pk3",
        "hud_minimal.pk3",
        "sigil_v1_21.wad",
        "gone.pk3",
        "changed.wad",
        "my_mod_wip.pk3",
    ];
    for name in names {
        fs::write(wads.join(name), name.as_bytes()).unwrap();
    }
    fs::write(altar.file("uzdoom.sh"), b"#!/bin/sh\n").unwrap();
    let wad = |name: &str| Wad::from_path(&wads.join(name)).unwrap();
    let mut wip = wad("my_mod_wip.pk3");
    wip.volatile = true;
    let profiles = [
        Profile::new(
            "brutal_doom".into(),
            vec![
                wad("brutalv22.pk3"),
                wad("brutal_extras.pk3"),
                wad("hud_minimal.pk3"),
            ],
        ),
        Profile::new(
            "sigil_run".into(),
            vec![
                wad("sigil_v1_21.wad"),
                wad("gone.pk3"),
                wad("changed.wad"),
                wip,
            ],
        ),
        Profile {
            uzdoom: Some(altar.file("no-such-uzdoom").into()),
            ..Profile::new("test_build".into(), vec![wad("hud_minimal.pk3")])
        },
    ];
    fs::remove_file(wads.join("gone.pk3")).unwrap();
    fs::write(wads.join("changed.wad"), b"changed-and-grown").unwrap();

    let root = altar.file("vault");
    let mut vault = Vault::new(root.join("config"), root.join("data"));
    vault.config.uzdoom = Some(altar.file("uzdoom.sh").into());
    vault.config.base_wad = Some(wad("DOOM2.WAD"));
    for profile in profiles {
        let mut index = vault.profiles.len();
        vault.insert(&mut index, profile).unwrap();
    }
    root
}
