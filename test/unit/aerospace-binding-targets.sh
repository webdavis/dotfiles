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
#   - Only WHOLE-LINE comments are dropped. A commented-out binding is not a
#     binding; a trailing comment on a live line is left in scope because
#     ignoring it could only hide a target, never invent one.
#   - Resolution is by package NAME, not by asking brew or the filesystem. A
#     binary's name is often not its formula's: `openhue` ships as
#     openhue/cli/openhue-cli and `borders` as felixkratz/formulae/borders. Both
#     read as undeclared to a naive basename comparison, so tap qualifiers are
#     stripped and the residual mismatches live in one named alias table.
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
# executable on this machine?", one per line. Reads the raw YAML with a two-level
# indent walk rather than a parser, because no YAML parser is reachable from the
# flake's run shell once /opt/homebrew is off PATH.
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
        unqualified_package_name "$value"
      elif [[ $section == "$MAS_PACKAGE_SECTION" && $value =~ ^name:[[:space:]]*(.+)$ ]]; then
        # Mac App Store entries carry the application's real name, so they are
        # matched verbatim rather than through the Homebrew naming convention.
        printf '%s\n' "${BASH_REMATCH[1]}"
      fi
    fi
  done <"$PACKAGE_DATA"
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

mapfile -t DECLARED_PACKAGES < <(declared_launchable_packages)
((${#DECLARED_PACKAGES[@]} > 0)) ||
  fail "no launchable packages were parsed out of $PACKAGE_DATA; invariant 1 would fail every binding for the wrong reason"
for expected_package in ghostty zen borders terminal-notifier Xcode; do
  list_contains "$expected_package" "${DECLARED_PACKAGES[@]}" ||
    fail "the package-data parser did not find '$expected_package' in $PACKAGE_DATA, so it is not reading the sections it claims to; invariant 1 cannot be trusted"
done
# The mirror direction: names that live in sections this parser must NOT read.
# graphifyy is a uv tool, happy an npm global, cask-upgrade the leaf of the
# buo/cask-upgrade tap. If any of them shows up, the section walk has widened and
# an unrelated package could vouch for a binding that installs nothing.
for absent_package in graphifyy happy cask-upgrade; do
  list_contains "$absent_package" "${DECLARED_PACKAGES[@]}" &&
    fail "the package-data parser found '$absent_package', which lives outside the launchable sections of $PACKAGE_DATA; invariant 1 would accept a binding vouched for by a package that installs no application"
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
# accumulated until the array closes. Whole-line comments never enter.
BINDING_KEYS=()
BINDING_VALUES=()
current_key=''
current_value=''
in_array=0
while IFS= read -r line; do
  [[ ${line#"${line%%[![:space:]]*}"} == \#* ]] && continue
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
checked_targets=0
for index in "${!BINDING_KEYS[@]}"; do
  value="${BINDING_VALUES[$index]}"
  key="${BINDING_KEYS[$index]}"

  while IFS= read -r match; do
    [[ -n $match ]] || continue
    target="${match#*exec-and-forget}"
    target="${target#"${target%%[![:space:]]*}"}"
    checked_targets=$((checked_targets + 1))
    if [[ $target == "$HOME_SCRIPT_PREFIX"* ]]; then
      script_target="$HOME_SCRIPT_TARGET_PREFIX${target#"$HOME_SCRIPT_PREFIX"}"
      printf '%s\n' "$MANAGED_TARGET_PATHS" | grep -Fxq -- "$script_target" ||
        fail "$AEROSPACE_CONFIG binds '$key' to $target, but this repo delivers nothing to $script_target (checked with 'chezmoi managed'). The chord runs nothing on a fresh machine"
      continue
    fi
    is_command_available "$(unqualified_package_name "$target")" ||
      fail "$AEROSPACE_CONFIG binds '$key' to the command '$target', which no package in $PACKAGE_DATA installs and which is not a macOS system command. The chord runs nothing on a fresh machine; declare the package, or add the binary-to-package alias if the names differ"
  done < <(grep -oE "exec-and-forget[[:space:]]+[^[:space:]'\"]+" <<<"$value" || true)

  while IFS= read -r match; do
    [[ -n $match ]] || continue
    # The capture is `open -<flags>a <app>`; the app name is everything past the
    # flag word, with escaped spaces unescaped ("Brave\ Browser").
    application="${match##*a }"
    application="${application//\\ / }"
    checked_targets=$((checked_targets + 1))
    is_application_available "$application" ||
      fail "$AEROSPACE_CONFIG binds '$key' to 'open -a $application', but no cask, formula or Mac App Store entry in $PACKAGE_DATA installs it and it is not an app macOS ships. The chord opens nothing on a fresh machine; declare the app, or bind the chord to one that is declared"
  done < <(grep -oE "open[[:space:]]+-[a-zA-Z]*a[[:space:]]+(\\\\[[:space:]]|[^[:space:]'\"])+" <<<"$value" || true)
done
((checked_targets > 0)) ||
  fail "invariant 1 examined no launch targets at all in $AEROSPACE_CONFIG; the extraction no longer matches the file"

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
