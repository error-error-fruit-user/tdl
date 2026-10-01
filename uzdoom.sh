#!/usr/bin/bash

FLAGS=()
FS_ARGS=()
FILES=()

while [[ $# -gt 0 ]]; do
    if [[ "$1" == -* ]]; then
        FLAGS+=("$1")
        if [[ "$1" == "-savedir" ]]; then
            FS_ARGS+=("--filesystem=$2")
            FLAGS+=("$2")
            shift
        elif [[ "$1" != "-file" && "$1" != "-deathmatch" && $# -gt 1 && "$2" != -* && ! "$2" =~ \.(pk3|wad|ipk3|deh)$ ]]; then
            FLAGS+=("$2")
            shift
        fi
    else
        FILES+=("$1")
    fi
    shift
done

CMD=(flatpak run --file-forwarding "${FS_ARGS[@]}" org.zdoom.UZDoom "${FLAGS[@]}")

if [ ${#FILES[@]} -gt 0 ]; then
    CMD+=(@@ "${FILES[@]}" @@)
fi

"${CMD[@]}"
