#!/usr/bin/bash

FLAGS=()
FS_ARGS=()
FILES=()

while [[ $# -gt 0 ]]; do
    if [[ "$1" == -* || "$1" == +* ]]; then
        FLAGS+=("$1")
        if [[ "$1" == "-config" || "$1" == "-iwad" ]]; then
            FLAGS+=(@@ "$2" @@)  
            shift
        elif [[ "$1" == "-savedir" ]]; then
            FLAGS+=("$2")
            if [[ "$2" == /* ]]; then
                FS_ARGS+=(--filesystem="$2")
            else
                FS_ARGS+=(--filesystem=/"$2")
            fi
            shift
        elif [[ "$1" != "-file" && "$1" != "-deathmatch" ]]; then
            FLAGS+=("$2")
            shift
        fi
    elif [[ -d "$1" ]]; then
        # a mod folder: --file-forwarding only forwards files ("Invalid fd passed"), so the folder
        # is let into the sandbox for this run, read-only, and passed by its own path
        dir="$(readlink -f "$1")"
        FS_ARGS+=(--filesystem="$dir:ro")
        FILES+=("dir:$dir")
    else
        FILES+=("file:$1")
    fi
    shift
done

CMD=(flatpak run --file-forwarding "${FS_ARGS[@]}" org.zdoom.UZDoom "${FLAGS[@]}")

# in load order, each run of files between @@ marks so flatpak forwards them, folders outside them
forwarding=0
for f in "${FILES[@]}"; do
    if [[ "$f" == file:* ]]; then
        (( forwarding )) || { CMD+=(@@); forwarding=1; }
        CMD+=("${f#file:}")
    else
        (( forwarding )) && { CMD+=(@@); forwarding=0; }
        CMD+=("${f#dir:}")
    fi
done
(( forwarding )) && CMD+=(@@)

echo "${CMD[@]}"
"${CMD[@]}"
