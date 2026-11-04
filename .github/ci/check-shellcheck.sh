#!/usr/bin/env bash
set -euo pipefail

severity="${MERGE_CHECKS_SHELLCHECK_SEVERITY:-warning}"
exclude="${MERGE_CHECKS_SHELLCHECK_EXCLUDE:-^$}"

tracked="$(mktemp)"
trap 'rm -f "$tracked"' EXIT
git ls-files -z > "$tracked" || { echo "Could not list the tracked files." >&2; exit 1; }

scripts=()
while IFS= read -r -d '' path; do
  if [[ "$path" =~ $exclude ]]; then continue; fi
  if [[ "$path" == *.sh ]]; then scripts+=("$path"); continue; fi
  kind="$(file -b -- "$path")" || { echo "Could not inspect $path." >&2; exit 1; }
  shopt -s nocasematch
  if [[ "$kind" == *"shell script"* ]]; then scripts+=("$path"); fi
  shopt -u nocasematch
done < "$tracked"

if [ "${#scripts[@]}" -eq 0 ]; then
  echo "No tracked shell scripts."
  exit 0
fi

shellcheck --severity="$severity" -- "${scripts[@]}" || exit 1
echo "shellcheck: ${#scripts[@]} script(s) clean at --severity=$severity."
