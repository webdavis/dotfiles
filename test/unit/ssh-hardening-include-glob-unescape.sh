#!/usr/bin/env bash
# ssh-hardening-include-glob-unescape.sh -- sshd unescapes an Include path
# TWICE, and the scan must reach the same file the daemon reaches.
#
# Stage 1 is argv_split, which tokenizes the line: it consumes `\"`, `\'`,
# `\\`, and (outside a quoted segment) `\<space>`, and keeps every other
# backslash literally. Stage 2 is glob(3), which sshd hands that token to: it
# consumes EVERY remaining `\X` and yields a literal X, so a backslash the
# tokenizer preserved disappears before the daemon opens the file.
#
# The scan used to model stage 1 only and then hand the token to bash pathname
# expansion, which does NOT unescape a word containing no glob metacharacter --
# bash leaves such a word exactly as it found it. So `Include <dir>/pay\load.conf`
# left the scan testing a path with a backslash in it, the `-f` test failed, the
# file was never opened, and a Match block inside it re-enabled password access
# with --verify still reporting the tree clean.
#
# Why this suite is a UNIT test and stubs the daemon: the scan is pure bash and
# a clean runner may have no sshd at all. SSHD_BIN points at a stub that answers
# every resolution HARDENED, so check_global and check_connection_specs pass
# unconditionally and the ONLY thing that can fail --verify here is the include
# walk. Nothing in this file needs a tool the base system does not ship, and the
# absent-daemon case is pinned rather than skipped: the stub log is asserted, so
# a run that quietly fell back to a real /usr/sbin/sshd fails.
#
# The expectations below come from measurements against OpenSSH 10.0p2; the
# differential corpus in test/integration re-derives them from the real binary
# on every run, so a wrong expectation here cannot stay wrong quietly.
set -euo pipefail

# Scrubbed at SCRIPT scope. Git exports GIT_DIR to every hook it runs and this
# suite runs from the pre-commit and pre-push hooks.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

# shellcheck source=../fixtures/ssh-hardening-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)/ssh-hardening-lib.bash"

ssh_sandbox_setup
trap 'ssh_sandbox_teardown' EXIT

BACKSLASH=$'\\'
HORIZONTAL_TAB=$'\t'
MATCH_OFF_LOOPBACK='Match Address *,!127.0.0.1'

# --- the stubbed daemon -------------------------------------------------------
# Its resolution is generated from the shared SSH_HARDENED_PAIRS, so adding a
# directive to policy cannot leave this stub answering the old, shorter set and
# turning a real failure into a pass.
SSHD_STUB="$SSH_SANDBOX/bin/sshd-stub"
SSHD_STUB_LOG="$SSH_SANDBOX/sshd-stub.log"
: >"$SSHD_STUB_LOG"
{
  printf '#!/bin/bash\n'
  # shellcheck disable=SC2016  # deliberate: this line is the stub's SOURCE, and
  # "$*" and ${SSHD_STUB_LOG} must reach it unexpanded to be evaluated when the
  # stub runs, not while it is being written.
  printf 'printf "%%s\\n" "$*" >>"${SSHD_STUB_LOG:?}"\n'
  printf "cat <<'RESOLUTION'\n"
  printf '%s\n' "${SSH_HARDENED_PAIRS[@]}"
  printf 'RESOLUTION\n'
} >"$SSHD_STUB"
chmod +x "$SSHD_STUB"
SSHD_BIN="$SSHD_STUB"
export SSHD_BIN SSHD_STUB_LOG

# The absent-daemon pin: this suite must never reach a real sshd. If SSHD_BIN
# ever escaped the sandbox, every case below would be measuring the host's
# installed daemon instead of the scan.
case $SSHD_BIN in
  "$SSH_SANDBOX"/*) ;;
  *) fail "SSHD_BIN must point inside the sandbox, got '$SSHD_BIN'" ;;
esac

hostile_file="$SSHD_CONFIG_D/500-hostile.conf"

# --- the include targets ------------------------------------------------------
# One hostile file per shape the derivation turned up. Each name is the file
# sshd ends up opening for exactly one Include form below, so an assertion can
# name the resolved path instead of settling for "something failed".
hostile_target() { # <path>
  mkdir -p "$(dirname "$1")"
  printf '%s\n' 'PasswordAuthentication yes' >"$1" ||
    fail "could not create the include target '$1'; this filesystem cannot hold the name the case needs"
}

plain_target="$SSH_SANDBOX/payload.conf"
spaced_target="$SSH_SANDBOX/with space/payload.conf"
tab_target="$SSH_SANDBOX/pay${HORIZONTAL_TAB}load.conf"
# The one file whose NAME contains a backslash lives in a directory of its own.
# Beside payload.conf it would mask the single-backslash bypass: the unfixed
# scan tests the un-unescaped word, so a same-named neighbour lets it open SOME
# file and the case stops distinguishing "reached the right file" from "reached
# any file at all".
backslash_target="$SSH_SANDBOX/nested/pay${BACKSLASH}load.conf"
star_target="$SSH_SANDBOX/pay*load.conf"
question_target="$SSH_SANDBOX/pay?load.conf"
bracket_target="$SSH_SANDBOX/pay[ab]load.conf"
open_bracket_target="$SSH_SANDBOX/pay[load.conf"
# Beside no payload.conf of its own, for the same reason: a scan that DROPPED
# the trailing backslash would otherwise land on a same-named neighbour and the
# case would pass without proving anything.
trailing_backslash_target="$SSH_SANDBOX/nested/payload.conf${BACKSLASH}"
escaped_close_target="$SSH_SANDBOX/z[ab]load.conf"
close_member_target="$SSH_SANDBOX/z]load.conf"
empty_bracket_target="$SSH_SANDBOX/z[]load.conf"

for target in "$plain_target" "$spaced_target" "$tab_target" \
  "$backslash_target" "$star_target" "$question_target" "$bracket_target" \
  "$open_bracket_target" "$trailing_backslash_target" \
  "$escaped_close_target" "$close_member_target" "$empty_bracket_target"; do
  hostile_target "$target"
done

write_hardened_dropin

# --- case runners -------------------------------------------------------------

# Every case RECORDS rather than aborts, and the run fails at the end with the
# whole list. Aborting on the first miss hides how wide a divergence is, and the
# width is the thing worth knowing: the last fix in this class was judged by its
# targets alone and shipped a regression nobody measured.
missed_forms=()
false_alarm_forms=()

run_include_case() { # <include argument, verbatim>
  printf '%s\nInclude %s\n' "$MATCH_OFF_LOOPBACK" "$1" >"$hostile_file"
  run_ssh_hardening --verify
  rm -f "$hostile_file"
}

# assert_scan_reaches <label> <include argument> <path sshd opens>: the scan
# must open that exact file and report the Match-scoped re-enable inside it.
assert_scan_reaches() {
  local label="$1" argument="$2" expected="$3"
  run_include_case "$argument"
  if [[ $SSH_RUN_STATUS -eq 0 ]]; then
    printf '  [MISSED  ] %s -> --verify PASSED\n' "$label"
    missed_forms+=("$label: sshd opens '$expected', which re-enables password authentication inside a Match block, yet --verify exited 0")
    return 0
  fi
  if ! grep -qF -- "$expected" <<<"$SSH_RUN_ERR"; then
    printf '  [WRONG   ] %s -> refused, but not for that file\n' "$label"
    missed_forms+=("$label: --verify refused the tree but never names '$expected', the file sshd opens (stderr: $SSH_RUN_ERR)")
    return 0
  fi
  printf '  [reaches ] %s\n' "$label"
}

# assert_scan_reaches_nothing <label> <include argument>: sshd resolves this
# form to no file at all, so the scan must not invent one. Over-strictness here
# is a false alarm that would make install roll its own hardening back.
assert_scan_reaches_nothing() {
  local label="$1" argument="$2"
  run_include_case "$argument"
  if [[ $SSH_RUN_STATUS -ne 0 ]]; then
    printf '  [ALARM   ] %s -> --verify refused a tree sshd opens nothing for\n' "$label"
    false_alarm_forms+=("$label: sshd resolves this Include to no file at all, yet --verify exited $SSH_RUN_STATUS (stderr: $SSH_RUN_ERR)")
    return 0
  fi
  printf '  [inert   ] %s\n' "$label"
}

# The vacuity control. If a plain Include of the plain payload stops failing
# --verify, every "reaches" assertion below is measuring nothing.
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "positive control: --verify must PASS on the clean hardened tree (stderr: $SSH_RUN_ERR)"
[[ -s $SSHD_STUB_LOG ]] ||
  fail 'the stubbed daemon was never invoked; this suite is not exercising what it claims'
grep -qF -- '-G' "$SSHD_STUB_LOG" ||
  fail "the stub must have answered a 'sshd -G' resolution (log: $(cat "$SSHD_STUB_LOG"))"

printf 'include-path unescaping (expectations measured on OpenSSH 10.0p2):\n'

assert_scan_reaches 'control: plain absolute path' \
  "$plain_target" "$plain_target"

# --- stage 2 alone: a backslash the tokenizer kept, glob(3) removes -----------
assert_scan_reaches 'single backslash before an ordinary character' \
  "$SSH_SANDBOX/pay${BACKSLASH}load.conf" "$plain_target"
assert_scan_reaches 'single backslash inside a quoted path' \
  "\"$SSH_SANDBOX/pay${BACKSLASH}load.conf\"" "$plain_target"

# --- both stages: argv_split eats one backslash, glob(3) eats the next --------
assert_scan_reaches 'doubled backslash, unquoted' \
  "$SSH_SANDBOX/pay${BACKSLASH}${BACKSLASH}load.conf" "$plain_target"
assert_scan_reaches 'doubled backslash, quoted' \
  "\"$SSH_SANDBOX/pay${BACKSLASH}${BACKSLASH}load.conf\"" "$plain_target"
# Three backslashes leave one standing after BOTH stages, so this one reaches a
# file whose name really does contain a backslash. It is the case that pins the
# unescape COUNT at exactly two: a scan that unescaped once would look for the
# two-backslash name, one that unescaped three times would look for payload.conf.
assert_scan_reaches 'tripled backslash reaches the backslash-named file' \
  "$SSH_SANDBOX/nested/pay${BACKSLASH}${BACKSLASH}${BACKSLASH}load.conf" "$backslash_target"

# --- the quoted escape argv_split declines to consume -------------------------
# Outside quotes `\<space>` is argv_split's own escape; INSIDE quotes it is not,
# and the backslash survives to glob(3), which removes it there. Measured with a
# directory named `q\ s` beside one named `q s`: the three-backslash quoted form
# reaches `q\ s`, which is only possible if stage 1 left the escape alone.
assert_scan_reaches 'quoted backslash-space' \
  "\"$SSH_SANDBOX/with${BACKSLASH} space/payload.conf\"" "$spaced_target"
# `\<TAB>` is an escape to NEITHER stage 1 (the tab separates instead) nor to a
# reader that only models stage 1 -- but glob(3) removes the backslash all the
# same once a quoted segment has carried the tab into the token.
assert_scan_reaches 'quoted backslash-tab' \
  "\"$SSH_SANDBOX/pay${BACKSLASH}${HORIZONTAL_TAB}load.conf\"" "$tab_target"

# --- a backslash before a glob metacharacter ---------------------------------
# glob(3) strips the backslash AND the metacharacter loses its magic, so the
# form names one literal file. bash agrees only when the word carries a SECOND,
# live metacharacter; with the escaped one alone bash performs no pathname
# expansion at all and the backslash survives into the -f test.
assert_scan_reaches 'escaped asterisk names the literal file' \
  "$SSH_SANDBOX/pay${BACKSLASH}*load.conf" "$star_target"
assert_scan_reaches 'escaped question mark names the literal file' \
  "$SSH_SANDBOX/pay${BACKSLASH}?load.conf" "$question_target"
assert_scan_reaches 'escaped bracket pair names the literal file' \
  "$SSH_SANDBOX/pay${BACKSLASH}[ab${BACKSLASH}]load.conf" "$bracket_target"
assert_scan_reaches 'escaped unmatched bracket names the literal file' \
  "$SSH_SANDBOX/pay${BACKSLASH}[load.conf" "$open_bracket_target"
# An unescaped '[' whose only ']' is ESCAPED opens no bracket at all: glob(3)
# needs an UNESCAPED ']' at least one character further on, so the whole run is
# literal text (measured).
assert_scan_reaches 'unescaped bracket closed only by an escaped ] is literal' \
  "$SSH_SANDBOX/z[ab${BACKSLASH}]load.conf" "$escaped_close_target"
# A ']' sitting immediately after the '[' is a MEMBER, not the closing bracket,
# so with no further ']' the whole run is literal text (measured).
assert_scan_reaches 'a ] immediately after [ leaves the run literal' \
  "$SSH_SANDBOX/z[]load.conf" "$empty_bracket_target"

# --- forms where bash and glob(3) already agree, kept as regression cover -----
# These pass on the unfixed scan too. They are here so a fix that reaches the
# forms above by discarding backslashes wholesale, or by replacing pattern
# expansion with a literal path test, fails instead of passing quietly.
assert_scan_reaches 'trailing backslash stays a literal backslash' \
  "$SSH_SANDBOX/nested/payload.conf${BACKSLASH}" "$trailing_backslash_target"
assert_scan_reaches 'escape beside a live metacharacter still expands' \
  "$SSH_SANDBOX/pa${BACKSLASH}yload*.conf" "$plain_target"
assert_scan_reaches 'escaped ] is a MEMBER when a live ] closes the bracket' \
  "$SSH_SANDBOX/z[ab${BACKSLASH}]c]load.conf" "$close_member_target"

# --- forms sshd resolves to nothing: the scan must not invent a file ----------
# `\<TAB>` outside quotes is not an escape at all: argv_split ends the argument
# at the tab, so the first path ends in a bare backslash (which glob(3) keeps as
# a literal backslash, matching no file here) and `load.conf` becomes a further,
# relative argument that resolves nowhere.
assert_scan_reaches_nothing 'unquoted backslash-tab resolves to nothing' \
  "$SSH_SANDBOX/pay${BACKSLASH}${HORIZONTAL_TAB}load.conf"
# argv_split turns `\\` into one backslash and the following space then
# SEPARATES, so this leaves `<dir>/with\` and a relative `space/payload.conf`,
# neither of which exists.
assert_scan_reaches_nothing 'unquoted doubled-backslash-space resolves to nothing' \
  "$SSH_SANDBOX/with${BACKSLASH}${BACKSLASH} space/payload.conf"
# A live bracket really is a pattern: nothing named payaload.conf or
# paybload.conf exists, so sshd opens no file even though a file whose NAME is
# the pattern text is sitting right there. A fix that answered every form with
# the literal unescape would open it and raise a false alarm.
assert_scan_reaches_nothing 'live bracket matching nothing opens nothing' \
  "$SSH_SANDBOX/pa${BACKSLASH}y[ab]load.conf"

# The hostile file is removed by every case, so one final check proves no case
# leaked state into the tree.
run_ssh_hardening --verify
[[ $SSH_RUN_STATUS -eq 0 ]] ||
  fail "--verify must PASS again on the clean tree once every case is done; state leaked (stderr: $SSH_RUN_ERR)"

status=0
if [[ ${#missed_forms[@]} -gt 0 ]]; then
  printf '\nFAIL: %d Include form(s) sshd follows into a Match-scoped re-enable that the scan does not reach:\n' \
    "${#missed_forms[@]}" >&2
  printf '  - %s\n' "${missed_forms[@]}" >&2
  status=1
fi
# Reported separately and after the misses, because a false alarm is a different
# defect: it blocks an install rather than certifying a hole.
if [[ ${#false_alarm_forms[@]} -gt 0 ]]; then
  printf '\nFAIL: %d Include form(s) sshd opens no file for that the scan refused anyway:\n' \
    "${#false_alarm_forms[@]}" >&2
  printf '  - %s\n' "${false_alarm_forms[@]}" >&2
  status=1
fi
if [[ $status -ne 0 ]]; then
  exit "$status"
fi

printf 'ssh-hardening-include-glob-unescape: OK (every Include form resolves to the file sshd opens, and no form invents one)\n'
