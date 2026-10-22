# tdl — agent context

## Start here

- Read `CLAUDE.md` for the detailed architecture and persistence rules; its project guidance also applies to OpenCode.
- Rust/ratatui terminal launcher for UZDoom: ordered profiles, launching, host/join and HTTP mod sharing. This is a separate project, not part of the ZScript library.
- `Cargo.toml` is authoritative for toolchain requirements: it currently declares edition 2024 and Rust 1.99.1, despite older minimum-version wording in `CLAUDE.md`.
- User docs: `README.md`; persistence: `DATA_LAYOUT.md`; theme/splash references: `THEME.md` and `SPLASH.md`.

## Architecture map

- `src/ritual.rs`: screen/popup framework and event loop. Preserve the deliberately themed vocabulary and aliases rather than normalizing them.
- `src/screens/`, `src/popups/`, `src/widgets/`: full screens, modal rituals and reusable UI. Shared text editing is `widgets/text.rs`; search/selection helpers are under `screens/`.
- `src/loadout/`: profiles, wad paths/hashes, settings and editioned persistence. Settings live in `uac.rite`; loadout configuration in `config.rite`.
- `src/slayer/`: launch/host/join/share flows; `src/net/`: manifest, serving and downloading; `src/uzdoom.rs`: engine command construction and process handling.
- `src/theme/`: editioned TOML dressings and bundled themes. Keep Rust defaults, bundled assets and documented examples consistent.
- `pack_pk3.py`: reproducible mod-folder packing, embedded by `src/loadout/pack.rs` for hosting. `uzdoom.sh` wraps Flatpak and grants folder access.

## Build and verify

- Build: `cargo build --release`.
- Stable tests: `cargo test`; nightly tests: `cargo +nightly test --features spooky`. Keep both supported. New nightly-only syntax belongs inside `after_dark!` as described in `CLAUDE.md`.
- UI snapshots: review with `cargo insta review`; do not hand-edit snapshots.
- Publishing: `python3 publish.py` cross-builds Windows and collects third-party licenses; see `CLAUDE.md` for prerequisites.

## Data safety and working rules

- Saved MessagePack is wrapped as `{version, data}`. Append migration functions for breaking changes; never alter old migration entries. Test loading the previous edition and update `DATA_LAYOUT.md` when saved contents change.
- Never serialize bare `OsString`; use the portable saved representations. Preserve atomic writes, backups and corrupt-file handling.
- Theme/splash/pride TOML migrations are append-only too. Screens request theme roles, not hard-coded colors.
- Ask before editing project files; an explicit request authorizes only its scope. Preserve the README AI notice.
- Version control is jj colocated with git. For ChatGPT via OpenCode work, use `secrets/commit.sh "message" --opencode` when committing is requested. `--claude` remains available separately.
- Never read tokens or commit `secrets/` or `published/`.
- Current project tasks: `secrets/TODO.md`; workspace integration tasks: `../TODO.md`.
