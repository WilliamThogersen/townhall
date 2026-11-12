#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
PARITY="$ROOT/scripts/parity"
OUT="$ROOT/docs/plan/gpui-parity"
DAEMON_PORT=${PARITY_DAEMON_PORT:-7792}
WEB_PORT=${PARITY_WEB_PORT:-5192}
PROXY_PORT=${PARITY_PROXY_PORT:-7793}
READY_TRIES=600

DIR=""
DEMO_PID=""
PROXY_PID=""

usage() {
  echo "usage: scripts/parity.sh [scene...]"
  echo "scenes: see scripts/parity/scenes.json; add -light for the light theme (for example home-light)"
}

say() { echo "info: parity: $*"; }
fail() { echo "error: parity: $*" >&2; exit 1; }

alive() { [ -n "${1:-}" ] && kill -0 "$1" 2>/dev/null; }

cleanup() {
  trap - INT TERM EXIT
  if alive "$PROXY_PID"; then kill "$PROXY_PID" 2>/dev/null || true; fi
  if [ -n "$DIR" ] && [ -f "$DIR/.townhall-demo" ]; then "$ROOT/scripts/demo.sh" --stop --dir "$DIR" || true; fi
  if alive "$DEMO_PID"; then wait "$DEMO_PID" 2>/dev/null || true; fi
  say "stopped the demo and the proxy"
}

wait_ready() {
  for _ in $(seq "$READY_TRIES"); do
    [ -f "$DIR/run/ready" ] && return 0
    alive "$DEMO_PID" || fail "the demo exited before it was ready, see $DIR/logs"
    sleep 0.5
  done
  fail "the demo was not ready, see $DIR/logs"
}

wait_for_url() {
  for _ in $(seq 40); do
    curl -sf -o /dev/null "$1" && return 0
    sleep 0.25
  done
  fail "$1 did not answer"
}

case "${1:-}" in -h|--help) usage; exit 0 ;; esac
command -v python3 >/dev/null || fail "python3 is required"
python3 -c "import PIL" 2>/dev/null || fail "python3 needs Pillow: python3 -m pip install pillow"
[ -x "$ROOT/web/node_modules/.bin/playwright" ] || fail "web dependencies are missing: npm --prefix web ci"

mkdir -p "$OUT"
PARENT=$(mktemp -d "${TMPDIR:-/tmp}/townhall-parity.XXXXXX")
DIR="$PARENT/demo"
trap 'cleanup; exit 130' INT
trap 'cleanup; exit 143' TERM
trap cleanup EXIT

say "starting the seeded demo in $DIR"
"$ROOT/scripts/demo.sh" --daemon-port "$DAEMON_PORT" --web-port "$WEB_PORT" --dir "$DIR" >"$PARENT/demo.out" 2>&1 &
DEMO_PID=$!
wait_ready

node "$PARITY/proxy.mjs" "$PROXY_PORT" "$DAEMON_PORT" >"$DIR/logs/proxy.log" 2>&1 &
PROXY_PID=$!
wait_for_url "http://127.0.0.1:$PROXY_PORT/api/system"

say "building townhall-native"
cargo build -q --manifest-path "$ROOT/crates/townhall-gpui/Cargo.toml" || fail "the native build failed"
NATIVE_TARGET=${CARGO_TARGET_DIR:-$ROOT/crates/townhall-gpui/target}
NATIVE_BIN="$NATIVE_TARGET/debug/townhall-native"
[ -x "$NATIVE_BIN" ] || fail "$NATIVE_BIN was not built"

WANTED=$(IFS=,; echo "$*")
say "web shots"
(
  cd "$ROOT/web"
  PARITY_OUT="$OUT" PARITY_PROXY="http://127.0.0.1:$PROXY_PORT" PARITY_DEMO_DIR="$DIR" \
    PARITY_WEB_URL="http://localhost:$WEB_PORT" PARITY_SCENES="$WANTED" \
    ./node_modules/.bin/playwright test --config playwright.parity.config.ts
)

say "native shots and diffs"
mkdir -p "$DIR/native-home"
python3 -I "$PARITY/parity.py" --bin "$NATIVE_BIN" --url "http://127.0.0.1:$PROXY_PORT" \
  --demo-dir "$DIR" --native-home "$DIR/native-home" --out "$OUT" "$@"
say "report: docs/plan/gpui-parity/report.md"
