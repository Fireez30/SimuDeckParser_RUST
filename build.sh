#!/usr/bin/env bash
# Builds SimuDeckParser in release mode and packages it into dist/.
#
# Usage: ./build.sh [linux|windows|all]   (default: linux)
#   linux     native build      -> dist/simu_deck_parser-<version>-linux-<arch>.tar.gz
#   windows   MinGW cross build -> dist/simu_deck_parser-<version>-windows-x86_64.zip
#   all       both
set -euo pipefail
cd "$(dirname "$0")"

name="simu_deck_parser"
version="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
what="${1:-linux}"

# Files shipped next to the executable (the fonts are embedded, OFL.txt is their licence).
extras=(README.md LICENSE fonts/OFL.txt)

stage() {
    local dir="$1" exe="$2"
    rm -rf "$dir"
    mkdir -p "$dir"
    cp "$exe" "$dir/"
    cp "${extras[@]}" "$dir/"
}

build_linux() {
    echo "==> Building for Linux"
    cargo build --release --locked
    local pkg="$name-$version-linux-$(uname -m)"
    stage "dist/$pkg" "target/release/$name"
    tar -C dist -czf "dist/$pkg.tar.gz" "$pkg"
    echo "    dist/$pkg.tar.gz"
}

build_windows() {
    echo "==> Building for Windows (x86_64-pc-windows-gnu)"
    cargo build --release --locked --target x86_64-pc-windows-gnu
    local pkg="$name-$version-windows-x86_64"
    stage "dist/$pkg" "target/x86_64-pc-windows-gnu/release/$name.exe"
    rm -f "dist/$pkg.zip"
    (cd dist && zip -qr "$pkg.zip" "$pkg")
    echo "    dist/$pkg.zip"
}

case "$what" in
    linux) build_linux ;;
    windows) build_windows ;;
    all) build_linux; build_windows ;;
    -h|--help) sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//' ;;
    *) echo "Unknown target: $what (expected linux, windows or all)" >&2; exit 1 ;;
esac
