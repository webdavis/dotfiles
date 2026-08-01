#!/usr/bin/env bash
# espanso-trigger-reachability.sh, four invariants over the espanso match set:
# every file loads and every import resolves, every trigger is typeable (in the
# literal namespace and in the case-folded one espanso uses for propagate_case),
# no trigger is declared twice, and no bare-word correction can fire in the
# middle of a word.
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
# rather than "no trigger may be a prefix of another": it frees the same nine
# triggers without changing a single trigger's spelling.
#
# WHAT THE BOUNDARY COSTS. Stated because the same source says so, and because
# an earlier version of this comment claimed it cost nothing. Parking a terminal
# behind a separator edge is precisely what makes the shorter trigger WAIT for
# one: `;;re` no longer expands the instant its last character is typed, it
# expands on the next word separator. espanso's default separator set is space,
# TAB, CR, LF, form feed, non-breaking space and , ; : . ? ! ( ) { } [ ] < > ' "
# (espanso-config/src/config/resolve.rs, `word_separators`), so a space or a
# punctuation mark fires it, and the separator survives into the result
# (espanso-engine/src/process/middleware/render.rs formats the body as
# `{body}{right_separator}`). Those are CHARACTER separators: the worker builds
# the matcher with `char_word_separators` only and leaves `key_word_separators`
# at its empty default (espanso/src/cli/worker/engine/mod.rs), so whether a
# given KEY fires the boundary depends on the character its keypress reports,
# which is not something this comment has measured. That is the same keystroke
# shape the bare-word corrections in this repo already have, and it is a real
# behaviour change rather than a free one.
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
# than quietly parsing fewer triggers. That includes espanso's PLURAL trigger
# spelling, `- triggers: [a, b]`, which is a first-class form the loader accepts
# (espanso-config/.../yaml/mod.rs: `if let Some(trigger) = yaml_match.trigger {
# Some(vec![trigger]) } else { yaml_match.triggers }`) and which this parser
# does not read. It is refused rather than parsed: the repo does not use it, and
# a refusal cannot silently cover fewer triggers than it claims to.
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
# The extensions espanso loads a match file under. Both spellings are
# first-class, and the search is RECURSIVE: espanso's own include patterns are
# `../match/**/[!_]*.yml` and `../match/**/[!_]*.yaml` (espanso-config/src/
# config/resolve.rs, STANDARD_INCLUDES), and .yaml has been accepted since
# 2.3.0. A flat *.yml-only search would leave a sub-folder or a .yaml file
# carrying live triggers that no invariant here ever looks at.
MATCH_FILE_EXTENSIONS=(.yml .yaml)
# Files that may sit in the match tree without being a match file. Kept as a
# closed list so that discovery cannot narrow without failing: anything else
# found there is either a match file this test must read or a mistake, and both
# deserve to be said out loud rather than skipped. .DS_Store is macOS Finder
# litter, gitignored and .chezmoiignore'd, so it never reaches a target.
NON_MATCH_FILE_NAMES=(.DS_Store)
# The match-option keys that make espanso require a word separator on each side.
LEFT_BOUNDARY_OPTIONS=(word left_word)
RIGHT_BOUNDARY_OPTIONS=(word right_word)
# The match-option keys that make espanso compare a trigger without regard to
# case. `propagate_case` is handed straight to the matcher as
# `case_insensitive` (espanso/src/cli/worker/engine/process/middleware/matcher/
# convert.rs), which compiles the trigger into CharInsensitive items
# (espanso-match/src/rolling/mod.rs, `from_string`), so such a trigger occupies
# its whole case-folded neighbourhood in espanso's matching namespace, not just
# its literal spelling.
CASE_INSENSITIVE_OPTIONS=(propagate_case)
# Field separator for the parser's records. The trigger is emitted LAST so that
# a trigger containing this character lands in the final read variable intact
# (bug class 5: never let a field boundary depend on the data).
RECORD_SEPARATOR='|'
# Joins a trigger's case-folded key to its literal spelling for the sort that
# finds folded prefix relations. It has to sort BEFORE every character a trigger
# can contain, or the sort would order `abcd<sep>` ahead of `abc<sep>` and the
# neighbour comparison would stop finding relations; 0x01 is below every
# printable byte.
FOLD_KEY_SEPARATOR=$'\x01'
# Every YAML key that can carry a trigger cause, singular and plural. Invariant
# 0 looks for both and accepts only the singular one-line form this parser
# reads.
TRIGGER_CAUSE_KEY_PATTERN='^[[:space:]]*-?[[:space:]]*triggers?:'
# The one trigger-line shape parse_match_records understands, as it appears in
# `grep -n` output.
PARSEABLE_TRIGGER_LINE_PATTERN=':[0-9]+:  - trigger: "[^"\\]*"$'

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

# Answers "would espanso read this file as a match file at all?", by the target
# name's extension. Pure, and the single place discovery's shape is decided.
is_match_file_name() {
  local target_name extension
  target_name="$(target_file_name "$1")"
  for extension in "${MATCH_FILE_EXTENSIONS[@]}"; do
    [[ $target_name == *"$extension" ]] && return 0
  done
  return 1
}

# Answers "is this a file the match tree is allowed to hold without it being a
# match file?". Pure. Deliberately a closed list: see NON_MATCH_FILE_NAMES.
is_exempt_non_match_file_name() {
  local source_name="$1" exempt_name
  for exempt_name in "${NON_MATCH_FILE_NAMES[@]}"; do
    [[ $source_name == "$exempt_name" ]] && return 0
  done
  return 1
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

# Answers "does this option set make espanso compare the trigger without regard
# to case?". Pure.
options_ignore_case() {
  local options="$1" option
  for option in "${CASE_INSENSITIVE_OPTIONS[@]}"; do
    [[ " $options " == *" $option "* ]] && return 0
  done
  return 1
}

# Answers "what does espanso compare this trigger as, once case stops
# mattering?". Pure. ASCII folding only, which is what every trigger in this
# repo is; espanso folds with UniCase, so a non-ASCII trigger would fold more
# broadly there than here and this answer would be too narrow rather than wrong.
case_folded_trigger() {
  local trigger="$1"
  printf '%s\n' "${trigger,,}"
}

# Answers "can one typed input walk both of these triggers' paths as far as the
# shorter one ends?", which is what makes a relation between their case-folded
# forms reachable rather than notional. If neither trigger ignores case, only
# their literal spellings can relate and invariant 2 has already asked; if
# either one does, the case-insensitive edge accepts the other's spelling and
# the relation is live. Pure.
folded_relation_is_reachable() {
  local shorter_options="$1" longer_options="$2"
  options_ignore_case "$shorter_options" || options_ignore_case "$longer_options"
}

# Answers "which file on disk does that import name?", resolving the path the
# way espanso does (espanso-config/src/matches/group/path.rs: a relative import
# joins the IMPORTING file's directory, an absolute one is used as is) and then
# allowing for chezmoi's template suffix, since the source tree spells
# identity.yml as identity.yml.tmpl. Emits the path, or returns non-zero when
# nothing is there. Not pure: it asks the filesystem, which is the whole point.
# The directory part is what matters here. espanso treats an unresolvable
# import as a NON-FATAL error, logging and carrying on, so a mistyped directory
# leaves the intended file loading nothing and says so nowhere the operator
# looks.
import_target_path() {
  local importing_file="$1" import_path="$2" resolved candidate
  if [[ $import_path == /* ]]; then
    resolved="$import_path"
  else
    resolved="$(dirname "$importing_file")/$import_path"
  fi
  for candidate in "$resolved" "$resolved$CHEZMOI_TEMPLATE_SUFFIX"; do
    [[ -f $candidate ]] || continue
    printf '%s\n' "$candidate"
    return 0
  done
  return 1
}

# Answers "which discovered match file is this path?", comparing by device and
# inode so that two spellings of one file (./_pqi.yml against _pqi.yml) answer
# the same. Reads the MATCH_FILES discovery above; emits the discovered
# spelling, or returns non-zero when the path is outside it.
discovered_match_file() {
  local path="$1" discovered
  for discovered in "${MATCH_FILES[@]}"; do
    [[ $discovered -ef $path ]] && {
      printf '%s\n' "$discovered"
      return 0
    }
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

# Emits every import path the given match file declares, VERBATIM. The path is
# kept whole rather than reduced to a basename, because the directory part is
# exactly what decides whether espanso can resolve the import at all: a wrong
# directory leaves the file unloaded and every trigger in it inert, which is the
# defect this suite exists to catch.
parse_import_paths() {
  awk '
    /^imports:/ { in_imports = 1; next }
    /^[^ #]/    { in_imports = 0 }
    in_imports && /^[[:space:]]*-[[:space:]]*/ {
      path = $0
      sub(/^[[:space:]]*-[[:space:]]*/, "", path)
      gsub(/["\x27]/, "", path)
      if (path != "") print path
    }
  ' "$1"
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
  # stdout is discarded: this asserts on the answer, and a helper that also
  # emits a value would otherwise print it into the suite's output.
  "$predicate" "$@" >/dev/null && actual=yes
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
assert_predicate is_match_file_name yes snippets.yml
assert_predicate is_match_file_name yes overrides.yaml
assert_predicate is_match_file_name yes identity.yml.tmpl
assert_predicate is_match_file_name no .DS_Store
assert_predicate is_match_file_name no README.md
assert_predicate is_exempt_non_match_file_name yes .DS_Store
assert_predicate is_exempt_non_match_file_name no snippets.yml
assert_predicate options_ignore_case yes ' propagate_case '
assert_predicate options_ignore_case no ' word '
assert_predicate options_ignore_case no ''
assert_equals "case_folded_trigger on a mixed-case trigger" "$(case_folded_trigger "IT's")" "it's"
assert_equals "case_folded_trigger on an already-folded trigger" "$(case_folded_trigger ';;re')" ';;re'
assert_predicate folded_relation_is_reachable yes ' propagate_case ' ''
assert_predicate folded_relation_is_reachable yes '' ' propagate_case '
assert_predicate folded_relation_is_reachable no ' word ' ' right_word '
assert_predicate folded_relation_is_reachable no '' ''

# ---- discovery -------------------------------------------------------------
# Recursive and both extensions, because espanso's own include patterns are.
# Every regular file under the match tree is then classified: a match file this
# test reads, or a name on the exempt list. Nothing may be silently neither,
# which is what stops discovery quietly narrowing (a flat glob, a dropped
# extension) and taking triggers out of every invariant's reach with it. Sorted
# so that which of two duplicate declarations is named "first" is a property of
# the tree and not of directory order.
MATCH_FILES=()
while IFS= read -r -d '' found_file; do
  found_name="$(basename "$found_file")"
  if is_match_file_name "$found_name"; then
    MATCH_FILES+=("$found_file")
  elif ! is_exempt_non_match_file_name "$found_name"; then
    fail "$found_file sits in the espanso match tree but is neither a match file (a name ending ${MATCH_FILE_EXTENSIONS[*]}, optionally with chezmoi's $CHEZMOI_TEMPLATE_SUFFIX suffix) nor an exempt name. espanso reads that tree recursively; a file this test cannot classify is one it cannot say anything about"
  fi
done < <(find "$MATCH_DIR" -type f -print0 | LC_ALL=C sort -z)
((${#MATCH_FILES[@]} > 0)) || fail "no match files found under $MATCH_DIR; every invariant would pass vacuously"

# ---- import-resolution self-tests, on a fixture ----------------------------
# A fixture rather than the live tree, so that these keep answering for the
# resolver even when the repo's own imports change. All four arms matter: the
# relative join, chezmoi's template suffix, the wrong directory that is the
# defect, and the file outside the discovered tree. The expected answers keep
# the un-normalised `dir/./name` spelling on purpose: import_target_path joins
# rather than canonicalises, which is exactly why discovered_match_file compares
# by device and inode instead of by string.
import_fixture="$(mktemp -d)"
trap 'rm -rf "$import_fixture"' EXIT
mkdir -p "$import_fixture/nested"
: >"$import_fixture/importer.yml"
: >"$import_fixture/sibling.yml"
: >"$import_fixture/templated.yml$CHEZMOI_TEMPLATE_SUFFIX"
: >"$import_fixture/nested/deep.yml"
assert_equals "import_target_path on a relative sibling" \
  "$(import_target_path "$import_fixture/importer.yml" ./sibling.yml)" \
  "$import_fixture/./sibling.yml"
assert_equals "import_target_path on a relative sub-folder" \
  "$(import_target_path "$import_fixture/importer.yml" nested/deep.yml)" \
  "$import_fixture/nested/deep.yml"
assert_equals "import_target_path on a name chezmoi delivers from a template" \
  "$(import_target_path "$import_fixture/importer.yml" ./templated.yml)" \
  "$import_fixture/./templated.yml$CHEZMOI_TEMPLATE_SUFFIX"
assert_equals "import_target_path on an absolute path" \
  "$(import_target_path "$import_fixture/importer.yml" "$import_fixture/sibling.yml")" \
  "$import_fixture/sibling.yml"
assert_predicate import_target_path no "$import_fixture/importer.yml" ./no-such-dir/sibling.yml
assert_predicate import_target_path no "$import_fixture/importer.yml" ./no-such-file.yml
assert_predicate discovered_match_file yes "${MATCH_FILES[0]}"
assert_predicate discovered_match_file yes "$(dirname "${MATCH_FILES[0]}")/./$(basename "${MATCH_FILES[0]}")"
assert_predicate discovered_match_file no "$import_fixture/sibling.yml"
rm -rf "$import_fixture"
trap - EXIT

# ---- 0: the files still have the shape this parser assumes -----------------
# Fail closed. A trigger line in any other shape would be skipped silently, and
# a skipped trigger is a trigger no invariant here can protect. The plural
# `triggers:` key is caught here too: espanso accepts it, this parser does not
# read it, and refusing it is what keeps "every trigger is covered" true.
while IFS= read -r offender; do
  [[ -n $offender ]] &&
    fail "a trigger is declared in a shape this test cannot parse: $offender. Every trigger must be written as '  - trigger: \"...\"' on one line, one trigger per match. espanso also accepts the plural '  - triggers: [a, b]' form, which this parser does not read, so a match written that way would drop out of every invariant here without a word; split it into one match per trigger"
done < <(grep -HnE "$TRIGGER_CAUSE_KEY_PATTERN" "${MATCH_FILES[@]}" |
  grep -vE "$PARSEABLE_TRIGGER_LINE_PATTERN" || true)

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
# Keyed on the discovered file itself, not on its name: the search is recursive,
# so two sub-folders may hold files with the same name and a name-keyed map
# would report one of them loaded on the other's behalf.
declare -A LOADED_FILE=()
for match_file in "${MATCH_FILES[@]}"; do
  is_auto_loaded_match_file "$(basename "$match_file")" &&
    LOADED_FILE["$match_file"]=auto-loaded
done
# imports are transitive, so this walks to a fixed point rather than one level.
while :; do
  loaded_before=${#LOADED_FILE[@]}
  for match_file in "${MATCH_FILES[@]}"; do
    [[ -n ${LOADED_FILE[$match_file]+set} ]] || continue
    while IFS= read -r import_path; do
      [[ -n $import_path ]] || continue
      imported_target="$(import_target_path "$match_file" "$import_path")" ||
        fail "$match_file imports '$import_path', and nothing is at that path. espanso resolves a relative import against the IMPORTING file's own directory, and treats one it cannot resolve as a non-fatal error: it logs, carries on, and every trigger in the file you meant to import stays inert. A right file name behind a wrong directory reads as correct in the source and loads nothing"
      imported_file="$(discovered_match_file "$imported_target")" ||
        fail "$match_file imports '$import_path', which resolves to $imported_target, a file outside the espanso match tree at $MATCH_DIR. espanso would load it and no invariant here has read it, so its triggers are covered by nothing. Move it into the match tree, or drop the import"
      [[ -n ${LOADED_FILE[$imported_file]+set} ]] ||
        LOADED_FILE["$imported_file"]="imported by $match_file"
    done < <(parse_import_paths "$match_file")
  done
  ((${#LOADED_FILE[@]} == loaded_before)) && break
done
for match_file in "${MATCH_FILES[@]}"; do
  [[ -n ${LOADED_FILE[$match_file]+set} ]] ||
    fail "espanso never loads $match_file: its name starts with '$PRIVATE_MATCH_FILE_PREFIX', which excludes it from auto-loading, and no loaded match file imports it. Every trigger in it is inert. Add it to an 'imports:' list, or drop the prefix"
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

# ---- 2b: the same, in espanso's CASE-FOLDED namespace ----------------------
# Invariant 2 compares literal spellings, which is the whole answer only while
# every trigger is matched literally. A `propagate_case` trigger is not: espanso
# compiles it into case-insensitive characters, so it also occupies every other
# casing of itself. `dont` with propagate_case and a separate `DONT` are one
# trigger to the matcher and two to a literal comparison, and `dont` would
# shadow `DONTCARE` the same way `;;re` shadowed `;;review`.
#
# Same neighbour walk as invariant 2, over case-folded keys, reporting only the
# relations a literal comparison could not already see and that at least one
# case-insensitive side makes reachable. This finds nothing in the current set,
# which is the correct answer for it (every trigger is matched literally or is
# already all-lowercase) and the reason there is no non-vacuity guard on the
# count here: the guard is instead that the walk must SEE every trigger, which
# is what a broken key split would take away.
mapfile -t SORTED_FOLDED_TRIGGERS < <(
  for trigger in "${!TRIGGER_LOCATION[@]}"; do
    printf '%s%s%s\n' "$(case_folded_trigger "$trigger")" "$FOLD_KEY_SEPARATOR" "$trigger"
  done | LC_ALL=C sort
)
((${#SORTED_FOLDED_TRIGGERS[@]} == trigger_count)) ||
  fail "the case-folded walk was handed ${#SORTED_FOLDED_TRIGGERS[@]} entries for $trigger_count triggers; it is not looking at the whole set and would miss a collision in the part it dropped"
folded_relation_checks=0
for ((index = 0; index + 1 < ${#SORTED_FOLDED_TRIGGERS[@]}; index++)); do
  shorter_folded="${SORTED_FOLDED_TRIGGERS[index]%%"$FOLD_KEY_SEPARATOR"*}"
  shorter="${SORTED_FOLDED_TRIGGERS[index]#*"$FOLD_KEY_SEPARATOR"}"
  longer_folded="${SORTED_FOLDED_TRIGGERS[index + 1]%%"$FOLD_KEY_SEPARATOR"*}"
  longer="${SORTED_FOLDED_TRIGGERS[index + 1]#*"$FOLD_KEY_SEPARATOR"}"
  [[ -n ${TRIGGER_LOCATION[$shorter]+set} && -n ${TRIGGER_LOCATION[$longer]+set} ]] ||
    fail "the case-folded walk read '$shorter' and '$longer' out of its own keys and at least one of them is not a trigger; the key separator is no longer splitting where it was written to"
  [[ $longer_folded == "$shorter_folded"* ]] || continue
  [[ $longer != "$shorter"* ]] || continue
  folded_relation_is_reachable "${TRIGGER_OPTIONS[$shorter]}" "${TRIGGER_OPTIONS[$longer]}" || continue
  folded_relation_checks=$((folded_relation_checks + 1))
  [[ $longer_folded != "$shorter_folded" ]] ||
    fail "'$shorter' (${TRIGGER_LOCATION[$shorter]}) and '$longer' (${TRIGGER_LOCATION[$longer]}) differ only in case, and one of them is matched case-insensitively (propagate_case), so espanso sees a single trigger declared twice. It expands whichever terminal it reaches first, and which one that is is not something the files say. Give them different spellings, or drop one"
  options_require_boundary RIGHT "${TRIGGER_OPTIONS[$shorter]}" ||
    fail "'$shorter' (${TRIGGER_LOCATION[$shorter]}) makes '$longer' (${TRIGGER_LOCATION[$longer]}) impossible to type: one of them is matched case-insensitively (propagate_case), so espanso compares them folded and '$shorter' is a prefix of '$longer' there even though the spellings differ. Give '$shorter' a right-hand word boundary (right_word: true, or word: true when it is a bare word)"
done

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

printf 'espanso-trigger-reachability: OK (%d files all loaded and all imports resolved, %d triggers all unique and typeable, %d literal and %d case-folded prefix relations all shielded, %d bare-word triggers all bounded)\n' \
  "${#MATCH_FILES[@]}" "$trigger_count" "$shadow_checks" "$folded_relation_checks" "$boundary_checks"
