# tdl

A terminal launcher for [UZDoom](https://github.com/UZDoom/UZDoom). Group your wads into named profiles, launch them in one keypress, and host or join multiplayer games, with missing wads downloaded automatically from the host.

![The tdl splash screen: the title over the Doom fire](demo.gif)

## Features

- **Profiles**: an ordered list of wads with an optional per-profile UZDoom binary, base wad and `.ini`
- **One-key launch**: pick a profile and press `Enter`
- **Host**: choose player count, deathmatch or co-op, and a starting map
- **Join**: enter the host's IP; tdl picks a compatible profile, or offers to download the wads and create one
- **Import** a whole folder of wads into a profile
- **Per-profile saves** and a `session.log` of UZDoom's output
- **Status at a glance**: a panel under the launcher shows what the selected profile would run with (UZDoom, base wad, ini, wads) and whether anything is broken
- **Placeholders, not surprises**: a wad that moved is shown in the editor with what's wrong, and only that profile is held back from launching
- **Fits any terminal** from 40×20 up: every window is sized from what it holds, and long lists scroll

## Screenshots

The profiles here are examples made for the screenshots.

| | |
| --- | --- |
| ![The launcher, with badges on broken and volatile profiles and the status panel below](docs/screenshots/launcher.png) | ![The profile editor, showing a missing, a changed and a volatile wad](docs/screenshots/editor.png) |
| **Launcher.** Every profile on screen gets a badge: `!` broken, `~` changed, `*` volatile. The panel underneath shows what the selected one would launch with. | **Editor.** The same badges for each wad, drawn on top so the cursor never hides them. |
| ![The inspect popup for a missing wad](docs/screenshots/inspect.png) | ![The host form with players, mode and map](docs/screenshots/host.png) |
| **Inspecting a wad.** `Space` on a marked wad says what's wrong and how to fix it. | **Hosting.** Player count, mode and map; an empty map lets UZDoom pick. |

## Requirements

- A recent **nightly** Rust toolchain (the project uses unstable features)
- [UZDoom](https://github.com/UZDoom/UZDoom) installed
- A Doom base wad (e.g. `DOOM2.WAD`, or the free [Freedoom](https://freedoom.github.io/))

## Install

```sh
git clone https://codeberg.org/Error-ErrorFruitUser/tdl.git
cd tdl
cargo +nightly build --release
./target/release/tdl
```

## Usage

On first run, press `U` to select your UZDoom executable and `W` to select a base wad, then `A` to create a profile.

| Key | Action |
| --- | --- |
| `Enter` | Launch the selected profile (on `+`, create a profile) |
| `H` | Host a game |
| `N` | Join a game |
| `J`/`K`, `↓`/`↑` | Move selection |
| `}` / `{` | Move the profile down / up |
| `A` | Add a profile |
| `E` | Edit a profile |
| `D` | Delete a profile |
| `U` | Select the UZDoom executable |
| `W` | Select the base wad |
| `Esc` | Save and quit |
| `?` | Show the keys for the current screen |

Inside the profile editor, `Tab` switches between the name and the wad list, where `A` adds a wad, `F` imports a folder, `D` removes one, `}`/`{` reorder, and `U`/`W`/`I` set the profile's own UZDoom, base wad and ini.

### Multiplayer

The host serves its wad list on port **8080** while the game runs, so that port must be reachable by joiners. Joiners enter the host's IP; if no local profile matches, tdl offers to download the wads into a new profile.

### Data locations

Config and profiles live in your platform's config directory, saves, logs and downloads in its data directory:

| OS | Config | Data |
| --- | --- | --- |
| Linux | `~/.config/tdl` | `~/.local/share/tdl` |
| macOS | `~/Library/Application Support/error.error.fruit.eef.tdl` | same as config |
| Windows | `%APPDATA%\eef\tdl\config` | `%APPDATA%\eef\tdl\data` |

To use a different location, pass a vault folder:

```sh
tdl --vault /path/to/dir   # uses /path/to/dir/config and /path/to/dir/data
```

The fire splash plays until you press a key; `tdl --no-splash` goes straight to the launcher. The splash plays "Inevitable Doom"; `tdl --music` keeps it going in the launcher.

## Tuning the look

Everything visual lives in a few small files, so it can be changed without touching the screens:

| File | What's in it |
| --- | --- |
| `src/theme.rs` | The palette (blood, hellfire, bone, nukage…), what each part of the interface is drawn with, border styles, and the status glyphs |
| `src/layout.rs` | The smallest terminal size, margins, padding, how far the launcher and editor grow into free space (`GROW_PERCENT`, `MAX_GROWN_WIDTH`), and how windows are centered and stacked |
| `src/screens/splash.rs` | The startup logo, tagline, the "press any key" hint and how fast it pulses |
| `src/widgets/fire.rs` | The fire itself: its palette and how high the flames reach |
| `src/popups.rs` | How wide popup text gets before it wraps |

Setting `BACKGROUND` in `theme.rs` to `Color::Reset` keeps your terminal's own background.

## Recording the demo

```sh
cargo build --release
vhs demo.tape
```

The tape records ten seconds of the splash screen into `demo.gif`. It runs against an empty throwaway vault in `/tmp`, so your real profiles are never read or written.

## Credits

- Music: "Inevitable Doom" by celestialghost8 (CC0), played on the splash screen (and in the launcher with `--music`)

## License

[0BSD](LICENSE)
