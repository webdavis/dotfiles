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
# TWO DECODING LAYERS, KEPT APART. A binding's text passes through TOML and then
# through a shell before AeroSpace launches anything, and each layer removes a
# layer of quoting. `"exec-and-forget open -a \"Zen\""` is the TOML spelling of
# the command `exec-and-forget open -a "Zen"`, which is the shell spelling of
# the word `Zen`. An earlier version of this file matched application names out
# of the RAW SOURCE TEXT with one regex, which conflated the two layers and got
# both directions wrong: it read that binding's application as `\` and refused a
# valid file, and it read `open -a "Zen"Beta` (which a shell joins into the
# single word `ZenBeta`) as the declared `Zen`. So the pipeline here is explicit
# and each step is its own pure function: strip comments, split the value into
# TOML strings and decode them, tokenise each decoded command line the way a
# shell does, then read the launch targets out of the tokens.
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
# WHY THERE IS NO LONGER A COUNT RECONCILIATION. An earlier version counted the
# launch keywords with one regex and the launch targets with another, and
# demanded the two agree, on the reasoning that a text extractor fails by
# quietly answering about fewer targets rather than by answering wrong. The
# reasoning holds; the implementation did not, because the counter and the
# extractor shared a pattern and so shared its blind spots. `open -n -a Arc`,
# which open(1) accepts and which spells the application flag as a separate
# argument, was invisible to BOTH: the run stayed green having silently checked
# one target fewer, with the counts agreeing at the lower number. What replaces
# it is stronger and cannot be defeated that way: every token that NAMES a
# launch (`exec-and-forget`, `open`) has to resolve to a target, and an
# invocation this test cannot read an argument out of is a failure rather than a
# skip. The remaining counters exist only to refuse a vacuous pass.
#
# NOT MODELLED, on purpose and out loud: TOML multi-line strings (''' and """).
# They are valid TOML, this test reads the file one line at a time, and a value
# spanning lines would desynchronise the quote scan. A binding that uses one is
# refused by name rather than mis-parsed, so the limit cannot pass for coverage.
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
# Filled from the package data below. Declared here so the predicate self-tests
# can exercise the availability predicates before it is populated, and so a
# mutation that removed a guard would fail on the assertion rather than on an
# unbound variable.
DECLARED_PACKAGES=()
# The hjkl convention this file follows everywhere else. Bindings on these keys
# that name no direction at all (resize mode says `resize width -50`) are not
# covered by it and are skipped rather than guessed at.
declare -A DIRECTION_FOR_KEY=(
  [h]=left
  [j]=down
  [k]=up
  [l]=right
)
# The direction words a binding can name, as their own list so the scan below
# reads tokens against data rather than an inline alternation.
DIRECTION_WORDS=(left down up right)
# How this config spells the path to a deployed helper script. The tilde is
# literal on purpose: this is the text AeroSpace stores and hands to the shell,
# not a path this test resolves, so expanding it here would stop it matching.
# shellcheck disable=SC2088
HOME_SCRIPT_PREFIX='~/.local/bin/'
# The same location as a chezmoi target path, which is what `chezmoi managed`
# answers in.
HOME_SCRIPT_TARGET_PREFIX='.local/bin/'
CHEZMOI_DELIVERING_ENTRY_TYPES='files'
# Lines of filler the delivery-lookup probe below builds. Each is about 27
# bytes, so this clears a 64 KiB pipe buffer several times over, which is what
# the probe needs in order to reproduce the SIGPIPE it exists to rule out.
PIPE_BUFFER_OVERRUN_LINE_COUNT=20000
# Field separator for the package parser's records. Neither a section name
# ([a-z_]+) nor a package name can contain it.
PACKAGE_RECORD_SEPARATOR='|'
# The two tokens that name a launch in a binding. Every occurrence of either has
# to yield a target; see the header on why they are not merely counted.
EXEC_AND_FORGET_KEYWORD='exec-and-forget'
OPEN_COMMAND='open'
# open(1) reads `-a <application>`, its short options cluster (`-na X`), and it
# stops reading its own arguments at `--args`. Verified against `open -h` on
# macOS 26.2.
OPEN_APPLICATION_OPTION_LETTER='a'
OPEN_SHORT_OPTION_CLUSTER_PATTERN='^-[a-zA-Z]+$'
OPEN_ARGUMENTS_END_MARKER='--args'
# The quoting characters both decoding layers turn on, named so the scanners
# below can match them as data rather than as fragile literals. BACKSLASH is
# spelled $'\\' rather than '\' because the latter reads as an escaped quote to
# a linter even where it is not one.
SINGLE_QUOTE="'"
DOUBLE_QUOTE='"'
BACKSLASH=$'\\'
# TOML's multi-line string delimiters. Not modelled; see the header.
MULTILINE_STRING_DELIMITERS=("'''" '"""')
# What a structural scan puts in place of a string's body, so that a bracket
# inside a string cannot answer a question about the line's structure. Any byte
# that is neither a bracket, a quote nor `#` works.
TOML_STRING_BODY_MASK='_'
# The two modes the one TOML line scanner runs in. Named rather than spelled
# yes/no at each call site, because which one a caller wants is the whole
# difference between reading a binding's text and reading its structure.
SCAN_KEEPING_STRING_BODIES=keep
SCAN_MASKING_STRING_BODIES=mask

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
# literal string ('...') takes no escapes, a basic string ("...") does.
#
# In SCAN_MASKING_STRING_BODIES mode every character inside a string is replaced
# by TOML_STRING_BODY_MASK, which is what lets a caller ask a STRUCTURAL
# question of the line (does it close an array?) without a bracket inside a
# string answering it: `-message "]"` closed the array accumulator four lines
# early and dropped every target below it, silently.
#
# Pure, and the single place quote state is tracked, so the text answer and the
# structural answer cannot drift apart. Returns non-zero when the line ends
# inside an unterminated string.
scan_toml_line() {
  local mode="$1" line="$2" character index scanned=''
  local in_literal_string=0 in_basic_string=0 escaped=0
  case "$mode" in
    "$SCAN_KEEPING_STRING_BODIES" | "$SCAN_MASKING_STRING_BODIES") ;;
    *) fail "scan_toml_line was asked for mode '$mode', which is neither $SCAN_KEEPING_STRING_BODIES nor $SCAN_MASKING_STRING_BODIES" ;;
  esac
  for ((index = 0; index < ${#line}; index++)); do
    character="${line:index:1}"
    if ((escaped)); then
      escaped=0
      [[ $mode == "$SCAN_MASKING_STRING_BODIES" ]] && character="$TOML_STRING_BODY_MASK"
      scanned+="$character"
      continue
    fi
    if ((in_basic_string)); then
      case "$character" in
        "$BACKSLASH")
          escaped=1
          [[ $mode == "$SCAN_MASKING_STRING_BODIES" ]] && character="$TOML_STRING_BODY_MASK"
          ;;
        "$DOUBLE_QUOTE") in_basic_string=0 ;;
        *) [[ $mode == "$SCAN_MASKING_STRING_BODIES" ]] && character="$TOML_STRING_BODY_MASK" ;;
      esac
      scanned+="$character"
      continue
    fi
    if ((in_literal_string)); then
      if [[ $character == "$SINGLE_QUOTE" ]]; then
        in_literal_string=0
      elif [[ $mode == "$SCAN_MASKING_STRING_BODIES" ]]; then
        character="$TOML_STRING_BODY_MASK"
      fi
      scanned+="$character"
      continue
    fi
    case "$character" in
      "$SINGLE_QUOTE") in_literal_string=1 ;;
      "$DOUBLE_QUOTE") in_basic_string=1 ;;
      '#')
        printf '%s\n' "$scanned"
        return 0
        ;;
    esac
    scanned+="$character"
  done
  printf '%s\n' "$scanned"
  ((in_literal_string == 0 && in_basic_string == 0))
}

# Answers "what does this TOML line say once its comment is removed?". Pure.
toml_line_without_comment() {
  scan_toml_line "$SCAN_KEEPING_STRING_BODIES" "$1"
}

# Answers "what is this TOML line's structure?", i.e. the same line with every
# string's body blanked out, so brackets and separators inside a string cannot
# be mistaken for the real ones. Pure.
toml_line_structure() {
  scan_toml_line "$SCAN_MASKING_STRING_BODIES" "$1"
}

# Answers "does this line hold a TOML multi-line string delimiter?". Pure.
names_multiline_string() {
  local line="$1" delimiter
  for delimiter in "${MULTILINE_STRING_DELIMITERS[@]}"; do
    [[ $line == *"$delimiter"* ]] && return 0
  done
  return 1
}

# Answers "which strings does this TOML value hold?", emitting each one DECODED,
# one per line: a literal string's body verbatim, a basic string's escapes
# resolved. A binding's value is one string or an array of them, and what
# AeroSpace hands to a shell is the DECODED text, so this is the layer at which
# `\"Zen\"` becomes `"Zen"`. Pure. Returns non-zero when the value ends inside
# an unterminated string.
toml_strings_in_value() {
  local value="$1" character index body=''
  local in_literal_string=0 in_basic_string=0 escaped=0
  for ((index = 0; index < ${#value}; index++)); do
    character="${value:index:1}"
    if ((escaped)); then
      escaped=0
      case "$character" in
        "$DOUBLE_QUOTE" | "$BACKSLASH") body+="$character" ;;
        n) body+=$'\n' ;;
        t) body+=$'\t' ;;
        *) body+="$BACKSLASH$character" ;;
      esac
      continue
    fi
    if ((in_basic_string)); then
      case "$character" in
        "$BACKSLASH") escaped=1 ;;
        "$DOUBLE_QUOTE")
          in_basic_string=0
          printf '%s\n' "$body"
          body=''
          ;;
        *) body+="$character" ;;
      esac
      continue
    fi
    if ((in_literal_string)); then
      if [[ $character == "$SINGLE_QUOTE" ]]; then
        in_literal_string=0
        printf '%s\n' "$body"
        body=''
      else
        body+="$character"
      fi
      continue
    fi
    case "$character" in
      "$SINGLE_QUOTE") in_literal_string=1 ;;
      "$DOUBLE_QUOTE") in_basic_string=1 ;;
    esac
  done
  ((in_literal_string == 0 && in_basic_string == 0))
}

# Answers "which words does this shell command line consist of?", one per line,
# applying the shell's own rules: a single-quoted run is verbatim, a
# double-quoted run resolves the four escapes a shell honours there, a bare
# backslash escapes the next character, and adjacent runs JOIN into one word, so
# `"Zen"Beta` is the single word `ZenBeta` and not the declared app `Zen`.
# AeroSpace hands a binding's command to a shell, so these words are what
# open(1) and the launched program actually receive. Pure. Returns non-zero when
# the line ends inside a quote or on a dangling backslash.
shell_words_of_command_line() {
  local line="$1" character index word='' have_word=0
  local in_single_quotes=0 in_double_quotes=0 escaped=0
  for ((index = 0; index < ${#line}; index++)); do
    character="${line:index:1}"
    if ((escaped)); then
      escaped=0
      if ((in_double_quotes)); then
        case "$character" in
          "$DOUBLE_QUOTE" | "$BACKSLASH" | '$' | '`') word+="$character" ;;
          *) word+="$BACKSLASH$character" ;;
        esac
      else
        word+="$character"
      fi
      have_word=1
      continue
    fi
    if ((in_single_quotes)); then
      if [[ $character == "$SINGLE_QUOTE" ]]; then
        in_single_quotes=0
      else
        word+="$character"
      fi
      continue
    fi
    if ((in_double_quotes)); then
      case "$character" in
        "$BACKSLASH") escaped=1 ;;
        "$DOUBLE_QUOTE") in_double_quotes=0 ;;
        *) word+="$character" ;;
      esac
      continue
    fi
    case "$character" in
      "$SINGLE_QUOTE")
        in_single_quotes=1
        have_word=1
        ;;
      "$DOUBLE_QUOTE")
        in_double_quotes=1
        have_word=1
        ;;
      "$BACKSLASH")
        escaped=1
        have_word=1
        ;;
      [[:space:]])
        ((have_word)) && printf '%s\n' "$word"
        word=''
        have_word=0
        ;;
      *)
        word+="$character"
        have_word=1
        ;;
    esac
  done
  ((have_word)) && printf '%s\n' "$word"
  ((in_single_quotes == 0 && in_double_quotes == 0 && escaped == 0))
}

# Answers "which application does this `open` argument list launch?", walking it
# the way open(1) reads it: `-a` takes the NEXT argument and short options
# cluster, so `-a X`, `-na X` and `-n -a X` all name X, and everything past
# `--args` belongs to the launched program. Prints the application. Returns
# non-zero when the list names no application through `-a`, which the caller
# must refuse rather than skip: a spelling this cannot read (`open -b <bundle
# id>`, `open <file>`) is a target going unchecked. Pure.
application_of_open_arguments() {
  local -a arguments=("$@")
  local index argument
  for ((index = 0; index < ${#arguments[@]}; index++)); do
    argument="${arguments[index]}"
    [[ $argument == "$OPEN_ARGUMENTS_END_MARKER" ]] && return 1
    if [[ $argument =~ $OPEN_SHORT_OPTION_CLUSTER_PATTERN &&
      $argument == *"$OPEN_APPLICATION_OPTION_LETTER"* ]]; then
      ((index + 1 < ${#arguments[@]})) || return 1
      printf '%s\n' "${arguments[index + 1]}"
      return 0
    fi
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
    # Any key shallower than the section keys closes the current section. A
    # section written as an empty inline list (`mas: []`) needs no arm of its
    # own: it opens no section, and the next key at any modelled indent closes
    # whatever was open, so it can contribute nothing and can capture nothing.
    # The fixture below pins that.
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
# An empty name is refused rather than looked up: bash treats an empty
# associative-array subscript as an error, so `open -a ""` used to abort the run
# with a bash diagnostic instead of a named failure.
is_application_available() {
  local application="$1" package
  [[ -n $application ]] || return 1
  list_contains "$application" "${APPLE_SYSTEM_APPS[@]}" && return 0
  list_contains "$application" "${DECLARED_PACKAGES[@]}" && return 0
  package="${APPLICATION_TO_PACKAGE[$application]:-$(package_name_for_application "$application")}"
  list_contains "$package" "${DECLARED_PACKAGES[@]}"
}

# Answers "does this repo install something that provides this command?". An
# empty name is refused for the same reason is_application_available refuses it.
is_command_available() {
  local command_name="$1" package
  [[ -n $command_name ]] || return 1
  list_contains "$command_name" "${SYSTEM_COMMANDS[@]}" && return 0
  package="${COMMAND_TO_PACKAGE[$command_name]:-$command_name}"
  list_contains "$package" "${DECLARED_PACKAGES[@]}"
}

# Answers "is this target path in this managed-path list?". Takes the list as an
# argument rather than reading a global, so the self-tests below can drive it.
# Reads it from a here-string rather than through a pipe on purpose: `printf ...
# | grep -q` makes printf die of SIGPIPE the moment grep finds an early match in
# a list longer than the pipe buffer, and under `set -o pipefail` that 141 reads
# as "no match" and refuses a path the repo does in fact deliver. Today's list
# is 5.7 kB, so the defect is latent rather than live; a here-string cannot have
# it at any size. Pure.
delivers_target_path() {
  local managed_target_paths="$1" target_path="$2"
  grep -Fxq -- "$target_path" <<<"$managed_target_paths"
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
assert_predicate() {
  local predicate="$1" expected="$2" actual=no
  shift 2
  "$predicate" "$@" >/dev/null && actual=yes
  [[ $actual == "$expected" ]] ||
    fail "$predicate answered '$actual' for [$*], expected '$expected'; a helper these invariants depend on no longer discriminates"
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
assert_predicate toml_line_without_comment no "    alt-b = 'exec-and-forget open -a Zen"
assert_predicate toml_line_without_comment yes "    alt-b = 'exec-and-forget open -a Zen'"
# The structural scan is what decides where a multi-line array ends. A bracket
# inside a string must not show up in its answer; one outside must.
assert_equals "toml_line_structure blanks a bracket inside a literal string" \
  "$(toml_line_structure "      'notify -message \"]\" -group x',")" \
  "      '____________________________',"
assert_equals "toml_line_structure keeps a bracket outside a string" \
  "$(toml_line_structure "    ]")" '    ]'
assert_equals "toml_line_structure blanks an escaped quote inside a basic string" \
  "$(toml_line_structure '    k = "a \"b\" c"')" '    k = "_________"'
assert_predicate names_multiline_string yes "    k = '''"
assert_predicate names_multiline_string yes '    k = """'
assert_predicate names_multiline_string no "    k = 'exec-and-forget open -a Zen'"

assert_equals "toml_strings_in_value on a literal string" \
  "$(toml_strings_in_value "'exec-and-forget open -a Zen'")" 'exec-and-forget open -a Zen'
assert_equals "toml_strings_in_value on a basic string with escaped quotes" \
  "$(toml_strings_in_value '"exec-and-forget open -a \"Zen\""')" 'exec-and-forget open -a "Zen"'
assert_equals "toml_strings_in_value on an array of two strings" \
  "$(toml_strings_in_value "['join-with down', 'mode main']" | paste -sd, -)" 'join-with down,mode main'
assert_equals "toml_strings_in_value on an array holding a bracket in a string" \
  "$(toml_strings_in_value "['notify -message \"]\"', 'open -a Arc']" | paste -sd, -)" \
  'notify -message "]",open -a Arc'
assert_equals "toml_strings_in_value on an empty array" "$(toml_strings_in_value '[]')" ''
assert_predicate toml_strings_in_value no "'exec-and-forget open -a Zen"

assert_equals "shell_words_of_command_line on a bare argument list" \
  "$(shell_words_of_command_line 'open -a Zen' | paste -sd, -)" 'open,-a,Zen'
assert_equals "shell_words_of_command_line on an escaped space" \
  "$(shell_words_of_command_line 'open -a Activity\ Monitor' | paste -sd, -)" 'open,-a,Activity Monitor'
assert_equals "shell_words_of_command_line on a double-quoted argument" \
  "$(shell_words_of_command_line 'open -a "Brave Browser"' | paste -sd, -)" 'open,-a,Brave Browser'
assert_equals "shell_words_of_command_line joining adjacent quoted runs" \
  "$(shell_words_of_command_line 'open -a "Zen"Beta' | paste -sd, -)" 'open,-a,ZenBeta'
assert_equals "shell_words_of_command_line on a single-quoted argument holding spaces" \
  "$(shell_words_of_command_line "osascript -e 'tell application \"System Events\" to quit'" | paste -sd, -)" \
  'osascript,-e,tell application "System Events" to quit'
assert_equals "shell_words_of_command_line collapsing runs of whitespace" \
  "$(shell_words_of_command_line '  open   -a   Zen  ' | paste -sd, -)" 'open,-a,Zen'
assert_predicate shell_words_of_command_line no 'open -a "Zen'
assert_predicate shell_words_of_command_line yes 'open -a "Zen"'

assert_equals "application_of_open_arguments on the plain flag" \
  "$(application_of_open_arguments -a Zen)" Zen
assert_equals "application_of_open_arguments on a clustered flag" \
  "$(application_of_open_arguments -na 'Brave Browser' --args --incognito)" 'Brave Browser'
assert_equals "application_of_open_arguments on separately spelled options" \
  "$(application_of_open_arguments -n -a Arc)" Arc
assert_predicate application_of_open_arguments no -b com.example.app
assert_predicate application_of_open_arguments no -a
assert_predicate application_of_open_arguments no --args -a Zen
# An empty name is a bash associative-array subscript error, not a lookup miss.
assert_predicate is_application_available no ''
assert_predicate is_command_available no ''

# The delivery lookup, against a list longer than a pipe buffer (64 KiB on this
# platform), with the match deliberately on the FIRST line: that is what makes
# grep exit while the writer still has the rest to push, which is the shape a
# pipe-based lookup loses to SIGPIPE. Both directions, so the probe cannot pass
# by answering yes to everything.
LONG_MANAGED_LIST_PROBE="$(
  printf '%s\n' "${HOME_SCRIPT_TARGET_PREFIX}probe-target"
  seq 1 "$PIPE_BUFFER_OVERRUN_LINE_COUNT" | sed 's|^|.local/share/filler/|'
)"
delivers_target_path "$LONG_MANAGED_LIST_PROBE" "${HOME_SCRIPT_TARGET_PREFIX}probe-target" ||
  fail "the managed-path lookup lost a path that is on line 1 of a ${#LONG_MANAGED_LIST_PROBE}-byte list; a pipe into 'grep -q' does that, because grep exits on the match, the writer dies of SIGPIPE and 'set -o pipefail' turns 141 into 'not found'"
if delivers_target_path "$LONG_MANAGED_LIST_PROBE" "${HOME_SCRIPT_TARGET_PREFIX}absent-target"; then
  fail "the managed-path lookup claimed a path that is not in the list; invariant 1 would vouch for a helper script this repo does not deliver"
fi
unset LONG_MANAGED_LIST_PROBE

# The package parser is exercised against a FIXTURE, not against whatever is
# declared today. Naming live packages here would fail this test the day the
# operator swaps a browser, which is a config choice and not a defect; invariant
# 1 is what ties the parser to the live file, by demanding a declaration for
# every application a binding actually launches.
#
# The fixture is where per-SECTION completeness is asserted, and it carries an
# entry in every section this parser claims to read, so a section going quiet
# (a renamed key, a changed indent) fails here. It deliberately does NOT get
# asserted against the live file: `mas: []` is an empty list the package
# template supports, and a live per-section count would refuse it. That is safe
# to give up because a section going quiet can only make invariant 1 STRICTER,
# never laxer: fewer declared packages means a binding that used to resolve now
# fails by name.
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
for fixture_section in "${LAUNCHABLE_PACKAGE_SECTIONS[@]}" "$MAS_PACKAGE_SECTION"; do
  assert_equals "the package parser reads the '$fixture_section' section" \
    "$(declared_launchable_packages <<<"$PACKAGE_DATA_FIXTURE" |
      grep -c "^$fixture_section$PACKAGE_RECORD_SEPARATOR" || true)" \
    "$([[ $fixture_section == formulae ]] && printf 2 || printf 1)"
done
# An empty inline list declares no entries, contributes none, and captures none
# from the sections around it. `mas: []` is a shape the package template
# supports (it renders to no `mas` lines), so the parser has to answer for it:
# an earlier live per-section count refused it outright.
assert_equals "the package parser on a section written as an empty list" \
  "$(declared_launchable_packages <<<'packages:
  macos:
    homebrew:
      casks:
        - example-browser
      mas: []
    uv:
      - example-uv-tool' | LC_ALL=C sort | paste -sd, -)" \
  "casks|example-browser"

mapfile -t PACKAGE_RECORDS < <(declared_launchable_packages <"$PACKAGE_DATA")
DECLARED_PACKAGES=()
for record in "${PACKAGE_RECORDS[@]}"; do
  DECLARED_PACKAGES+=("${record#*"$PACKAGE_RECORD_SEPARATOR"}")
done
((${#DECLARED_PACKAGES[@]} > 0)) ||
  fail "no launchable packages were parsed out of $PACKAGE_DATA; invariant 1 would fail every binding for the wrong reason"

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
# accumulated until the array closes, and whether it closes is read off the
# STRUCTURAL scan so a bracket inside a string cannot end it early. Comment text
# never enters, whole-line or trailing: a commented-out binding is not a
# binding, and a comment naming a target the file does not bind would be checked
# as though it did.
#
# The read loop keeps a final line that carries no newline, which `read` alone
# reports as end of input: a binding written on the last line of the file would
# otherwise drop out of both invariants without a word.
BINDING_KEYS=()
BINDING_VALUES=()
current_key=''
current_value=''
in_array=0
line_number=0
while IFS= read -r raw_line || [[ -n $raw_line ]]; do
  line_number=$((line_number + 1))
  ! names_multiline_string "$raw_line" ||
    fail "$AEROSPACE_CONFIG line $line_number uses a TOML multi-line string (''' or \"\"\"): '$raw_line'. That is valid TOML, but this test reads the file one line at a time and does not model a value that spans lines; keep each binding's value on one line"
  line="$(toml_line_without_comment "$raw_line")" ||
    fail "$AEROSPACE_CONFIG line $line_number ends inside an unterminated string: '$raw_line'. This test decides where a comment starts by tracking quote state and cannot answer for a line TOML itself would reject"
  line_structure="$(toml_line_structure "$raw_line")"
  [[ -n ${line//[[:space:]]/} ]] || continue
  if ((in_array)); then
    current_value+=" $line"
    [[ $line_structure == *']'* ]] || continue
    in_array=0
    BINDING_KEYS+=("$current_key")
    BINDING_VALUES+=("$current_value")
    continue
  fi
  [[ $line =~ ^[[:space:]]*([A-Za-z0-9_-]+)[[:space:]]*=[[:space:]]*(.*)$ ]] || continue
  current_key="${BASH_REMATCH[1]}"
  current_value="${BASH_REMATCH[2]}"
  if [[ $line_structure == *'['* && $line_structure != *']'* ]]; then
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
# Every token that NAMES a launch has to resolve to a target. An invocation this
# test cannot read a target out of fails by name; it is never skipped, which is
# how `open -n -a Arc` used to go unchecked while the run stayed green.
checked_commands=0
checked_applications=0
for index in "${!BINDING_KEYS[@]}"; do
  key="${BINDING_KEYS[$index]}"
  while IFS= read -r command_line; do
    [[ -n ${command_line//[[:space:]]/} ]] || continue
    mapfile -t command_words < <(shell_words_of_command_line "$command_line") ||
      fail "$AEROSPACE_CONFIG binds '$key' to '$command_line', which ends inside a quote; a shell would reject it and this test cannot say what it launches"
    for word_index in "${!command_words[@]}"; do
      case "${command_words[$word_index]}" in
        "$EXEC_AND_FORGET_KEYWORD")
          ((word_index + 1 < ${#command_words[@]})) ||
            fail "$AEROSPACE_CONFIG binds '$key' to '$command_line', whose $EXEC_AND_FORGET_KEYWORD names no command at all"
          target="${command_words[word_index + 1]}"
          checked_commands=$((checked_commands + 1))
          if [[ $target == "$HOME_SCRIPT_PREFIX"* ]]; then
            script_target="$HOME_SCRIPT_TARGET_PREFIX${target#"$HOME_SCRIPT_PREFIX"}"
            delivers_target_path "$MANAGED_TARGET_PATHS" "$script_target" ||
              fail "$AEROSPACE_CONFIG binds '$key' to $target, but this repo delivers nothing to $script_target (checked with 'chezmoi managed'). The chord runs nothing on a fresh machine"
            continue
          fi
          is_command_available "$(unqualified_package_name "$target")" ||
            fail "$AEROSPACE_CONFIG binds '$key' to the command '$target', which no package in $PACKAGE_DATA installs and which is not a macOS system command. The chord runs nothing on a fresh machine; declare the package, or add the binary-to-package alias if the names differ"
          ;;
        "$OPEN_COMMAND")
          application="$(application_of_open_arguments "${command_words[@]:word_index+1}")" ||
            fail "$AEROSPACE_CONFIG binds '$key' to '$command_line', an '$OPEN_COMMAND' invocation that names its application in a spelling this test does not read (it reads '-a <application>' and its clustered forms). Rewrite it as 'open -a <application>', or teach application_of_open_arguments the spelling; leaving it would let the chord's target go unchecked"
          checked_applications=$((checked_applications + 1))
          is_application_available "$application" ||
            fail "$AEROSPACE_CONFIG binds '$key' to 'open -a $application', but no cask, formula or Mac App Store entry in $PACKAGE_DATA installs it and it is not an app macOS ships. The chord opens nothing on a fresh machine; declare the app, or bind the chord to one that is declared"
          ;;
      esac
    done
  done < <(toml_strings_in_value "${BINDING_VALUES[$index]}") ||
    fail "$AEROSPACE_CONFIG binds '$key' to a value that ends inside an unterminated string; the file is not valid TOML"
done
checked_targets=$((checked_commands + checked_applications))
((checked_commands > 0)) ||
  fail "invariant 1 examined no '$EXEC_AND_FORGET_KEYWORD' commands at all in $AEROSPACE_CONFIG; the extraction no longer matches the file"
((checked_applications > 0)) ||
  fail "invariant 1 examined no '$OPEN_COMMAND' applications at all in $AEROSPACE_CONFIG; the extraction no longer matches the file"

# ---- 2: hjkl bindings name the right direction -----------------------------
checked_directions=0
for index in "${!BINDING_KEYS[@]}"; do
  letter="$(hjkl_letter_of_binding_key "${BINDING_KEYS[$index]}")"
  [[ -n $letter ]] || continue
  expected_direction="${DIRECTION_FOR_KEY[$letter]}"
  while IFS= read -r command_line; do
    [[ -n ${command_line//[[:space:]]/} ]] || continue
    mapfile -t command_words < <(shell_words_of_command_line "$command_line")
    for word in "${command_words[@]}"; do
      list_contains "$word" "${DIRECTION_WORDS[@]}" || continue
      checked_directions=$((checked_directions + 1))
      [[ $word == "$expected_direction" ]] ||
        fail "$AEROSPACE_CONFIG binds '${BINDING_KEYS[$index]}' to a command naming '$word', but every other hjkl group in this file maps $letter to $expected_direction. A transposed pair is not an error AeroSpace can report; it just moves windows the wrong way"
    done
  done < <(toml_strings_in_value "${BINDING_VALUES[$index]}")
done
((checked_directions > 0)) ||
  fail "invariant 2 examined no hjkl direction words at all in $AEROSPACE_CONFIG; the extraction no longer matches the file"

printf 'aerospace-binding-targets: OK (%d launch targets all installable, %d hjkl direction words all consistent)\n' \
  "$checked_targets" "$checked_directions"
