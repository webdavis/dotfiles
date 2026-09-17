#!/usr/bin/env bash
#
# scripts/chezmoi-apply-logged.sh keeps a transcript of an apply for an agent to
# read. Seventeen of this repository's targets render vault secrets, so the one
# behavior that must never regress is that content never reaches the log while
# the changed-target list, the messages and the exit status do.
#
# The apply runs without -v so no diff body is ever produced; the content
# filter is a backstop for anything that prints one anyway, and these tests
# pin the backstop as well as the record-keeping.
#
# chezmoi is replaced by a stub on PATH, because the behavior under test is
# this script, not chezmoi.

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/scripts/chezmoi-apply-logged.sh"

SECRET="sk-live-THIS-MUST-NEVER-BE-LOGGED"

# Writes a `chezmoi` stub that answers `status` with one fixed target, prints
# $1 for any other subcommand, and exits $2. The stub body is a QUOTED heredoc
# so nothing in it expands here, and it reads the output and the code out of
# files rather than having them interpolated in, which keeps the generated
# script free of this test's own variables.
function stub_chezmoi() {
  local body=$1 code=$2
  mkdir -p "$BIN"
  printf '%s\n' "$body" >"$WORK/apply.out"
  printf '%s' "$code" >"$WORK/apply.code"
  cat >"$BIN/chezmoi" <<'CHEZMOI_STUB'
#!/usr/bin/env bash
here=${0%/*}
printf '%s\n' "$*" >>"$here/../invocations"
if [ "$1" = "status" ]; then
  printf ' M .config/pns/config.toml\n'
  exit 0
fi
cat "$here/../apply.out"
exit "$(cat "$here/../apply.code")"
CHEZMOI_STUB
  chmod +x "$BIN/chezmoi"
}

function run_logged() {
  PATH="$BIN:$PATH" CHEZMOI_APPLY_LOG_DIR="$STATE" bash "$SCRIPT" >"$WORK/term.out" 2>"$WORK/term.err"
  printf '%s' $? >"$WORK/status"
}

function log_body() {
  cat "$STATE/latest.apply.log"
}

function set_up() {
  WORK="$(mktemp -d)"
  BIN="$WORK/bin"
  STATE="$WORK/state"
}

function tear_down() {
  [[ -n ${WORK:-} && -d $WORK ]] && rm -rf "$WORK"
}

function test_a_diff_body_never_reaches_the_log() {
  stub_chezmoi "diff --git a/.aws/credentials b/.aws/credentials
--- a/.aws/credentials
+++ b/.aws/credentials
@@ -1,2 +1,2 @@
 [default]
-aws_secret_access_key = old-value-also-a-secret
+aws_secret_access_key = $SECRET" 0
  run_logged
  assert_not_contains "$SECRET" "$(log_body)"
  assert_not_contains "old-value-also-a-secret" "$(log_body)"
}

function test_the_withheld_lines_are_counted_rather_than_silently_dropped() {
  stub_chezmoi "diff --git a/x b/x
@@ -1,3 +1,3 @@
 context
-removed
+added $SECRET" 0
  run_logged
  # Three content lines: one context, one removal, one addition.
  assert_contains "3 content line(s) withheld" "$(log_body)"
}

function test_the_changed_target_and_the_messages_survive() {
  stub_chezmoi "chezmoi: .config/pns/config.toml: a message worth reading" 0
  run_logged
  assert_contains "a message worth reading" "$(log_body)"
}

# The regression this pins: an earlier version recorded `chezmoi status` before
# the apply. `status` renders templates, so it prompts for the vault, and its
# output went only to the log, which hid the prompt and hung the run. Nothing
# may reach the vault on the apply's behalf again.
function test_nothing_but_the_apply_is_ever_invoked() {
  stub_chezmoi "chezmoi: fine" 0
  run_logged
  # One invocation, and it is the apply. Flags are free to change; reaching a
  # second subcommand is what must not happen.
  assert_same "1" "$(wc -l <"$WORK/invocations" | tr -d ' ')"
  assert_matches "^apply" "$(cat "$WORK/invocations")"
  assert_not_contains "status" "$(cat "$WORK/invocations")"
}

function test_the_exit_code_is_recorded_and_returned() {
  stub_chezmoi "chezmoi: it went wrong" 7
  run_logged
  assert_same "7" "$(cat "$WORK/status")"
  assert_contains "exit_code:  7" "$(log_body)"
  assert_contains "result:     FAILED" "$(log_body)"
}

function test_a_clean_apply_records_its_success() {
  stub_chezmoi "" 0
  run_logged
  assert_same "0" "$(cat "$WORK/status")"
  assert_contains "exit_code:  0" "$(log_body)"
  assert_contains "result:     OK" "$(log_body)"
}

function test_the_operators_terminal_still_gets_the_unredacted_output() {
  stub_chezmoi "+$SECRET" 0
  run_logged
  assert_contains "$SECRET" "$(cat "$WORK/term.out")"
}

function test_the_log_is_not_readable_by_anyone_else() {
  stub_chezmoi "chezmoi: fine" 0
  run_logged
  # -L follows the symlink: `latest.apply.log` points at the timestamped file,
  # and a symlink's own mode is 755 on macOS whatever its target is.
  assert_same "600" "$(/usr/bin/stat -Lf '%OLp' "$STATE/latest.apply.log")"
  assert_same "700" "$(/usr/bin/stat -Lf '%OLp' "$STATE")"
}
