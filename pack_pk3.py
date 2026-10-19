#!/usr/bin/env python3
"""Pack mod folders into .pk3s (zips with another extension), the way UZDoom loads a folder.

usage: python3 pack_pk3.py FOLDER... [--into DIR]
       python3 pack_pk3.py FOLDER --out FILE

Each folder becomes <folder name>.pk3 (a folder already named like nacht.pk3 keeps its name), with
the folder's contents at the root of the pk3, in DIR (default: the working directory). --out names
the pk3 of a single folder instead. tdl runs this to host a profile that holds a folder, since a
joiner can only download files.

Dependency-free Python 3.8+. The pk3 is reproducible: files go in sorted order with a fixed
timestamp, so the same folder always packs to the same bytes (and hashes the same). Editor
leftovers (Ultimate Doom Builder's .dbs and .backup files, OS junk) and dot files are left out, as
the projects' own publish.py scripts do.
"""

import argparse
import os
import re
import sys
import zipfile

# 1980-01-01, the earliest time a zip can hold
EPOCH = (1980, 1, 1, 0, 0, 0)

SKIPPED_NAMES = {".DS_Store", "Thumbs.db", "desktop.ini"}
SKIPPED_SUFFIXES = (".dbs", ".bak", ".tmp", "~")
# map01.wad.backup1, map01.wad.autosave1 and friends from Ultimate Doom Builder
SKIPPED_PATTERN = re.compile(r"\.wad\.\w+$", re.IGNORECASE)


def skipped(name):
    return (name in SKIPPED_NAMES or name.startswith(".") or name.endswith(SKIPPED_SUFFIXES)
            or SKIPPED_PATTERN.search(name) is not None)


def gather(root):
    """Every file under root to pack, as (path on disk, path in the pk3), sorted by pk3 path."""
    files = []
    for folder, dirs, names in os.walk(root):
        dirs[:] = sorted(d for d in dirs if not d.startswith("."))
        for name in names:
            if skipped(name):
                continue
            path = os.path.join(folder, name)
            files.append((path, os.path.relpath(path, root).replace(os.sep, "/")))
    return sorted(files, key=lambda f: f[1].lower())


def pack(files, target):
    """Write the pk3 next to target and rename it into place, so a half-written pk3 never
    replaces a good one."""
    partial = target + ".tmp"
    with zipfile.ZipFile(partial, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as pk3:
        for path, inside in files:
            info = zipfile.ZipInfo(inside, EPOCH)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            with open(path, "rb") as f:
                pk3.writestr(info, f.read())
    with zipfile.ZipFile(partial) as pk3:
        bad = pk3.testzip()
        if bad is not None:
            os.remove(partial)
            sys.exit(f"{partial} is corrupt at {bad}")
    os.replace(partial, target)


def pk3_name(folder):
    name = os.path.basename(os.path.normpath(folder))
    return name if name.lower().endswith(".pk3") else name + ".pk3"


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("folders", nargs="+", metavar="FOLDER", help="a mod folder to pack")
    where = parser.add_mutually_exclusive_group()
    where.add_argument("--into", metavar="DIR", default=".",
                       help="where the pk3s go (default: the working directory)")
    where.add_argument("--out", metavar="FILE", help="the pk3 to write, for a single folder")
    options = parser.parse_args()

    if options.out and len(options.folders) > 1:
        parser.error("--out takes a single folder; use --into for several")
    for folder in options.folders:
        if not os.path.isdir(folder):
            sys.exit(f"{folder} is not a folder")

    for folder in options.folders:
        target = options.out or os.path.join(options.into, pk3_name(folder))
        os.makedirs(os.path.dirname(os.path.abspath(target)), exist_ok=True)
        files = gather(folder)
        pack(files, target)
        size = os.path.getsize(target) / (1024 * 1024)
        print(f"{target}: {len(files)} files, {size:.1f} MB")


if __name__ == "__main__":
    main()
