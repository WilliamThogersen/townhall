#!/usr/bin/env bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

dir=crates/townhall-core/src/store/migrations
registry=crates/townhall-core/src/store/migrate.rs
usage='usage: scripts/migration-number.sh next <range-start> | check <base> [head]'

ranges() {
  grep -Eo '[0-9]+\.\.=[0-9]+' "$registry"
}

range_for() {
  ranges | while IFS= read -r pair; do
    if [ "${pair%%..=*}" = "$1" ]; then
      echo "$pair"
    fi
  done | head -n 1
}

range_start_of() {
  local n=$((10#$1)) pair lo hi
  for pair in $(ranges); do
    lo=${pair%%..=*}
    hi=${pair##*..=}
    if [ "$n" -ge "$lo" ] && [ "$n" -le "$hi" ]; then
      echo "$lo"
      return
    fi
  done
  echo "<range-start>"
}

files_at() {
  git ls-tree --name-only "$1" -- "$dir/" | sed 's|.*/||' | grep -E '^[0-9]{4}_.*\.sql$' || true
}

cmd_next() {
  local start=$1 pair lo hi max numbers ref name path n
  pair=$(range_for "$start")
  if [ -z "$pair" ]; then
    echo "no migration range starts at $start; ranges: $(ranges | tr '\n' ' ' | sed 's/ $//')" >&2
    exit 2
  fi
  lo=${pair%%..=*}
  hi=${pair##*..=}
  git fetch --quiet origin >/dev/null 2>&1 || true
  numbers=$(
    {
      for path in "$dir"/[0-9][0-9][0-9][0-9]_*.sql; do
        if [ -e "$path" ]; then
          echo "${path##*/}"
        fi
      done
      files_at HEAD
      for ref in $(git for-each-ref --format='%(refname)' refs/remotes/); do
        files_at "$ref"
      done
    } | while IFS= read -r name; do
      echo "$((10#${name:0:4}))"
    done
  )
  max=$((lo - 1))
  for n in $numbers; do
    if [ "$n" -ge "$lo" ] && [ "$n" -le "$hi" ] && [ "$n" -gt "$max" ]; then
      max=$n
    fi
  done
  if [ $((max + 1)) -gt "$hi" ]; then
    echo "range $lo..=$hi is full" >&2
    exit 1
  fi
  echo $((max + 1))
}

cmd_check() {
  local base=$1 head=${2:-HEAD} failed=0
  local head_files base_files mergebase prev name num other status path
  head_files=$(files_at "$head")
  base_files=$(files_at "$base")

  prev=""
  while IFS= read -r name; do
    [ -n "$name" ] || continue
    num=${name:0:4}
    if [ "${prev:0:4}" = "$num" ] && [ -n "$prev" ]; then
      echo "migration $num used twice: $prev, $name" >&2
      failed=1
    fi
    prev=$name
  done <<EOF
$(printf '%s\n' "$head_files" | sort)
EOF

  while IFS= read -r name; do
    [ -n "$name" ] || continue
    if printf '%s\n' "$base_files" | grep -qxF "$name"; then
      continue
    fi
    num=${name:0:4}
    other=$(printf '%s\n' "$base_files" | grep -E "^$num" | head -n 1 || true)
    if [ -n "$other" ]; then
      echo "migration $num is $name here but $other on $base: pick a new number with scripts/migration-number.sh next $(range_start_of "$num")" >&2
      failed=1
    fi
  done <<EOF
$head_files
EOF

  mergebase=$(git merge-base "$base" "$head")
  while IFS=$'\t' read -r status path _; do
    [ -n "$status" ] || continue
    case "$status" in
      M | D | R*)
        echo "migration ${path##*/} is on $base and must never be edited or renumbered" >&2
        failed=1
        ;;
    esac
  done <<EOF
$(git diff --name-status "$mergebase" "$head" -- "$dir/")
EOF

  if [ "$failed" -ne 0 ]; then
    exit 1
  fi
  echo "migration numbers: ok"
}

case "${1:-}" in
  next)
    [ $# -eq 2 ] || { echo "$usage" >&2; exit 2; }
    cmd_next "$2"
    ;;
  check)
    [ $# -ge 2 ] && [ $# -le 3 ] || { echo "$usage" >&2; exit 2; }
    cmd_check "$2" "${3:-HEAD}"
    ;;
  *)
    echo "$usage" >&2
    exit 2
    ;;
esac
