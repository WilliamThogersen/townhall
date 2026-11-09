#!/usr/bin/env bash
# Scan the commits an MR adds for secrets with gitleaks. The project's allow list is a file of
# [[allowlists]] tables (gitleaks syntax), each with a non-empty `description` that is the reason.
# Env: SECRET_DETECTION_BASE (scan BASE..tip, tip = CI_MERGE_REQUEST_SOURCE_BRANCH_SHA or HEAD; empty scans all history),
#      SECRET_DETECTION_ALLOW_LIST (default .merge-checks/secret-allow-list.toml; optional),
#      SECRET_DETECTION_REPORT (default gitleaks-report.json).
set -uo pipefail

allow_list="${SECRET_DETECTION_ALLOW_LIST:-.merge-checks/secret-allow-list.toml}"
report="${SECRET_DETECTION_REPORT:-gitleaks-report.json}"
base="${SECRET_DETECTION_BASE:-}"
config="$(mktemp)"
trap 'rm -f "$config"' EXIT

printf '[extend]\nuseDefault = true\n' >"$config"

if [ -f "$allow_list" ]; then
  if ! awk '
    function close_block() { if (inblock && !desc) { printf "%s: [[allowlists]] at line %d has no description (the reason)\n", FILENAME, start; bad = 1 } }
    /^[[:space:]]*(#|$)/ { next }
    /^[[:space:]]*\[\[allowlists\]\][[:space:]]*(#.*)?$/ { close_block(); inblock = 1; desc = 0; start = NR; next }
    /^[[:space:]]*\[/ && !/^[[:space:]]*\[[[:space:]]*(["\x27]|$)/ { printf "%s:%d: only [[allowlists]] tables are allowed: %s\n", FILENAME, NR, $0; bad = 1; next }
    /^[[:space:]]*description[[:space:]]*=[[:space:]]*("[^"]*[^"[:space:]][^"]*"|\x27[^\x27]*[^\x27[:space:]][^\x27]*\x27)/ { desc = 1 }
    END { close_block(); if (!NR || !inblock) { printf "%s: no [[allowlists]] table\n", FILENAME; bad = 1 } exit bad }
  ' "$allow_list" >&2; then
    echo "secret-detection: invalid allow list $allow_list" >&2
    exit 1
  fi
  printf '\n' >>"$config"
  cat "$allow_list" >>"$config"
fi

opts=()
if [ -n "$base" ]; then
  tip="${CI_MERGE_REQUEST_SOURCE_BRANCH_SHA:-HEAD}"
  if ! git cat-file -e "$base^{commit}" 2>/dev/null; then
    git fetch --quiet origin "$base" 2>/dev/null || true
  fi
  if ! git merge-base --is-ancestor "$base" "$tip" 2>/dev/null; then
    git fetch --quiet --unshallow origin 2>/dev/null || true
  fi
  if ! git merge-base --is-ancestor "$base" "$tip" 2>/dev/null; then
    echo "secret-detection: base commit $base is not an ancestor of $tip, cannot scan the MR range" >&2
    exit 1
  fi
  opts=(--log-opts="$base..$tip")
fi

gitleaks git --config "$config" --redact --no-banner --exit-code 3 --report-format json --report-path "$report" "${opts[@]}" .
status=$?
if [ "$status" -eq 3 ]; then
  echo "secret-detection: new secrets found (values redacted, details in $report)." >&2
  echo "Remove the secret and rotate it. If it is deliberate, add an [[allowlists]] entry with a description to $allow_list." >&2
  exit 1
fi
if [ "$status" -ne 0 ]; then
  echo "secret-detection: gitleaks failed (exit $status), this is not a finding. Check the output above, e.g. a regex in $allow_list that does not compile." >&2
  exit 1
fi
