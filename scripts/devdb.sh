#!/usr/bin/env bash
# Project-local Postgres dev cluster — isolated, trust-auth, on its own port.
#
# This does NOT touch your system Postgres. It creates a throwaway cluster under
# ./.devdb (gitignored) so the server's real sqlx persistence path can be run
# and tested with zero credential setup. The server itself defaults to an
# in-memory stub when DATABASE_URL is unset, so this is only needed to exercise
# durable persistence.
#
# Usage:
#   scripts/devdb.sh init     # create the cluster + database (idempotent)
#   scripts/devdb.sh start    # start it
#   scripts/devdb.sh stop     # stop it
#   scripts/devdb.sh status   # is it running?
#   scripts/devdb.sh url      # print the DATABASE_URL to use
#   scripts/devdb.sh nuke     # stop and delete everything
set -euo pipefail

# Work around a Homebrew/macOS Postgres startup quirk ("postmaster became
# multithreaded during startup") by pinning a locale.
export LC_ALL="${LC_ALL:-C}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="$ROOT/.devdb"
PORT="${DEVDB_PORT:-5433}"
DB_NAME="stellar_syndicates"
SOCK_DIR="$DATA_DIR/sock"
URL="postgres://postgres@127.0.0.1:$PORT/$DB_NAME"

cmd="${1:-}"
case "$cmd" in
  init)
    if [ -f "$DATA_DIR/PG_VERSION" ]; then
      echo "cluster already initialised at $DATA_DIR"
    else
      initdb -D "$DATA_DIR" -U postgres --auth-local=trust --auth-host=trust >/dev/null
      mkdir -p "$SOCK_DIR"
      echo "unix_socket_directories = '$SOCK_DIR'" >> "$DATA_DIR/postgresql.conf"
      echo "port = $PORT" >> "$DATA_DIR/postgresql.conf"
      echo "listen_addresses = '127.0.0.1'" >> "$DATA_DIR/postgresql.conf"
      echo "initialised cluster at $DATA_DIR"
    fi
    pg_ctl -D "$DATA_DIR" -l "$DATA_DIR/server.log" start >/dev/null 2>&1 || true
    sleep 1
    createdb -h "$SOCK_DIR" -p "$PORT" -U postgres "$DB_NAME" 2>/dev/null \
      && echo "created database $DB_NAME" \
      || echo "database $DB_NAME already exists"
    echo "DATABASE_URL=$URL"
    ;;
  start)
    pg_ctl -D "$DATA_DIR" -l "$DATA_DIR/server.log" start
    ;;
  stop)
    pg_ctl -D "$DATA_DIR" stop -m fast || true
    ;;
  status)
    pg_ctl -D "$DATA_DIR" status || true
    ;;
  url)
    echo "$URL"
    ;;
  nuke)
    pg_ctl -D "$DATA_DIR" stop -m immediate 2>/dev/null || true
    rm -rf "$DATA_DIR"
    echo "removed $DATA_DIR"
    ;;
  *)
    echo "usage: scripts/devdb.sh {init|start|stop|status|url|nuke}" >&2
    exit 1
    ;;
esac
