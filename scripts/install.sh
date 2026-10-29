#!/bin/sh
set -eu

REPO="GOFNY/minelab-linux-installer"
NAME="minelab-linux-installer"
DEST="$HOME/.local/bin"

case "$(uname -m)" in
  x86_64) TARGET="x86_64" ;;
  aarch64|arm64) TARGET="aarch64" ;;
  *)
    echo "Unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

ARCHIVE="$NAME-linux-$TARGET.tar.gz"

mkdir -p "$DEST"

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT HUP INT TERM

curl -fsSL \
  "https://github.com/$REPO/releases/latest/download/$ARCHIVE" \
  -o "$TMP_DIR/$ARCHIVE"

tar -xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR"

install -m 755 "$TMP_DIR/$NAME-linux-$TARGET" "$DEST/$NAME"

exec "$DEST/$NAME" "$PWD" "$@" < /dev/tty
