#!/usr/bin/env bash

# Exit immediately if any command fails; treat unset vars as errors.
set -euo pipefail

# Check if a command is installed.
function verify_command_installed() {
  type "$1" >/dev/null 2>&1 || {
    printf "%s command is required but it's not installed.\n" "$1" >&2
    exit 1
  }
}

verify_command_installed jq
verify_command_installed rg

JSON_OBJECT="${1:-}"
REGEX="${2:-}"

[[ -z $JSON_OBJECT ]] && {
  printf "Error: first argument (JSON object selector) is empty.\n" >&2
  exit 1
}
[[ -z $REGEX ]] && {
  printf "Error: second argument (regex) is empty.\n" >&2
  exit 1
}

# The selector is the one argument that stays PROGRAM text: a jq path is what
# this tool takes, and a path cannot be passed as data without changing that.
# So its shape is bounded rather than trusted, and anything beyond a plain
# dotted path is refused before it reaches the filter below.
[[ $JSON_OBJECT =~ ^\.[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*$ ]] || {
  printf "Error: first argument must be a plain dotted jq path such as .name or .meta.id (got '%s').\n" "$JSON_OBJECT" >&2
  exit 1
}

timestamp="$(date +%Y%m%d%H%M%S)"
backup_dir="backup_$timestamp"
mkdir -p "$backup_dir"

# Backup and delete JSON objects from files containing regex.
# Uses a tempfile+mv instead of `sponge` (moreutils) to avoid a conflict
# between moreutils' bundled `parallel` and GNU parallel; see commit log
# for the drop-moreutils rationale.
rg --files-with-matches "$REGEX" | while IFS= read -r file; do
  cp "$file" "${backup_dir}/"
  tmp=$(mktemp "${file}.XXXXXX")
  # The value is DATA, through --arg. It used to be spliced into the program in
  # single quotes, which jq has no such thing as: the filter was a compile error
  # and this helper failed on the first file it matched, every time. A value
  # carrying jq syntax would otherwise be compiled as part of the filter too.
  if jq --arg value "$REGEX" "del(.[] | select(${JSON_OBJECT}? == \$value))" "$file" >"$tmp"; then
    mv "$tmp" "$file"
  else
    rm -f "$tmp"
    printf "Error: jq failed on %s; leaving original untouched.\n" "$file" >&2
    exit 1
  fi
done
