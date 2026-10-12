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
    else
        FILES+=("$1")
    fi
    shift
done

CMD=(flatpak run --file-forwarding "${FS_ARGS[@]}" org.zdoom.UZDoom "${FLAGS[@]}")

if [ ${#FILES[@]} -gt 0 ]; then
    CMD+=(@@ "${FILES[@]}" @@)
fi

echo "${CMD[@]}"
"${CMD[@]}"
