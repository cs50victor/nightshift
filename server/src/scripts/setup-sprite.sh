#!/usr/bin/env bash
set -euo pipefail

: "${DAEMON_RELEASE_URL:?DAEMON_RELEASE_URL is required}"
: "${NIGHTSHIFT_SERVER_URL:?NIGHTSHIFT_SERVER_URL is required}"
: "${NIGHTSHIFT_PUBLIC_URL:?NIGHTSHIFT_PUBLIC_URL is required}"
: "${NIGHTSHIFT_PROXY_PORT:?NIGHTSHIFT_PROXY_PORT is required}"

NIGHTSHIFT_DIR="$HOME/.nightshift"
DAEMON_BIN="$NIGHTSHIFT_DIR/nightshift-daemon"

source /etc/profile.d/languages_env || echo "skipping languages_env (not on sprite)"

echo "--- installing opencode ---"
bun install -g opencode-ai@1.2.1

echo "--- downloading daemon ---"
mkdir -p "$NIGHTSHIFT_DIR"
curl -fsSL -o "$DAEMON_BIN" "$DAEMON_RELEASE_URL"
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
sprite-env curl -X PUT /v1/services/nightshift \
  -d "{\"cmd\":\"$DAEMON_BIN\",\"args\":[\"daemon\"],\"env\":{\"BUN_INSTALL\":\"${BUN_INSTALL:-}\"}}"

echo "--- done ---"
