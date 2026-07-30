#!/usr/bin/env bash
# macos-defaults-validate-before-write.sh -- a malformed macos_defaults.yaml must
# be refused ENTIRELY, before the first `defaults write`, by every tool that
# reads it.
#
# macos_defaults.yaml is a DECLARATIVE description of the machine. Applying part
# of it leaves the Mac in a state neither the old file nor the new one describes,
# and the operator has no record of where the run stopped. Refusing outright is
# strictly better, and it is what the Tier 1 runner template already does: its
# validation pass covers every record before the render emits a single write, and
# a rejected render produces no script at all.
#
# The shell reader did not hold that line. Three divergences, each reproduced
# against a stubbed `defaults` before this suite existed:
#
#   1. A valid record followed by one with an unknown scope: the valid record was
#      WRITTEN, then the run aborted with status 2. Scope validation sat inside
#      the consuming loop, after earlier writes had already landed.
#   2. A valid record followed by one with no domain: the valid record was
#      written and the malformed one was SILENTLY SKIPPED, exit 0. A malformed
#      file reported success.
#   3. A valid record followed by one with no value: BOTH were written, the
#      second with an empty value, exit 0. Same false success.
#
# Two more of the same class are pinned here because they are reachable the same
# way: a record with no type (which reached `defaults write dom key - true`, exit
# 0), and a system record whose plist_path is outside the write allowlist placed
# behind a valid record.
#
# What each case asserts is the ABSENCE of the earlier write, not merely that the
# run failed. A run that refuses AFTER writing the first record still passes a
# status check, so the status alone pins nothing about ordering.
#
# The control row comes FIRST: a file whose records are all valid must apply
# every one of them. Without it every assertion below is satisfied by an apply
# that writes nothing, ever.
#
# Real chezmoi and yq; `defaults`, `sudo`, `osascript` and `killall` are stubbed.
# Never runs real sudo, never touches /Library.
set -euo pipefail

# Scrubbed at SCRIPT scope, before any git or chezmoi call: a linked worktree
# exports GIT_DIR to the hooks it runs, and the library's own override may be
# exported on a developer machine.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"
APPLY="$REPO_ROOT/dot_local/bin/executable_macos-defaults-apply.sh"
DRIFT="$REPO_ROOT/dot_local/bin/executable_macos-defaults-drift.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# A bare `! grep` is dead under `set -e` unless it happens to be the final
# statement, so every negative goes through these helpers.
assert_file_contains() { # <file> <fixed-string> <message>
  grep -qF -- "$2" "$1" || fail "$3"
}

refute_file_contains() { # <file> <fixed-string> <message>
  if grep -qF -- "$2" "$1"; then
    fail "$3"
  fi
}

for tool in chezmoi yq; do
  command -v "$tool" >/dev/null 2>&1 ||
    fail "$tool is not on PATH; this suite renders a real template and streams real YAML, so it cannot be meaningfully skipped"
done
for required_file in "$TEMPLATE" "$LIB" "$APPLY" "$DRIFT"; do
  [[ -f $required_file ]] || fail "missing file: $required_file"
done

# Canonicalize away macOS's /var -> /private/var symlink.
sandbox="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$sandbox"' EXIT
mkdir -p "$sandbox/home" "$sandbox/render-home"

defaults_log="$sandbox/defaults.log"
sudo_log="$sandbox/sudo.log"
render_error="$sandbox/render.err"

stub_bin="$sandbox/bin"
mkdir -p "$stub_bin"
printf '#!/bin/bash\nexit 0\n' >"$stub_bin/osascript"
printf '#!/bin/bash\nexit 0\n' >"$stub_bin/killall"
cat >"$stub_bin/sudo" <<EOF
#!/bin/bash
printf '%s\n' "\$*" >>"$sudo_log"
exec "\$@"
EOF
# The stub records every invocation and answers reads with a value that DIFFERS
# from every fixture's declared value, so the drift rows below are real rows a
# truncated report would visibly lose.
cat >"$stub_bin/defaults" <<EOF
#!/bin/bash
printf '%s\n' "\$*" >>"$defaults_log"
if [[ \$1 == read ]]; then printf 'unexpected-live-value\n'; exit 0; fi
exit 0
EOF
chmod +x "$stub_bin/osascript" "$stub_bin/killall" "$stub_bin/sudo" "$stub_bin/defaults"

# make_source_dir -- create a chezmoi source tree whose macos_defaults.yaml is
# read from stdin; print the tree's path.
make_source_dir() {
  local source_dir
  source_dir="$(mktemp -d "$sandbox/src.XXXXXX")"
  mkdir -p "$source_dir/.chezmoidata"
  cat >"$source_dir/.chezmoidata/macos_defaults.yaml"
  printf '%s\n' "$source_dir"
}

run_tool() { # <source-dir> <script>
  (
    cd "$sandbox" || exit 1
    MACOS_DEFAULTS_SOURCE_DIR="$1" HOME="$sandbox/home" \
      PATH="$stub_bin:$PATH" bash "$2"
  )
}

render_template() { # <source-dir> <out-file>
  HOME="$sandbox/render-home" chezmoi --source "$1" execute-template --no-tty \
    <"$TEMPLATE" >"$2" 2>"$render_error"
}

stream_records() { # <source-dir> <err-file>
  bash -c 'source "$1"; defaults_records_unit_separated "$2"' _ \
    "$LIB" "$1/.chezmoidata/macos_defaults.yaml" 2>"$2"
}

# ---- control: a wholly valid file applies every record ------------------------

# Two enforce records, the second reached only if the first did not end the run.
# Every refusal case below asserts an EMPTY defaults log, and an apply that never
# writes satisfies all of them, so this row is what proves the log observes
# writes at all.
control_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.controlfirst
      key: ControlFirstKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.controlsecond
      key: ControlSecondKey
      type: bool
      value: false
      tier: enforce
  killall: []
EOF
)"
: >"$defaults_log"
: >"$sudo_log"
run_tool "$control_src" "$APPLY" 2>"$sandbox/control-apply.err" ||
  fail "control: apply must succeed on a wholly valid file (stderr: $(cat "$sandbox/control-apply.err"))"
assert_file_contains "$defaults_log" 'write com.example.controlfirst ControlFirstKey -bool true' \
  "control: apply must write the first valid record (defaults log: $(cat "$defaults_log"))"
assert_file_contains "$defaults_log" 'write com.example.controlsecond ControlSecondKey -bool false' \
  "control: apply must write the second valid record (defaults log: $(cat "$defaults_log"))"

render_template "$control_src" "$sandbox/rendered-control" ||
  fail "control: the template must render a wholly valid file (stderr: $(cat "$render_error"))"
assert_file_contains "$sandbox/rendered-control" \
  "defaults write 'com.example.controlsecond' 'ControlSecondKey' -bool 'false'" \
  "control: the render must carry the second record's write (render: $(cat "$sandbox/rendered-control"))"

control_stream="$(stream_records "$control_src" "$sandbox/control-stream.err")" ||
  fail "control: the record stream must accept a wholly valid file (stderr: $(cat "$sandbox/control-stream.err"))"
[[ $(printf '%s\n' "$control_stream" | wc -l | tr -d ' ') -eq 2 ]] ||
  fail "control: the record stream must emit both records (got: $(printf '%s' "$control_stream" | cat -v))"

control_drift_status=0
run_tool "$control_src" "$DRIFT" >"$sandbox/control-drift.out" 2>"$sandbox/control-drift.err" ||
  control_drift_status=$?
[[ $control_drift_status -eq 1 ]] ||
  fail "control: drift must report drift (status 1) against the stub's differing live value (got $control_drift_status, stderr: $(cat "$sandbox/control-drift.err"))"
assert_file_contains "$sandbox/control-drift.out" 'com.example.controlsecond' \
  "control: drift must reach and report the SECOND record (stdout: $(cat "$sandbox/control-drift.out"))"

# ---- the refusal table -------------------------------------------------------

# assert_refused_before_any_write <label> <apply-stderr-fragment>
#   (fixture on stdin)
#
# Every fixture puts a VALID enforce record first and the offending record
# second, so "wrote nothing" is a statement about ordering and not about a file
# that had nothing to write. Asserted per fixture:
#   - apply exits 2, the tools' shared "data file unusable" status;
#   - the defaults and sudo logs are EMPTY, so the valid FIRST record was not
#     applied either;
#   - apply's message names the offending record and the rule it broke;
#   - the shared record stream emits nothing;
#   - drift refuses the same file and reports no row;
#   - the runner template refuses the same file, so one YAML cannot render
#     cleanly and then make every tool refuse it.
assert_refused_before_any_write() { # <label> <apply-stderr-fragment>
  local label="$1" apply_stderr_fragment="$2"
  local source_dir apply_status=0 drift_status=0 stream_status=0 stream_output
  source_dir="$(make_source_dir)"

  : >"$defaults_log"
  : >"$sudo_log"
  run_tool "$source_dir" "$APPLY" >"$sandbox/refused-apply.out" 2>"$sandbox/refused-apply.err" ||
    apply_status=$?
  [[ $apply_status -eq 2 ]] ||
    fail "$label: apply must refuse the whole file with status 2 (got $apply_status, stderr: $(cat "$sandbox/refused-apply.err"))"
  [[ ! -s $defaults_log ]] ||
    fail "$label: apply must write NOTHING, including the valid record that precedes the offender (defaults log: $(cat "$defaults_log"))"
  [[ ! -s $sudo_log ]] ||
    fail "$label: apply must invoke no sudo at all on a file it refuses (sudo log: $(cat "$sudo_log"))"
  assert_file_contains "$sandbox/refused-apply.err" "$apply_stderr_fragment" \
    "$label: the refusal must name the rule that was broken (stderr: $(cat "$sandbox/refused-apply.err"))"
  assert_file_contains "$sandbox/refused-apply.err" 'com.example.offender' \
    "$label: the refusal must name the offending record (stderr: $(cat "$sandbox/refused-apply.err"))"

  stream_output="$(stream_records "$source_dir" "$sandbox/refused-stream.err")" || stream_status=$?
  [[ $stream_status -eq 2 ]] ||
    fail "$label: the shared record stream must refuse the file with status 2 (got $stream_status, stderr: $(cat "$sandbox/refused-stream.err"))"
  [[ -z $stream_output ]] ||
    fail "$label: a refused stream must emit nothing a caller could act on (got: $(printf '%s' "$stream_output" | cat -v))"

  : >"$defaults_log"
  run_tool "$source_dir" "$DRIFT" >"$sandbox/refused-drift.out" 2>"$sandbox/refused-drift.err" ||
    drift_status=$?
  [[ $drift_status -eq 2 ]] ||
    fail "$label: drift must refuse the same file with status 2 (got $drift_status, stderr: $(cat "$sandbox/refused-drift.err"))"
  [[ ! -s $sandbox/refused-drift.out ]] ||
    fail "$label: drift must report no row from a file it refuses (stdout: $(cat "$sandbox/refused-drift.out"))"

  if render_template "$source_dir" "$sandbox/refused-render"; then
    fail "$label: the runner template must refuse the same file, or one YAML renders cleanly and then makes every tool refuse it (render: $(cat "$sandbox/refused-render"))"
  fi
}

# 1. Unknown scope on the second record. Validation used to sit inside the write
#    loop, so the first record was already on disk when this was caught.
assert_refused_before_any_write 'unknown scope behind a valid record' 'unknown scope' <<'EOF'
macos:
  defaults:
    - domain: com.example.leader
      key: LeaderKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.offender
      key: OffenderKey
      type: bool
      value: true
      tier: enforce
      scope: bogus
  killall: []
EOF

# 2. No domain on the second record. The write loop skipped it and the run
#    reported success, which is worse than the partial apply above: a malformed
#    file looked applied.
assert_refused_before_any_write 'blank domain behind a valid record' 'blank domain' <<'EOF'
macos:
  defaults:
    - domain: com.example.leader
      key: LeaderKey
      type: bool
      value: true
      tier: enforce
    - key: com.example.offender
      type: bool
      value: true
      tier: enforce
  killall: []
EOF

# 3. No value on the second record. BOTH records were written, the second with an
#    empty value, and the run reported success.
assert_refused_before_any_write 'blank value behind a valid record' 'blank value' <<'EOF'
macos:
  defaults:
    - domain: com.example.leader
      key: LeaderKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.offender
      key: OffenderKey
      type: bool
      tier: enforce
  killall: []
EOF

# 4. No type on the second record. Same class: the write went out as
#    `defaults write com.example.offender OffenderKey - true`, exit 0.
assert_refused_before_any_write 'blank type behind a valid record' 'unsupported type' <<'EOF'
macos:
  defaults:
    - domain: com.example.leader
      key: LeaderKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.offender
      key: OffenderKey
      value: true
      tier: enforce
  killall: []
EOF

# 5. A blank key is the other half of the identity rule. Pinned separately from
#    the blank domain so neither half can be dropped without a test going red.
assert_refused_before_any_write 'blank key behind a valid record' 'blank key' <<'EOF'
macos:
  defaults:
    - domain: com.example.leader
      key: LeaderKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.offender
      type: bool
      value: true
      tier: enforce
  killall: []
EOF

# ---- the write-time allowlist is apply's own gate, and it moved too ------------

# The plist_path write allowlist is deliberately NOT part of the shared stream:
# drift only READS, and refusing an odd path there would hide the row instead of
# reporting it. So it stays apply's own gate, which means apply must reach it
# before its first write rather than in the middle of the loop.
allowlist_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.leader
      key: LeaderKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.offender
      key: OffenderKey
      type: bool
      value: true
      tier: enforce
      scope: system
      plist_path: /etc/example.evil.plist
  killall: []
EOF
)"
: >"$defaults_log"
: >"$sudo_log"
allowlist_status=0
run_tool "$allowlist_src" "$APPLY" 2>"$sandbox/allowlist-apply.err" || allowlist_status=$?
[[ $allowlist_status -eq 2 ]] ||
  fail "out-of-allowlist plist_path: apply must refuse with status 2 (got $allowlist_status, stderr: $(cat "$sandbox/allowlist-apply.err"))"
[[ ! -s $defaults_log ]] ||
  fail "out-of-allowlist plist_path: the valid leading record must not be written either (defaults log: $(cat "$defaults_log"))"
[[ ! -s $sudo_log ]] ||
  fail "out-of-allowlist plist_path: apply must invoke no sudo at all (sudo log: $(cat "$sudo_log"))"
assert_file_contains "$sandbox/allowlist-apply.err" 'permitted plist director' \
  "out-of-allowlist plist_path: the refusal must name the containment rule (stderr: $(cat "$sandbox/allowlist-apply.err"))"

# drift must still READ that same file: the allowlist is a write-time rule, and
# a read of an odd path mutates nothing. Losing this row would turn a documented
# asymmetry into an accident.
allowlist_drift_status=0
run_tool "$allowlist_src" "$DRIFT" >"$sandbox/allowlist-drift.out" 2>"$sandbox/allowlist-drift.err" ||
  allowlist_drift_status=$?
[[ $allowlist_drift_status -ne 2 ]] ||
  fail "out-of-allowlist plist_path: drift must NOT refuse a read-only pass over it (stderr: $(cat "$sandbox/allowlist-drift.err"))"
assert_file_contains "$sandbox/allowlist-drift.out" 'com.example.offender' \
  "out-of-allowlist plist_path: drift must still report the record it cannot write (stdout: $(cat "$sandbox/allowlist-drift.out"))"

# ---- the tools' OWN ordering, reached by bypassing the stream gate -------------

# Everything above is satisfied by the shared stream refusing the file, which is
# where the rule belongs. But apply keeps its own validation behind that gate as
# defence in depth for the day the stream's rules drift, and defence that always
# runs AFTER the first write is not defence at all: that is precisely the bug
# this suite exists for. So reach it. Copy the tools beside a doctored library
# whose stream emits a valid record FOLLOWED by one the real gate would refuse,
# and require the tool's own pass to refuse before writing the valid one.
bypass_dir="$sandbox/bypass-tools"
mkdir -p "$bypass_dir"
cp "$APPLY" "$bypass_dir/macos-defaults-apply.sh"
cp "$DRIFT" "$bypass_dir/macos-defaults-drift.sh"
cp "$LIB" "$bypass_dir/macos-defaults-lib.sh"
cat >>"$bypass_dir/macos-defaults-lib.sh" <<'EOF'

# TEST DOUBLE: a valid enforce record, then one carrying a scope the real gate
# refuses. The caller's own validation is the only guard left standing, and WHEN
# it runs is what decides whether the valid record was already on disk.
defaults_records_unit_separated() {
  printf 'com.example.bypassvalid\x1fBypassValidKey\x1fbool\x1ftrue\x1f\x1fuser\x1f\x1fenforce\n'
  printf 'com.example.bypassbogus\x1fBypassBogusKey\x1fbool\x1ftrue\x1f\x1fbogus\x1f\x1fenforce\n'
}
EOF
: >"$defaults_log"
: >"$sudo_log"
bypass_apply_status=0
run_tool "$control_src" "$bypass_dir/macos-defaults-apply.sh" 2>"$sandbox/bypass-apply.err" ||
  bypass_apply_status=$?
[[ $bypass_apply_status -eq 2 ]] ||
  fail "stream bypassed: apply's own validation must refuse with status 2 (got $bypass_apply_status, stderr: $(cat "$sandbox/bypass-apply.err"))"
refute_file_contains "$defaults_log" 'com.example.bypassvalid' \
  "stream bypassed: apply must validate EVERY record before writing ANY, so the valid record that precedes the offender must not reach disk (defaults log: $(cat "$defaults_log"))"
assert_file_contains "$sandbox/bypass-apply.err" 'unknown scope' \
  "stream bypassed: apply's own refusal must name the rule (stderr: $(cat "$sandbox/bypass-apply.err"))"

# drift's own backstop: a record with no domain aborts the report instead of
# being silently skipped. drift never writes, so the ordering question does not
# arise for it; what matters is that an unreadable record is never quietly
# dropped from a report whose whole job is to say what is out of line.
cat >>"$bypass_dir/macos-defaults-lib.sh" <<'EOF'

defaults_records_unit_separated() {
  printf 'com.example.bypassnamed\x1fBypassNamedKey\x1fbool\x1ftrue\x1f\x1fuser\x1f\x1fenforce\n'
  printf '\x1fBypassNamelessKey\x1fbool\x1ftrue\x1f\x1fuser\x1f\x1fenforce\n'
}
EOF
bypass_drift_status=0
run_tool "$control_src" "$bypass_dir/macos-defaults-drift.sh" \
  >"$sandbox/bypass-drift.out" 2>"$sandbox/bypass-drift.err" || bypass_drift_status=$?
[[ $bypass_drift_status -eq 2 ]] ||
  fail "stream bypassed: drift's own validation must refuse a record with no domain (got $bypass_drift_status, stderr: $(cat "$sandbox/bypass-drift.err"))"
refute_file_contains "$sandbox/bypass-drift.out" 'BypassNamelessKey' \
  'stream bypassed: drift must not report a row for a record it could not identify'
assert_file_contains "$sandbox/bypass-drift.err" 'blank domain' \
  "stream bypassed: drift's refusal must name the rule (stderr: $(cat "$sandbox/bypass-drift.err"))"

# ---- the type closed set is ONE list, held in two places -----------------------

# The type is the one field both readers put into a command as a bare option
# word, so neither can quote it and both constrain it to the same closed set.
# Two lists that can drift apart is exactly how apply came to accept a record the
# render refused, which is why the plist_path allowlists are pinned the same way
# in the system-scope suite. Source-form pin, beside the behavioral rows above.
template_supported_types="$(grep -F 'has .type (list' "$TEMPLATE" | grep -o '"[^"]*"' | tr -d '"' | LC_ALL=C sort)"
library_supported_types="$(bash -c 'source "$1"; printf "%s\n" "${MACOS_DEFAULTS_SUPPORTED_TYPES[@]}"' _ "$LIB" | LC_ALL=C sort)"
[[ -n $template_supported_types ]] ||
  fail 'could not extract the supported-type set from the runner template'
[[ -n $library_supported_types ]] ||
  fail 'could not extract the supported-type set from the shared library'
[[ $template_supported_types == "$library_supported_types" ]] ||
  fail "the two readers' supported-type sets have drifted apart (template: $(printf '%s' "$template_supported_types" | tr '\n' ' '); library: $(printf '%s' "$library_supported_types" | tr '\n' ' '))"

# ---- a legitimate mid-run write failure is NOT a validation failure ------------

# Validation moving ahead of the writes must not change what happens when a write
# itself fails: `defaults` failing on record two is a runtime failure, not a
# malformed file, and the first record's write legitimately stands.
runtime_failure_src="$(
  make_source_dir <<'EOF'
macos:
  defaults:
    - domain: com.example.writes
      key: WritesKey
      type: bool
      value: true
      tier: enforce
    - domain: com.example.writefails
      key: WriteFailsKey
      type: bool
      value: true
      tier: enforce
  killall: []
EOF
)"
cat >"$stub_bin/defaults" <<EOF
#!/bin/bash
printf '%s\n' "\$*" >>"$defaults_log"
if [[ \$* == *WriteFailsKey* ]]; then exit 9; fi
if [[ \$1 == read ]]; then printf 'unexpected-live-value\n'; exit 0; fi
exit 0
EOF
chmod +x "$stub_bin/defaults"
: >"$defaults_log"
runtime_failure_status=0
run_tool "$runtime_failure_src" "$APPLY" 2>"$sandbox/runtime-failure.err" || runtime_failure_status=$?
[[ $runtime_failure_status -ne 0 ]] ||
  fail 'a defaults write that fails must still end the run nonzero'
assert_file_contains "$defaults_log" 'write com.example.writes WritesKey -bool true' \
  "a validated record that wrote successfully before a later runtime failure must still have been written (defaults log: $(cat "$defaults_log"))"
assert_file_contains "$defaults_log" 'write com.example.writefails WriteFailsKey -bool true' \
  "the failing write must have been attempted (defaults log: $(cat "$defaults_log"))"

printf 'macos-defaults-validate-before-write: OK (a valid file applies every record; an unknown scope, a blank domain, key, value or type, and an out-of-allowlist plist_path behind a valid record each refuse the WHOLE file with no write attempted, in apply, in drift and in the render; a runtime write failure is still a runtime failure)\n'
