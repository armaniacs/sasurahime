#!/bin/sh
# Sample sasurahime plugin: an external `sasurahime-*` executable that extends
# sasurahime with one target. See docs/HOWTO-ADD-target.md for the protocol.
#
# Install:
#   cp examples/sample-plugin.sh /usr/local/bin/sasurahime-sample
#   chmod +x /usr/local/bin/sasurahime-sample
# Then `sasurahime scan` shows a `sample` row and `sasurahime targets` lists
# it with a (plugin) marker.

# Target this plugin manages (adjust to your tool).
CACHE_DIR="${SASURAHIME_SAMPLE_CACHE_DIR:-$HOME/.cache/sample-plugin}"

case "$1" in
  scan)
    # The protocol: `scan --json` prints {"status": ..., "bytes": <n>} and
    # exits 0. Status is one of pruneable | clean | notfound | unavailable.
    if [ ! -d "$CACHE_DIR" ]; then
      echo '{"status": "notfound", "bytes": 0}'
      exit 0
    fi
    # Apparent size: blocks * 512.
    bytes=$(du -sk "$CACHE_DIR" 2>/dev/null | cut -f1)
    bytes=$((bytes * 1024))
    if [ "$bytes" -gt 0 ]; then
      echo "{\"status\": \"pruneable\", \"bytes\": $bytes}"
    else
      echo '{"status": "clean", "bytes": 0}'
    fi
    exit 0
    ;;
  clean)
    # The protocol: `clean [--dry-run]` performs (or plans) the clean.
    # Exit 0 on success; any non-zero exit is isolated and reported.
    if [ "$2" = "--dry-run" ]; then
      echo "[dry-run] would remove: $CACHE_DIR"
      exit 0
    fi
    rm -rf "$CACHE_DIR"
    exit 0
    ;;
  *)
    echo "usage: sasurahime-sample scan --json | clean [--dry-run]" >&2
    exit 2
    ;;
esac
