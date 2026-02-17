#!/usr/bin/env bash
set -euo pipefail

: "${DAEMON_RELEASE_URL:?DAEMON_RELEASE_URL is required}"
: "${NIGHTSHIFT_SERVER_URL:?NIGHTSHIFT_SERVER_URL is required}"
: "${NIGHTSHIFT_PUBLIC_URL:?NIGHTSHIFT_PUBLIC_URL is required}"
: "${NIGHTSHIFT_PROXY_PORT:?NIGHTSHIFT_PROXY_PORT is required}"

: "${SPRITE_NAME:?SPRITE_NAME is required}"

NIGHTSHIFT_DIR="$HOME/.nightshift"
DAEMON_BIN="$NIGHTSHIFT_DIR/nightshift-daemon"

hostname "$SPRITE_NAME"

echo "--- installing opencode ---"
bun install -g opencode-ai@1.2.1

# NOTE(victor): bun install -g puts binaries in a dir not on PATH.
# Ask bun where its global bin is and derive BUN_INSTALL from that.
GLOBAL_BIN="$(bun pm bin -g)"
BUN_INSTALL="$(dirname "$GLOBAL_BIN")"
export BUN_INSTALL
echo "BUN_INSTALL=$BUN_INSTALL (global bin: $GLOBAL_BIN)"

echo "--- downloading daemon ---"
mkdir -p "$NIGHTSHIFT_DIR"
# NOTE(victor): no -s flag -- progress output keeps the WebSocket alive (45s keepalive timeout)
curl -fSL -o "$DAEMON_BIN" "$DAEMON_RELEASE_URL"
chmod +x "$DAEMON_BIN"

echo "--- writing config ---"
cat > "$NIGHTSHIFT_DIR/config.json" <<EOF
{
  "version": 1,
  "serverUrl": "$NIGHTSHIFT_SERVER_URL",
  "publicUrl": "$NIGHTSHIFT_PUBLIC_URL",
  "proxyPort": $NIGHTSHIFT_PROXY_PORT
}
EOF

echo "--- starting daemon service ---"
# NOTE(victor): service env object isn't reliably applied (confirmed by browserd project).
# Inline PATH via bash -c so the daemon can find opencode.
# NOTE(victor): http_port tells the sprites platform to route the sprite's public URL to this port.
SERVICE_JSON="{\"cmd\":\"bash\",\"args\":[\"-c\",\"PATH=$GLOBAL_BIN:\$PATH exec $DAEMON_BIN daemon\"],\"http_port\":$NIGHTSHIFT_PROXY_PORT}"
sprite-env curl -X PUT /v1/services/nightshift -d "$SERVICE_JSON"

echo "--- done ---"
