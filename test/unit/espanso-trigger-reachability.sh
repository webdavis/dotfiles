#!/usr/bin/env bash
# espanso-trigger-reachability.sh, four invariants over the espanso match set:
# every file loads, every trigger is typeable, no trigger is declared twice, and
# no bare-word correction can fire in the middle of a word.
#
# WHY A TRIGGER CAN BE UNTYPEABLE. espanso's rolling matcher fires the FIRST
# terminal it reaches and throws the rest away. From espanso-match/src/rolling/
# matcher.rs, in `process`:
#
#     MatcherTreeRef::Matches(matches) => {
#         ...
#         // Reset the state and return the matches
#         return (RollingMatcherState::default(), results);
#
# So when one trigger is a strict prefix of another, typing the longer one
# expands the shorter one partway through and discards the rest: `;;re` fired
# "return" and `;;review` could never be typed at all. Nine triggers were dead
# this way, and espanso reports nothing, because from its side both matches
# loaded fine.
#
# WHY A WORD BOUNDARY FIXES IT, rather than a rename. The same crate compiles
# the boundary options into the trigger sequence (espanso-match/src/rolling/
# mod.rs, `from_string`):
#
#     if opt.left_word  { items.push(RollingItem::WordSeparator); }
#     ... chars ...
#     if opt.right_word { items.push(RollingItem::WordSeparator); }
#
# and the tree keeps `word_separators` as an edge SEPARATE from `chars`
# (rolling/tree.rs), which `find_refs` follows only for a separator event. A
# trigger carrying a right-hand boundary therefore parks its terminal behind
# that edge: typing a letter after it walks the `chars` edge instead, the
# shorter match does not fire, and the longer trigger stays reachable. This is
# why the invariant below is "shorter trigger must require a right boundary"
# rather than "no trigger may be a prefix of another": it fixes the same nine
# triggers while changing no trigger's spelling, and no muscle memory with it.
#
# The same mechanism is what stops `wont` expanding inside `wonton`, which is
# the mid-word defect invariant 4 covers.
#
# Everything here is read from the raw file text. identity.yml.tmpl calls
# keepassxc, so rendering the match set from automation is off the table; the
# `trigger:` lines are plain literals in every file, and only `replace:` values
# are ever templated, so the raw text is the whole answer. Parsing is regex
# rather than YAML because no YAML parser is reachable from the flake's `run`
# shell once /opt/homebrew is off PATH (measured: yq absent, and the system
# python3 has no yaml module). Invariant 0 exists to keep that honest: it fails
# loudly the moment a file stops matching the shape this parser assumes, rather
# than quietly parsing fewer triggers.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MATCH_DIR="$REPO_ROOT/Library/Application Support/espanso/match"
# espanso does not auto-load a match file whose name starts with this; such a
# file reaches the match set only through an `imports:` entry.
# https://espanso.org/docs/matches/organizing-matches/
PRIVATE_MATCH_FILE_PREFIX='_'
# chezmoi's template suffix. espanso sees the TARGET name, so identity.yml.tmpl
# is identity.yml to the auto-load rule.
CHEZMOI_TEMPLATE_SUFFIX='.tmpl'
# The match-option keys that make espanso require a word separator on each side.
LEFT_BOUNDARY_OPTIONS=(word left_word)
RIGHT_BOUNDARY_OPTIONS=(word right_word)
# Field separator for the parser's records. The trigger is emitted LAST so that
# a trigger containing this character lands in the final read variable intact
# (bug class 5: never let a field boundary depend on the data).
RECORD_SEPARATOR='|'

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Answers "what does espanso see this file called?", dropping chezmoi's template
# suffix. Pure.
target_file_name() {
  local source_name="$1"
  printf '%s\n' "${source_name%"$CHEZMOI_TEMPLATE_SUFFIX"}"
}

# Answers "does espanso load this file without being told to?". Pure.
is_auto_loaded_match_file() {
  local target_name
  target_name="$(target_file_name "$1")"
  [[ $target_name != "$PRIVATE_MATCH_FILE_PREFIX"* ]]
}

# Answers "is this character one a word can be made of?", which is what decides
# whether a trigger's edge sits inside word space and can therefore fire (or be
# fired into) mid-word. Pure.
is_word_character() {
  local character="$1"
  [[ $character == [[:alnum:]] ]]
}

# Answers "does this option set require a separator on the given side?". Pure,
# and the single place either side's answer is defined, so the two arms of
# invariant 4 and the shadow check in invariant 2 cannot drift apart.
options_require_boundary() {
  local side="$1" options="$2" option
  local -a required_options
  case "$side" in
    LEFT) required_options=("${LEFT_BOUNDARY_OPTIONS[@]}") ;;
    RIGHT) required_options=("${RIGHT_BOUNDARY_OPTIONS[@]}") ;;
    *) fail "options_require_boundary was asked about side '$side', which is neither LEFT nor RIGHT" ;;
  esac
  for option in "${required_options[@]}"; do
    [[ " $options " == *" $option "* ]] && return 0
  done
  return 1
}

# Emits one record per match: file|line|options|trigger. The option keys are the
# ones written at the four-space indent that YAML reserves for a match's own
# keys; a `replace: |` block's body is indented deeper, and a line at four
# spaces would end that block, so no block content can be misread as an option.
parse_match_records() {
  awk -v separator="$RECORD_SEPARATOR" '
    function flush() {
      if (have_match) print file separator line separator options separator trigger
      have_match = 0
      options = ""
    }
    FILENAME != file { flush(); file = FILENAME }
    /^  - trigger: "/ {
      flush()
      trigger = $0
      sub(/^  - trigger: "/, "", trigger)
      sub(/"$/, "", trigger)
      line = FNR
      have_match = 1
      next
    }
    /^    [a-z_]+:/ {
      if (have_match) {
        key = $0
        sub(/^    /, "", key)
        sub(/:.*$/, "", key)
        if ($0 ~ /: *true *$/) options = options " " key
      }
    }
    END { flush() }
  ' "$@"
}

# Emits the basename of every file the given match files import. Import paths
# may be relative or absolute; espanso resolves them, and only the file name is
# needed to decide whether a file in this directory is reachable.
parse_imported_file_names() {
  awk '
    /^imports:/ { in_imports = 1; next }
    /^[^ #]/    { in_imports = 0 }
    in_imports && /^[[:space:]]*-[[:space:]]*/ {
      path = $0
      sub(/^[[:space:]]*-[[:space:]]*/, "", path)
      gsub(/["\x27]/, "", path)
      sub(/^.*\//, "", path)
      if (path != "") print path
    }
  ' "$@"
}

[[ -d $MATCH_DIR ]] || fail "missing espanso match directory: $MATCH_DIR"

# ---- predicate self-tests --------------------------------------------------
assert_equals() {
  local what="$1" actual="$2" expected="$3"
  [[ $actual == "$expected" ]] ||
    fail "$what answered '$actual', expected '$expected'; a helper these invariants depend on no longer discriminates"
}
assert_predicate() {
  local predicate="$1" expected="$2" actual=no
  shift 2
  "$predicate" "$@" && actual=yes
  [[ $actual == "$expected" ]] ||
    fail "$predicate answered '$actual' for [$*], expected '$expected'; a helper these invariants depend on no longer discriminates"
}
assert_equals "target_file_name identity.yml.tmpl" "$(target_file_name identity.yml.tmpl)" identity.yml
assert_equals "target_file_name snippets.yml" "$(target_file_name snippets.yml)" snippets.yml
assert_predicate is_auto_loaded_match_file yes snippets.yml
assert_predicate is_auto_loaded_match_file yes identity.yml.tmpl
assert_predicate is_auto_loaded_match_file no _pqi.yml
assert_predicate is_word_character yes a
assert_predicate is_word_character yes 9
assert_predicate is_word_character no ';'
assert_predicate is_word_character no ' '
assert_predicate is_word_character no '.'
assert_predicate options_require_boundary yes LEFT ' word '
assert_predicate options_require_boundary yes LEFT ' left_word '
assert_predicate options_require_boundary no LEFT ' right_word '
assert_predicate options_require_boundary yes RIGHT ' word '
assert_predicate options_require_boundary yes RIGHT ' right_word '
assert_predicate options_require_boundary no RIGHT ' left_word '
assert_predicate options_require_boundary no RIGHT ' propagate_case '
assert_predicate options_require_boundary no RIGHT ''

shopt -s nullglob
MATCH_FILES=("$MATCH_DIR"/*.yml "$MATCH_DIR"/*.yml"$CHEZMOI_TEMPLATE_SUFFIX")
shopt -u nullglob
((${#MATCH_FILES[@]} > 0)) || fail "no match files found under $MATCH_DIR; every invariant would pass vacuously"

# ---- 0: the files still have the shape this parser assumes -----------------
# Fail closed. A trigger line in any other shape would be skipped silently, and
# a skipped trigger is a trigger no invariant here can protect.
while IFS= read -r offender; do
  [[ -n $offender ]] &&
    fail "a trigger is declared in a shape this test cannot parse: $offender. Every trigger must be written as '  - trigger: \"...\"' on one line, or the reachability invariants silently stop covering it"
done < <(grep -nE '^[[:space:]]*-?[[:space:]]*trigger:' "${MATCH_FILES[@]}" |
  grep -vE ':[0-9]+:  - trigger: "[^"\\]*"$' || true)

# ---- gather every trigger --------------------------------------------------
declare -A TRIGGER_OPTIONS=()
declare -A TRIGGER_LOCATION=()
trigger_count=0
while IFS="$RECORD_SEPARATOR" read -r record_file record_line record_options record_trigger; do
  [[ -n $record_trigger ]] || continue
  location="$(basename "$record_file"):$record_line"
  if [[ -n ${TRIGGER_LOCATION[$record_trigger]+set} ]]; then
    fail "the trigger '$record_trigger' is declared twice, at ${TRIGGER_LOCATION[$record_trigger]} and at $location. espanso loads both and expands whichever it reaches first, so one of them is dead and which one is not something the files say"
  fi
  TRIGGER_LOCATION["$record_trigger"]="$location"
  TRIGGER_OPTIONS["$record_trigger"]="$record_options"
  trigger_count=$((trigger_count + 1))
done < <(parse_match_records "${MATCH_FILES[@]}")
((trigger_count > 0)) || fail "no triggers were parsed out of $MATCH_DIR; every invariant would pass vacuously"

# ---- 1: every match file reaches the match set -----------------------------
declare -A LOADED_FILE=()
for match_file in "${MATCH_FILES[@]}"; do
  source_name="$(basename "$match_file")"
  is_auto_loaded_match_file "$source_name" &&
    LOADED_FILE["$(target_file_name "$source_name")"]=auto-loaded
done
# imports are transitive, so this walks to a fixed point rather than one level.
while :; do
  loaded_before=${#LOADED_FILE[@]}
  for match_file in "${MATCH_FILES[@]}"; do
    target_name="$(target_file_name "$(basename "$match_file")")"
    [[ -n ${LOADED_FILE[$target_name]+set} ]] || continue
    while IFS= read -r imported_name; do
      [[ -n $imported_name ]] || continue
      [[ -n ${LOADED_FILE[$imported_name]+set} ]] ||
        LOADED_FILE["$imported_name"]="imported by $target_name"
    done < <(parse_imported_file_names "$match_file")
  done
  ((${#LOADED_FILE[@]} == loaded_before)) && break
done
for match_file in "${MATCH_FILES[@]}"; do
  target_name="$(target_file_name "$(basename "$match_file")")"
  [[ -n ${LOADED_FILE[$target_name]+set} ]] ||
    fail "espanso never loads $target_name: its name starts with '$PRIVATE_MATCH_FILE_PREFIX', which excludes it from auto-loading, and no loaded match file imports it. Every trigger in it is inert. Add it to an 'imports:' list, or drop the prefix"
done

# ---- 2: no trigger is shadowed by a shorter one ----------------------------
# A prefix relation is found by sorting and comparing neighbours: if one trigger
# is a prefix of another, every string that sorts between them shares that
# prefix too, so the shorter one's immediate successor always extends it.
mapfile -t SORTED_TRIGGERS < <(printf '%s\n' "${!TRIGGER_LOCATION[@]}" | LC_ALL=C sort)
shadow_checks=0
for ((index = 0; index + 1 < ${#SORTED_TRIGGERS[@]}; index++)); do
  shorter="${SORTED_TRIGGERS[index]}"
  longer="${SORTED_TRIGGERS[index + 1]}"
  [[ $longer == "$shorter"* ]] || continue
  shadow_checks=$((shadow_checks + 1))
  options_require_boundary RIGHT "${TRIGGER_OPTIONS[$shorter]}" ||
    fail "'$shorter' (${TRIGGER_LOCATION[$shorter]}) makes '$longer' (${TRIGGER_LOCATION[$longer]}) impossible to type: espanso fires the first terminal it reaches and discards the rest, so typing '$longer' expands '$shorter' partway through. Give '$shorter' a right-hand word boundary (right_word: true, or word: true when it is a bare word), which parks its expansion behind a separator and lets the longer trigger through"
done
((shadow_checks > 0)) ||
  fail "invariant 2 found no prefix relations at all among $trigger_count triggers; with a set this size that means the comparison stopped working, not that the set is clean"

# ---- 3: bare-word triggers cannot fire mid-word ----------------------------
# Scope: triggers that BEGIN with a word character, which is the autocorrect
# namespace. A sigil-prefixed trigger (";;re", ",,cc") is deliberately outside
# it: requiring a boundary there is a reachability question, answered above for
# the ones that need it, not a mid-word one.
boundary_checks=0
for trigger in "${!TRIGGER_LOCATION[@]}"; do
  is_word_character "${trigger:0:1}" || continue
  boundary_checks=$((boundary_checks + 1))
  options="${TRIGGER_OPTIONS[$trigger]}"
  options_require_boundary LEFT "$options" ||
    fail "'$trigger' (${TRIGGER_LOCATION[$trigger]}) is a bare-word correction with no left word boundary, so it fires when those letters end a longer word. Add 'word: true' (or 'left_word: true' when the trigger already ends in a separator)"
  is_word_character "${trigger: -1}" || continue
  options_require_boundary RIGHT "$options" ||
    fail "'$trigger' (${TRIGGER_LOCATION[$trigger]}) is a bare-word correction with no right word boundary, so it fires in the middle of a longer word: this is what turns 'wonton' into 'won'ton'. Add 'word: true'"
done
((boundary_checks > 0)) ||
  fail "invariant 3 examined no bare-word triggers at all; the autocorrect namespace is not being seen"

printf 'espanso-trigger-reachability: OK (%d files all loaded, %d triggers all unique and typeable, %d prefix relations all shielded, %d bare-word triggers all bounded)\n' \
  "${#MATCH_FILES[@]}" "$trigger_count" "$shadow_checks" "$boundary_checks"
