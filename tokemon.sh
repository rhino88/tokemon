#!/bin/bash
# Convenience wrapper to run tokemon via Docker
# Usage: ./tokemon.sh [tokemon args...]
# Examples:
#   ./tokemon.sh discover
#   ./tokemon.sh daily --since 2026-02-01 --offline
#   ./tokemon.sh monthly --json
#   ./tokemon.sh --no-cost -p claude-code

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
IMAGE="tokemon-dev"
BINARY="./target/release/tokemon"

# Check if Docker image exists
if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
    echo "[tokemon] Building Docker image..."
    docker build -t "$IMAGE" "$SCRIPT_DIR"
fi

# Check if binary exists (mount and check)
if ! docker run --rm -v "$SCRIPT_DIR":/app -w /app "$IMAGE" test -f "$BINARY" 2>/dev/null; then
    echo "[tokemon] Building tokemon binary..."
    # Export CA certs for cargo to fetch crates
    security export -t certs -f pemseq -k /Library/Keychains/System.keychain -o /tmp/tokemon_system_certs.pem 2>/dev/null
    security export -t certs -f pemseq -k /System/Library/Keychains/SystemRootCertificates.keychain -o /tmp/tokemon_root_certs.pem 2>/dev/null
    cat /tmp/tokemon_system_certs.pem /tmp/tokemon_root_certs.pem > /tmp/tokemon_ca_bundle.pem 2>/dev/null

    docker run --rm \
        -v "$SCRIPT_DIR":/app \
        -v /tmp/tokemon_ca_bundle.pem:/etc/ssl/certs/ca-certificates.crt:ro \
        -e SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
        -e CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt \
        -w /app \
        "$IMAGE" cargo build --release
fi

# Build mount arguments as an array to handle paths with spaces
DOCKER_ARGS=(
    --rm
    -v "$SCRIPT_DIR:/app"
    -v "$HOME/.claude:/root/.claude:ro"
)

# Mount pricing cache if available
if [ -d "$HOME/.cache/tokemon" ]; then
    DOCKER_ARGS+=(-v "$HOME/.cache/tokemon:/root/.cache/tokemon:ro")
fi

# Mount optional provider dirs
for dir in .codex .gemini .kimi .droid .openclaw .qwen .pi-agent; do
    if [ -d "$HOME/$dir" ]; then
        DOCKER_ARGS+=(-v "$HOME/$dir:/root/$dir:ro")
    fi
done

# Mount VSCode dirs for Cline/Roo/Kilo/Copilot
for fork in Code Cursor Windsurf VSCodium Positron; do
    vscode_dir="$HOME/Library/Application Support/$fork/User/globalStorage"
    if [ -d "$vscode_dir" ]; then
        DOCKER_ARGS+=(-v "$vscode_dir:/root/Library/Application Support/$fork/User/globalStorage:ro")
    fi
done

# Mount local share for amp/opencode
if [ -d "$HOME/.local/share" ]; then
    DOCKER_ARGS+=(-v "$HOME/.local/share:/root/.local/share:ro")
fi

DOCKER_ARGS+=(-w /app "$IMAGE" "$BINARY")

docker run "${DOCKER_ARGS[@]}" "$@"
