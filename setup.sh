#!/usr/bin/env bash
# Installs what is needed to develop and test SimuDeckParser on Linux:
# system packages (linker, windowing libraries, a software Vulkan driver for the
# snapshot tests), the Rust toolchain through rustup, and the Windows
# cross-compilation target used by `./build.sh windows`.
#
# Usage: ./setup.sh [--no-cross]
#   --no-cross   skip the Windows cross-compilation target and MinGW
set -euo pipefail
cd "$(dirname "$0")"

cross=1
for arg in "$@"; do
    case "$arg" in
        --no-cross) cross=0 ;;
        -h|--help) sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown option: $arg" >&2; exit 1 ;;
    esac
done

sudo=""
if [ "$(id -u)" -ne 0 ]; then
    sudo="sudo"
fi

echo "==> Installing system packages"
if command -v dnf >/dev/null; then
    pkgs=(gcc pkgconf-pkg-config git curl zip
          libxkbcommon libxkbcommon-x11 wayland-devel libX11 libXcursor libXrandr libXi
          mesa-libEGL mesa-vulkan-drivers vulkan-loader)
    [ "$cross" -eq 1 ] && pkgs+=(mingw64-gcc mingw64-gcc-c++)
    $sudo dnf install -y "${pkgs[@]}"
elif command -v apt-get >/dev/null; then
    pkgs=(build-essential pkg-config git curl zip
          libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libx11-6 libxcursor1 libxrandr2 libxi6
          libegl1 mesa-vulkan-drivers libvulkan1)
    [ "$cross" -eq 1 ] && pkgs+=(mingw-w64)
    $sudo apt-get update
    $sudo apt-get install -y "${pkgs[@]}"
elif command -v pacman >/dev/null; then
    pkgs=(base-devel pkgconf git curl zip
          libxkbcommon libxkbcommon-x11 wayland libx11 libxcursor libxrandr libxi
          mesa vulkan-swrast vulkan-icd-loader)
    [ "$cross" -eq 1 ] && pkgs+=(mingw-w64-gcc)
    $sudo pacman -S --needed --noconfirm "${pkgs[@]}"
else
    echo "Unsupported package manager: install a C toolchain (gcc), git, curl, zip," >&2
    echo "libxkbcommon, Wayland/X11 client libraries and a Vulkan driver yourself," >&2
    echo "then run this script again with the packages already present." >&2
fi

echo "==> Installing the Rust toolchain"
fresh_rustup=0
if ! command -v rustup >/dev/null; then
    fresh_rustup=1
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    # shellcheck source=/dev/null
    . "$HOME/.cargo/env"
fi
rustup toolchain install stable --profile minimal --component rustfmt,clippy
rustup default stable
if [ "$cross" -eq 1 ]; then
    rustup target add x86_64-pc-windows-gnu
fi

echo "==> Downloading crate dependencies"
cargo fetch --locked

echo
echo "Done. Next steps:"
echo "  cargo run            # run a debug build"
echo "  cargo test           # run the tests"
echo "  ./build.sh           # build a release archive into dist/"
echo "  ./install.sh         # build and install to ~/.local"
if [ "$fresh_rustup" -eq 1 ]; then
    echo
    echo "Open a new terminal (or run '. \$HOME/.cargo/env') so cargo is on your PATH."
fi
