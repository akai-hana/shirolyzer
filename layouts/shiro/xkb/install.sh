#!/bin/sh
# Apply this layout to the current X display.
#
# Usage:
#   install.sh [SYMBOLS_FILE] [NAME]
#   install.sh [NAME]
#
# With no arguments, the symbols file next to this script is installed.
set -e

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
xkb_dir="${XDG_CONFIG_HOME:-$HOME/.config}/xkb"
display="${DISPLAY:-:0}"

if [ "$#" -ge 2 ]; then
    symbols_file="$1"
    name="$2"
elif [ "$#" -ge 1 ]; then
    if [ -f "$1" ]; then
        symbols_file="$1"
    else
        symbols_file="$repo_dir/$1"
    fi
else
    symbols_file=""
    for f in "$repo_dir"/*; do
        [ -f "$f" ] || continue
        [ "$(basename -- "$f")" = "install.sh" ] && continue
        symbols_file="$f"
        break
    done
fi

if [ -z "$symbols_file" ] || [ ! -f "$symbols_file" ]; then
    echo "install.sh: symbols file not found" >&2
    exit 1
fi

name=$(basename -- "$symbols_file")
mkdir -p "$xkb_dir/symbols"
cp "$symbols_file" "$xkb_dir/symbols/$name"

setxkbmap -layout "$name" -print | xkbcomp "-I$xkb_dir" - "$display"
