# Where tdl keeps things

tdl stores two kinds of files: **config** (what you set up) and **data** (what the games produce and what is downloaded). Both dirs are created automatically.

## Locations

| OS | Config dir | Data dir |
| --- | --- | --- |
| Linux | `~/.config/tdl` | `~/.local/share/tdl` |
| macOS | `~/Library/Application Support/error.error.fruit.eef.tdl` | same dir as config |
| Windows | `%APPDATA%\eef\tdl\config` | `%APPDATA%\eef\tdl\data` |

A **loadout** is one pair of these dirs: the standard ones, or any others. Pass `--loadout /some/dir` to use `/some/dir/config` and `/some/dir/data` instead (the dir on its own, `tdl /some/dir`, is the same). tdl then asks whether to open that loadout every time; if you say yes, it is remembered in `uac.rite` (below) and opened whenever tdl starts without `--loadout`. If it has gone missing, tdl says so and opens the standard dirs. With `--session` it opens the loadout without asking, and `--keep-loadout /some/dir` remembers it without asking. `tdl --forget-loadout` opens the standard dirs and asks whether to forget the remembered loadout; with `--session` it opens them without asking, and `--keep-forget-loadout` forgets it without asking.

`uac.rite` always lives in the standard config dir, whichever loadout is open, since it is the one place every loadout can find. (Before tdl 1.0.0 it was `global.rite`; see [Legacy files](#legacy-files).) It holds every setting:

| Field | Meaning |
| --- | --- |
| `loadout` | The loadout to open by default, kept from `--loadout`. |
| `wad_dirs` | Wad dirs kept for every loadout from `--wads`, used after the loadout's own. |
| `music` | `Off`, `Splash` or `Kept`: no music, music on the splash screen only, or music that keeps playing in the launcher. |
| `splash_quip` | The splash screen's quip: `Random`, or `Custom` with the text as typed, in which every `#<number>` stands for that built-in quip (a number this tdl has no quip for shows a random one). |
| `no_quips` | `true` to leave the quips out of every window. |
| `song` | The path of a song to play instead of the built-in track, picked in the settings. |
| `uzdoom` | The UZDoom executable used by profiles that don't set their own, `U` in the launcher, or kept from `--uzdoom`. |
| `base_wad` | The default base wad, saved like a profile's: `path`, `hash`, `size`, `volatile` and `modified`; `W` in the launcher, or kept from `--base-wad`. |
| `uzdoom_args` | Extra arguments for UZDoom, as one line of text split at spaces, passed on every launch, host and join before the wads; **uzdoom args** in the settings. |
| `skip_splash` | `true` to go straight to the launcher; **splash** in the settings. |
| `host_port` | The port the last game was hosted on, offered again in the host form; empty for the default, 8080. |
| `pride` | `true` to play the pride splash every time; **pride** in the settings. |
| `ini` | The UZDoom ini used by profiles that don't set their own, instead of UZDoom's usual one; **ini** in the settings, or kept from `--ini`. |
| `theme` | The `theme.toml` tdl wears instead of its built-in theme; **theme** in the settings, or kept from `--theme`. Empty for the built-in one. |

Any field missing from the file counts as unset, `false` or empty (`Splash` for `music`, `Random` for `splash_quip`).

What `--uzdoom`, `--base-wad` and `--ini` give is used for the run over every profile's own and the global one; it reaches `uac.rite` only if you agree to keep it, or with the flag's `--keep-` form. With `--session`, nothing passed on the command line is kept or asked about.

**`theme.toml`** is a theme of your own, written to the standard config dir when you pick **New** for the **theme** setting (an existing one is never written over), and referenced by `theme` above. Unlike the `.rite` files it is plain text ([TOML](https://toml.io)), for you to change: see [Themes](README.md#themes) in the README. A theme may live anywhere; `theme` holds its path.

The macOS paths follow the `directories` crate's documentation and have not been tried on a Mac. On macOS the two dirs are the same, so the files below share one dir. That is why the profiles dir and the downloads dir start with a dot: profile names can't contain one, so those dirs can never be mistaken for a profile's data dir.

## Layout

```
<config dir>/
├── config.rite                 the order of profiles and the loadout's own wad dirs
├── config.rite.bak             the config as it was before the last save
├── uac.rite                    (standard config dir only) every setting, the default loadout and global wad dirs
├── theme.toml                  (standard config dir only) a theme of your own, once the settings write one
└── .profiles/                  (the dot keeps it from ever matching a profile's name)
    ├── brutal.rite             one file per profile, named after the profile
    ├── brutal.rite.bak         that profile as it was before the last save
    └── nacht.rite

<data dir>/
├── brutal/                     one dir per profile, named after the profile
│   ├── saves/                  UZDoom's save games for this profile (-savedir)
│   ├── uzdoom.ini              only if you chose "New" for the profile's UZDoom config (see below)
│   ├── host.ini                only once you joined with the host's ini: the last host's, with your
│   │                           own player, controls, screen and sound (written again every join)
│   └── session.log             everything UZDoom printed during the last session
├── nacht/
│   ├── saves/
│   └── session.log
└── .downloads/                 wads fetched from hosts, shared by every profile
    └── <hash>/
        └── <file name>         stored by the BLAKE3 hash of its contents
```

If a file can't be read when tdl starts, it is renamed to `<file>.corrupt` (for example `brutal.rite.corrupt`) and left there for you to inspect. tdl never overwrites it.

While a file is being written you may briefly see `<file>.tmp`, and a download in progress is `<file name>.part`. Both are deleted automatically if the write or download fails.

## What the files contain

Every `.rite` file is MessagePack, a compact binary format, in a wrapper that records which edition of tdl wrote it:

```
{ version: <number>, data: { ... } }
```

**`config.rite`** (`data`)

| Field | Meaning |
| --- | --- |
| `profile_names` | The profiles in the order the launcher shows them. |
| `wad_dirs` | Wad dirs kept for this loadout from `--wads` (or `--keep-loadout-wads`), used whenever tdl starts without it (before the global ones). Missing means none. |

Only what belongs to the loadout is here; every setting is in `uac.rite`, so it is the same whichever loadout is open.

**`.profiles/<name>.rite`** (`data`)

| Field | Meaning |
| --- | --- |
| `name` | The profile's name. Letters, digits and `_` only, since it is also the file and dir name. |
| `dirs` | Every dir the profile's wads are in, each listed once. |
| `wads` | The wads in load order. Each names its `dir` (a position in `dirs`) and its file `name`, plus its `hash`, `size`, `modified` and `volatile`. The size (0 if unknown) lets tdl notice when a file has changed since it was added. `modified` (nanoseconds since 1970, 0 if unknown) is the file's modified time when it was hashed: when joining, a wad of unchanged size and modified time is trusted without being read again. `volatile` marks a wad you are still developing: see below. A wad whose file name isn't valid UTF-8 is saved with its whole `path` instead. |
| `base_wad` | Overrides the global base wad. Saved like an entry in `wads`. |
| `uzdoom` | Overrides the global UZDoom path. |
| `ini` | A UZDoom config file to pass with `-config`. If unset, the global one from `uac.rite` is used, or else UZDoom uses its own usual config. Press `I` in the editor to choose: **Global** (unset), **New** (an empty `uzdoom.ini` in the profile's data dir, which UZDoom fills in on first run) or **Select** (an existing file). A profile's own ini moves with it when the profile is renamed. |
| `host_defaults` | What the host form starts with for this profile: `player_count`, `is_deathmatch`, `map`, `port`, `code` and `share_ini` (whether the ini is served to joiners; missing in older files, which count as not sharing it), set with **save as default** in the host form and removed with **clear default**. An empty `port` or `code` means the default port and a random code. A saved code is stored as plain text. Missing in older files, which count as none. |
| `uzdoom_args` | Extra UZDoom arguments for this profile only, set with `G` in the editor, used instead of the global ones in `uac.rite`. Missing (or empty) means the global ones. |
| `watched` | The dir the profile watches, made with `F` in the launcher or set with `T` in the editor. Wads put in it (at any depth) are added at the end, and wads whose files leave it are taken out, whenever tdl starts, before the profile is launched or hosted, and when it is opened in the editor. Missing in older files, which count as none. |
| `dismissed` | Wads in the watched dir that were taken out of the profile by hand, so they aren't added back; each is forgotten once its file is gone. Missing in older files, which count as none. |

Paths are saved as text (or, for the rare path that isn't valid UTF-8, in the saving OS's own form), so a profile or config file reads on any OS. The dirs are still the ones on the machine that saved them, so a profile copied to another computer will show its wads as missing until they are found; a `.scroll` (below) leaves the dirs out entirely.

## Scrolls

A `.scroll` is one or more profiles sealed up to share (`S` in the launcher: the profile under the cursor, the ones selected, or the whole loadout), written wherever you choose. Like the other files it is MessagePack in an edition wrapper, holding:

| Field | Meaning |
| --- | --- |
| `profiles` | The profiles, in the launcher's order. Each has a `name` (kept on import unless it is taken, or offered to rename for a scroll of one), `wads` (in load order, each only its `name`, `size` (0 if unknown), `hash` and `volatile`: no dirs, no paths), `base_wad` (the profile's own, the same way, if it has one) and `host_defaults` (with the join code left out, and `share_ini` off). |
| `settings` | The maker's `music`, `no_quips` and `splash_quip` from their `uac.rite`, if they chose to seal them in. Never their song, loadout or wad dirs. Missing if they didn't. A splash quip over 120 characters or with control characters gets the scroll refused. |

Scrolls of edition 0 (from tdl 0.6.0) held one profile, with its `name`, `wads`, `base_wad` and `host_defaults` at the top level beside `settings`. tdl 1.0.0 doesn't import them; `python3 upgrade_legacy.py --scroll <file>` makes one a scroll of that one profile (see [Legacy files](#legacy-files)). tdl 0.6.0 can't read the newer edition.

Importing (`I`) trusts nothing in it: there are at most 512 profiles and 4096 wads in all, every profile name must be one tdl could have made (letters, digits and `_`, so it can't name a path), every wad name a plain file name (so `../x` can't reach outside a dir) and every hash a real BLAKE3 hash, or the scroll is refused.

## How the files are written

- **Atomic saves.** A file is written to `<file>.tmp`, synced to disk, and only then renamed over the real file, so a crash leaves either the old file or the new one, never half of one.
- **Safe ordering.** Adding, renaming and deleting a profile save the new profile file and the settings first. Only then are saves moved or old files deleted, so a failed save (a full disk, say) leaves the old profile and its saves exactly as they were.
- **One backup.** Before a file is replaced, its previous version is copied to `<file>.bak`. Each save overwrites the previous `.bak`.
- **Editions.** `version` is the edition number: tdl 1.0.0 writes `uac.rite` as edition 2, `config.rite` as 5, profiles as 4 and scrolls as 1. When a later tdl changes what a file means, it offers to upgrade an older file, checks the upgraded file by reading it back, and asks before deleting the original (the `.bak`). A file from a newer tdl is never touched; tdl tells you and exits. A file from before tdl 1.0.0 is a legacy file (below).

## Legacy files

tdl 1.0.0 no longer reads the files of earlier tdls itself. It looks for them when it starts, in the standard config dir and in the loadout it opens: `global.rite` (when there is no `uac.rite`), a `config.rite` from before edition 5 or a profile from before edition 4 (or a file saved before editions existed at all, as a bare array), the `.doom` files from before `.rite` (unless a `.rite` of the same name is beside it), and the old `profiles` dir from before it had a dot. If it finds any, it lists them and the command that upgrades them, changes nothing, and closes (with `--headless` or `--seal`, it prints them and fails).

`upgrade_legacy.py`, which comes with tdl, upgrades them all. It needs Python 3.8 or later and nothing else:

```sh
python3 upgrade_legacy.py [--loadout DIR]... [--scroll FILE]... [--standard DIR] [--dry-run]
```

It works on the standard config dir, the loadout `global.rite` remembers opening by default, and every `--loadout DIR` (the loadout's `DIR/config`); `--standard DIR` stands in for the standard config dir. For each, it:

- moves the old `profiles` dir's files into `.profiles` (never over a file already there);
- writes `global.rite` as `uac.rite` (its `vault` is now `loadout`, and `wad_folders` is `wad_dirs`);
- brings every `config.rite` (or `config.doom`) up to edition 5. Before edition 5 a config also held `uzdoom`, `base_wad`, `skip_splash`, `host_port` and `uzdoom_args`; each goes to `uac.rite`, but only if `uac.rite` doesn't have it yet, so the standard loadout's win, then the remembered one's, then those given with `--loadout`, in order;
- brings every profile (`.rite`, `.doom`, and their `.bak` copies, so a backup restored later can be read) up to edition 4: OS-tagged paths become text wherever they can, `folders` and `folder` become `dirs` and `dir`, and a wad saved before sizes were recorded (before edition 3, with a size of 0) gets the size of its file, if it is there (nothing is hashed, so it is still read again before it is trusted when joining);
- with `--scroll`, makes a scroll of edition 0 a scroll of its one profile.

Every file it upgrades is kept beside the new one as `<file>.legacy`, and a `.doom` file or `global.rite` it has upgraded is moved to `<file>.legacy`; nothing is deleted. A file that is current already is left alone, so it can be run again safely, and a file from a newer tdl is never touched. `--dry-run` says what it would do and changes nothing.

## When a wad can't be found

tdl never searches for moved files. A wad (or the base wad, UZDoom or ini) that has gone missing, isn't a file, or can't be opened is shown as a placeholder in the editor (a red row marked `!`), and `Space` on it says exactly what is wrong. `R` picks the file again. A profile with such a problem can't be launched or hosted, and the launcher marks it when the cursor is on it; other profiles are unaffected. A wad whose size changed since it was added is only marked `~`, since it still runs.

### Volatile wads

A wad you are still working on changes constantly, so its stored hash goes stale. Press `V` on it in the editor ("Toggle Volatility") to mark it volatile; it shows a cyan `*`. A volatile wad is not flagged when its size changes, replacing it never asks about a different hash, and when you host, tdl hashes it again right before building the list, so players download the current version. When you join a game, a volatile wad matches the host's wad of the same name; it is hashed again, and if it really differs it is replaced by the host's version without asking, its old file kept beside it as `<file>.bak` (any older `.bak` is overwritten), and the new hash saved in every profile that uses it. A wad that isn't volatile but has the name of one of the host's and other contents is replaced the same way only if you say so, or renamed so both are kept.

## When a file is unreadable

On startup tdl tells you what it found and what it did:

- **A profile or the settings can't be read.** The bad file is renamed to `.corrupt`, and its `.bak` takes its place if that reads fine ("Restored 'brutal' from its backup"). Otherwise the profile is empty but keeps its name, so you can still see it and delete it ("'brutal' was unreadable, saved as .corrupt").
- **The settings are lost with no usable backup.** The profile list is rebuilt from the files in `.profiles/`, in alphabetical order, so your profiles don't disappear.
- **A profile's file is gone.** The profile is listed empty with a notice.
- **A file from a newer tdl.** Nothing is changed, and tdl exits with a message.

## Restoring from a backup

Close tdl, then copy `brutal.rite.bak` over `brutal.rite`. The backup is the version from before the last save, so it will not contain anything changed in that last save.

Deleting a profile in the launcher removes its `.rite`, its `.bak`, and its whole data dir, **including the saves**.
