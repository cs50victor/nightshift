set dotenv-load
set script-interpreter := ['bash', '-euo', 'pipefail']

alias b := build
alias t := test
alias l := lint
alias f := fmt
alias d := dev

default:
    @just --list --unsorted

# --- dev ---

[doc('Run daemon + server + ui concurrently')]
[group('dev')]
dev:
    bunx concurrently --kill-others --names daemon,server,ui "just dev-daemon" "just dev-server" "just dev-ui"

[doc('Run daemon in dev mode')]
[group('dev')]
dev-daemon:
    NIGHTSHIFT_NO_UPDATE=1 cargo run --manifest-path daemon/Cargo.toml -- daemon

[doc('Run API server (bun)')]
[group('dev')]
dev-server:
    cd server && bun run start

[doc('Run Next.js frontend with Turbopack')]
[group('dev')]
dev-ui:
    cd ui && bun run dev

# --- build ---

[doc('Build all components')]
[group('build')]
build: build-daemon build-ui

[doc('Build daemon (debug)')]
[group('build')]
build-daemon:
    cargo build --manifest-path daemon/Cargo.toml

[doc('Build daemon (release)')]
[group('build')]
build-daemon-release:
    cargo build --release --manifest-path daemon/Cargo.toml

[doc('Build UI (Next.js)')]
[group('build')]
build-ui:
    cd ui && bun run build

# --- test ---

[doc('Run all tests')]
[group('test')]
test: test-daemon test-server

[doc('Run daemon tests')]
[group('test')]
test-daemon:
    cargo test --manifest-path daemon/Cargo.toml

[doc('Run server tests')]
[group('test')]
test-server:
    cd server && bun test

[doc('Test sprite setup script in Docker')]
[group('test')]
test-sprite-setup:
    docker run --rm \
      -v "$(pwd)/server/src/scripts/setup-sprite.sh:/setup.sh:ro" \
      -e DAEMON_RELEASE_URL="https://github.com/cs50victor/nightshift/releases/latest/download/nightshift-daemon-x86_64-unknown-linux-gnu" \
      -e NIGHTSHIFT_SERVER_URL="http://localhost:4001" \
      -e NIGHTSHIFT_PUBLIC_URL="http://localhost:8080" \
      -e NIGHTSHIFT_PROXY_PORT="8080" \
      oven/bun:latest bash -c 'apt-get update -qq && apt-get install -y -qq curl > /dev/null 2>&1 && bash /setup.sh'

# --- lint ---

[doc('Run all linters + typecheck')]
[group('lint')]
lint: lint-daemon lint-ui typecheck

[doc('Run clippy on daemon')]
[group('lint')]
lint-daemon:
    cargo clippy --manifest-path daemon/Cargo.toml -- -D warnings

[doc('Run biome CI on UI')]
[group('lint')]
lint-ui:
    cd ui && bunx @biomejs/biome ci .

[doc('Typecheck UI')]
[group('lint')]
typecheck:
    cd ui && bun run typecheck

[doc('Format all code')]
[group('lint')]
fmt: fmt-daemon fmt-ui

[doc('Format daemon code')]
[group('lint')]
fmt-daemon:
    cargo fmt --manifest-path daemon/Cargo.toml

[doc('Format UI code')]
[group('lint')]
fmt-ui:
    cd ui && bunx @biomejs/biome format --write .

# --- setup ---

[doc('Install all dependencies')]
[group('setup')]
[script]
setup:
    echo "Installing server dependencies..."
    cd server && bun install
    echo "Installing UI dependencies..."
    cd ui && bun install
    echo "Done."

# --- other ---

[doc('Remove build artifacts')]
[script]
clean:
    echo "Cleaning build artifacts..."
    rm -rf daemon/target
    rm -rf ui/.next
    rm -rf ui/node_modules
    rm -rf server/node_modules
    echo "Done."
