#!/usr/bin/env bash
# aerospace-binding-targets.sh, two invariants of dot_aerospace.toml: every
# binding launches something this repo installs, and the hjkl bindings all point
# the same way.
#
# Nothing else in this repo reads dot_aerospace.toml. treefmt excludes it from
# taplo (treefmt.nix, programs.taplo.excludes) to preserve the operator's manual
# column alignment, so no formatter parses it and no linter looks at it. This
# test is the only gate the file has, which is why it parses the raw text rather
# than leaning on a TOML parser: there is none on PATH in the flake's `run`
# shell (measured: taplo ships inside the treefmt wrapper, not as a binary, and
# yq is absent once /opt/homebrew is off PATH).
#
# The invariants:
#   1. Every application and executable a live binding launches is declared in
#      .chezmoidata/system_packages_autoinstall.yaml, is a macOS system app or
#      system command, or is a script this repo deploys. A chord bound to
#      something no fresh machine installs is a key that silently does nothing:
#      `open -a Arc` was exactly that, with Arc never declared and never
#      installed, and the working replacement sitting commented out below it.
#   2. Every hjkl binding that names a direction names the RIGHT one. AeroSpace
#      has no notion of a canonical hjkl mapping, so a transposed pair is not an
#      error anywhere: service-mode join-with had j bound to `up` and k to
#      `down`, inverted against every other hjkl group in the file, and nothing
#      but muscle memory could report it.
#
# Two deliberate scoping decisions, both of which cost a false positive when got
# wrong (each was hit while writing this):
#   - Comments are stripped from every line, whole-line or trailing, by tracking
#     TOML quote state so that a `#` inside a string stays put. An earlier
#     version dropped only whole-line comments, on the reasoning that ignoring a
#     trailing one "could only hide a target, never invent one". That reasoning
#     is wrong in the direction that matters: `= 'exec-and-forget open -a Zen'
#     # replaces open -a Arc` made the test demand a package for Arc, a target
#     nobody binds. A comment invents targets, so it goes.
#   - Resolution is by package NAME, not by asking brew or the filesystem. A
#     binary's name is often not its formula's: `openhue` ships as
#     openhue/cli/openhue-cli and `borders` as felixkratz/formulae/borders. Both
#     read as undeclared to a naive basename comparison, so tap qualifiers are
#     stripped and the residual mismatches live in one named alias table.
#
# Extraction is counted, not just performed. Every `exec-and-forget` in a live
# binding must yield exactly one command target and every `open -a` exactly one
# application target, because the failure mode of a text extractor is not a
# wrong answer, it is silently answering about fewer targets: `open -a "Arc"`,
# quoted, matched no application pattern at all and the run stayed green with
# one fewer target checked.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
AEROSPACE_CONFIG="$REPO_ROOT/dot_aerospace.toml"
PACKAGE_DATA="$REPO_ROOT/.chezmoidata/system_packages_autoinstall.yaml"
# The package-data sections that install a launchable macOS application. taps,
# uv, npm and volta are deliberately excluded: nothing in this config launches
# from them, and admitting them would let an unrelated npm package name vouch
# for a missing cask.
LAUNCHABLE_PACKAGE_SECTIONS=(formulae casks)
MAS_PACKAGE_SECTION=mas
# Apps that ship with macOS. They have no package anywhere and never will.
APPLE_SYSTEM_APPS=(
  "Activity Monitor"
  "Finder"
  "Messages"
  "System Settings"
)
# Commands that ship with macOS.
SYSTEM_COMMANDS=(open osascript)
# Where a binary's name differs from the package that installs it. Kept as data,
# and small on purpose: a growing table means the naming convention broke, not
# that the table needs another row.
declare -A COMMAND_TO_PACKAGE=(
  [openhue]=openhue-cli
)
declare -A APPLICATION_TO_PACKAGE=(
  [Todoist]=todoist-app
)
# The hjkl convention this file follows everywhere else. Bindings on these keys
# that name no direction at all (resize mode says `resize width -50`) are not
# covered by it and are skipped rather than guessed at.
declare -A DIRECTION_FOR_KEY=(
  [h]=left
  [j]=down
  [k]=up
  [l]=right
)
# How this config spells the path to a deployed helper script. The tilde is
# literal on purpose: this is the text AeroSpace stores and hands to the shell,
# not a path this test resolves, so expanding it here would stop it matching.
# shellcheck disable=SC2088
HOME_SCRIPT_PREFIX='~/.local/bin/'
# The same location as a chezmoi target path, which is what `chezmoi managed`
# answers in.
HOME_SCRIPT_TARGET_PREFIX='.local/bin/'
CHEZMOI_DELIVERING_ENTRY_TYPES='files'
# Field separator for the package parser's records. Neither a section name
# ([a-z_]+) nor a package name can contain it.
PACKAGE_RECORD_SEPARATOR='|'
# The two commands a binding launches something with, as they appear in binding
# text. Both are counted as well as parsed: see the header on why.
EXEC_AND_FORGET_PATTERN='exec-and-forget[[:space:]]+'
OPEN_APPLICATION_FLAG_PATTERN='open[[:space:]]+-[a-zA-Z]*a[[:space:]]+'
SINGLE_QUOTE="'"
# TOML's escape character inside a basic string, named so the quote-state scan
# below can match it as data. Spelled $'\\' rather than '\' because the latter
# reads as an escaped quote to a linter even where it is not one.
BACKSLASH=$'\\'
# One `open -a <application>` occurrence. The application argument is bare with
# escaped spaces (Brave\ Browser) or quoted ("Arc"); AeroSpace passes the whole
# binding to a shell, so both spellings launch the same app and an extractor
# blind to either one silently checks one target fewer.
OPEN_APPLICATION_PATTERN="${OPEN_APPLICATION_FLAG_PATTERN}(\"[^\"]*\"|${SINGLE_QUOTE}[^${SINGLE_QUOTE}]*${SINGLE_QUOTE}|(\\\\[[:space:]]|[^[:space:]${SINGLE_QUOTE}\"])+)"
# One `exec-and-forget <command>` occurrence, same two spellings.
EXEC_AND_FORGET_COMMAND_PATTERN="${EXEC_AND_FORGET_PATTERN}(\"[^\"]*\"|${SINGLE_QUOTE}[^${SINGLE_QUOTE}]*${SINGLE_QUOTE}|[^[:space:]${SINGLE_QUOTE}\"]+)"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Bug class 11: this suite runs inside a linked worktree, where git hands hooks
# an absolute GIT_DIR. chezmoi below is given an explicit --source/--destination
# so no inherited state can redirect it.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE
export GIT_PAGER=cat PAGER=cat

# Answers "is this exact string in this list?". Pure, and the only membership
# test used below, so no arm can drift into a substring comparison.
list_contains() {
  local needle="$1"
  shift
  local candidate
  for candidate in "$@"; do
    [[ $candidate == "$needle" ]] && return 0
  done
  return 1
}

# Answers "what does this TOML line say once its comment is removed?". A `#`
# opens a comment only outside a string, so the scan carries quote state: a
# literal string ('...') takes no escapes, a basic string ("...") does. Pure, and
# the single place a comment boundary is decided. Returns non-zero when the line
# ends inside an unterminated string, which is a line TOML itself would reject
# and one this scan cannot answer for.
toml_line_without_comment() {
  local line="$1" character index
  local in_literal_string=0 in_basic_string=0 escaped=0
  for ((index = 0; index < ${#line}; index++)); do
    character="${line:index:1}"
    if ((escaped)); then
      escaped=0
      continue
    fi
    if ((in_basic_string)); then
      case "$character" in
        "$BACKSLASH") escaped=1 ;;
        '"') in_basic_string=0 ;;
      esac
      continue
    fi
    if ((in_literal_string)); then
      [[ $character == "$SINGLE_QUOTE" ]] && in_literal_string=0
      continue
    fi
    case "$character" in
      "$SINGLE_QUOTE") in_literal_string=1 ;;
      '"') in_basic_string=1 ;;
      '#')
        printf '%s\n' "${line:0:index}"
        return 0
        ;;
    esac
  done
  printf '%s\n' "$line"
  ((in_literal_string == 0 && in_basic_string == 0))
}

# Answers "what word does this shell argument name?", removing one layer of
# quoting, or unescaping escaped spaces when there is none. Pure.
unquoted_shell_word() {
  local word="$1"
  case "$word" in
    '"'*'"' | "$SINGLE_QUOTE"*"$SINGLE_QUOTE") printf '%s\n' "${word:1:${#word}-2}" ;;
    *) printf '%s\n' "${word//\\ / }" ;;
  esac
}

# Answers "which application does this `open -a` invocation launch?". Pure.
application_of_open_invocation() {
  local invocation="$1"
  [[ $invocation =~ ^${OPEN_APPLICATION_FLAG_PATTERN}(.+)$ ]] || return 1
  unquoted_shell_word "${BASH_REMATCH[1]}"
}

# Answers "which command does this `exec-and-forget` invocation run?". Pure.
command_of_exec_and_forget_invocation() {
  local invocation="$1"
  [[ $invocation =~ ^${EXEC_AND_FORGET_PATTERN}(.+)$ ]] || return 1
  unquoted_shell_word "${BASH_REMATCH[1]}"
}

# Answers "how many times does this pattern occur in this text?". Pure.
occurrence_count() {
  local pattern="$1" text="$2"
  grep -oE -- "$pattern" <<<"$text" | grep -c '' || true
}

# Answers "what would this application be called as a Homebrew package?".
# Homebrew cask tokens are lower-case with spaces as hyphens, which is what
# turns "Brave Browser" into brave-browser. Pure.
package_name_for_application() {
  local application="$1"
  printf '%s\n' "${application// /-}" | tr '[:upper:]' '[:lower:]'
}

# Answers "what is the bare package name?", dropping any owner/tap qualifier, so
# felixkratz/formulae/borders answers borders. Pure.
unqualified_package_name() {
  local package="$1"
  printf '%s\n' "${package##*/}"
}

# Answers "which packages does this repo install that can put an application or
# executable on this machine?", emitting one `section|package` record per line.
# Reads the YAML on stdin with a two-level indent walk rather than a parser,
# because no YAML parser is reachable from the flake's run shell once
# /opt/homebrew is off PATH. Reading stdin rather than a fixed path is what lets
# the self-tests below drive it with a fixture instead of pinning it to whatever
# packages happen to be declared today.
declared_launchable_packages() {
  local section='' line value
  while IFS= read -r line; do
    if [[ $line =~ ^\ {6}([a-z_]+):[[:space:]]*$ ]]; then
      section="${BASH_REMATCH[1]}"
      continue
    fi
    # Any key shallower than the section keys closes the current section.
    if [[ $line =~ ^\ {0,5}[a-z_]+:[[:space:]]*$ ]]; then
      section=''
      continue
    fi
    if [[ $line =~ ^\ {8}-\ (.+)$ ]]; then
      value="${BASH_REMATCH[1]}"
      if list_contains "$section" "${LAUNCHABLE_PACKAGE_SECTIONS[@]}"; then
        printf '%s%s%s\n' "$section" "$PACKAGE_RECORD_SEPARATOR" "$(unqualified_package_name "$value")"
      elif [[ $section == "$MAS_PACKAGE_SECTION" && $value =~ ^name:[[:space:]]*(.+)$ ]]; then
        # Mac App Store entries carry the application's real name, so they are
        # matched verbatim rather than through the Homebrew naming convention.
        printf '%s%s%s\n' "$section" "$PACKAGE_RECORD_SEPARATOR" "${BASH_REMATCH[1]}"
      fi
    fi
  done
}

# Answers "does this repo install something that provides this application?".
is_application_available() {
  local application="$1" package
  list_contains "$application" "${APPLE_SYSTEM_APPS[@]}" && return 0
  list_contains "$application" "${DECLARED_PACKAGES[@]}" && return 0
  package="${APPLICATION_TO_PACKAGE[$application]:-$(package_name_for_application "$application")}"
  list_contains "$package" "${DECLARED_PACKAGES[@]}"
}

# Answers "does this repo install something that provides this command?".
is_command_available() {
  local command_name="$1" package
  list_contains "$command_name" "${SYSTEM_COMMANDS[@]}" && return 0
  package="${COMMAND_TO_PACKAGE[$command_name]:-$command_name}"
  list_contains "$package" "${DECLARED_PACKAGES[@]}"
}

# Answers "which direction word, if any, does this binding name?", printing each
# one found. Direction words are matched as whole tokens so that a command word
# like wrap-around-the-workspace cannot masquerade as one, and TOML's own
# punctuation is turned into whitespace first so that the last token of
# 'join-with up' is `up` and not `up'`. Globbing is off because a binding value
# legitimately contains `*`. Pure.
direction_words_in_binding() {
  local binding="$1" token
  # The `]` has to come first: inside a bracket expression bash treats a leading
  # `]` as a literal and any later one as the closing bracket, so spelling this
  # set in the obvious order builds a pattern that silently matches nothing.
  local -r TOML_PUNCTUATION="][,\"'"
  local tokenized="${binding//[$TOML_PUNCTUATION]/ }"
  set -f
  for token in $tokenized; do
    case "$token" in
      left | down | up | right) printf '%s\n' "$token" ;;
    esac
  done
  set +f
}

# Answers "which key letter does this binding name end on?", printing h, j, k or
# l for a binding on one of them and nothing otherwise. Only the final chord
# segment counts, which is what keeps alt-ctrl-cmd-shift-leftSquareBracket from
# reading as an `l` binding. Pure.
hjkl_letter_of_binding_key() {
  local key="$1" letter="${1##*-}"
  [[ -n ${DIRECTION_FOR_KEY[$letter]+set} ]] && printf '%s\n' "$letter"
  return 0
}

[[ -f $AEROSPACE_CONFIG ]] || fail "missing config: $AEROSPACE_CONFIG"
[[ -f $PACKAGE_DATA ]] || fail "missing package data: $PACKAGE_DATA"
command -v chezmoi >/dev/null 2>&1 ||
  fail "chezmoi is not on PATH; this test cannot tell which helper scripts the repo deploys"

# ---- predicate self-tests --------------------------------------------------
assert_equals() {
  local what="$1" actual="$2" expected="$3"
  [[ $actual == "$expected" ]] ||
    fail "$what answered '$actual', expected '$expected'; a helper these invariants depend on no longer discriminates"
}
assert_equals "package_name_for_application 'Brave Browser'" \
  "$(package_name_for_application 'Brave Browser')" brave-browser
assert_equals "package_name_for_application 'Zen'" "$(package_name_for_application 'Zen')" zen
assert_equals "unqualified_package_name felixkratz/formulae/borders" \
  "$(unqualified_package_name felixkratz/formulae/borders)" borders
assert_equals "unqualified_package_name jq" "$(unqualified_package_name jq)" jq
assert_equals "hjkl_letter_of_binding_key alt-shift-j" \
  "$(hjkl_letter_of_binding_key alt-shift-j)" j
assert_equals "hjkl_letter_of_binding_key h" "$(hjkl_letter_of_binding_key h)" h
assert_equals "hjkl_letter_of_binding_key alt-ctrl-cmd-shift-leftSquareBracket" \
  "$(hjkl_letter_of_binding_key alt-ctrl-cmd-shift-leftSquareBracket)" ''
assert_equals "hjkl_letter_of_binding_key alt-ctrl-cmd-shift-down" \
  "$(hjkl_letter_of_binding_key alt-ctrl-cmd-shift-down)" ''
assert_equals "direction_words_in_binding on a quoted focus binding" \
  "$(direction_words_in_binding "'focus --boundaries-action wrap-around-the-workspace left'")" left
assert_equals "direction_words_in_binding on a quoted array binding" \
  "$(direction_words_in_binding "['join-with up', 'mode main']")" up
assert_equals "direction_words_in_binding on a resize binding" \
  "$(direction_words_in_binding "'resize width -50'")" ''
assert_equals "direction_words_in_binding on a notifier binding" \
  "$(direction_words_in_binding "'exec-and-forget terminal-notifier -title Aerospace -group aerospace-config'")" ''
assert_equals "direction_words_in_binding on an empty binding" \
  "$(direction_words_in_binding "[]")" ''

assert_equals "toml_line_without_comment on a whole-line comment" \
  "$(toml_line_without_comment "# alt-b = 'exec-and-forget open -a Arc'")" ''
assert_equals "toml_line_without_comment on an indented whole-line comment" \
  "$(toml_line_without_comment "    # alt-b = 'exec-and-forget open -a Arc'")" '    '
assert_equals "toml_line_without_comment on a trailing comment" \
  "$(toml_line_without_comment "    alt-b = 'exec-and-forget open -a Zen' # replaces open -a Arc")" \
  "    alt-b = 'exec-and-forget open -a Zen' "
assert_equals "toml_line_without_comment on a hash inside a literal string" \
  "$(toml_line_without_comment "    alt-t = 'exec-and-forget open -a Zen #tag'")" \
  "    alt-t = 'exec-and-forget open -a Zen #tag'"
assert_equals "toml_line_without_comment on a hash inside a basic string" \
  "$(toml_line_without_comment '    alt-t = "exec-and-forget echo \"a#b\"" # tail')" \
  '    alt-t = "exec-and-forget echo \"a#b\"" '
assert_equals "toml_line_without_comment on a comment containing quotes" \
  "$(toml_line_without_comment "    alt-s = 'layout v_accordion' # 'layout stacking' in i3")" \
  "    alt-s = 'layout v_accordion' "
assert_predicate() {
  local predicate="$1" expected="$2" actual=no
  shift 2
  "$predicate" "$@" >/dev/null && actual=yes
  [[ $actual == "$expected" ]] ||
    fail "$predicate answered '$actual' for [$*], expected '$expected'; a helper these invariants depend on no longer discriminates"
}
assert_predicate toml_line_without_comment no "    alt-b = 'exec-and-forget open -a Zen"
assert_predicate toml_line_without_comment yes "    alt-b = 'exec-and-forget open -a Zen'"
assert_equals "unquoted_shell_word on a double-quoted argument" \
  "$(unquoted_shell_word '"Brave Browser"')" 'Brave Browser'
assert_equals "unquoted_shell_word on a single-quoted argument" \
  "$(unquoted_shell_word "'Arc'")" Arc
assert_equals "unquoted_shell_word on an escaped-space argument" \
  "$(unquoted_shell_word 'Activity\ Monitor')" 'Activity Monitor'
assert_equals "unquoted_shell_word on a bare argument" "$(unquoted_shell_word Zen)" Zen
assert_equals "application_of_open_invocation on a bare name" \
  "$(application_of_open_invocation 'open -a Zen')" Zen
assert_equals "application_of_open_invocation on a quoted name" \
  "$(application_of_open_invocation 'open -a "Arc"')" Arc
assert_equals "application_of_open_invocation on an escaped-space name" \
  "$(application_of_open_invocation 'open -a Activity\ Monitor')" 'Activity Monitor'
assert_equals "application_of_open_invocation on a flag cluster" \
  "$(application_of_open_invocation 'open -na Zen')" Zen
assert_equals "command_of_exec_and_forget_invocation on a bare command" \
  "$(command_of_exec_and_forget_invocation 'exec-and-forget open')" open
assert_equals "command_of_exec_and_forget_invocation on a quoted path" \
  "$(command_of_exec_and_forget_invocation 'exec-and-forget "/opt/homebrew/bin/borders"')" \
  /opt/homebrew/bin/borders
assert_equals "occurrence_count of the open -a flag, twice" \
  "$(occurrence_count "$OPEN_APPLICATION_FLAG_PATTERN" "['open -a Zen', 'open -a \"Arc\"']")" 2
assert_equals "occurrence_count of the open -a flag, none" \
  "$(occurrence_count "$OPEN_APPLICATION_FLAG_PATTERN" "'join-with down'")" 0
assert_equals "occurrence_count of the whole application pattern on a quoted argument" \
  "$(occurrence_count "$OPEN_APPLICATION_PATTERN" "'open -a \"Arc\"'")" 1

# The package parser is exercised against a FIXTURE, not against whatever is
# declared today. Naming live packages here would fail this test the day the
# operator swaps a browser, which is a config choice and not a defect; invariant
# 1 is what ties the parser to the live file, by demanding a declaration for
# every application a binding actually launches.
PACKAGE_DATA_FIXTURE='packages:
  macos:
    homebrew:
      taps:
        - buo/cask-upgrade
      formulae:
        - jq
        - felixkratz/formulae/borders
      casks:
        - example-browser
      mas:
        - name: Example App
          id: "497_799_835"
    uv:
      - example-uv-tool
    npm:
      - example-npm-global'
assert_equals "the package parser on a fixture" \
  "$(declared_launchable_packages <<<"$PACKAGE_DATA_FIXTURE" | LC_ALL=C sort | paste -sd, -)" \
  "casks|example-browser,formulae|borders,formulae|jq,mas|Example App"

mapfile -t PACKAGE_RECORDS < <(declared_launchable_packages <"$PACKAGE_DATA")
DECLARED_PACKAGES=()
declare -A PACKAGE_COUNT_IN_SECTION=()
for record in "${PACKAGE_RECORDS[@]}"; do
  record_section="${record%%"$PACKAGE_RECORD_SEPARATOR"*}"
  DECLARED_PACKAGES+=("${record#*"$PACKAGE_RECORD_SEPARATOR"}")
  PACKAGE_COUNT_IN_SECTION["$record_section"]=$((${PACKAGE_COUNT_IN_SECTION[$record_section]:-0} + 1))
done
((${#DECLARED_PACKAGES[@]} > 0)) ||
  fail "no launchable packages were parsed out of $PACKAGE_DATA; invariant 1 would fail every binding for the wrong reason"
# Every section this parser claims to read has to have yielded something. One
# section going quiet, through an indentation change or a renamed key, is what a
# whole-file count guard misses: it would silently stop vouching for every
# binding that section covers while the other sections keep the count healthy.
for launchable_section in "${LAUNCHABLE_PACKAGE_SECTIONS[@]}" "$MAS_PACKAGE_SECTION"; do
  ((${PACKAGE_COUNT_IN_SECTION[$launchable_section]:-0} > 0)) ||
    fail "the package-data parser read no entries at all from the '$launchable_section' section of $PACKAGE_DATA, so it is not reading the sections it claims to; invariant 1 cannot be trusted"
done

MANAGED_TARGET_PATHS="$(chezmoi managed \
  --source "$REPO_ROOT" \
  --destination "$HOME" \
  --include="$CHEZMOI_DELIVERING_ENTRY_TYPES" \
  --path-style=relative)" ||
  fail "chezmoi managed failed against source $REPO_ROOT; the helper-script arm of invariant 1 cannot be decided"
[[ -n $MANAGED_TARGET_PATHS ]] ||
  fail "chezmoi managed listed no file entries for source $REPO_ROOT; the helper-script arm would fail every path for the wrong reason"

# ---- gather the live bindings ----------------------------------------------
# A binding's value can span several lines as a TOML array, so lines are
# accumulated until the array closes. Comment text never enters, whole-line or
# trailing: a commented-out binding is not a binding, and a comment naming a
# target the file does not bind would be checked as though it did.
BINDING_KEYS=()
BINDING_VALUES=()
current_key=''
current_value=''
in_array=0
line_number=0
while IFS= read -r raw_line; do
  line_number=$((line_number + 1))
  line="$(toml_line_without_comment "$raw_line")" ||
    fail "$AEROSPACE_CONFIG line $line_number ends inside an unterminated string: '$raw_line'. This test decides where a comment starts by tracking quote state and cannot answer for a line TOML itself would reject"
  [[ -n ${line//[[:space:]]/} ]] || continue
  if ((in_array)); then
    current_value+=" $line"
    [[ $line == *']'* ]] || continue
    in_array=0
    BINDING_KEYS+=("$current_key")
    BINDING_VALUES+=("$current_value")
    continue
  fi
  [[ $line =~ ^[[:space:]]*([A-Za-z0-9_-]+)[[:space:]]*=[[:space:]]*(.*)$ ]] || continue
  current_key="${BASH_REMATCH[1]}"
  current_value="${BASH_REMATCH[2]}"
  if [[ $current_value == \[* && $current_value != *\]* ]]; then
    in_array=1
    continue
  fi
  BINDING_KEYS+=("$current_key")
  BINDING_VALUES+=("$current_value")
done <"$AEROSPACE_CONFIG"
((in_array == 0)) ||
  fail "$AEROSPACE_CONFIG ends inside an unterminated array starting at key '$current_key'; the file is not valid TOML"
((${#BINDING_KEYS[@]} > 0)) ||
  fail "no key/value lines were parsed out of $AEROSPACE_CONFIG; both invariants would pass vacuously"

# ---- 1: every launched application, command and script exists --------------
checked_commands=0
checked_applications=0
declared_commands=0
declared_applications=0
for index in "${!BINDING_KEYS[@]}"; do
  value="${BINDING_VALUES[$index]}"
  key="${BINDING_KEYS[$index]}"
  # What the binding SAYS it launches, counted before anything is parsed out of
  # it. The two counts are reconciled below, so an extraction that stops seeing
  # a spelling fails instead of quietly checking fewer targets.
  declared_commands=$((declared_commands + $(occurrence_count "$EXEC_AND_FORGET_PATTERN" "$value")))
  declared_applications=$((declared_applications + $(occurrence_count "$OPEN_APPLICATION_FLAG_PATTERN" "$value")))

  while IFS= read -r match; do
    [[ -n $match ]] || continue
    target="$(command_of_exec_and_forget_invocation "$match")" ||
      fail "$AEROSPACE_CONFIG binds '$key' to '$match', which this test recognised as an exec-and-forget but could not read a command out of"
    checked_commands=$((checked_commands + 1))
    if [[ $target == "$HOME_SCRIPT_PREFIX"* ]]; then
      script_target="$HOME_SCRIPT_TARGET_PREFIX${target#"$HOME_SCRIPT_PREFIX"}"
      printf '%s\n' "$MANAGED_TARGET_PATHS" | grep -Fxq -- "$script_target" ||
        fail "$AEROSPACE_CONFIG binds '$key' to $target, but this repo delivers nothing to $script_target (checked with 'chezmoi managed'). The chord runs nothing on a fresh machine"
      continue
    fi
    is_command_available "$(unqualified_package_name "$target")" ||
      fail "$AEROSPACE_CONFIG binds '$key' to the command '$target', which no package in $PACKAGE_DATA installs and which is not a macOS system command. The chord runs nothing on a fresh machine; declare the package, or add the binary-to-package alias if the names differ"
  done < <(grep -oE -- "$EXEC_AND_FORGET_COMMAND_PATTERN" <<<"$value" || true)

  while IFS= read -r match; do
    [[ -n $match ]] || continue
    application="$(application_of_open_invocation "$match")" ||
      fail "$AEROSPACE_CONFIG binds '$key' to '$match', which this test recognised as an 'open -a' but could not read an application name out of"
    checked_applications=$((checked_applications + 1))
    is_application_available "$application" ||
      fail "$AEROSPACE_CONFIG binds '$key' to 'open -a $application', but no cask, formula or Mac App Store entry in $PACKAGE_DATA installs it and it is not an app macOS ships. The chord opens nothing on a fresh machine; declare the app, or bind the chord to one that is declared"
  done < <(grep -oE -- "$OPEN_APPLICATION_PATTERN" <<<"$value" || true)
done
checked_targets=$((checked_commands + checked_applications))
((checked_targets > 0)) ||
  fail "invariant 1 examined no launch targets at all in $AEROSPACE_CONFIG; the extraction no longer matches the file"
((checked_commands == declared_commands)) ||
  fail "$AEROSPACE_CONFIG names $declared_commands exec-and-forget command(s) in its live bindings but invariant 1 checked $checked_commands of them. An extraction blind to one spelling reports nothing; it just stops asking about that target"
((checked_applications == declared_applications)) ||
  fail "$AEROSPACE_CONFIG names $declared_applications 'open -a' application(s) in its live bindings but invariant 1 checked $checked_applications of them. A quoted argument ('open -a \"Arc\"') is the spelling that used to slip through this way"

# ---- 2: hjkl bindings name the right direction -----------------------------
checked_directions=0
for index in "${!BINDING_KEYS[@]}"; do
  letter="$(hjkl_letter_of_binding_key "${BINDING_KEYS[$index]}")"
  [[ -n $letter ]] || continue
  expected_direction="${DIRECTION_FOR_KEY[$letter]}"
  while IFS= read -r direction; do
    [[ -n $direction ]] || continue
    checked_directions=$((checked_directions + 1))
    [[ $direction == "$expected_direction" ]] ||
      fail "$AEROSPACE_CONFIG binds '${BINDING_KEYS[$index]}' to a command naming '$direction', but every other hjkl group in this file maps $letter to $expected_direction. A transposed pair is not an error AeroSpace can report; it just moves windows the wrong way"
  done < <(direction_words_in_binding "${BINDING_VALUES[$index]}")
done
((checked_directions > 0)) ||
  fail "invariant 2 examined no hjkl direction words at all in $AEROSPACE_CONFIG; the extraction no longer matches the file"

printf 'aerospace-binding-targets: OK (%d launch targets all installable, %d hjkl direction words all consistent)\n' \
  "$checked_targets" "$checked_directions"
