#!/bin/sh
# MemoryWhale one-line installer — downloads prebuilt CLI binaries, no Rust needed.
#
#   curl -fsSL https://raw.githubusercontent.com/wuisabel-gif/MemWhale/main/install.sh | sh
#
# Installs mw, mw-serve, mw-run, mw-remember, mw-view, mw-recover, mw-screenshot, mw-mcp
# into ~/.local/bin (override with PREFIX=/usr/local, needs write access).
set -eu

REPO="wuisabel-gif/MemWhale"
PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"

os="$(uname -s)"
arch="$(uname -m)"
case "$os-$arch" in
  Linux-x86_64)            target="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64|Linux-arm64) target="aarch64-unknown-linux-gnu" ;;
  Darwin-x86_64)           target="x86_64-apple-darwin" ;;
  Darwin-arm64)            target="aarch64-apple-darwin" ;;
  *) echo "unsupported platform: $os-$arch" >&2
     echo "build from source instead: cargo install --git https://github.com/$REPO mw-cli" >&2
     exit 1 ;;
esac

echo "==> Finding latest MemoryWhale release…"
tag="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
       | grep '"tag_name"' | head -1 | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')"
[ -n "$tag" ] || { echo "could not find a release; is one published yet?" >&2; exit 1; }
ver="${tag#v}"

asset="memorywhale-${ver}-${target}.tar.gz"
url="https://github.com/$REPO/releases/download/$tag/$asset"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "==> Downloading $asset ($tag)…"
curl -fSL "$url" -o "$tmp/$asset"
tar xzf "$tmp/$asset" -C "$tmp"

mkdir -p "$BIN_DIR"
cp "$tmp/memorywhale-${ver}-${target}/bin/"* "$BIN_DIR/"
chmod +x "$BIN_DIR/"mw*

echo "==> Installed to $BIN_DIR"
case ":$PATH:" in
  *":$BIN_DIR:"*) : ;;
  *) echo "   NOTE: $BIN_DIR is not on your PATH. Add this to your shell startup file:"
     echo "         export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac
echo "==> Done. Run 'mw' to get started, or 'mw-serve' for the web dashboard."
