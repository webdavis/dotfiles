#!/usr/bin/env bash
#
# find-and-remove-json-objects.sh deletes objects from JSON files whose selected
# field equals a value, keeping a backup copy first.
#
# The value it compares against arrives as an ARGUMENT and used to be spliced
# into the jq PROGRAM in single quotes. jq has no single-quoted strings, so the
# filter was a compile error and the helper failed on the first file it matched,
# every time; and a value carrying jq syntax would have been compiled as part of
# the filter rather than compared as data. Both are pinned below.
#
# Unit test: the real helper against fixture files in a throwaway directory.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_find-and-remove-json-objects.sh"

fail() {
  printf 'find-and-remove-json-objects: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $SCRIPT ]] || fail "missing helper: $SCRIPT"
command -v jq >/dev/null 2>&1 || {
  printf 'SKIP: jq not on PATH\n'
  exit 0
}
command -v rg >/dev/null 2>&1 || {
  printf 'SKIP: rg not on PATH, and the helper requires it\n'
  exit 0
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# ── 1. The plain case: the matching object goes, the other stays, and a backup
#      of the original is kept. This is the whole job, and it exited 1 without
#      touching a file for as long as the value was program text. ────────────
mkdir -p "$tmp/plain"
cd "$tmp/plain" || fail "could not enter the sandbox"
jq -n '[{name: "keep"}, {name: "drop"}]' >inventory.json
out="$(bash "$SCRIPT" .name drop 2>&1)" ||
  fail "the helper exited non-zero on a plain value: $out"
jq -e 'length == 1 and .[0].name == "keep"' inventory.json >/dev/null 2>&1 ||
  fail "the matching object was not removed (or the wrong one was): $(cat inventory.json)"

shopt -s nullglob
backups=(backup_*/inventory.json)
shopt -u nullglob
[[ ${#backups[@]} -eq 1 ]] ||
  fail "the original was not backed up before being rewritten (found ${#backups[@]} backups)"
jq -e 'length == 2' "${backups[0]}" >/dev/null 2>&1 ||
  fail "the backup does not hold the original two objects: $(cat "${backups[0]}")"

# ── 2. A VALUE CARRYING jq SYNTAX is compared, not compiled. Spliced into the
#      program, these characters are operators rather than the string somebody
#      asked to match. ─────────────────────────────────────────────────────
mkdir -p "$tmp/quoted"
cd "$tmp/quoted" || fail "could not enter the sandbox"
jq -n '[{name: "safe"}, {name: "a|b"}]' >inventory.json
out="$(bash "$SCRIPT" .name 'a|b' 2>&1)" ||
  fail "the helper exited non-zero on a value containing jq operators: $out"
jq -e 'length == 1 and .[0].name == "safe"' inventory.json >/dev/null 2>&1 ||
  fail "a value containing jq operators did not match its object as data: $(cat inventory.json)"

# ── 3. A SELECTOR THAT IS NOT A PLAIN PATH is refused. It is the one argument
#      that stays program text, because a jq path is what this tool takes, so
#      the shape it may have is bounded rather than trusted. ────────────────
mkdir -p "$tmp/selector"
cd "$tmp/selector" || fail "could not enter the sandbox"
jq -n '[{name: "keep"}]' >inventory.json
selector_rc=0
bash "$SCRIPT" '.name) | .[] | halt_error, (.x' keep >/dev/null 2>&1 || selector_rc=$?
[[ $selector_rc -ne 0 ]] ||
  fail "a selector carrying filter code was accepted"
jq -e 'length == 1' inventory.json >/dev/null 2>&1 ||
  fail "a refused selector still altered the file: $(cat inventory.json)"
# Refused BEFORE any work starts, which is what distinguishes a validated
# argument from one jq happens to choke on later: without the check the helper
# gets as far as creating a backup directory and copying files into it.
shopt -s nullglob
selector_backups=(backup_*)
shopt -u nullglob
[[ ${#selector_backups[@]} -eq 0 ]] ||
  fail "a refused selector still created ${selector_backups[*]}, so the argument is checked only after this tool has started working"

cd "$REPO_ROOT" || fail "could not leave the sandbox"
echo "find-and-remove-json-objects: OK"
