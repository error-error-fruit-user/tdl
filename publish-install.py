#!/usr/bin/env python3
"""Publish tdl, then install it with the tdl-install just published.

usage: python3 publish-install.py [tdl-install options]

Runs publish.py (published/releases/), then published/releases/tdl-install (the AppImage, or the
.exe on Windows), which copies the tdl beside it, puts UZDoom and Freedoom 2 next to it and asks
where to install unless given --mode. Every option goes to tdl-install (--help lists them).

Dependency-free Python 3.8+.
"""

import os
import platform
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
RELEASES = os.path.join(HERE, "published", "releases")


def run(command):
    print("+", " ".join(command), flush=True)
    subprocess.run(command, check=True, cwd=HERE)


def main():
    run([sys.executable, "publish.py"])
    installer = "tdl-install.exe" if platform.system() == "Windows" else "tdl-install.AppImage"
    run([os.path.join(RELEASES, installer), *sys.argv[1:]])


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        sys.exit(f"publish-install failed: {error}")
