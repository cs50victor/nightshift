#!/usr/bin/env bash
set -euo pipefail

# Verify a git tag version matches daemon/Cargo.toml version.
# Usage:
#   check-version.sh <tag>        # e.g. check-version.sh v0.1.0
#   check-version.sh              # uses GITHUB_REF_NAME if set

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CARGO_TOML="$REPO_ROOT/daemon/Cargo.toml"

TAG="${1:-${GITHUB_REF_NAME:-}}"

if [ -z "$TAG" ]; then
  echo "error: no tag provided. Pass as argument or set GITHUB_REF_NAME." >&2
  exit 1
fi

TAG_VERSION="${TAG#v}"
CARGO_VERSION=$(grep '^version' "$CARGO_TOML" | head -1 | sed 's/.*"\(.*\)"/\1/')

if [ -z "$CARGO_VERSION" ]; then
  echo "error: could not read version from $CARGO_TOML" >&2
  exit 1
fi

if [ "$TAG_VERSION" != "$CARGO_VERSION" ]; then
  echo "error: tag version ($TAG_VERSION) != daemon/Cargo.toml version ($CARGO_VERSION)" >&2
  echo "Bump daemon/Cargo.toml to $TAG_VERSION before tagging." >&2
  exit 1
fi

echo "ok: versions match ($TAG_VERSION)"
