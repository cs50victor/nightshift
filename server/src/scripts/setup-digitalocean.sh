#!/usr/bin/env bash
set -euo pipefail
exec > /var/log/nightshift-setup.log 2>&1

export NIGHTSHIFT_SERVER_URL="${NIGHTSHIFT_SERVER_URL}"

DAEMON_RELEASE_URL="${DAEMON_RELEASE_URL:-https://github.com/cs50victor/nightshift/releases/latest/download/nightshift-daemon-x86_64-unknown-linux-gnu}"

echo "--- discovering public IP ---"
PUBLIC_IP="${NIGHTSHIFT_PUBLIC_IP:-$(curl -s http://169.254.169.254/metadata/v1/interfaces/public/0/ipv4/address)}"
HOSTNAME="${NIGHTSHIFT_HOSTNAME:-$(curl -s http://169.254.169.254/metadata/v1/hostname)}"
echo "IP: $PUBLIC_IP, hostname: $HOSTNAME"

echo "--- installing opencode ---"
curl -fsSL https://opencode.ai/install | bash
export BUN_INSTALL="$HOME/.bun"
export PATH="$HOME/.opencode/bin:$BUN_INSTALL/bin:$PATH"

echo "--- downloading daemon ---"
NIGHTSHIFT_DIR="$HOME/.nightshift"
mkdir -p "$NIGHTSHIFT_DIR"
curl -fSL -o "$NIGHTSHIFT_DIR/nightshift-daemon" "$DAEMON_RELEASE_URL"
chmod +x "$NIGHTSHIFT_DIR/nightshift-daemon"

echo "--- writing config ---"
cat > "$NIGHTSHIFT_DIR/config.json" <<CONF
{
  "version": 1,
  "serverUrl": "${NIGHTSHIFT_SERVER_URL}",
  "publicUrl": "http://${PUBLIC_IP}:8080",
  "proxyPort": 8080
}
CONF

echo "--- starting daemon ---"
nohup "$NIGHTSHIFT_DIR/nightshift-daemon" daemon > "$NIGHTSHIFT_DIR/daemon.log" 2>&1 &

touch /var/run/nightshift-ready
echo "--- done ---"
