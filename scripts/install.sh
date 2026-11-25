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

cd "$root"
cargo build --release --locked
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
