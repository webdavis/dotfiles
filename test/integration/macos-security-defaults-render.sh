#!/usr/bin/env bash
# macos-security-defaults-render.sh -- the security-defaults baseline: the two
# settable security controls declared in .chezmoidata/macos_defaults.yaml,
# rendered by the Tier 1 runner and compared by the drift checker.
#
# The properties pinned, one per acceptance criterion:
#
#   1. The rendered Tier 1 runner writes SoftwareUpdate AutomaticCheckEnabled
#      to /Library/Preferences under sudo, with the -bool type. Asserted PER
#      FIELD on the argument vector the stubs actually receive (bash has
#      already stripped the quoting), never by whole-line substring, plus
#      exactly one `sudo -v` prelude strictly before any write.
#   2. The Safari AutoOpenSafeDownloads record renders as a user-scope write,
#      -bool false, with NO plist_path in the data and NO sudo and NO
#      -currentHost in the executed argument vector.
#   3. Drift: against the REAL data file, a matching live read reports no
#      drift, and a differing read reports drift, in BOTH directions (the
#      system record and the user record each get a mismatch scenario). The
#      matching scenario also proves both new records were actually READ, so
#      an empty or dead drift run cannot satisfy it.
#   4. The seven pre-existing Aerospace records render exactly as before, in
#      order, each pinned as its own exact line.
#   5. Both new records carry tier: enforce, and the real file still passes
#      the slice-3 tier validation (the render and the record stream both
#      refuse a tierless file, so the green render and the green drift runs
#      are the validation passing).
#
# The execution-order golden at the end of the render section is the
# completeness guard: it enumerates EVERY command the rendered runner may
# invoke, in order, so a new unasserted write cannot hide beside the pinned
# ones. Real chezmoi and yq; `defaults`, `sudo`, `osascript`, and `killall`
# are stubbed. Never runs real sudo, never writes any real preference.
set -euo pipefail

# Scrubbed at SCRIPT scope, before any git or chezmoi call. Git exports
# GIT_DIR to every hook it runs and this suite runs from the pre-push hook;
# the library's own override may be exported on a developer machine.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl"
DRIFT="$REPO_ROOT/dot_local/bin/executable_macos-defaults-drift.sh"
DEFAULTS_YAML="$REPO_ROOT/.chezmoidata/macos_defaults.yaml"

SOFTWAREUPDATE_PLIST_PATH='/Library/Preferences/com.apple.SoftwareUpdate'
LULU_PLIST_PATH='/Library/Objective-See/LuLu/preferences.plist'

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

assert_file_contains() { # <file> <fixed-string> <message>
  grep -qF -- "$2" "$1" || fail "$3"
}

# Host-tool guard: the de-homebrewed CI-faithful run has no chezmoi/yq on PATH,
# and this test cannot render templates or read records without them.
for tool in chezmoi yq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise the security-defaults baseline\n' "$tool"
    exit 0
  }
done
for required_file in "$TEMPLATE" "$DRIFT" "$DEFAULTS_YAML"; do
  [[ -f $required_file ]] || fail "missing file: $required_file"
done

# Validated BEFORE any trap is armed and before any cd: on bash 3.2 `cd ""`
# succeeds without moving, so an unguarded `cd "$(mktemp -d)"` after a failed
# mktemp would leave the suite in the worktree with an `rm -rf` trap aimed at
# it. The second assignment canonicalizes away macOS's /var -> /private/var
# symlink.
sandbox="$(mktemp -d)"
[[ -n $sandbox && -d $sandbox ]] ||
  fail "mktemp -d produced no usable sandbox directory (got '$sandbox')"
sandbox="$(cd "$sandbox" && pwd -P)"
trap 'rm -rf "$sandbox"' EXIT
render_home="$sandbox/render-home"
mkdir -p "$render_home"

# ---- render the Tier 1 runner against the repo's REAL data ------------------

rendered="$sandbox/rendered-real-data"
render_error="$sandbox/render.err"
HOME="$render_home" chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$TEMPLATE" >"$rendered" 2>"$render_error" ||
  fail "the Tier 1 runner must render against the real data (stderr: $(cat "$render_error"))"
# The skip is gated on the ACTUAL host OS, never on the render coming out
# empty: emptiness conflates "non-darwin host" (skip, by design) with "the
# template's OS guard is broken on darwin" (a failure this test exists to
# catch). An empty render on darwin must fail loudly, not skip.
[[ "$(uname)" == Darwin ]] || {
  printf 'SKIP: non-darwin host; the runner renders empty by design off darwin\n'
  exit 0
}
[[ -n "$(tr -d '[:space:]' <"$rendered")" ]] ||
  fail "the Tier 1 runner rendered EMPTY on a darwin host; its OS guard is broken (stderr: $(cat "$render_error"))"

# ---- execute the render under logging stubs ---------------------------------

# Every stub appends ONE line to the shared log: its own name, then each
# argument, all joined by the ASCII unit separator (0x1f). Bash has already
# performed quote removal by the time the stub sees its argv, so the log is
# the render's REAL per-field content, which is what criteria 1 and 2 assert.
stub_bin="$sandbox/bin"
mkdir -p "$stub_bin"
combined_log="$sandbox/execution.log"

write_logging_stub() { # <command-name>
  cat >"$stub_bin/$1" <<EOF
#!/bin/bash
{
  printf '%s' '$1'
  for argument in "\$@"; do printf '\\x1f%s' "\$argument"; done
  printf '\\n'
} >>"$combined_log"
exit 0
EOF
  chmod +x "$stub_bin/$1"
}
for stubbed_command in defaults sudo osascript killall; do
  write_logging_stub "$stubbed_command"
done

# log_line <command> [args...] -- one expected log line, same joining rule as
# the stubs, used to build expected content and grep patterns.
log_line() { # <command> [args...]
  printf '%s' "$1"
  shift
  local argument
  for argument in "$@"; do printf '\x1f%s' "$argument"; done
  printf '\n'
}

: >"$combined_log"
(
  cd "$sandbox" || exit 1
  PATH="$stub_bin:$PATH" bash "$rendered"
) >"$sandbox/execution.out" 2>&1 ||
  fail "the rendered runner must run cleanly under the stubs (output: $(cat "$sandbox/execution.out"))"
[[ -s $combined_log ]] ||
  fail 'the execution log is empty; every assertion below would pass on a dead run'

# ---- criterion 1: the SoftwareUpdate write, per field, under sudo -----------

# Command substitution strips log_line's trailing newline, leaving the exact
# whole-line pattern for grep -x.
sudo_prelude_pattern="$(log_line sudo -v)"
sudo_prelude_count="$(grep -cxF "$sudo_prelude_pattern" "$combined_log" || true)"
[[ $sudo_prelude_count -eq 1 ]] ||
  fail "the runner must validate sudo exactly once up front (got $sudo_prelude_count 'sudo -v' invocations)"

# The sudo-routed defaults-write set is exactly the enforced system-scope
# records: the SoftwareUpdate record alone. The six LuLu policy records are
# tier: verify (LuLu's extension loads its preferences once at start and
# writes them back from memory, so an external write is unobserved and
# clobbered) and must execute NOTHING, so nothing else may escalate.
sudo_write_lines="$(grep -F "$(printf 'sudo\x1fdefaults')" "$combined_log" || true)"
sudo_write_count=0
[[ -n $sudo_write_lines ]] && sudo_write_count="$(printf '%s\n' "$sudo_write_lines" | wc -l | tr -d ' ')"
[[ $sudo_write_count -eq 1 ]] ||
  fail "exactly one write must be routed through sudo, the SoftwareUpdate record (got $sudo_write_count: $sudo_write_lines)"
lulu_executed_lines="$(grep -F "$LULU_PLIST_PATH" "$combined_log" || true)"
[[ -z $lulu_executed_lines ]] ||
  fail "the six verify-tier LuLu policy records must execute NO command (got: $lulu_executed_lines)"
sudo_write_lines="$(grep -F "$(printf 'sudo\x1fdefaults')" "$combined_log" | grep -F 'AutomaticCheckEnabled' || true)"
[[ -n $sudo_write_lines ]] ||
  fail 'the SoftwareUpdate record must execute under sudo'
IFS=$'\x1f' read -r -a sudo_write_fields <<<"$sudo_write_lines"
[[ ${#sudo_write_fields[@]} -eq 7 ]] ||
  fail "the sudo write must carry exactly 7 fields, command plus 6 arguments (got ${#sudo_write_fields[@]})"
[[ ${sudo_write_fields[0]} == sudo ]] || fail "sudo write field 1 must be 'sudo' (got '${sudo_write_fields[0]}')"
[[ ${sudo_write_fields[1]} == defaults ]] || fail "sudo write field 2 must be 'defaults' (got '${sudo_write_fields[1]}')"
[[ ${sudo_write_fields[2]} == write ]] || fail "sudo write field 3 must be 'write' (got '${sudo_write_fields[2]}')"
[[ ${sudo_write_fields[3]} == "$SOFTWAREUPDATE_PLIST_PATH" ]] ||
  fail "sudo write field 4 must be the resolved plist path $SOFTWAREUPDATE_PLIST_PATH (got '${sudo_write_fields[3]}')"
[[ ${sudo_write_fields[4]} == AutomaticCheckEnabled ]] ||
  fail "sudo write field 5 must be the key AutomaticCheckEnabled (got '${sudo_write_fields[4]}')"
[[ ${sudo_write_fields[5]} == -bool ]] ||
  fail "sudo write field 6 must be the -bool type option (got '${sudo_write_fields[5]}')"
[[ ${sudo_write_fields[6]} == true ]] ||
  fail "sudo write field 7 must be the value true (got '${sudo_write_fields[6]}')"

# The prelude must come strictly before ANY write, user-scope ones included: a
# write that runs first would prompt mid-loop, the exact thing the prelude
# exists to prevent.
prelude_line_number="$(grep -nxF "$sudo_prelude_pattern" "$combined_log" | cut -d: -f1)"
first_write_line_number="$(grep -nE $'^(defaults\x1fwrite\x1f|sudo\x1fdefaults\x1fwrite\x1f)' "$combined_log" | head -1 | cut -d: -f1)"
[[ -n $first_write_line_number ]] || fail 'the execution log must contain defaults writes'
[[ $prelude_line_number -lt $first_write_line_number ]] ||
  fail "sudo -v must run before any write (sudo -v at log line $prelude_line_number, first write at $first_write_line_number)"

# ---- criterion 2: the Safari write, per field, user scope, no sudo ----------

safari_lines="$(grep -F "$(printf '\x1fcom.apple.Safari\x1f')" "$combined_log" || true)"
safari_line_count=0
[[ -n $safari_lines ]] && safari_line_count="$(printf '%s\n' "$safari_lines" | wc -l | tr -d ' ')"
[[ $safari_line_count -eq 1 ]] ||
  fail "exactly one executed command must touch com.apple.Safari (got $safari_line_count: $safari_lines)"
IFS=$'\x1f' read -r -a safari_write_fields <<<"$safari_lines"
[[ ${#safari_write_fields[@]} -eq 6 ]] ||
  fail "the Safari write must carry exactly 6 fields, so no -currentHost and no extra argument can hide (got ${#safari_write_fields[@]}: $safari_lines)"
[[ ${safari_write_fields[0]} == defaults ]] ||
  fail "Safari write field 1 must be 'defaults', not sudo: a user-scope record must not render under sudo (got '${safari_write_fields[0]}')"
[[ ${safari_write_fields[1]} == write ]] || fail "Safari write field 2 must be 'write' (got '${safari_write_fields[1]}')"
[[ ${safari_write_fields[2]} == com.apple.Safari ]] ||
  fail "Safari write field 3 must be the bare domain, never a plist path (got '${safari_write_fields[2]}')"
[[ ${safari_write_fields[3]} == AutoOpenSafeDownloads ]] ||
  fail "Safari write field 4 must be the key AutoOpenSafeDownloads (got '${safari_write_fields[3]}')"
[[ ${safari_write_fields[4]} == -bool ]] ||
  fail "Safari write field 5 must be the -bool type option (got '${safari_write_fields[4]}')"
[[ ${safari_write_fields[5]} == false ]] ||
  fail "Safari write field 6 must be the value false, do not auto-open downloads (got '${safari_write_fields[5]}')"
if grep -F "$(printf 'sudo\x1f')" "$combined_log" | grep -qF 'com.apple.Safari'; then
  fail 'no sudo invocation may touch com.apple.Safari'
fi

# The record itself must not carry a plist_path (it is user scope, where the
# field is meaningless and refused), and its scope must resolve to user. The
# yq selections double as existence checks: zero or two matching records
# breaks the joined string.
safari_plist_path_declared="$(yq eval -r \
  '[.macos.defaults[] | select(.domain == "com.apple.Safari" and .key == "AutoOpenSafeDownloads") | has("plist_path")] | join(",")' \
  "$DEFAULTS_YAML")"
[[ $safari_plist_path_declared == 'false' ]] ||
  fail "exactly one Safari record must exist and carry NO plist_path (has-plist_path per record: '$safari_plist_path_declared')"
safari_scope_resolved="$(yq eval -r \
  '[.macos.defaults[] | select(.domain == "com.apple.Safari" and .key == "AutoOpenSafeDownloads") | (.scope // "user")] | join(",")' \
  "$DEFAULTS_YAML")"
[[ $safari_scope_resolved == 'user' ]] ||
  fail "the Safari record's scope must resolve to user (got '$safari_scope_resolved')"

# ---- criterion 4: the seven Aerospace records render exactly as before ------

aerospace_expected_lines=(
  "defaults write 'com.apple.dock' 'mru-spaces' -bool 'false'"
  "defaults write 'com.apple.dock' 'expose-group-apps' -bool 'false'"
  "defaults write 'com.apple.WindowManager' 'GloballyEnabled' -bool 'false'"
  "defaults write 'com.apple.WindowManager' 'EnableStandardClickToShowDesktop' -bool 'false'"
  "defaults write 'com.apple.WindowManager' 'EnableTilingByEdgeDrag' -bool 'false'"
  "defaults write 'com.apple.WindowManager' 'EnableTilingOptionAccelerator' -bool 'false'"
  "defaults write 'com.apple.WindowManager' 'EnableTopTilingByEdgeDrag' -bool 'false'"
)
previous_line_number=0
for expected_line in "${aerospace_expected_lines[@]}"; do
  occurrence_count="$(grep -cxF "$expected_line" "$rendered" || true)"
  [[ $occurrence_count -eq 1 ]] ||
    fail "the pre-existing record must render exactly as before, exactly once (got $occurrence_count of: $expected_line)"
  line_number="$(grep -nxF "$expected_line" "$rendered" | cut -d: -f1)"
  [[ $line_number -gt $previous_line_number ]] ||
    fail "the pre-existing records must keep their relative order (line $line_number not after $previous_line_number for: $expected_line)"
  previous_line_number="$line_number"
done

# ---- execution-order golden: the completeness guard -------------------------

# EVERY command the rendered runner invokes, in order. An added, dropped, or
# reordered invocation of any kind fails this compare, so nothing can render
# beside the pinned records unasserted.
expected_execution_log="$sandbox/execution.expected"
{
  log_line osascript -e 'tell application "System Settings" to quit'
  log_line sudo -v
  log_line defaults write com.apple.dock mru-spaces -bool false
  log_line defaults write com.apple.dock expose-group-apps -bool false
  log_line defaults write com.apple.WindowManager GloballyEnabled -bool false
  log_line defaults write com.apple.WindowManager EnableStandardClickToShowDesktop -bool false
  log_line defaults write com.apple.WindowManager EnableTilingByEdgeDrag -bool false
  log_line defaults write com.apple.WindowManager EnableTilingOptionAccelerator -bool false
  log_line defaults write com.apple.WindowManager EnableTopTilingByEdgeDrag -bool false
  log_line sudo defaults write "$SOFTWAREUPDATE_PLIST_PATH" AutomaticCheckEnabled -bool true
  log_line sudo chown root:wheel "$SOFTWAREUPDATE_PLIST_PATH.plist"
  log_line sudo chmod 644 "$SOFTWAREUPDATE_PLIST_PATH.plist"
  log_line defaults write com.apple.Safari AutoOpenSafeDownloads -bool false
  log_line killall Dock
  log_line killall Finder
  log_line killall SystemUIServer
  log_line killall cfprefsd
} >"$expected_execution_log"
cmp -s "$expected_execution_log" "$combined_log" ||
  fail "the runner must invoke exactly the expected commands in order (diff: $(diff <(cat -v "$expected_execution_log") <(cat -v "$combined_log") | head -20))"

# ---- criterion 5: both records carry tier: enforce --------------------------

# The joined selection pins existence AND tier for both records at once. The
# file-level slice-3 validation (a missing, blank, or unrecognized tier
# refuses the whole file) is exercised for real above and below: the render
# would have aborted, and the drift runs would exit 2, on a file that fails it.
new_record_tiers="$(yq eval -r \
  '[.macos.defaults[] | select((.domain == "com.apple.SoftwareUpdate" and .key == "AutomaticCheckEnabled") or (.domain == "com.apple.Safari" and .key == "AutoOpenSafeDownloads")) | .tier] | join(",")' \
  "$DEFAULTS_YAML")"
[[ $new_record_tiers == 'enforce,enforce' ]] ||
  fail "both security records must exist and carry tier: enforce (got tiers: '$new_record_tiers')"

# ---- criterion 3: drift against the REAL data, both directions --------------

# The drift checker file-checks the system record's plist before consulting
# `defaults`. The scenarios below need that check to pass through to the
# stubbed read, so the real path must be absent or readable (on macOS it is a
# root-owned 0644 plist). An unreadable one is an environment this test
# cannot run in, and saying so beats a silent wrong answer.
for plist_candidate in "$SOFTWAREUPDATE_PLIST_PATH" "$SOFTWAREUPDATE_PLIST_PATH.plist" "$LULU_PLIST_PATH"; do
  if [[ -e $plist_candidate && ! -r $plist_candidate ]]; then
    fail "environment: $plist_candidate exists but is unreadable; the drift scenarios would be classified indeterminate for reasons outside the code under test"
  fi
done

# write_drift_defaults_stub <softwareupdate-answer> <safari-answer> -- replace
# the defaults stub with a read-answering one: every `read` is logged, the two
# security records answer as told, the four true-valued LuLu policy records
# answer their declared 1, and every other record answers its declared value
# (the seven Aerospace records and the two false-valued LuLu records are all
# bool false, normalized 0).
write_drift_defaults_stub() { # <softwareupdate-answer> <safari-answer>
  local softwareupdate_answer="$1" safari_answer="$2"
  cat >"$stub_bin/defaults" <<EOF
#!/bin/bash
{
  printf '%s' defaults
  for argument in "\$@"; do printf '\\x1f%s' "\$argument"; done
  printf '\\n'
} >>"$combined_log"
if [[ \$1 == read ]]; then
  case "\$2 \$3" in
    '$SOFTWAREUPDATE_PLIST_PATH AutomaticCheckEnabled') printf '%s\\n' '$softwareupdate_answer' ;;
    'com.apple.Safari AutoOpenSafeDownloads') printf '%s\\n' '$safari_answer' ;;
    '$LULU_PLIST_PATH allowLocalHost') printf '1\\n' ;;
    '$LULU_PLIST_PATH allowApple') printf '1\\n' ;;
    '$LULU_PLIST_PATH allowDNS') printf '1\\n' ;;
    '$LULU_PLIST_PATH allowInstalled') printf '1\\n' ;;
    *) printf '0\\n' ;;
  esac
fi
exit 0
EOF
  chmod +x "$stub_bin/defaults"
}

run_drift() { # <stdout-file> <stderr-file>; returns drift's status
  (
    cd "$sandbox" || exit 1
    MACOS_DEFAULTS_SOURCE_DIR="$REPO_ROOT" PATH="$stub_bin:$PATH" bash "$DRIFT"
  ) >"$1" 2>"$2"
}

drift_out="$sandbox/drift.out"
drift_err="$sandbox/drift.err"

# Matching: every live read equals its declared value; no drift, and BOTH new
# records were genuinely read. The full read log is compared, so a record
# silently skipped, or read from the wrong place, fails here.
# The `cmp` below is load-bearing: the stub's `*` fallthrough answers 0 and
# exits 0 for ANY unexpected read, so a drift that read the wrong key or the
# wrong place would get a fabricated match and still exit 0. Only this exact
# log comparison catches that (proven by mutation: drift reading a truncated
# key still exited 0; only this cmp failed). Never weaken it into a count or
# a grep.
write_drift_defaults_stub 1 0
: >"$combined_log"
run_drift "$drift_out" "$drift_err" ||
  fail "drift must exit 0 when every declared value matches (stderr: $(cat "$drift_err"))"
[[ ! -s $drift_out ]] ||
  fail "drift must print no rows when every declared value matches (got: $(cat "$drift_out"))"
expected_drift_reads="$sandbox/drift-reads.expected"
{
  log_line defaults read com.apple.dock mru-spaces
  log_line defaults read com.apple.dock expose-group-apps
  log_line defaults read com.apple.WindowManager GloballyEnabled
  log_line defaults read com.apple.WindowManager EnableStandardClickToShowDesktop
  log_line defaults read com.apple.WindowManager EnableTilingByEdgeDrag
  log_line defaults read com.apple.WindowManager EnableTilingOptionAccelerator
  log_line defaults read com.apple.WindowManager EnableTopTilingByEdgeDrag
  log_line defaults read "$SOFTWAREUPDATE_PLIST_PATH" AutomaticCheckEnabled
  log_line defaults read com.apple.Safari AutoOpenSafeDownloads
  log_line defaults read "$LULU_PLIST_PATH" allowLocalHost
  log_line defaults read "$LULU_PLIST_PATH" allowApple
  log_line defaults read "$LULU_PLIST_PATH" allowDNS
  log_line defaults read "$LULU_PLIST_PATH" allowInstalled
  log_line defaults read "$LULU_PLIST_PATH" blockMode
  log_line defaults read "$LULU_PLIST_PATH" passiveMode
} >"$expected_drift_reads"
cmp -s "$expected_drift_reads" "$combined_log" ||
  fail "drift must read every tracked record, the two security records included, from its declared place (diff: $(diff <(cat -v "$expected_drift_reads") <(cat -v "$combined_log") | head -20))"

# The system record differs (live 0, declared 1): exactly one drift row, per
# field, and the drift exit status, exactly 1, never the error status 2.
write_drift_defaults_stub 0 0
: >"$combined_log"
drift_status=0
run_drift "$drift_out" "$drift_err" || drift_status=$?
[[ $drift_status -eq 1 ]] ||
  fail "a differing SoftwareUpdate value must exit 1, drift detected (got $drift_status, stderr: $(cat "$drift_err"))"
drift_row_count="$(grep -c '' "$drift_out" || true)"
[[ $drift_row_count -eq 2 ]] ||
  fail "the SoftwareUpdate mismatch must print the header plus exactly one row (got $drift_row_count lines: $(cat "$drift_out"))"
IFS=$'\t' read -r row_domain row_key row_expected row_actual <<<"$(sed -n '2p' "$drift_out")"
[[ $row_domain == com.apple.SoftwareUpdate ]] || fail "drift row field 1 must be the domain (got '$row_domain')"
[[ $row_key == AutomaticCheckEnabled ]] || fail "drift row field 2 must be the key (got '$row_key')"
[[ $row_expected == 1 ]] || fail "drift row field 3 must be the normalized declared value 1 (got '$row_expected')"
[[ $row_actual == 0 ]] || fail "drift row field 4 must be the differing live value 0 (got '$row_actual')"
assert_file_contains "$drift_err" '1 drift row(s) detected' \
  "the mismatch summary must count exactly one drifted row (stderr: $(cat "$drift_err"))"

# The user record differs the other way (live 1, declared 0), so the compare
# is proved two-sided, not just for the system record.
write_drift_defaults_stub 1 1
: >"$combined_log"
drift_status=0
run_drift "$drift_out" "$drift_err" || drift_status=$?
[[ $drift_status -eq 1 ]] ||
  fail "a differing Safari value must exit 1, drift detected (got $drift_status, stderr: $(cat "$drift_err"))"
IFS=$'\t' read -r row_domain row_key row_expected row_actual <<<"$(sed -n '2p' "$drift_out")"
[[ $row_domain == com.apple.Safari ]] || fail "Safari drift row field 1 must be the domain (got '$row_domain')"
[[ $row_key == AutoOpenSafeDownloads ]] || fail "Safari drift row field 2 must be the key (got '$row_key')"
[[ $row_expected == 0 ]] || fail "Safari drift row field 3 must be the normalized declared value 0 (got '$row_expected')"
[[ $row_actual == 1 ]] || fail "Safari drift row field 4 must be the differing live value 1 (got '$row_actual')"
assert_file_contains "$drift_err" '1 drift row(s) detected' \
  "the Safari mismatch summary must count exactly one drifted row (stderr: $(cat "$drift_err"))"

printf 'macos-security-defaults-render: OK (SoftwareUpdate writes to /Library/Preferences under one up-front sudo -v, per field; Safari writes user-scope -bool false with no sudo, no -currentHost, no plist_path; the seven Aerospace records render exactly as before; both records carry tier: enforce; drift is silent on match and reports each mismatch direction as exactly one row)\n'
