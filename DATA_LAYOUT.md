# Where tdl keeps things

tdl stores two kinds of files: **config** (what you set up) and **data** (what the games produce and what is downloaded). Both folders are created automatically.

## Locations

| OS | Config folder | Data folder |
| --- | --- | --- |
| Linux | `~/.config/tdl` | `~/.local/share/tdl` |
| macOS | `~/Library/Application Support/error.error.fruit.eef.tdl` | same folder as config |
| Windows | `%APPDATA%\eef\tdl\config` | `%APPDATA%\eef\tdl\data` |

Pass `--vault /some/folder` to use `/some/folder/config` and `/some/folder/data` instead. The older form, the folder on its own (`tdl /some/folder`), still works.

The macOS paths follow the `directories` crate's documentation and have not been tried on a Mac. On macOS the two folders are the same, so the files below share one folder. That is why the profiles folder and the downloads folder start with a dot: profile names can't contain one, so those folders can never be mistaken for a profile's data folder.

## Layout

```
<config folder>/
├── config.rite                 global settings and the order of profiles
├── config.rite.bak             the config as it was before the last save
└── .profiles/                  (the dot keeps it from ever matching a profile's name)
    ├── brutal.rite             one file per profile, named after the profile
    ├── brutal.rite.bak         that profile as it was before the last save
    └── nacht.rite

<data folder>/
├── brutal/                     one folder per profile, named after the profile
│   ├── saves/                  UZDoom's save games for this profile (-savedir)
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
| `base_wad` | The default base wad (`path` and `hash`), used by profiles that don't set their own. |
| `profile_names` | The profiles in the order the launcher shows them. |

**`.profiles/<name>.rite`** (`data`)

| Field | Meaning |
| --- | --- |
| `name` | The profile's name. Letters, digits and `_` only, since it is also the file and folder name. |
| `wads` | The wads in load order, each with its full `path` and `hash`. |
| `base_wad` | Overrides the global base wad. |
| `uzdoom` | Overrides the global UZDoom path. |
| `ini` | A UZDoom config file to pass with `-config`. If unset, UZDoom uses its own default. |

All paths are the full paths on the machine that saved them, so a profile file copied to another computer will not find its wads.

## How the files are written

- **Atomic saves.** A file is written to `<file>.tmp`, synced to disk, and only then renamed over the real file, so a crash leaves either the old file or the new one, never half of one.
- **Safe ordering.** Adding, renaming and deleting a profile save the new profile file and the settings first. Only then are saves moved or old files deleted, so a failed save (a full disk, say) leaves the old profile and its saves exactly as they were.
- **One backup.** Before a file is replaced, its previous version is copied to `<file>.bak`. Each save overwrites the previous `.bak`.
- **Editions.** `version` is the edition number. When a file is older than the running tdl, tdl offers to upgrade it, checks the upgraded file by reading it back, and asks before deleting the original. A file from a newer tdl is never touched; tdl tells you and exits.
- **Old folder name.** The profiles folder used to be called `profiles`. tdl renames it to `.profiles` on startup if the new one doesn't exist yet.
- **Old files.** Before `.rite` files existed, tdl used `.doom` files. They are still read. After an upgrade the original `.doom` (or the old-format file's `.bak`) is deleted only if you say yes.

## When a file is unreadable

On startup tdl tells you what it found and what it did:

- **A profile or the settings can't be read.** The bad file is renamed to `.corrupt`, and its `.bak` takes its place if that reads fine ("Restored 'brutal' from its backup"). Otherwise the profile is empty but keeps its name, so you can still see it and delete it ("'brutal' was unreadable, saved as .corrupt").
- **The settings are lost with no usable backup.** The profile list is rebuilt from the files in `.profiles/`, in alphabetical order, so your profiles don't disappear.
- **A profile's file is gone.** The profile is listed empty with a notice.
- **A file from a newer tdl.** Nothing is changed, and tdl exits with a message.

## Restoring from a backup

Close tdl, then copy `brutal.rite.bak` over `brutal.rite`. The backup is the version from before the last save, so it will not contain anything changed in that last save.

Deleting a profile in the launcher removes its `.rite`, its `.bak`, and its whole data folder, **including the saves**.
