#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
DEMO="$ROOT/scripts/demo"
MARKER=".townhall-demo"
RESERVED_PORTS=" 7777 5173 "
READY_TRIES=120

DAEMON_PORT=""
WEB_PORT=""
DIR=""
EMPTY=""
STOP=""
DAEMON_PID=""
VITE_PID=""
SOCKET=""

usage() {
  echo "usage: scripts/demo.sh --daemon-port N --web-port M [--dir D] [--empty]"
  echo "       scripts/demo.sh --stop --dir D"
}

say() { echo "info: demo: $*"; }
fail() { echo "error: demo: $*" >&2; exit 1; }

parse_args() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --daemon-port) DAEMON_PORT=${2:-}; shift 2 || fail "--daemon-port needs a value" ;;
      --web-port) WEB_PORT=${2:-}; shift 2 || fail "--web-port needs a value" ;;
      --dir) DIR=${2:-}; shift 2 || fail "--dir needs a value" ;;
      --empty) EMPTY=1; shift ;;
      --stop) STOP=1; shift ;;
      -h|--help) usage; exit 0 ;;
      *) usage >&2; fail "unknown argument $1" ;;
    esac
  done
}

check_port() {
  case "$2" in ''|*[!0-9]*) usage >&2; fail "$1 must be a port number" ;; esac
  [ "$2" -ge 1024 ] && [ "$2" -le 65535 ] || fail "$1 $2 is outside 1024 to 65535"
  case "$RESERVED_PORTS" in *" $2 "*) fail "$1 $2 is a default townhall port: pick your own" ;; esac
  if lsof -nP -iTCP:"$2" -sTCP:LISTEN >/dev/null 2>&1; then fail "port $2 is already in use"; fi
}

alive() { [ -n "${1:-}" ] && kill -0 "$1" 2>/dev/null; }

pid_in() { [ -f "$1" ] && cat "$1" || true; }

stop_pid() {
  alive "${1:-}" || return 0
  kill "$1" 2>/dev/null || true
  for _ in 1 2 3 4 5 6 7 8 9 10; do alive "$1" || return 0; sleep 0.5; done
  kill -9 "$1" 2>/dev/null || true
}

kill_tmux() {
  [ -n "${1:-}" ] || return 0
  tmux -L "$1" kill-server >/dev/null 2>&1 || true
}

wipe_old_demo() {
  alive "$(pid_in "$DIR/run/demo.pid")" && fail "a demo already runs in $DIR: scripts/demo.sh --stop --dir $DIR"
  kill_tmux "$(pid_in "$DIR/run/tmux-socket")"
  find "$DIR" -mindepth 1 -maxdepth 1 ! -name "$MARKER" -exec rm -rf {} +
}

prepare_dir() {
  [ -n "$DIR" ] || DIR=$(mktemp -d "${TMPDIR:-/tmp}/townhall-demo.XXXXXX")
  mkdir -p "$DIR"
  DIR=$(cd "$DIR" && pwd -P)
  if [ -f "$DIR/$MARKER" ]; then
    wipe_old_demo
  elif [ -n "$(ls -A "$DIR")" ]; then
    fail "$DIR is not empty and holds no demo: pass an empty or new folder"
  fi
  touch "$DIR/$MARKER"
  mkdir -p "$DIR/home" "$DIR/repos" "$DIR/claude-projects" "$DIR/logs" "$DIR/run" "$DIR/start" "$DIR/bin"
  echo "$$" > "$DIR/run/demo.pid"
}

isolate() {
  SOCKET="townhall-demo-$DAEMON_PORT-$$"
  echo "$SOCKET" > "$DIR/run/tmux-socket"
  printf '[user]\n\tname = William Th\303\270gersen\n\temail = william@example.com\n[commit]\n\tgpgsign = false\n[init]\n\tdefaultBranch = main\n' > "$DIR/gitconfig"
  ln -sf "$DEMO/fake-claude.mjs" "$DIR/bin/claude"
  unset TMUX TMUX_PANE CLAUDECODE
  export TOWNHALL_HOME="$DIR/home"
  export TOWNHALL_TMUX_SOCKET="$SOCKET"
  export TOWNHALL_CLAUDE_PROJECTS="$DIR/claude-projects"
  export TOWNHALL_CLAUDE_BIN="$DEMO/fake-claude.mjs"
  export TOWNHALL_REPO="$DIR/start"
  export TOWNHALL_BIND="127.0.0.1:$DAEMON_PORT"
  export TOWNHALL_DAEMON_URL="http://127.0.0.1:$DAEMON_PORT"
  export GIT_CONFIG_GLOBAL="$DIR/gitconfig"
  export GIT_CONFIG_NOSYSTEM=1
  export PATH="$DIR/bin:$PATH"
}

daemon_binary() {
  local target=${CARGO_TARGET_DIR:-target}
  case "$target" in /*) ;; *) target="$ROOT/$target" ;; esac
  echo "$target/debug/townhall-daemon"
}

build_daemon() {
  say "building the daemon"
  (cd "$ROOT" && cargo build -q -p townhall-daemon) || fail "the daemon build failed"
}

wait_for_url() {
  for _ in $(seq "$READY_TRIES"); do
    curl -sf -o /dev/null "$1" && return 0
    alive "$2" || return 1
    sleep 0.5
  done
  return 1
}

start_daemon() {
  "$(daemon_binary)" >"$DIR/logs/daemon.log" 2>&1 &
  DAEMON_PID=$!
  echo "$DAEMON_PID" > "$DIR/run/daemon.pid"
  wait_for_url "$TOWNHALL_DAEMON_URL/api/info" "$DAEMON_PID" || fail "the daemon did not start, see $DIR/logs/daemon.log"
}

start_vite() {
  [ -x "$ROOT/web/node_modules/.bin/vite" ] || fail "web dependencies are missing: npm --prefix web install"
  (cd "$ROOT/web" && exec ./node_modules/.bin/vite --port "$WEB_PORT" --strictPort) >"$DIR/logs/vite.log" 2>&1 &
  VITE_PID=$!
  echo "$VITE_PID" > "$DIR/run/vite.pid"
}

seed() {
  local web="http://localhost:$WEB_PORT"
  local flags=(--daemon "$TOWNHALL_DAEMON_URL" --web "$web" --dir "$DIR")
  [ -z "$EMPTY" ] || flags+=(--empty)
  say "seeding demo data into $DIR"
  if ! node "$DEMO/seed.mjs" "${flags[@]}" 2>&1 | tee "$DIR/logs/seed.log"; then
    fail "seeding failed, see $DIR/logs"
  fi
  wait_for_url "$web/" "$VITE_PID" || fail "vite did not start, see $DIR/logs/vite.log"
}

cleanup() {
  trap - INT TERM EXIT
  stop_pid "$VITE_PID"
  stop_pid "$DAEMON_PID"
  kill_tmux "$SOCKET"
  rm -f "$DIR/run/ready" "$DIR/run/demo.pid"
  say "stopped the demo in $DIR"
}

run_demo() {
  check_port --daemon-port "$DAEMON_PORT"
  check_port --web-port "$WEB_PORT"
  [ "$DAEMON_PORT" != "$WEB_PORT" ] || fail "the daemon and web ports must differ"
  build_daemon
  prepare_dir
  trap 'cleanup; exit 130' INT
  trap 'cleanup; exit 143' TERM
  trap cleanup EXIT
  isolate
  start_daemon
  start_vite
  seed
  touch "$DIR/run/ready"
  say "ready at http://localhost:$WEB_PORT (daemon $TOWNHALL_DAEMON_URL, data in $DIR)"
  say "stop with ctrl-c or scripts/demo.sh --stop --dir $DIR"
  while alive "$DAEMON_PID" && alive "$VITE_PID"; do sleep 1; done
  echo "warn: demo: the daemon or vite exited, see $DIR/logs" >&2
}

stop_demo() {
  [ -n "$DIR" ] || fail "--stop needs --dir"
  [ -f "$DIR/$MARKER" ] || fail "$DIR holds no demo"
  stop_pid "$(pid_in "$DIR/run/demo.pid")"
  stop_pid "$(pid_in "$DIR/run/vite.pid")"
  stop_pid "$(pid_in "$DIR/run/daemon.pid")"
  kill_tmux "$(pid_in "$DIR/run/tmux-socket")"
  rm -f "$DIR/run/ready" "$DIR/run/demo.pid"
  say "stopped the demo in $DIR"
}

parse_args "$@"
if [ -n "$STOP" ]; then stop_demo; else run_demo; fi
