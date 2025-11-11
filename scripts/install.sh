#!/usr/bin/env bash
set -euo pipefail

REPO="arian-shamaei/sgram-tui"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
BIN_NAME="sgram-tui"
VERSION="${SGRAM_VERSION:-latest}"

need_cmd() { command -v "$1" >/dev/null 2>&1 || { echo "Error: '$1' is required" >&2; exit 1; }; }

need_cmd uname
need_cmd curl

platform="$(uname -s)"
arch="$(uname -m)"

case "$platform" in
  Darwin) plat="darwin" ;;
  Linux)  plat="linux" ;;
  *) echo "Unsupported OS: $platform" >&2; exit 1 ;;
esac

case "$arch" in
  x86_64|amd64) cpu="amd64" ;;
  arm64|aarch64) cpu="arm64" ;;
  *) echo "Unsupported CPU arch: $arch" >&2; exit 1 ;;
esac

target="${plat}-${cpu}"

# Resolve release API URL
if [ "$VERSION" = "latest" ]; then
  api_url="https://api.github.com/repos/${REPO}/releases/latest"
else
  # Accept forms like v0.1.1 or 0.1.1
  tag="$VERSION"
  case "$tag" in v*) ;; *) tag="v$tag" ;; esac
  api_url="https://api.github.com/repos/${REPO}/releases/tags/${tag}"
fi

echo "Determined target: $target"
echo "Querying release metadata: $api_url"

json=$(curl -fsSL "$api_url")

# Extract matching asset URL without jq
asset_url=$(printf "%s" "$json" | grep -E '"browser_download_url"' | sed -E 's/\s*"browser_download_url"\s*:\s*"([^"]+)".*/\1/' | grep "/${BIN_NAME}-${plat}-${cpu}$" || true)

if [ -z "$asset_url" ]; then
  echo "No prebuilt binary found for $target in this release." >&2
  echo "You can build from source: cargo install --locked --path ." >&2
  exit 1
fi

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT
outfile="$tmpdir/${BIN_NAME}-${target}"

echo "Downloading: $asset_url"
curl -fL --progress-bar -o "$outfile" "$asset_url"
chmod +x "$outfile"

mkdir -p "$INSTALL_DIR"
dest="$INSTALL_DIR/$BIN_NAME"

if [ -w "$INSTALL_DIR" ]; then
  mv "$outfile" "$dest"
else
  echo "Elevated permissions needed to write to $INSTALL_DIR"
  sudo mv "$outfile" "$dest"
fi

echo "Installed $BIN_NAME to $dest"
echo "Version: $($dest --version || true)"
echo "Done."

