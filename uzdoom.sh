#!/usr/bin/bash

FLAGS=()
FILES=()

while [[ $# -gt 0 ]]; do
    if [[ "$1" == -* ]]; then
        FLAGS+=("$1")
        if [[ "$1" != "-file" && "$1" != "-deathmatch" && $# -gt 1 && "$2" != -* && ! "$2" =~ \.(pk3|wad|ipk3|deh)$ ]]; then
            FLAGS+=("$2")
            shift
        fi
    else
        FILES+=("$1")
    fi
    shift
done

CMD=(flatpak run --file-forwarding org.zdoom.UZDoom "${FLAGS[@]}")

if [ ${#FILES[@]} -gt 0 ]; then
    CMD+=(@@ "${FILES[@]}" @@)
fi

"${CMD[@]}"
