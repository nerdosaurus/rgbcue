#!/usr/bin/env bash
# Build and install rgbcue outside the source tree.
#
# Defaults: ~/.local/bin/rgbcue and a desktop entry in
# ~/.local/share/applications. Override the prefix with PREFIX=/somewhere.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
prefix="${PREFIX:-$HOME/.local}"
bindir="$prefix/bin"
appdir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

build=1
for arg in "$@"; do
    case "$arg" in
        --no-build) build=0 ;;
        *) printf 'Unknown option: %s\n' "$arg" >&2; exit 2 ;;
    esac
done

cd "$root"
if (( build )); then
    cargo build --release --locked
fi
if [[ ! -x target/release/rgbcue ]]; then
    printf 'No release binary at %s; run without --no-build first.\n' "$root/target/release/rgbcue" >&2
    exit 1
fi
install -Dm755 target/release/rgbcue "$bindir/rgbcue"

mkdir -p "$appdir"
cat > "$appdir/rgbcue.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=RGBCUE
Comment=Configure Corsair keyboards and mice via OpenLinkHub
Exec=$bindir/rgbcue
Terminal=false
Categories=Settings;HardwareSettings;
Icon=input-gaming
StartupWMClass=rgbcue
EOF

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$appdir" >/dev/null 2>&1 || true
fi

printf 'Installed %s\n' "$bindir/rgbcue"
printf 'Desktop entry: %s\n' "$appdir/rgbcue.desktop"
case ":$PATH:" in
    *":$bindir:"*) ;;
    *) printf 'Note: %s is not on PATH; launch from the application menu or add it.\n' "$bindir" ;;
esac
