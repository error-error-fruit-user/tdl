# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`tdl` is a ratatui terminal launcher for UZDoom: named profiles (ordered wad lists), one-key launch, and host/join multiplayer where the host serves its wad list over HTTP on port 8080 and joiners download missing wads. User-facing docs are in `README.md`; the on-disk layout is in `DATA_LAYOUT.md`; planned work is in `secrets/TODO.md` (local, untracked).

## Commands

Requires **nightly** Rust (`#![feature(associated_type_defaults)]`, edition 2024).

- Build / run: `cargo +nightly build --release`, `cargo +nightly run -- [--vault DIR | --vault-once DIR | --forget-vault] [--wads DIR]... [--unseal SCROLL] [--no-splash] [--splash-quip QUIP | --no-quips] [[--music SONG] [--keep-music] | --no-music] [--uzdoom FILE] [--base-wad FILE] [launch PROFILE | host PROFILE | join IP [--headless]] [--seal SCROLL [--with-settings] [--delete [--force]]] [--with-pride]` (`--help` for the full list; flags are parsed by clap in `src/cli.rs`, its help colored from `theme.rs`)
- Tests: `cargo +nightly test`; single test: `cargo +nightly test <name_substring>`
- UI tests are `insta` snapshots (`src/**/snapshots/*.snap`). Review changes with `cargo insta review` (or `INSTA_UPDATE`) rather than hand-editing.
- `./publish.sh` cross-builds the Windows release (`x86_64-pc-windows-gnu`) into `published/` and generates `THIRD_PARTY_LICENSES.txt` (needs `cargo-license`). Crates that ship no license file get theirs from `licenses/` (by crate name, or the part before the first `-`); the script fails, naming them, if any has none.
- Version control is **jj** (colocated with git). `secrets/commit.sh "msg" [--claude]` commits and moves the `main` bookmark; use `--claude` for work Claude helped write (adds the Co-Authored-By trailer). `secrets/push.sh` pushes `main` using the token next to it. Both run from anywhere in the project.

## Architecture

- **`ritual.rs`** is the UI framework core. The `Ritual` trait is the screen/popup abstraction: `render(&mut Frame)`, `prompt(grimoire, key) -> Phase<T>` (`Render` / `Prompt` / `Return`), and `invoke` runs the event loop until a value is returned. Ctrl+C becomes `Error::Interrupted`, which unwinds out of every nested ritual. It also holds the shared window helpers (`conjure_window`, `conjure_veil`, `conjure_sigil` for the `[?]` legend hint). The file (and parts of the codebase) uses a deliberately themed vocabulary via `as` aliases (Grimoire = Terminal, Costume = Frame, Chant = KeyCode, Omen = Result, etc.); follow the existing style in that file instead of "normalizing" it.
- **`screens/`** are full-screen rituals (launcher, editor, splash, host settings, join address, pickers). **`popups/`** are small modal rituals (dialog, three-way dialog, progress, status, warning, legend, resize notice). Screens nest popups by calling `.invoke(grimoire)` on them. **`widgets/`** are reusable drawing pieces, and `widgets/text.rs` is the one text-editing helper every typed field uses (cursor, arrows, Home/End). The launcher and editor share `screens/search.rs` (ranked search, history), `screens/selection.rs` (`Shift+V` ranges, `M` marks, moving rows) and the session-only `clipboard.rs`; `tag.rs` numbers copies (`brutal_2`). Settings are `screens/settings.rs`: the splash and UZDoom args per vault, the music, song and quips in `vault/global.rs`. `quips.rs` and `music.rs` hold global switches (`quips::show`, `music::mute`/`start`/`stop`).
- **`slayer/`** (`host_game.rs`, `join_game.rs`, `share.rs` for `.scroll` export and import, whose format is `vault/scroll.rs`) orchestrates the multiplayer and sharing flows end to end, using `net.rs` (address, join codes, name safety) and `net/` (`manifest.rs`, `serve.rs` with the tiny_http server, `download.rs` with ureq downloads and BLAKE3 verification) and `uzdoom.rs` (building the command line and spawning the game, capturing `session.log`).
- **`vault/`** is persistence. `Vault` holds the config dir and data dir plus the loaded `Config` and `Profile`s, and writes through on every mutation (except reordering, saved on launcher quit). `scribe.rs` does atomic writes (`.tmp` then rename, `.bak` of the previous version, unreadable files renamed `.corrupt` and never overwritten). `wad.rs`/`hash.rs` track wads by path plus content hash so moved/changed/volatile wads show as placeholders instead of breaking the whole profile.
- **`layout.rs`** sizes every window from its content and shrinks it to the terminal (minimum 40×20); long lists scroll. **`theme.rs`** holds colors.
- **`testing.rs`** (`#[cfg(test)]`) provides `draw(width, height, scene)` for rendering to text for snapshots and `example_vault` (the same three example profiles used in the README screenshots).

## Saved-file format rules

Everything in `config.rite` / `.profiles/*.rite` is MessagePack wrapped as `{version, data}`, via the `Edition` trait in `src/vault/edition.rs`. Adding a `#[serde(default)]` field is not breaking. Any change that alters what existing data means (rename, retype, unit or default change) is breaking and requires: an upgrade fn `vN_to_vM`, **appended** to the type's `Edition::UPGRADES` (never reorder, edit or remove entries; the version number is the list length), and a test that loads a file saved by the previous edition. Update `DATA_LAYOUT.md` when the file contents change.

The in-memory types and the saved shapes differ on purpose: `Wad` keeps a whole `OsString` path in memory, but saves through `vault/saved.rs` (`SavedPath` text, `SavedWad`), and `Profile` saves through `SavedProfile` (a dir table plus `(dir, name)` per wad) via `#[serde(try_from/into)]`. The saved enums are `#[serde(untagged)]` so every older shape still deserializes; that is why the edition upgrade functions can stay identity functions. Never serialize a bare `OsString`: serde tags it `Unix`/`Windows` and the other OS can't read it back. `Vault::lookup()` is an `Option` built lazily (only joining uses it) and is dropped by `insert`/`replace`/`remove`/`forget_lookup`.

## Working in this repo

- Ask before editing project files (standing preference of the repo owner).
- `secrets/` (the token, `commit.sh`, `push.sh`, `TODO.md`, test `wads/`) and `published/` are local/untracked; don't commit them, and never read `secrets/token.txt`.
