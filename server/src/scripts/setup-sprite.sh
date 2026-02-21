#!/usr/bin/env bash
set -euo pipefail

: "${DAEMON_RELEASE_URL:?DAEMON_RELEASE_URL is required}"
: "${NIGHTSHIFT_SERVER_URL:?NIGHTSHIFT_SERVER_URL is required}"
: "${NIGHTSHIFT_PUBLIC_URL:?NIGHTSHIFT_PUBLIC_URL is required}"
: "${NIGHTSHIFT_PROXY_PORT:?NIGHTSHIFT_PROXY_PORT is required}"

: "${SPRITE_NAME:?SPRITE_NAME is required}"

OPENCODE_VERSION="1.2.10"

NIGHTSHIFT_DIR="$HOME/.nightshift"
DAEMON_BIN="$NIGHTSHIFT_DIR/nightshift-daemon"

hostname "$SPRITE_NAME"

echo "--- installing opencode ---"
curl -fsSL https://opencode.ai/install | bash -s -- --version "$OPENCODE_VERSION"
OPENCODE_BIN="$HOME/.opencode/bin"
export PATH="$OPENCODE_BIN:$PATH"
echo "opencode installed at: $(which opencode) (version $(opencode --version))"

echo "--- downloading daemon ---"
mkdir -p "$NIGHTSHIFT_DIR"
# NOTE(victor): no -s flag -- progress output keeps the WebSocket alive (45s keepalive timeout)
curl -fSL -o "$DAEMON_BIN" "$DAEMON_RELEASE_URL"
chmod +x "$DAEMON_BIN"

echo "--- writing config ---"
cat >"$NIGHTSHIFT_DIR/config.json" <<EOF
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
SERVICE_JSON="{\"cmd\":\"bash\",\"args\":[\"-c\",\"PATH=$OPENCODE_BIN:\$PATH exec $DAEMON_BIN daemon\"],\"http_port\":$NIGHTSHIFT_PROXY_PORT}"
sprite-env curl -X PUT /v1/services/nightshift -d "$SERVICE_JSON"

echo "--- done ---"
