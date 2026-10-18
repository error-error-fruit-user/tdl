# tdl

> **AI notice:** this project is developed with the help of AI. Much of the code and documentation was written with [Claude](https://claude.ai) (Anthropic), working under my direction and review. Commits that Claude helped write carry a `Co-Authored-By: Claude` trailer, so you can see which ones in the history.

A terminal launcher for [UZDoom](https://github.com/UZDoom/UZDoom). Group your wads into named profiles, launch them in one keypress, and host or join multiplayer games, with missing wads downloaded automatically from the host.

![The tdl splash screen: the title over the Doom fire, with the quip "Hell on Earth, heaven in a terminal!"](demo.gif)

## Features

- **Profiles**: an ordered list of wads with an optional per-profile UZDoom binary, base wad and `.ini`
- **One-key launch**: pick a profile and press `Enter`
- **Host**: choose player count, deathmatch or co-op, a starting map, the port and a join code
- **Join**: enter the host's IP and join code; tdl picks a compatible profile, or offers to download the wads and create one
- **Import** a whole dir of wads into a profile, or make a profile that **watches a dir** and takes in whatever wads you put there
- **Search** with `/`: profiles by name (or by their wads with `*`), and wads by file name in the editor
- **Copy, cut and paste** profiles and wads, one or many at a time, with `Shift+V`/`M` to select, `Y`, `X` and `P`
- **Wads screen** (`L`): every wad in every wad dir and the downloads, by dir or all in one list, to copy or move between dirs or paste into a profile
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
| ![The wads screen: two wad dirs and the downloads, each with its wads and their sizes](docs/screenshots/wads.png) | ![Every wad in one list, the two brutalv22.pk3 told apart by their dirs](docs/screenshots/every-wad.png) |
| **Wads screen.** `L`: every wad dir and the downloads, each with every wad in it, to copy or move between dirs or paste into a profile. | **Every wad.** `Tab` on the wads screen: one list by name, where wads with the same name are told apart by their dirs. |
| ![The settings: splash, pride, music, quips, song, UZDoom arguments and splash quip](docs/screenshots/settings.png) | |
| **Settings.** `O` in the launcher: the splash and the pride splash, the music, quips and song, the UZDoom arguments and the splash quip, the same in every vault. | |

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
tdl [--vault <dir> | --vault-once <dir> | --forget-vault | --forget-vault-once] [--wads <dir>... | --wads-once <dir>... | --forget-wads | --forget-wads-once] [--unseal <scroll> | --unseal-delete <scroll> | --unseal-delete-force <scroll>] [--no-splash] [--splash-quip <quip> | --no-quips] [[--music <song>] [--keep-music] | --no-music] [--uzdoom <file>] [--base-wad <file>] [--with-pride]
tdl launch|host|join ...                  # see Commands below
tdl --seal <scroll> [--with-settings]
tdl --list-quips
```

`tdl --help` (or `tdl host --help`, and so on) lists everything, and `tdl --version` says which tdl it is. A mistyped option is answered with the closest real one.

- `--vault <dir>` keeps everything in `<dir>/config` and `<dir>/data` instead of the standard dirs. tdl asks whether to open that vault every time it starts.
- `--vault-once <dir>` opens a vault the same way, just for this run, without asking.
- `--unseal <scroll>` imports a shared `.scroll` as soon as tdl starts, just like `I` in the launcher, looking for its wads in the wad dirs: `tdl --unseal shared.scroll --wads ~/wads`.
- `--unseal-delete <scroll>` unseals a scroll in place of the vault's profiles: every profile is deleted first, with its saved games and logs, and the scroll's go in. The downloaded wads stay, so nothing has to be downloaded again, and so do your global settings. It asks first; `--unseal-delete-force <scroll>` doesn't. Restoring a backup made with `--seal` is `tdl --unseal-delete-force backup.scroll`.
- `--splash-quip <quip>` shows your own quip under the splash logo instead of a random one, just for this run: `tdl --splash-quip "Rip and tear, bestie!"`. Every `#<number>` in it is replaced by that built-in quip, counting from 1 (`tdl --list-quips` prints them all with their numbers): `#12` is quip 12 alone, and `#12, hun!` is quip 12 with `, hun!` right after it, no space added. Write `##` for a plain `#` in front of digits. It plays the splash even if the **splash** setting is off, and can't go with `--no-splash`. The progress windows keep their usual quips. `--no-quips` leaves the quips out of every window for this run, the same as the **quips** setting.
- `--music <song>` plays your own song (wav, mp3, flac or ogg) instead of "Inevitable Doom". It is checked before tdl starts, so a file it can't play is reported right away. `--keep-music` keeps the music going in the launcher after the splash, as the **music** setting does. To use your song every time, pick it in the **song** setting. `--no-music` plays no music at all for this run, not even on the splash, and can't go with the other two.
- `--uzdoom <file>` and `--base-wad <file>` play with that UZDoom or base wad for this run only, over every profile's own and the global one, for trying another UZDoom build or IWAD quickly. They are never saved and never asked about; the launcher's panels mark them **session**. A file that isn't there stops tdl before it starts.
- `--seal <scroll>` seals every profile in the vault into a new `.scroll`, with no windows, and closes: `tdl --seal backup.scroll`. It never writes over a file that is already there. `--with-settings` seals your global settings in too (never your song); whoever unseals it gets them in place of their own.
- `--with-pride` plays the pride splash for the run (the **pride** setting plays it every time): a fire that burns through every color of the rainbow, under the logo in the whole trans flag, a stripe a row: blue, pink, white, pink, blue. Without it the logo cools a step a row, from white-hot flame at the top to blood at the bottom.
- `--forget-vault` opens the standard vault, and asks whether to stop opening the one remembered from `--vault`. `--forget-vault-once` opens the standard vault just for this run, and asks nothing.
- `--wads <dir>` (as many times as you like) says where your wads are: file pickers open there instead of the current directory, and joining looks there for wads you already have. tdl asks whether to keep the dirs for this **[S]ession** only, **[G]lobal**ly for every vault, or for this **[V]ault**. Kept dirs are used whenever tdl starts without `--wads`: the vault's own first, then the global ones. `--wads-once <dir>` uses the dirs the same way, just for this run, without asking.
- `--forget-wads` looks for wads only in the downloads this run, and asks whether to forget the wad dirs kept **[G]lobal**ly, for this **[V]ault**, or **[B]oth**. `--forget-wads-once` leaves them out just for this run, and asks nothing.

#### Commands

`launch`, `host` and `join` do one thing straight away, without the launcher, and tdl closes when they are done (the game ends, or something stops it). tdl's own options can go before or after the command, and each command's own flags go after it (`tdl host brutal_doom --coop`). `Ctrl+C` stops the game (and the wad server, when hosting) and closes tdl with exit code 130, as an interrupted program does.

```sh
tdl launch brutal_doom                                # play a profile
tdl host brutal_doom --players 4 --coop --map MAP07  # the host form, filled in from these
tdl join 10.0.0.2 --code abc234 --port 9000           # the join form, filled in from these
```

- `host` opens the host form for the profile, filled in from its saved host defaults and changed by `--players` (2–8), `--coop` or `--deathmatch`, `--map`, `--port` and `--code`; `Enter` starts the game.
- `join` opens the join form, filled in with the IP, `--code` and `--port` (8080 if not given), then finds or downloads a profile. With `--always-download` the wads you don't have are downloaded without asking first.
- `--headless`, with `launch`, `host` or `join`, shows no forms and no windows at all: just plain lines in the terminal, so `join` needs `--code`. Joining still asks `[Y/N]` before downloading wads that aren't installed (no answer, or no terminal to ask on, is no), unless `--always-download` is given, and asks `[r/n/S]` about a wad of yours that isn't the host's version (see Multiplayer; no answer is skip, and `--always-download` replaces it without asking); a profile made for it is named after the host (`joined_10_0_0_2`). Anything that stops the game ends tdl with an error code, and `Ctrl+C` stops a game or a download cleanly, leaving any finished downloads for next time.

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
| `X` | Cut the selected profiles (or the one under the cursor): nothing moves until `P` pastes them, which moves them in front of the cursor, saves and all |
| `P` | Paste: copied profiles go in front of the cursor, and copied wads go at the end of the profile (asks first) or, on `+`, start a new profile |
| `A` | Add a profile |
| `F` | Add a profile that watches a dir (see below) |
| `E` | Edit a profile |
| `D` | Delete a profile |
| `S` | Seal the profile (or the selected ones, or the whole vault) into a `.scroll` to share |
| `I` | Import a `.scroll`, adding its profiles |
| `U` | Select the UZDoom executable |
| `W` | Select the base wad |
| `L` | The wads screen: every wad dir and its wads (see below) |
| `O` | Settings: the splash screen, the music, the song, the quips, and extra UZDoom arguments |
| `Esc` | Save and quit |
| `F1` / `?` | Show the keys for the current screen (`F1` works everywhere, even where `?` is typed as text) |

Inside the profile editor, `Enter` saves and leaves; `Esc` leaves too, but if the profile changed it first asks whether to **[S]ave**, **[D]iscard**, or go **[B]ack**. `Tab` switches between the name and the wad list, where `A` adds a wad, `F` imports a dir, `D` removes one, `}`/`{` reorder, `U`/`W`/`I` set the profile's own UZDoom, base wad and ini, and `G` its own UZDoom arguments (used instead of the global ones from the settings; left empty, the global ones count). `/` searches the wads by file name (never by dir) and `;`/`,` step through the matches. `Shift+V` and `M` select wads, `Y` copies the selected ones (or the one under the cursor), `Shift+Y` copies every wad, `X` cuts (copies, then takes them out), and `P` pastes in front of the cursor; copied profiles paste as all their wads. `L` opens the wads screen, and `T` makes the profile watch a dir, or stop watching one.

### Profiles that watch a dir

`F` in the launcher picks a dir and makes a profile named after it, with every wad in it (at any depth). From then on the profile keeps up with the dir by itself: whenever tdl starts, before the profile is launched or hosted, and when it is opened in the editor, wads put in the dir are added at the end (in path order) and wads whose files have left it are taken out. The panel under the launcher shows the dir it watches. Otherwise it is a profile like any other: wads can be reordered, and wads from anywhere else added. A wad from the dir taken out by hand stays out until it is put back. `T` in the editor gives any profile a dir to watch, or stops watching one (its wads stay). A watched dir that isn't there, like an unplugged drive, changes nothing, and a sealed scroll holds the profile's wads as they are, as an ordinary profile.

### Volatile wads

A wad you are still working on changes all the time, so the hash and size tdl stored for it go stale. `V` on it in the editor's wad list marks it **volatile** (and `V` again unmarks it). It gets a cyan `*`, and so does its profile in the launcher. A volatile wad is still a wad like any other, with a few differences:

- It is never marked `~` for changing size; only a missing or unreadable file counts as broken.
- `R` picks its file again without asking about a different hash, and it stays volatile.
- When you host, it is hashed again right before the wad list is served, so joiners get the version that is on your disk now, not the one you added.
- Copies, pasted profiles and sealed scrolls keep it volatile.

When you join, a volatile wad stands in for the host's wad of the same file name, whatever its contents, so a profile that only differs from the host's in volatile wads still counts as matching. tdl hashes each of those wads again first, since the stored hash is often only out of date. If the file turns out to be the host's version after all, the new hash is just saved. If it really differs, the host's version is downloaded and put in its place **without asking**: your old file is kept beside it as `<file>.bak` (writing over any older `.bak`), and every profile using it is updated with the new hash and stays volatile. The same goes for any file with the name of one of the host's wads that a profile of yours has as volatile, even when no profile matches. If the download fails, tdl says so and asks whether to join anyway with your own versions. Headless joins do all the same, saying each wad they replace, and join with your own versions if a download fails.

That keeps everyone in step with a mod that is still in progress: whoever hosts serves the latest build, and the others get it as they join. It also means a volatile wad of yours is never safe from an older version on the host's side, so if you are the one working on it, host the game yourself, or copy your work somewhere else before joining (only one `.bak` is kept). A wad that isn't volatile is never replaced without asking (see Multiplayer).

### The wads screen

`L`, in the launcher or the editor's wad list, shows every dir wads are looked for in (the ones from `--wads`, or kept from it, then `.downloads`), each with every wad under it at any depth. `Enter` opens or closes a dir, and `Tab` switches to every wad in one list by name, where wads with the same name are told apart by the fewest dirs that differ (`old`, `wads/new`, or `.downloads` and the start of its hash).

`Y` copies the selected wads (or the one under the cursor, or on a dir's row every wad in it) and `X` cuts them; nothing moves until `P` pastes. Pasted into a dir (the one the wad under the cursor is in, or the dir itself on its row), copied files are copied there and cut ones moved, once; a name that is taken there asks whether to **[R]eplace** it, **[K]eep Both** (`brutal_2.pk3`), or **[S]kip** it. Into `.downloads`, each is hashed and goes under its hash. Every profile using a moved wad follows it, and so does the global base wad. Wads copied here paste into a profile like any other (they are hashed as they go in), and wads or profiles copied in the launcher or the editor paste into a dir here as copies of their files.

`A` (or `Enter` on the last row) adds a dir to look in, asking whether to keep it for this **[S]ession**, **[G]lobal**ly or for this **[V]ault**, as `--wads` does. `D` on a dir's row dismisses it: tdl stops looking in it, everywhere it was kept, and its files stay. On a wad, `D` asks whether to **[E]rase** it from the disk (with wads selected, all of them; a profile using an erased wad shows it missing, and one watching its dir lets it go) or **[D]ismiss** the dir it is in. `.downloads` is always there. `/` searches the wads shown by file name.

### Arrow keys

Every place text is typed (profile names, the search line, the host and join forms, the settings, naming a copy) has a cursor: `←`/`→` move it, `Home`/`End` jump to either end, `Backspace` deletes before it and `Delete` under it. In forms `↑`/`↓` move between fields as well as `Tab`/`Backtab`, and `←`/`→` change the field's choice instead (the host's mode, its player count, a setting). In the editor `↓` goes from the name into the wads and `↑` from the first wad back to the name. `Home`/`End` jump to the top and the bottom of a list. On the search line `↑`/`↓` bring back earlier searches from this session. A legend too tall for the terminal is split into columns, or, if those don't fit either, shown a page at a time (`J`/`K` turn the pages). Dialogs can be answered with `←`/`→` and `Enter` as well as with their letters; nothing is picked until an arrow is pressed, so `Enter` alone never answers a question.

### Searching, selecting, copying

A search ranks the matches: the exact name first (a wad's extension can be left off), then names that start with what you typed, then names with a word that does (after `_`, `-`, `.` or a space), then names that only contain it. Case doesn't matter, and ties go to the first one after the cursor.

`Shift+V` starts a range at the cursor, which grows as the cursor moves; `M` marks single rows. Both can be used together, and whatever is selected is copied in list order, so the load order stays as it was. With rows selected, `}`/`{` move all of them together and `D` deletes all of them, after asking once. `Esc` lets go of a selection before it does anything else. The row under the window says how many rows are selected and what is copied. Copies last until tdl closes, so they can be pasted into any profile.

A pasted profile is a copy of the original: its wads, overrides and host defaults, and its own ini copied alongside, but not its saves. If its name is taken, tdl asks whether to **[T]ag** it with the next free number (`brutal_2`), **[N]ame** it yourself, or **[S]kip** it.

Pasting a wad whose file name the profile already has asks the same three things, but there the paste is a real copy of the file, made next to the original: **[T]ag** copies `brutal.pk3` to `brutal_2.pk3`, **[N]ame** lets you type the name (the extension stays), and **[S]kip** leaves it out. A copy never writes over a file that is already there, and a big pk3 takes as much disk space again.

### Settings

`O` in the launcher opens the settings. Everything here, like UZDoom and the base wad (`U` and `W`), is kept in `global.rite`, so it is the same whichever vault is open; a vault only keeps its profiles and its own wad dirs. `↑`/`↓` (or `Tab`) move between the rows, `J`/`K` or `←`/`→` switch a choice, `Enter` saves. `Esc` leaves, and if anything changed it first asks whether to **[S]ave** it, **[D]iscard** it, or go **[B]ack** to the form.

- **splash**: whether the fire splash plays when tdl starts.
- **pride**: **on** plays the pride splash every time, as `--with-pride` does for one run, from the next start on.
- **music**: **off** (no music at all, as `--no-music` does for one run), **splash only** (it fades out with the splash), or **keeps playing** in the launcher (as `--keep-music` does for one run). `J`/`→` goes to the next choice and `K`/`←` back.
- **quips**: **hidden** leaves the quips out of every window, the splash screen and the waiting windows alike, for every vault. `--splash-quip` still shows its quip on the splash.
- **song**: a song of your own (wav, mp3, flac or ogg) to play instead of "Inevitable Doom", for every vault. `Enter` opens a file picker, and `Backspace` goes back to the built-in track. A new song plays straight away if the music is playing. `--music` still wins at startup, and a song that has gone missing is mentioned at startup and the built-in track plays instead.
- **uzdoom args**: extra arguments passed to UZDoom on every launch, host and join, before the wads, such as `-nomusic +vid_fps 1`. They are split at spaces; quotes aren't understood.
- **splash quip**: the quip under the splash logo every time tdl starts, for every vault. Leave it empty for a random one, or type it the same way as `--splash-quip`: `#12` for built-in quip 12, your own words, or both (`#12, hun!`). `--splash-quip` still wins for the run it's given. In this field `?` is typed like any letter, so only `F1` shows its keys.

### Sharing profiles

`S` seals profiles into a `.scroll` file: it asks whether to seal the profile under the cursor (or the ones selected with `Shift+V`/`M`) or the whole vault. For each profile it keeps each wad's file name, size and hash, the base wad, and the host defaults (without the join code), but no paths, so it means the same thing on any computer. Send it to someone with the same wads and they press `I` to break the seal: tdl looks for each wad by name in their wad dirs (from `--wads`, or kept from it; with none, it asks for a dir), takes a file only if its size and then its hash match, and adds the profiles it finds. A scroll of one profile asks for its name; with several, each keeps its name, and one that is taken asks to **[T]ag** it (`brutal_2`), **[N]ame** it, or **[S]kip** it. Wads it can't find are left as placeholders in the editor, where `Space` says where they were looked for and `R` picks them.

Sealing also asks whether to seal in your global settings: the music, whether quips are shown, and the splash quip. Say no and the scroll carries none, so your friends' settings are left alone. Your song never goes in, nor anything about where things are on your computer. When a scroll carries settings, breaking its seal puts them in place of yours, once the profiles are in; a scroll without any leaves yours as they are.

### Multiplayer

The host serves its wad list on port **8080** (or the port set in the host form, which is remembered) while the game runs, so that port must be reachable by joiners. Every game gets a **join code**: type your own in the host form as a password, or leave it empty for a random one. tdl shows the port and code before the game starts. Without the code, nobody can fetch the wad list or the wads. The host form's **save as default** button stores the current settings for that profile, so its next game starts from them, and **clear default** removes them. Both ask first.

Joiners enter the host's IP, the port (empty means 8080) and the code. If no local profile matches, tdl looks for the host's wads already on this computer (in other profiles, in earlier downloads, and by name in any dir your profiles use) and offers to make a new profile from them, downloading only the ones that are missing. A file that only might be the right one is hashed first, in a green **hashing** window. The download window shows the size and speed, and `Esc` cancels it. Finished wads are kept, so trying again picks up where it stopped.

Joining also keeps your wads up to date with your friends'. When one of yours has the name of one of the host's but other contents, a newer version, say, tdl asks whether to **[R]eplace** it with the host's (yours is kept beside it as `brutal.pk3.bak`), **[N]ame Mine** to rename yours and keep both (the profiles using yours follow it to its new name), or **[S]kip** it and download the host's separately, as before. A wad you once downloaded from a host is simply swapped for the host's version or not, and the old download stays where it is. Every profile using a replaced wad is upgraded with it, so a profile that only differed by those wads joins as it is. A **volatile** wad is replaced without asking, with the same `.bak`, whenever its file really differs from the host's (see [Volatile wads](#volatile-wads)).

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
| `src/screens/splash.rs` | The startup logo and the color of each of its rows (flame to blood, or the trans flag's stripes), tagline, the "press any key" hint and how fast it pulses, and how fast the pride fire goes round the rainbow |
| `src/widgets/fire.rs` | The fire itself: its palette, its rainbow, and how high the flames reach |
| `src/popups.rs` | How wide popup text gets before it wraps |

Setting `BACKGROUND` in `theme.rs` to `Color::Reset` keeps your terminal's own background.

## Recording the demo and the screenshots

```sh
./demo-screenshots.sh
```

It builds the release binary, records ten seconds of the splash screen into `demo.gif` (`demo.tape`, always with quip 17, so the demo is the same every time), then lays out the same example profiles the tests use and saves each screen the README shows to `docs/screenshots/` (`screenshots.tape`). It needs `vhs`, `ttyd` and `ffmpeg`. Both tapes run tdl on a throwaway vault with its home in `/tmp`, so your profiles and settings are never read or written, and those dirs are removed afterwards.

## Credits

- Music: "Inevitable Doom" by celestialghost8 (CC0), played on the splash screen (and in the launcher with `--keep-music` or the **music** setting)

## License

[0BSD](LICENSE)
