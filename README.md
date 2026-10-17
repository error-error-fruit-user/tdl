# tdl

> **AI notice:** this project is developed with the help of AI. Much of the code and documentation was written with [Claude](https://claude.ai) (Anthropic), working under my direction and review. Commits that Claude helped write carry a `Co-Authored-By: Claude` trailer, so you can see which ones in the history.

A terminal launcher for [UZDoom](https://github.com/UZDoom/UZDoom). Group your wads into named profiles, launch them in one keypress, and host or join multiplayer games, with missing wads downloaded automatically from the host.

![The tdl splash screen: the title over the Doom fire](demo.gif)

## Features

- **Profiles**: an ordered list of wads with an optional per-profile UZDoom binary, base wad and `.ini`
- **One-key launch**: pick a profile and press `Enter`
- **Host**: choose player count, deathmatch or co-op, a starting map, the port and a join code
- **Join**: enter the host's IP and join code; tdl picks a compatible profile, or offers to download the wads and create one
- **Import** a whole dir of wads into a profile
- **Search** with `/`: profiles by name (or by their wads with `*`), and wads by file name in the editor
- **Copy and paste** profiles and wads, one or many at a time, with `Shift+V`/`M` to select, `Y` and `P`
- **Per-profile saves** and a `session.log` of UZDoom's output
- **Status at a glance**: a panel under the launcher shows what the selected profile would run with (UZDoom, base wad, ini, wads) and whether anything is broken, with the global settings (UZDoom, base wad, UZDoom arguments) beside it
- **Placeholders, not surprises**: a wad that moved is shown in the editor with what's wrong, and only that profile is held back from launching
- **Fits any terminal** from 40×20 up: every window is sized from what it holds, and long lists scroll

## Screenshots

The profiles here are examples made for the screenshots.

| | |
| --- | --- |
| ![The launcher, with badges on broken and volatile profiles and the status panel below](docs/screenshots/launcher.png) | ![The profile editor, showing a missing, a changed and a volatile wad](docs/screenshots/editor.png) |
| **Launcher.** Every profile on screen gets a badge: `!` broken, `~` changed, `*` volatile. The panel underneath shows what the selected one would launch with, and the one beside it the global settings it falls back on. | **Editor.** The same badges for each wad, drawn on top so the cursor never hides them, and under it what the profile overrides. |
| ![The inspect popup for a missing wad](docs/screenshots/inspect.png) | ![The host form with players, mode and map](docs/screenshots/host.png) |
| **Inspecting a wad.** `Space` on a marked wad says what's wrong and how to fix it. | **Hosting.** Player count, mode, map, port and join code; an empty map lets UZDoom pick. |
| ![The search line, looking for a wad by name with a star](docs/screenshots/search.png) | ![Two profiles selected, with one profile already copied](docs/screenshots/selection.png) |
| **Searching.** `/` and a name; a `*` in front looks in the wads instead. Nothing moves until `Enter`. | **Selecting and copying.** `Shift+V` and `M` pick rows; the row under the panel says what is selected and what is copied. |
| ![The settings: splash, music, quips, song, UZDoom arguments and splash quip](docs/screenshots/settings.png) | |
| **Settings.** `O` in the launcher: the splash and the UZDoom arguments for this vault, the music, song and quips for all of them. | |

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

### Command line

```sh
tdl [--vault <dir> | --vault-once <dir> | --forget-vault] [--wads <dir>]... [--unseal <scroll>] [--no-splash] [--splash-quip <quip> | --no-quips] [[--music <song>] [--keep-music] | --no-music] [--uzdoom <file>] [--base-wad <file>]
tdl launch|host|join ...                  # see Commands below
tdl --seal <scroll> [--with-settings] [--delete [--force]]
tdl --list-quips
```

`tdl --help` (or `tdl host --help`, and so on) lists everything, and `tdl --version` says which tdl it is. A mistyped option is answered with the closest real one.

- `--vault <dir>` keeps everything in `<dir>/config` and `<dir>/data` instead of the standard dirs. tdl asks whether to open that vault every time it starts.
- `--vault-once <dir>` opens a vault the same way, just for this run, without asking.
- `--unseal <scroll>` imports a shared `.scroll` as soon as tdl starts, just like `I` in the launcher, looking for its wads in the wad dirs: `tdl --unseal shared.scroll --wads ~/wads`.
- `--splash-quip <quip>` shows your own quip under the splash logo instead of a random one, just for this run: `tdl --splash-quip "Rip and tear, bestie!"`. Every `#<number>` in it is replaced by that built-in quip, counting from 1 (`tdl --list-quips` prints them all with their numbers): `#12` is quip 12 alone, and `#12, hun!` is quip 12 with `, hun!` right after it, no space added. Write `##` for a plain `#` in front of digits. It plays the splash even if the **splash** setting is off, and can't go with `--no-splash`. The progress windows keep their usual quips. `--no-quips` leaves the quips out of every window for this run, the same as the **quips** setting.
- `--music <song>` plays your own song (wav, mp3, flac or ogg) instead of "Inevitable Doom". It is checked before tdl starts, so a file it can't play is reported right away. `--keep-music` keeps the music going in the launcher after the splash, as the **music** setting does. To use your song every time, pick it in the **song** setting. `--no-music` plays no music at all for this run, not even on the splash, and can't go with the other two.
- `--uzdoom <file>` and `--base-wad <file>` play with that UZDoom or base wad for this run only, over every profile's own and the global one, for trying another UZDoom build or IWAD quickly. They are never saved and never asked about; the launcher's panels mark them **session**. A file that isn't there stops tdl before it starts.
- `--seal <scroll>` seals every profile in the vault into a new `.scroll`, with no windows, and closes: `tdl --seal backup.scroll`. It never writes over a file that is already there. `--with-settings` seals your global settings in too (never your song); whoever unseals it gets them in place of their own. With `--delete` it then deletes the vault: every profile with its saved games and logs. The downloaded wads stay, so nothing has to be downloaded again, and so do your global settings and anything in its dirs that tdl didn't make. It asks `[y/N]` first, showing what goes; `--force` skips the question. If it was the vault tdl remembered from `--vault`, tdl forgets it.
- `--with-pride` plays the pride splash for the run: a fire that burns through every color of the rainbow, under the logo in the trans flag's blue, pink and white.
- `--forget-vault` opens the standard vault, and asks whether to stop opening the one remembered from `--vault`.
- `--wads <dir>` (as many times as you like) says where your wads are: file pickers open there instead of the current directory, and joining looks there for wads you already have. tdl asks whether to keep the dirs for this **[S]ession** only, **[G]lobal**ly for every vault, or for this **[V]ault**. Kept dirs are used whenever tdl starts without `--wads`: the vault's own first, then the global ones.

#### Commands

`launch`, `host` and `join` do one thing straight away, without the launcher, and tdl closes when they are done (the game ends, or something stops it). tdl's own options can go before or after the command, and each command's own flags go after it (`tdl host brutal_doom --coop`). `Ctrl+C` stops the game (and the wad server, when hosting) and closes tdl with exit code 130, as an interrupted program does.

```sh
tdl launch brutal_doom                                # play a profile
tdl host brutal_doom --players 4 --coop --map MAP07  # the host form, filled in from these
tdl join 10.0.0.2 --code abc234 --port 9000           # the join form, filled in from these
```

- `host` opens the host form for the profile, filled in from its saved host defaults and changed by `--players` (2–8), `--coop` or `--deathmatch`, `--map`, `--port` and `--code`; `Enter` starts the game.
- `join` opens the join form, filled in with the IP, `--code` and `--port` (8080 if not given), then finds or downloads a profile. With `--always-download` the wads you don't have are downloaded without asking first.
- `--headless`, with `host` or `join`, shows no forms and no windows at all: just plain lines in the terminal, so `join` needs `--code`. Joining still asks `[Y/N]` before downloading wads that aren't installed (no answer, or no terminal to ask on, is no), unless `--always-download` is given; a profile made for it is named after the host (`joined_10_0_0_2`). Anything that stops the game ends tdl with an error code, and `Ctrl+C` stops a game or a download cleanly, leaving any finished downloads for next time.

| Key | Action |
| --- | --- |
| `Enter` | Launch the selected profile (on `+`, create a profile) |
| `H` | Host a game |
| `N` | Join a game |
| `J`/`K`, `↓`/`↑` | Move the cursor |
| `/` | Search profiles by name, or by their wads' file names with `*` in front (`*brutal`). Nothing moves while you type; `Enter` goes to the best match and `Esc` cancels |
| `;` / `,` | Go to the next / previous match of the last search, best first |
| `}` / `{` | Move the profile down / up |
| `Shift+V` | Start selecting a range of profiles (`J`/`K` stretch it), or stop |
| `M` | Mark or unmark the profile, to select profiles that aren't next to each other |
| `Y` / `Shift+Y` | Copy the selected profiles (or the one under the cursor) / every profile |
| `P` | Paste: copied profiles go in front of the cursor, and copied wads go at the end of the profile (asks first) or, on `+`, start a new profile |
| `A` | Add a profile |
| `E` | Edit a profile |
| `D` | Delete a profile |
| `X` | Export the profile (or the selected ones, or the whole vault) as a `.scroll` to share |
| `I` | Import a `.scroll`, adding its profiles |
| `U` | Select the UZDoom executable |
| `W` | Select the base wad |
| `O` | Settings: the splash screen, the music, the song, the quips, and extra UZDoom arguments |
| `Esc` | Save and quit |
| `F1` / `?` | Show the keys for the current screen (`F1` works everywhere, even where `?` is typed as text) |

Inside the profile editor, `Enter` saves and leaves; `Esc` leaves too, but if the profile changed it first asks whether to **[S]ave**, **[D]iscard**, or go **[B]ack**. `Tab` switches between the name and the wad list, where `A` adds a wad, `F` imports a dir, `D` removes one, `}`/`{` reorder, `U`/`W`/`I` set the profile's own UZDoom, base wad and ini, and `G` its own UZDoom arguments (used instead of the global ones from the settings; left empty, the global ones count). `/` searches the wads by file name (never by dir) and `;`/`,` step through the matches. `Shift+V` and `M` select wads, `Y` copies the selected ones (or the one under the cursor), `Shift+Y` copies every wad, `X` cuts (copies, then takes them out), and `P` pastes in front of the cursor; copied profiles paste as all their wads.

### Arrow keys

Every place text is typed (profile names, the search line, the host and join forms, the settings, naming a copy) has a cursor: `←`/`→` move it, `Home`/`End` jump to either end, `Backspace` deletes before it and `Delete` under it. In forms `↑`/`↓` move between fields as well as `Tab`/`Backtab`, and `←`/`→` change the field's choice instead (the host's mode, its player count, a setting). In the editor `↓` goes from the name into the wads and `↑` from the first wad back to the name. `Home`/`End` jump to the top and the bottom of a list. On the search line `↑`/`↓` bring back earlier searches from this session. A legend too tall for the terminal is split into columns, or, if those don't fit either, shown a page at a time (`J`/`K` turn the pages). Dialogs can be answered with `←`/`→` and `Enter` as well as with their letters; nothing is picked until an arrow is pressed, so `Enter` alone never answers a question.

### Searching, selecting, copying

A search ranks the matches: the exact name first (a wad's extension can be left off), then names that start with what you typed, then names with a word that does (after `_`, `-`, `.` or a space), then names that only contain it. Case doesn't matter, and ties go to the first one after the cursor.

`Shift+V` starts a range at the cursor, which grows as the cursor moves; `M` marks single rows. Both can be used together, and whatever is selected is copied in list order, so the load order stays as it was. With rows selected, `}`/`{` move all of them together and `D` deletes all of them, after asking once. `Esc` lets go of a selection before it does anything else. The row under the window says how many rows are selected and what is copied. Copies last until tdl closes, so they can be pasted into any profile.

A pasted profile is a copy of the original: its wads, overrides and host defaults, and its own ini copied alongside, but not its saves. If its name is taken, tdl asks whether to **[T]ag** it with the next free number (`brutal_2`), **[N]ame** it yourself, or **[S]kip** it.

Pasting a wad whose file name the profile already has asks the same three things, but there the paste is a real copy of the file, made next to the original: **[T]ag** copies `brutal.pk3` to `brutal_2.pk3`, **[N]ame** lets you type the name (the extension stays), and **[S]kip** leaves it out. A copy never writes over a file that is already there, and a big pk3 takes as much disk space again.

### Settings

`O` in the launcher opens the settings. The splash and the UZDoom arguments are kept for the vault that is open; the music, the song and the quips are kept in `global.rite`, for every vault. `↑`/`↓` (or `Tab`) move between the rows, `J`/`K` or `←`/`→` switch a choice, `Enter` saves. `Esc` leaves, and if anything changed it first asks whether to **[S]ave** it, **[D]iscard** it, or go **[B]ack** to the form.

- **splash**: whether the fire splash plays when tdl starts.
- **music**: **off** (no music at all, as `--no-music` does for one run), **splash only** (it fades out with the splash), or **keeps playing** in the launcher (as `--keep-music` does for one run). `J`/`→` goes to the next choice and `K`/`←` back.
- **uzdoom args**: extra arguments passed to UZDoom on every launch, host and join, before the wads, such as `-nomusic +vid_fps 1`. They are split at spaces; quotes aren't understood.
- **song**: a song of your own (wav, mp3, flac or ogg) to play instead of "Inevitable Doom", for every vault. `Enter` opens a file picker, and `Backspace` goes back to the built-in track. A new song plays straight away if the music is playing. `--music` still wins at startup, and a song that has gone missing is mentioned at startup and the built-in track plays instead.
- **quips**: **hidden** leaves the quips out of every window, the splash screen and the waiting windows alike, for every vault. `--splash-quip` still shows its quip on the splash.
- **splash quip**: the quip under the splash logo every time tdl starts, for every vault. Leave it empty for a random one, or type it the same way as `--splash-quip`: `#12` for built-in quip 12, your own words, or both (`#12, hun!`). `--splash-quip` still wins for the run it's given. In this field `?` is typed like any letter, so only `F1` shows its keys.

### Sharing profiles

`X` seals profiles into a `.scroll` file: it asks whether to seal the profile under the cursor (or the ones selected with `Shift+V`/`M`) or the whole vault. For each profile it keeps each wad's file name, size and hash, the base wad, and the host defaults (without the join code), but no paths, so it means the same thing on any computer. Send it to someone with the same wads and they press `I` to break the seal: tdl looks for each wad by name in their wad dirs (from `--wads`, or kept from it; with none, it asks for a dir), takes a file only if its size and then its hash match, and adds the profiles it finds. A scroll of one profile asks for its name; with several, each keeps its name, and one that is taken asks to **[T]ag** it (`brutal_2`), **[N]ame** it, or **[S]kip** it. Wads it can't find are left as placeholders in the editor, where `Space` says where they were looked for and `R` picks them.

Sealing also asks whether to seal in your global settings: the music, whether quips are shown, and the splash quip. Say no and the scroll carries none, so your friends' settings are left alone. Your song never goes in, nor anything about where things are on your computer. When a scroll carries settings, breaking its seal puts them in place of yours, once the profiles are in; a scroll without any leaves yours as they are.

### Multiplayer

The host serves its wad list on port **8080** (or the port set in the host form, which is remembered) while the game runs, so that port must be reachable by joiners. Every game gets a **join code**: type your own in the host form as a password, or leave it empty for a random one. tdl shows the port and code before the game starts. Without the code, nobody can fetch the wad list or the wads. The host form's **save as default** button stores the current settings for that profile, so its next game starts from them, and **clear default** removes them. Both ask first.

Joiners enter the host's IP, the port (empty means 8080) and the code. If no local profile matches, tdl looks for the host's wads already on this computer (in other profiles, in earlier downloads, and by name in any dir your profiles use) and offers to make a new profile from them, downloading only the ones that are missing. A file that only might be the right one is hashed first, in a green **hashing** window. The download window shows the size and speed, and `Esc` cancels it. Finished wads are kept, so trying again picks up where it stopped.

The code only guards tdl's wad server. UZDoom's own game port is still open to anyone who knows the IP.

### Data locations

Config and profiles live in your platform's config directory, saves, logs and downloads in its data directory:

| OS | Config | Data |
| --- | --- | --- |
| Linux | `~/.config/tdl` | `~/.local/share/tdl` |
| macOS | `~/Library/Application Support/error.error.fruit.eef.tdl` | same as config |
| Windows | `%APPDATA%\eef\tdl\config` | `%APPDATA%\eef\tdl\data` |

To use a different location, pass a vault dir:

```sh
tdl --vault /path/to/dir   # uses /path/to/dir/config and /path/to/dir/data
```

The fire splash plays until you press a key; `tdl --no-splash` goes straight to the launcher, and the **splash** setting (`O` in the launcher) turns it off (or back on) for good. The screen and music fade in, and fade out again when you press a key. The splash plays "Inevitable Doom", or your own song with `--music <song>`; `--keep-music`, or the **music** setting, keeps it going in the launcher.

## Tuning the look

Everything visual lives in a few small files, so it can be changed without touching the screens:

| File | What's in it |
| --- | --- |
| `src/theme.rs` | The palette (blood, hellfire, bone, nukage…), what each part of the interface is drawn with, border styles, the status glyphs, and the trans flag's colors for the pride splash |
| `src/layout.rs` | The smallest terminal size, margins, padding, how far the launcher and editor grow into free space (`GROW_PERCENT`, `MAX_GROWN_WIDTH`), and how windows are centered and stacked |
| `src/screens/splash.rs` | The startup logo (and its trans flag stripes), tagline, the "press any key" hint and how fast it pulses, and how fast the pride fire goes round the rainbow |
| `src/widgets/fire.rs` | The fire itself: its palette, its rainbow, and how high the flames reach |
| `src/popups.rs` | How wide popup text gets before it wraps |

Setting `BACKGROUND` in `theme.rs` to `Color::Reset` keeps your terminal's own background.

## Recording the demo and the screenshots

```sh
./demo-screenshots.sh
```

It builds the release binary, records ten seconds of the splash screen into `demo.gif` (`demo.tape`), then lays out the same example profiles the tests use and saves each screen the README shows to `docs/screenshots/` (`screenshots.tape`). It needs `vhs`, `ttyd` and `ffmpeg`. Both tapes run tdl on a throwaway vault with its home in `/tmp`, so your profiles and settings are never read or written, and those dirs are removed afterwards.

## Credits

- Music: "Inevitable Doom" by celestialghost8 (CC0), played on the splash screen (and in the launcher with `--keep-music` or the **music** setting)

## License

[0BSD](LICENSE)
