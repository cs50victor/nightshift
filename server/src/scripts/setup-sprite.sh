#!/usr/bin/env bash
set -euo pipefail

: "${DAEMON_RELEASE_URL:?DAEMON_RELEASE_URL is required}"
: "${NIGHTSHIFT_SERVER_URL:?NIGHTSHIFT_SERVER_URL is required}"
: "${NIGHTSHIFT_PUBLIC_URL:?NIGHTSHIFT_PUBLIC_URL is required}"
: "${NIGHTSHIFT_PROXY_PORT:?NIGHTSHIFT_PROXY_PORT is required}"

NIGHTSHIFT_DIR="$HOME/.nightshift"
DAEMON_BIN="$NIGHTSHIFT_DIR/nightshift-daemon"

echo "--- installing opencode ---"
bun install -g opencode-ai@1.2.1

echo "--- downloading daemon ---"
mkdir -p "$NIGHTSHIFT_DIR"
curl -fsSL "$DAEMON_RELEASE_URL" | tar -xz -C "$NIGHTSHIFT_DIR"
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
curl -sf -X PUT http://localhost/v1/services/nightshift \
  -H 'Content-Type: application/json' \
  -d "{\"cmd\":\"$DAEMON_BIN\",\"args\":[\"daemon\"]}"

echo "--- done ---"
