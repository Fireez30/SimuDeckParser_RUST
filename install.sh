#!/usr/bin/env bash
# Builds SimuDeckParser in release mode and installs it for the current user:
# the executable in $PREFIX/bin and a desktop entry so it shows in the app menu.
#
# Usage: ./install.sh [--uninstall]
#   PREFIX=/usr/local ./install.sh   install system-wide (uses sudo when needed)
#   default PREFIX is ~/.local
set -euo pipefail
cd "$(dirname "$0")"

name="simu_deck_parser"
prefix="${PREFIX:-$HOME/.local}"
bin="$prefix/bin/$name"
desktop="$prefix/share/applications/simu-deck-parser.desktop"

sudo=""
mkdir -p "$prefix" 2>/dev/null || true
if [ ! -w "$prefix" ] && [ "$(id -u)" -ne 0 ]; then
    sudo="sudo"
fi

if [ "${1:-}" = "--uninstall" ]; then
    $sudo rm -fv "$bin" "$desktop"
    exit 0
elif [ -n "${1:-}" ]; then
    sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'
    exit 1
fi

echo "==> Building"
cargo build --release --locked

echo "==> Installing to $prefix"
$sudo install -Dm755 "target/release/$name" "$bin"
tmp="$(mktemp)"
cat > "$tmp" <<EOF
[Desktop Entry]
Type=Application
Name=SimuDeckParser
Comment=Browse cards and build decks for the Weiss Schwarz Simulator
Exec=$bin
Terminal=false
Categories=Game;CardGame;
EOF
$sudo install -Dm644 "$tmp" "$desktop"
rm -f "$tmp"
command -v update-desktop-database >/dev/null && $sudo update-desktop-database -q "$prefix/share/applications" || true

echo "    $bin"
echo "    $desktop"
case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *) echo "Note: $prefix/bin is not on your PATH; add it to run '$name' from a terminal." ;;
esac
