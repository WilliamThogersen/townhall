#!/bin/sh
# Browser dev mode: daemon plus Vite dev server, no Tauri.
# Usage: npm run dev [-- /path/to/repo]   (or set TOWNHALL_REPO; default: current directory)
# Ctrl-C stops both processes. Agent tmux sessions are left running.
set -u
cd "$(dirname "$0")/.." || exit 1
ROOT=$(pwd)

if [ $# -gt 0 ]; then TOWNHALL_REPO=$1; fi
TOWNHALL_REPO=${TOWNHALL_REPO:-$ROOT}
export TOWNHALL_REPO

cargo build -p townhall-daemon || { echo "dev: daemon build failed" >&2; exit 1; }

DAEMON_PID=""
VITE_PID=""
cleanup() {
  trap - INT TERM EXIT
  for pid in $VITE_PID $DAEMON_PID; do
    [ -n "$pid" ] && kill "$pid" 2>/dev/null
  done
  wait 2>/dev/null
}
trap cleanup INT TERM EXIT

"$ROOT/target/debug/townhall-daemon" &
DAEMON_PID=$!
# Call vite directly so the pid we kill is the server itself, not an npm wrapper.
(cd "$ROOT/web" && exec ./node_modules/.bin/vite) &
VITE_PID=$!

# Exit as soon as either process dies, then clean up the other.
while kill -0 "$DAEMON_PID" 2>/dev/null && kill -0 "$VITE_PID" 2>/dev/null; do
  sleep 1
done
echo "dev: a process exited, shutting down" >&2
