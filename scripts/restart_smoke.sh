#!/usr/bin/env bash
# Isolated persistence/restart regression suite. Uses temporary galaxy folders;
# never truncates PostgreSQL tables or stops a running playtest server.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo test -p server persistence::store::tests -- --nocapture
cargo test -p server game_loop::durability::tests -- --nocapture
