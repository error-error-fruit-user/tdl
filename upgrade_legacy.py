#!/usr/bin/env python3
"""Upgrades the files of a tdl from before 1.0.0, which tdl 1.0.0 and later don't read.

    python3 upgrade_legacy.py [--loadout DIR]... [--scroll FILE]... [--standard DIR] [--dry-run]

It upgrades, in the platform's standard config dir and in every loadout:

  - global.rite, the global settings, which became uac.rite (its `vault` is now the `loadout`)
  - config.rite (and the older config.doom) from before edition 5, whose settings (UZDoom, the base
    wad, UZDoom's arguments, the splash and the last host port) move to uac.rite, each one only if
    uac.rite doesn't have it yet, so the standard loadout's win, then the remembered one's, then
    those given with --loadout, in order
  - every profile from before edition 4 (and the older .doom ones), from the old `profiles` dir too
  - the .bak copies tdl keeps of each, so a backup restored later is one tdl can read
  - with --scroll, a .scroll from tdl 0.6.0 (edition 0), which held a single profile

Nothing is ever deleted: every file it upgrades is kept beside the new one as <file>.legacy, and
a file that is already current is left alone, so running it twice is harmless. Close tdl first.

--loadout DIR is a loadout tdl was given with --loadout (or --vault, before 1.0.0): its files are
in DIR/config. The loadout tdl remembers opening by default is upgraded without being named.
--standard DIR replaces the platform's standard config dir. --dry-run says what would be done and
changes nothing.

Needs Python 3.8 or later, and nothing else.
"""

import argparse
import os
import shutil
import struct
import sys

# The editions tdl 1.0.0 writes: anything older is upgraded to these.
UAC_EDITION = 2
CONFIG_EDITION = 5
PROFILE_EDITION = 4
SCROLL_EDITION = 1

# Editions before this one never recorded a wad's size, so it is filled in from the file.
SIZED_EDITION = 3

LEGACY_SUFFIX = ".legacy"


class Unreadable(Exception):
    """A file that isn't one tdl wrote, or that is damaged."""


# --- MessagePack, the format of every tdl file -------------------------------------------------


def unpack(data):
    """The value MessagePack `data` holds, which must be one whole value."""
    value, end = _unpack(data, 0)
    if end != len(data):
        raise Unreadable("there is more after the end")
    return value


def _unpack(data, at):
    try:
        byte = data[at]
    except IndexError:
        raise Unreadable("it ends too soon") from None
    at += 1
    if byte <= 0x7F:
        return byte, at
    if byte >= 0xE0:
        return byte - 0x100, at
    if 0x80 <= byte <= 0x8F:
        return _unpack_map(data, at, byte & 0x0F)
    if 0x90 <= byte <= 0x9F:
        return _unpack_array(data, at, byte & 0x0F)
    if 0xA0 <= byte <= 0xBF:
        return _unpack_str(data, at, byte & 0x1F)
    fixed = {
        0xCC: ">B",
        0xCD: ">H",
        0xCE: ">I",
        0xCF: ">Q",
        0xD0: ">b",
        0xD1: ">h",
        0xD2: ">i",
        0xD3: ">q",
        0xCA: ">f",
        0xCB: ">d",
    }
    if byte == 0xC0:
        return None, at
    if byte == 0xC2:
        return False, at
    if byte == 0xC3:
        return True, at
    if byte in fixed:
        return _read(data, at, fixed[byte])
    lengths = {0xD9: ">B", 0xDA: ">H", 0xDB: ">I"}
    if byte in lengths:
        length, at = _read(data, at, lengths[byte])
        return _unpack_str(data, at, length)
    lengths = {0xC4: ">B", 0xC5: ">H", 0xC6: ">I"}
    if byte in lengths:
        length, at = _read(data, at, lengths[byte])
        return _take(data, at, length)
    if byte in (0xDC, 0xDD):
        length, at = _read(data, at, ">H" if byte == 0xDC else ">I")
        return _unpack_array(data, at, length)
    if byte in (0xDE, 0xDF):
        length, at = _read(data, at, ">H" if byte == 0xDE else ">I")
        return _unpack_map(data, at, length)
    raise Unreadable("it holds something tdl never writes (0x%02x)" % byte)


def _read(data, at, layout):
    size = struct.calcsize(layout)
    if at + size > len(data):
        raise Unreadable("it ends too soon")
    return struct.unpack_from(layout, data, at)[0], at + size


def _take(data, at, length):
    if at + length > len(data):
        raise Unreadable("it ends too soon")
    return bytes(data[at : at + length]), at + length


def _unpack_str(data, at, length):
    raw, at = _take(data, at, length)
    try:
        return raw.decode("utf-8"), at
    except UnicodeDecodeError:
        raise Unreadable("it holds text that isn't UTF-8") from None


def _unpack_array(data, at, length):
    items = []
    for _ in range(length):
        item, at = _unpack(data, at)
        items.append(item)
    return items, at


def _unpack_map(data, at, length):
    items = {}
    for _ in range(length):
        key, at = _unpack(data, at)
        value, at = _unpack(data, at)
        if not isinstance(key, str):
            raise Unreadable("it has a map whose keys aren't names")
        items[key] = value
    return items, at


def pack(value):
    """`value` as MessagePack, the way tdl writes it: maps keyed by field names."""
    out = bytearray()
    _pack(value, out)
    return bytes(out)


def _pack(value, out):
    if value is None:
        out.append(0xC0)
    elif value is True:
        out.append(0xC3)
    elif value is False:
        out.append(0xC2)
    elif isinstance(value, int):
        _pack_int(value, out)
    elif isinstance(value, float):
        out.append(0xCB)
        out += struct.pack(">d", value)
    elif isinstance(value, str):
        raw = value.encode("utf-8")
        _pack_length(len(raw), out, 0xA0, 31, (0xD9, 0xDA, 0xDB))
        out += raw
    elif isinstance(value, (bytes, bytearray)):
        _pack_length(len(value), out, None, 0, (0xC4, 0xC5, 0xC6))
        out += value
    elif isinstance(value, (list, tuple)):
        _pack_length(len(value), out, 0x90, 15, (None, 0xDC, 0xDD))
        for item in value:
            _pack(item, out)
    elif isinstance(value, dict):
        _pack_length(len(value), out, 0x80, 15, (None, 0xDE, 0xDF))
        for key, item in value.items():
            _pack(key, out)
            _pack(item, out)
    else:
        raise TypeError("can't write %r" % (value,))


def _pack_int(value, out):
    if 0 <= value <= 0x7F:
        out.append(value)
    elif -32 <= value < 0:
        out.append(value + 0x100)
    elif value >= 0:
        for marker, layout, top in ((0xCC, ">B", 0xFF), (0xCD, ">H", 0xFFFF), (0xCE, ">I", 0xFFFFFFFF)):
            if value <= top:
                out.append(marker)
                out += struct.pack(layout, value)
                return
        out.append(0xCF)
        out += struct.pack(">Q", value)
    else:
        for marker, layout, bottom in ((0xD0, ">b", -0x80), (0xD1, ">h", -0x8000), (0xD2, ">i", -0x80000000)):
            if value >= bottom:
                out.append(marker)
                out += struct.pack(layout, value)
                return
        out.append(0xD3)
        out += struct.pack(">q", value)


def _pack_length(length, out, fix, fix_max, markers):
    if fix is not None and length <= fix_max:
        out.append(fix | length)
    elif markers[0] is not None and length <= 0xFF:
        out.append(markers[0])
        out.append(length)
    elif length <= 0xFFFF:
        out.append(markers[1])
        out += struct.pack(">H", length)
    else:
        out.append(markers[2])
        out += struct.pack(">I", length)


# --- The shapes tdl has saved over the years ----------------------------------------------------


def opened(data):
    """A file's edition and data: `{version, data}`, or a bare array from before editions (0)."""
    value = unpack(data)
    if isinstance(value, dict) and isinstance(value.get("version"), int) and "data" in value:
        return value["version"], value["data"]
    if isinstance(value, list):
        return 0, value
    raise Unreadable("it isn't a tdl file")


def sealed(version, data):
    return pack({"version": version, "data": data})


def fields(value, names, aliases=None):
    """A struct as a dict of its fields, whether saved as a map or (edition 0) a bare array of its
    fields in `names` order. `aliases` maps old field names to new ones."""
    if isinstance(value, list):
        if len(value) > len(names):
            raise Unreadable("it has more fields than tdl ever saved")
        return dict(zip(names, value))
    if isinstance(value, dict):
        value = dict(value)
        for old, new in (aliases or {}).items():
            if old in value and new not in value:
                value[new] = value.pop(old)
        return value
    raise Unreadable("it has a struct that is neither a map nor an array")


def path(value):
    """A path as tdl 1.0.0 saves it: text, or for the rare one that isn't valid UTF-8, the OS's own
    form (`{"Unix": bytes}` or `{"Windows": wide chars}`), which older editions used for every path."""
    if value is None or isinstance(value, str):
        return value
    if isinstance(value, dict) and len(value) == 1:
        (tag, units), = value.items()
        if tag in ("Unix", "Windows") and isinstance(units, list):
            try:
                if tag == "Unix":
                    return bytes(units).decode("utf-8")
                return struct.pack("<%dH" % len(units), *units).decode("utf-16-le")
            except (UnicodeDecodeError, ValueError, struct.error):
                return {tag: units}
    raise Unreadable("it has a path tdl can't make out")


def paths(values):
    return [path(value) for value in values or []]


def sized(wad_path, size, backfill):
    """`size`, or with `backfill`, the file's own size if it was never recorded (0) and the file is
    there: the wads added before sizes were recorded."""
    if backfill and size == 0 and isinstance(wad_path, str) and os.path.isfile(wad_path):
        return os.path.getsize(wad_path)
    return size


def wad(value, backfill):
    """A wad saved with its whole path, as tdl 1.0.0 saves the global base wad (and any wad of a
    profile that has no dir): `{path, hash, size, volatile, modified}`."""
    saved = fields(value, ["path", "hash", "size", "volatile", "modified"])
    wad_path = path(saved.get("path"))
    return {
        "path": wad_path,
        "hash": saved.get("hash", ""),
        "size": sized(wad_path, saved.get("size", 0), backfill),
        "volatile": bool(saved.get("volatile", False)),
        "modified": saved.get("modified", 0),
    }


def host_settings(value):
    if value is None:
        return None
    saved = fields(value, ["player_count", "is_deathmatch", "map", "port", "code", "share_ini"])
    return {
        "player_count": saved.get("player_count", "2"),
        "is_deathmatch": bool(saved.get("is_deathmatch", True)),
        "map": saved.get("map", ""),
        "port": saved.get("port", ""),
        "code": saved.get("code", ""),
        "share_ini": bool(saved.get("share_ini", False)),
    }


def profile(version, data):
    """A profile as edition 4 saves it: its dirs listed once, each wad naming its dir and file."""
    saved = fields(
        data,
        ["name", "wads", "base_wad", "uzdoom", "ini", "host_defaults", "dirs", "uzdoom_args", "watched", "dismissed"],
        {"folders": "dirs"},
    )
    backfill = version < SIZED_EDITION
    dirs = paths(saved.get("dirs"))

    def entry(value):
        if isinstance(value, dict) and ("dir" in value or "folder" in value):
            number = value.get("dir", value.get("folder"))
            if not isinstance(number, int) or not 0 <= number < len(dirs):
                raise Unreadable("a wad names a dir the profile doesn't list")
            dir_path = dirs[number]
            name = value.get("name", "")
            whole = os.path.join(dir_path, name) if isinstance(dir_path, str) else None
            return {
                "dir": number,
                "name": name,
                "hash": value.get("hash", ""),
                "size": sized(whole, value.get("size", 0), backfill),
                "volatile": bool(value.get("volatile", False)),
                "modified": value.get("modified", 0),
            }
        # 📦 a wad with its whole path: tdl files it under its dir the next time it saves
        return wad(value, backfill)

    base_wad = saved.get("base_wad")
    return {
        "name": saved.get("name", ""),
        "wads": [entry(value) for value in saved.get("wads") or []],
        "base_wad": None if base_wad is None else entry(base_wad),
        "uzdoom": path(saved.get("uzdoom")),
        "ini": path(saved.get("ini")),
        "host_defaults": host_settings(saved.get("host_defaults")),
        "dirs": dirs,
        "uzdoom_args": saved.get("uzdoom_args"),
        "watched": path(saved.get("watched")),
        "dismissed": paths(saved.get("dismissed")),
    }


def config(version, data):
    """A loadout's config as edition 5 saves it, and the settings it held before edition 5, which
    belong in uac.rite now."""
    saved = fields(
        data,
        ["uzdoom", "base_wad", "profile_names", "skip_splash", "host_port", "wad_dirs", "uzdoom_args"],
        {"wad_folders": "wad_dirs"},
    )
    base_wad = saved.get("base_wad")
    settings = {
        "uzdoom": path(saved.get("uzdoom")),
        "base_wad": None if base_wad is None else wad(base_wad, version < SIZED_EDITION),
        "uzdoom_args": saved.get("uzdoom_args") or "",
        "skip_splash": bool(saved.get("skip_splash", False)),
        "host_port": saved.get("host_port"),
    }
    return {
        "profile_names": list(saved.get("profile_names") or []),
        "wad_dirs": paths(saved.get("wad_dirs")),
    }, settings


# What uac.rite holds, in order, with what each is when it isn't set.
UAC_DEFAULTS = {
    "loadout": None,
    "wad_dirs": [],
    "splash_quip": "Random",
    "no_quips": False,
    "music": "Splash",
    "song": None,
    "uzdoom": None,
    "base_wad": None,
    "uzdoom_args": "",
    "skip_splash": False,
    "host_port": None,
    "pride": False,
    "ini": None,
    "theme": None,
}


def uac(data):
    """global.rite (editions 0 and 1) as uac.rite: `vault` is now the `loadout`."""
    saved = fields(data, [], {"vault": "loadout", "wad_folders": "wad_dirs"})
    upgraded = dict(UAC_DEFAULTS)
    for key in UAC_DEFAULTS:
        if key in saved:
            upgraded[key] = saved[key]
    upgraded["loadout"] = path(upgraded["loadout"])
    upgraded["song"] = path(upgraded["song"])
    upgraded["uzdoom"] = path(upgraded["uzdoom"])
    upgraded["ini"] = path(upgraded["ini"])
    upgraded["theme"] = path(upgraded["theme"])
    upgraded["wad_dirs"] = paths(upgraded["wad_dirs"])
    if upgraded["base_wad"] is not None:
        upgraded["base_wad"] = wad(upgraded["base_wad"], False)
    return upgraded


def adopt(uac_data, settings):
    """Takes the settings an old config.rite held, each only if uac.rite doesn't have it yet.
    Returns whether any was taken."""
    taken = False
    for key in ("uzdoom", "base_wad", "host_port"):
        if settings[key] is not None and uac_data.get(key) is None:
            uac_data[key] = settings[key]
            taken = True
    if settings["uzdoom_args"] and not uac_data.get("uzdoom_args"):
        uac_data["uzdoom_args"] = settings["uzdoom_args"]
        taken = True
    if settings["skip_splash"] and not uac_data.get("skip_splash"):
        uac_data["skip_splash"] = True
        taken = True
    return taken


def scroll(data):
    """A scroll of edition 0 (tdl 0.6.0), one profile's fields beside the settings, as a scroll of
    that one profile."""
    saved = fields(data, [])
    one = {key: saved.get(key) for key in ("name", "wads", "base_wad", "host_defaults")}
    one["wads"] = one["wads"] or []
    one["host_defaults"] = host_settings(one["host_defaults"])
    return {"profiles": [one], "settings": saved.get("settings")}


# --- Upgrading files on disk --------------------------------------------------------------------


class Upgrader:
    def __init__(self, dry_run):
        self.dry_run = dry_run
        self.problems = []
        self.changed = 0

    def say(self, line):
        print(("would " if self.dry_run else "") + line)

    def problem(self, file, why):
        self.problems.append("%s: %s" % (file, why))
        print("could not upgrade %s: %s" % (file, why), file=sys.stderr)

    def keep_original(self, file):
        """Keeps `file` beside where it was as <file>.legacy, over any older .legacy of it."""
        legacy = file + LEGACY_SUFFIX
        self.say("keep the original as %s" % legacy)
        if not self.dry_run:
            shutil.copy2(file, legacy)

    def write(self, file, data):
        """Writes `data` to `file` the way tdl does: to a .tmp beside it, then renamed over it."""
        self.changed += 1
        if self.dry_run:
            return
        os.makedirs(os.path.dirname(file) or ".", exist_ok=True)
        tmp = file + ".tmp"
        with open(tmp, "wb") as out:
            out.write(data)
            out.flush()
            os.fsync(out.fileno())
        os.replace(tmp, file)

    def retire(self, file):
        """Moves an old-named original aside to <file>.legacy, once its contents are upgraded."""
        self.say("keep the original as %s" % (file + LEGACY_SUFFIX))
        if not self.dry_run:
            os.replace(file, file + LEGACY_SUFFIX)

    def read(self, file):
        with open(file, "rb") as source:
            return opened(source.read())

    def upgrade(self, file, edition, convert, kind, target=None):
        """Upgrades one file to `edition` with `convert(version, data)`, written to `target` (in
        place if `None`, else the original is moved aside). Returns the converted data, `None` if
        the file is current already or couldn't be read."""
        try:
            version, data = self.read(file)
        except (OSError, Unreadable) as error:
            self.problem(file, error)
            return None
        if version >= edition and target is None:
            if version > edition:
                self.problem(file, "it is from a newer tdl than 1.0.0, so it is left as it is")
            return None
        try:
            upgraded = convert(version, data)
        except Unreadable as error:
            self.problem(file, error)
            return None
        destination = target or file
        self.say("upgrade %s %s (edition %d to %d)" % (kind, destination, version, edition))
        if target is None:
            self.keep_original(file)
        self.write(destination, sealed(edition, upgraded))
        if target is not None:
            self.retire(file)
        return upgraded


def standard_config_dir():
    """Where tdl keeps its config when no loadout is given, as the `directories` crate has it."""
    home = os.path.expanduser("~")
    if sys.platform == "win32":
        base = os.environ.get("APPDATA") or os.path.join(home, "AppData", "Roaming")
        return os.path.join(base, "eef", "tdl", "config")
    if sys.platform == "darwin":
        return os.path.join(home, "Library", "Application Support", "error.error.fruit.eef.tdl")
    base = os.environ.get("XDG_CONFIG_HOME") or os.path.join(home, ".config")
    return os.path.join(base, "tdl")


def gather_profiles(upgrader, config_dir):
    """Moves the old `profiles` dir's files into `.profiles`, where tdl 1.0.0 looks, never over a
    file that is there already."""
    old = os.path.join(config_dir, "profiles")
    new = os.path.join(config_dir, ".profiles")
    if not os.path.isdir(old):
        return
    for name in sorted(os.listdir(old)):
        source = os.path.join(old, name)
        destination = os.path.join(new, name)
        if os.path.exists(destination):
            upgrader.problem(source, "%s is there already, so it is left where it is" % destination)
            continue
        upgrader.say("move %s to %s" % (source, destination))
        if not upgrader.dry_run:
            os.makedirs(new, exist_ok=True)
            os.replace(source, destination)
    if not upgrader.dry_run and not os.listdir(old):
        os.rmdir(old)


def upgrade_loadout(upgrader, config_dir, uac_data):
    """Upgrades the config and every profile in one loadout's config dir, handing the config's old
    settings to `uac_data`. Returns whether any was taken."""
    if not os.path.isdir(config_dir):
        return False
    gather_profiles(upgrader, config_dir)
    taken = False

    def convert_config(version, data):
        nonlocal taken
        upgraded, settings = config(version, data)
        taken |= adopt(uac_data, settings)
        return upgraded

    rite = os.path.join(config_dir, "config.rite")
    doom = os.path.join(config_dir, "config.doom")
    # 🗝️ the newest first, so its settings are the ones a backup's can't replace
    if os.path.exists(rite):
        upgrader.upgrade(rite, CONFIG_EDITION, convert_config, "the config")
    elif os.path.exists(doom):
        upgrader.upgrade(doom, CONFIG_EDITION, convert_config, "the config", target=rite)
    if os.path.exists(rite + ".bak"):
        upgrader.upgrade(rite + ".bak", CONFIG_EDITION, convert_config, "the config's backup")
    if os.path.exists(rite) and os.path.exists(doom):
        upgrader.retire(doom)

    profiles = os.path.join(config_dir, ".profiles")
    names = sorted(os.listdir(profiles)) if os.path.isdir(profiles) else []
    for name in names:
        file = os.path.join(profiles, name)
        if name.endswith(".rite") or name.endswith(".rite.bak"):
            upgrader.upgrade(file, PROFILE_EDITION, profile, "the profile")
        elif name.endswith(".doom"):
            rite = file[: -len(".doom")] + ".rite"
            if os.path.exists(rite):
                # 🪦 tdl already wrote this one anew: the .doom is only a leftover
                upgrader.retire(file)
            else:
                upgrader.upgrade(file, PROFILE_EDITION, profile, "the profile", target=rite)
    return taken


def run(standard, loadouts, scrolls, dry_run):
    upgrader = Upgrader(dry_run)
    uac_path = os.path.join(standard, "uac.rite")
    global_path = os.path.join(standard, "global.rite")

    # 🌐 the global settings: uac.rite if it is there already, else global.rite upgraded
    uac_data = None
    uac_changed = False
    if os.path.exists(uac_path):
        try:
            version, data = upgrader.read(uac_path)
            if version > UAC_EDITION:
                upgrader.problem(uac_path, "it is from a newer tdl than 1.0.0, so nothing is handed to it")
            else:
                uac_data = uac(data) if version < UAC_EDITION else dict(UAC_DEFAULTS, **data)
                uac_changed = version < UAC_EDITION
        except (OSError, Unreadable) as error:
            upgrader.problem(uac_path, error)
        if os.path.exists(global_path):
            upgrader.retire(global_path)
    elif os.path.exists(global_path):
        try:
            version, data = upgrader.read(global_path)
            uac_data = uac(data)
            uac_changed = True
            upgrader.say("upgrade the global settings %s to %s (edition %d to %d)" % (global_path, uac_path, version, UAC_EDITION))
        except (OSError, Unreadable) as error:
            upgrader.problem(global_path, error)
    else:
        uac_data = dict(UAC_DEFAULTS)
    writable = uac_data is not None
    if not writable:
        uac_data = dict(UAC_DEFAULTS)

    # 📂 every loadout: the standard one, the one opened by default, and the ones given, each once
    dirs = [standard]
    remembered = uac_data.get("loadout")
    if isinstance(remembered, str) and os.path.isdir(remembered):
        dirs.append(os.path.join(remembered, "config"))
    dirs += [os.path.join(loadout, "config") for loadout in loadouts]
    seen = set()
    for config_dir in dirs:
        key = os.path.realpath(config_dir)
        if key in seen:
            continue
        seen.add(key)
        if upgrade_loadout(upgrader, config_dir, uac_data):
            uac_changed = True
            upgrader.say("hand the old settings in %s to %s" % (config_dir, uac_path))

    if writable and uac_changed:
        if os.path.exists(uac_path):
            upgrader.keep_original(uac_path)
        upgrader.write(uac_path, sealed(UAC_EDITION, uac_data))
        if os.path.exists(global_path):
            upgrader.retire(global_path)
    elif uac_changed:
        upgrader.problem(uac_path, "the old settings had nowhere to go, so they are only in the .legacy files")

    for file in scrolls:
        upgrader.upgrade(file, SCROLL_EDITION, lambda version, data: scroll(data), "the scroll")

    if upgrader.problems:
        print("%d problem(s), see above; everything else is upgraded" % len(upgrader.problems), file=sys.stderr)
        return 1
    if upgrader.changed == 0:
        print("Nothing to upgrade: every file is from tdl 1.0.0 or later")
    elif dry_run:
        print("Nothing was changed (--dry-run)")
    else:
        print("Done: tdl 1.0.0 can read every file now")
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Upgrades the files of a tdl from before 1.0.0, which tdl 1.0.0 and later "
        "don't read. Every original is kept beside its upgrade as <file>.legacy.",
    )
    parser.add_argument("--loadout", metavar="DIR", action="append", default=[],
                        help="a loadout given to tdl with --loadout (or --vault before 1.0.0), as well as "
                        "the standard one and the one opened by default; give it more than once for more")
    parser.add_argument("--scroll", metavar="FILE", action="append", default=[],
                        help="a .scroll from tdl 0.6.0 to upgrade; give it more than once for more")
    parser.add_argument("--standard", metavar="DIR", default=None,
                        help="the standard config dir, instead of the platform's (%s)" % standard_config_dir())
    parser.add_argument("--dry-run", action="store_true", help="say what would be done, and change nothing")
    args = parser.parse_args(argv)
    standard = args.standard or standard_config_dir()
    return run(standard, args.loadout, args.scroll, args.dry_run)


if __name__ == "__main__":
    sys.exit(main())
