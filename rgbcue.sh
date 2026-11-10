#!/bin/sh
# Small launcher so the app runs like a normal program:  ./rgbcue
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec python3 "$here/main.py" "$@"
