#!/usr/bin/env bash
set -euo pipefail

root_list="${MERGE_CHECKS_ROOT_ALLOW_LIST:-.wexo/root-allow-list.txt}"
vendor_list="${MERGE_CHECKS_TREE_VENDOR_ALLOW_LIST:-.wexo/tree-hygiene-vendor-allow.txt}"
size_list="${MERGE_CHECKS_TREE_SIZE_ALLOW_LIST:-.wexo/tree-hygiene-size-allow.txt}"
deny_list="${MERGE_CHECKS_TREE_DENY_LIST:-.wexo/tree-hygiene-deny.txt}"
max_bytes="${MERGE_CHECKS_TREE_MAX_BYTES:-10485760}"
max_listed="${MERGE_CHECKS_TREE_MAX_LISTED:-50}"

cd "$(git rev-parse --show-toplevel)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failed=0

git -c core.quotePath=false ls-files > "$work/tracked"

read_list() {
  [ -f "$1" ] && sed -e 's/[[:space:]]*$//' -e '/^#/d' -e '/^$/d' "$1" || true
}

report() {
  local title="$1" hint="$2" file="$3" total
  total="$(wc -l < "$file" | tr -d ' ')"
  {
    echo "$title ($total):"
    head -n "$max_listed" "$file" | sed 's/^/  /'
    [ "$total" -gt "$max_listed" ] && echo "  ... and $((total - max_listed)) more"
    echo "Fix: $hint"
    echo
  } >&2
  failed=1
}

if [ ! -f "$root_list" ]; then
  {
    echo "Missing $root_list."
    echo "Create it with one allowed root file name per line, for example:"
    echo "  mkdir -p $(dirname "$root_list") && git -c core.quotePath=false ls-files | grep -v / | sort > $root_list"
    echo "Review the list before you commit it: every name in it is a file the project agrees to keep at the repo root."
  } >&2
  exit 1
fi

read_list "$root_list" | LC_ALL=C sort -u > "$work/allowed"
{ grep -v / "$work/tracked" || true; } | LC_ALL=C sort -u > "$work/roots"
LC_ALL=C comm -23 "$work/roots" "$work/allowed" > "$work/v-root"
if [ -s "$work/v-root" ]; then
  report "Tracked root files not in $root_list" \
    "move each into a subdirectory, untrack it, or (if it belongs at the root) add its name to $root_list." "$work/v-root"
fi

: > "$work/v-sqlite"
git grep --cached -l -a -F -z -e 'SQLite format 3' 2>/dev/null | tr '\0' '\n' > "$work/sqlite-candidates" || true
while IFS= read -r path; do
  [ -n "$path" ] || continue
  header="$(git show ":$path" 2>/dev/null | head -c 16 | od -An -tx1 | tr -d ' \n')" || true
  [ "$header" = "53514c69746520666f726d6174203300" ] && printf '%s\n' "$path" >> "$work/v-sqlite"
done < "$work/sqlite-candidates"
if [ -s "$work/v-sqlite" ]; then
  report "Tracked SQLite database files (detected by content)" \
    "untrack each (git rm --cached <path>) and add it to .gitignore; a database does not belong in git." "$work/v-sqlite"
fi

has_store() {
  case "$1" in
    */node_modules/*|*/vendor/*|*/.pnpm-store/*) return 0 ;;
    *) return 1 ;;
  esac
}

{
  echo 'resources/views/vendor/'
  echo 'lang/vendor/'
  read_list "$vendor_list"
} > "$work/vendor-allowed"
: > "$work/v-deps"
while IFS= read -r path; do
  case "/$path" in
    */node_modules/*|*/vendor/*|*/.pnpm-store/*) ;;
    *) continue ;;
  esac
  excepted=0
  q="/$path"
  while IFS= read -r prefix; do
    case "$q" in *"/$prefix"*) ;; *) continue ;; esac
    before="${q%%"/$prefix"*}"
    rest="/${q#*"/$prefix"}"
    if ! has_store "$before/" && ! has_store "$rest"; then excepted=1; break; fi
  done < "$work/vendor-allowed"
  [ "$excepted" -eq 1 ] || printf '%s\n' "$path" >> "$work/v-deps"
done < "$work/tracked"
if [ -s "$work/v-deps" ]; then
  report "Tracked dependency-store files (.pnpm-store/, node_modules/, vendor/)" \
    "untrack them (git rm -r --cached <dir>) and add the directory to .gitignore; a legitimate directory such as a published vendor/ override goes in $vendor_list (one path prefix per line)." "$work/v-deps"
fi

read_list "$size_list" | LC_ALL=C sort -u > "$work/size-allowed"
git -c core.quotePath=false ls-files -s | awk -F'\t' '{ split($1, a, " "); if (a[1] != "160000") print a[2] " " $2 }' \
  | git cat-file --batch-check='%(objectsize) %(rest)' \
  | awk -v max="$max_bytes" '$1 + 0 > max { sub(/^[0-9]+ /, ""); print }' \
  | LC_ALL=C sort -u > "$work/oversized"
LC_ALL=C comm -23 "$work/oversized" "$work/size-allowed" > "$work/v-size"
if [ -s "$work/v-size" ]; then
  report "Tracked files over $max_bytes bytes" \
    "untrack or shrink each (large files belong in object storage or Git LFS); a deliberate exception goes in $size_list (one exact path per line)." "$work/v-size"
fi

if [ -f "$deny_list" ]; then
  : > "$work/v-deny"
  while IFS= read -r line; do
    pattern="${line%% #*}"
    pattern="${pattern%"${pattern##*[![:space:]]}"}"
    reason=""
    case "$line" in *" #"*) reason="${line#*" #"}"; reason="${reason# }" ;; esac
    [ -n "$pattern" ] || continue
    while IFS= read -r path; do
      matched=0
      case "$pattern" in
        */) case "$path" in "$pattern"*) matched=1 ;; esac ;;
        *) # shellcheck disable=SC2254
           case "$path" in $pattern) matched=1 ;; esac ;;
      esac
      [ "$matched" -eq 1 ] || continue
      if [ -n "$reason" ]; then printf '%s (denied by %s: %s)\n' "$path" "$pattern" "$reason"; else printf '%s (denied by %s)\n' "$path" "$pattern"; fi
    done < "$work/tracked" >> "$work/v-deny"
  done < <(grep -v '^[[:space:]]*#' "$deny_list" | grep -v '^[[:space:]]*$' || true)
  if [ -s "$work/v-deny" ]; then
    report "Tracked paths this project denies in $deny_list" \
      "untrack each (git rm --cached <path>) and add it to .gitignore, or remove the rule from $deny_list if it no longer applies." "$work/v-deny"
  fi
fi

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "tree hygiene: $(wc -l < "$work/tracked" | tr -d ' ') tracked file(s) checked, no violations."
