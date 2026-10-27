#!/bin/sh
set -eu

REPO="GOFNY/minelab-linux-installer"
NAME="minelab-linux-installer"
DEST="$HOME/.local/bin"
ARCHIVE="$NAME-linux-x86_64.tar.gz"

if [ "$(uname -m)" != "x86_64" ]; then
  echo "Unsupported architecture: $(uname -m)" >&2
  exit 1
fi

mkdir -p "$DEST"

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT HUP INT TERM

curl -fsSL \
  "https://github.com/$REPO/releases/latest/download/$ARCHIVE" \
  -o "$TMP_DIR/$ARCHIVE"

tar -xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR"

install -m 755 "$TMP_DIR/$NAME-linux-x86_64" "$DEST/$NAME"

exec "$DEST/$NAME" "$PWD" "$@" < /dev/tty
