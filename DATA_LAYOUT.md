# Where tdl keeps things

tdl stores two kinds of files: **config** (what you set up) and **data** (what the games produce and what is downloaded). Both folders are created automatically.

## Locations

| OS | Config folder | Data folder |
| --- | --- | --- |
| Linux | `~/.config/tdl` | `~/.local/share/tdl` |
| macOS | `~/Library/Application Support/error.error.fruit.eef.tdl` | same folder as config |
| Windows | `%APPDATA%\eef\tdl\config` | `%APPDATA%\eef\tdl\data` |

Pass `--vault /some/folder` to use `/some/folder/config` and `/some/folder/data` instead. The older form, the folder on its own (`tdl /some/folder`), still works. tdl then asks whether to open that vault every time; if you say yes, it is remembered in `global.rite` (below) and opened whenever tdl starts without `--vault`. If it has gone missing, tdl says so and opens the standard folders. `--vault-once /some/folder` opens a vault the same way without asking. `tdl --forget-vault` opens the standard folders and asks whether to forget the remembered vault.

`global.rite` always lives in the standard config folder, whichever vault is open, since it is the one place every vault can find. It holds `vault` (the vault to open by default) and `wad_folders` (wad folders kept for every vault from `--wads`).

The macOS paths follow the `directories` crate's documentation and have not been tried on a Mac. On macOS the two folders are the same, so the files below share one folder. That is why the profiles folder and the downloads folder start with a dot: profile names can't contain one, so those folders can never be mistaken for a profile's data folder.

## Layout

```
<config folder>/
├── config.rite                 global settings and the order of profiles
├── config.rite.bak             the config as it was before the last save
├── global.rite                 (standard config folder only) the default vault and global wad folders
└── .profiles/                  (the dot keeps it from ever matching a profile's name)
    ├── brutal.rite             one file per profile, named after the profile
    ├── brutal.rite.bak         that profile as it was before the last save
    └── nacht.rite

<data folder>/
├── brutal/                     one folder per profile, named after the profile
│   ├── saves/                  UZDoom's save games for this profile (-savedir)
│   ├── uzdoom.ini              only if you chose "New" for the profile's UZDoom config (see below)
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
| `uzdoom` | Path to the UZDoom executable, used by profiles that don't set their own. |
| `base_wad` | The default base wad (`path`, `hash`, `size`, `volatile` and `modified`), used by profiles that don't set their own. |
| `profile_names` | The profiles in the order the launcher shows them. |
| `skip_splash` | `true` to go straight to the launcher without the splash screen. Missing in older files, which count as `false`. Toggled with `S` in the launcher. |
| `host_port` | The port the last game was hosted on, offered again in the host form. Empty for the default (8080). Missing in older files, which count as empty. |
| `wad_folders` | Wad folders kept for this vault from `--wads`, used whenever tdl starts without it (before the global ones). Missing in older files, which count as none. |

**`.profiles/<name>.rite`** (`data`)

| Field | Meaning |
| --- | --- |
| `name` | The profile's name. Letters, digits and `_` only, since it is also the file and folder name. |
| `folders` | Every folder the profile's wads are in, each listed once. |
| `wads` | The wads in load order. Each names its `folder` (a position in `folders`) and its file `name`, plus its `hash`, `size`, `modified` and `volatile`. The size (0 if unknown) lets tdl notice when a file has changed since it was added. `modified` (nanoseconds since 1970, 0 if unknown) is the file's modified time when it was hashed: when joining, a wad of unchanged size and modified time is trusted without being read again. `volatile` marks a wad you are still developing: see below. A wad whose file name isn't valid UTF-8 is saved with its whole `path` instead. |
| `base_wad` | Overrides the global base wad. Saved like an entry in `wads`. |
| `uzdoom` | Overrides the global UZDoom path. |
| `ini` | A UZDoom config file to pass with `-config`. If unset, UZDoom uses its own global config. Press `I` in the editor to choose: **Global** (unset), **New** (an empty `uzdoom.ini` in the profile's data folder, which UZDoom fills in on first run) or **Select** (an existing file). A profile's own ini moves with it when the profile is renamed. |
| `host_defaults` | What the host form starts with for this profile: `player_count`, `is_deathmatch`, `map`, `port` and `code`, set with **save as default** in the host form and removed with **clear default**. An empty `port` or `code` means the default port and a random code. A saved code is stored as plain text. Missing in older files, which count as none. |

Paths are saved as text (or, for the rare path that isn't valid UTF-8, in the saving OS's own form), so a profile or config file reads on any OS. The folders are still the ones on the machine that saved them, so a profile copied to another computer will show its wads as missing until they are found; the shared `.scroll` format (planned) leaves the folders out entirely.

Files saved before edition 2 kept each wad's whole path as an OS-tagged string, which only the OS that wrote it could read. They still load, and the upgrade prompt rewrites them in the new shape.

## How the files are written

- **Atomic saves.** A file is written to `<file>.tmp`, synced to disk, and only then renamed over the real file, so a crash leaves either the old file or the new one, never half of one.
- **Safe ordering.** Adding, renaming and deleting a profile save the new profile file and the settings first. Only then are saves moved or old files deleted, so a failed save (a full disk, say) leaves the old profile and its saves exactly as they were.
- **One backup.** Before a file is replaced, its previous version is copied to `<file>.bak`. Each save overwrites the previous `.bak`.
- **Editions.** `version` is the edition number. When a file is older than the running tdl, tdl offers to upgrade it, checks the upgraded file by reading it back, and asks before deleting the original. A file from a newer tdl is never touched; tdl tells you and exits.
- **Old folder name.** The profiles folder used to be called `profiles`. tdl renames it to `.profiles` on startup if the new one doesn't exist yet.
- **Old files.** Before `.rite` files existed, tdl used `.doom` files. They are still read. After an upgrade the original `.doom` (or the old-format file's `.bak`) is deleted only if you say yes.

## When a wad can't be found

tdl never searches for moved files. A wad (or the base wad, UZDoom or ini) that has gone missing, isn't a file, or can't be opened is shown as a placeholder in the editor (a red row marked `!`), and `Space` on it says exactly what is wrong. `R` picks the file again. A profile with such a problem can't be launched or hosted, and the launcher marks it when the cursor is on it; other profiles are unaffected. A wad whose size changed since it was added is only marked `~`, since it still runs.

### Volatile wads

A wad you are still working on changes constantly, so its stored hash goes stale. Press `V` on it in the editor ("Toggle Volatility") to mark it volatile; it shows a cyan `*`. A volatile wad is not flagged when its size changes, replacing it never asks about a different hash, and when you host, tdl hashes it again right before building the list, so players download the current version. When you join a game, a volatile wad matches the host's wad of the same name; if its hash differs you are asked whether to join anyway.

## When a file is unreadable

On startup tdl tells you what it found and what it did:

- **A profile or the settings can't be read.** The bad file is renamed to `.corrupt`, and its `.bak` takes its place if that reads fine ("Restored 'brutal' from its backup"). Otherwise the profile is empty but keeps its name, so you can still see it and delete it ("'brutal' was unreadable, saved as .corrupt").
- **The settings are lost with no usable backup.** The profile list is rebuilt from the files in `.profiles/`, in alphabetical order, so your profiles don't disappear.
- **A profile's file is gone.** The profile is listed empty with a notice.
- **A file from a newer tdl.** Nothing is changed, and tdl exits with a message.

## Restoring from a backup

Close tdl, then copy `brutal.rite.bak` over `brutal.rite`. The backup is the version from before the last save, so it will not contain anything changed in that last save.

Deleting a profile in the launcher removes its `.rite`, its `.bak`, and its whole data folder, **including the saves**.
