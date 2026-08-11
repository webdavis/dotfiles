#!/usr/bin/env bash
#
# The brew-shellenv cache self-heal in the rendered ~/.bashrc.
#
# This block is the ONLY automatic writer of the cache that ~/.bashrc sources for
# Homebrew's PATH. The retired excluded apply (`just a` until 2026-08-10, which
# what CLAUDE.md requires of automation) does not run templated scripts, so the
# old apply-time regen script only fired on a rare full interactive apply and was
# removed. Deleting or weakening this block therefore has no backstop.
#
# This DRIVES the block's two named predicates against fixture files, which is
# the only way to catch a predicate that is spelled plausibly and answers the
# wrong question (`! -f` on the paths file read a mode-000 file as fine while
# path_helper was failing on it):
#
#   1. __brew_shellenv_cache_needs_repair is true for every unusable cache state
#      (absent, empty, unreadable, a directory), for a stale cache, and for a
#      paths file that is missing OR present-but-unreadable; false when all three
#      inputs are healthy
#   2. __brew_shellenv_repair_is_due bounds the retries on BOTH sides and treats
#      every unusable stamp as a re-arm
#
# The structural half (the shape of the launch, the interactive gate, which
# writer is invoked) was deleted 2026-08-05: it asserted wiring, not behavior.
#
# Unit test: render dot_bashrc.tmpl, slice the block, source its definitions and
# call the predicates. Darwin-only, because the block lives inside the template's
# `{{ if eq .chezmoi.os "darwin" }}` guard.
#
# Almost every needle below is a literal fragment of the rendered shell source
# being inspected, so single quotes and unexpanded `$` are the point of the file.
# shellcheck disable=SC2016
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/dot_bashrc.tmpl"
WRITER_SOURCE="$REPO_ROOT/dot_local/libexec/executable_brew-shellenv-cache-refresh.sh"
# The rate-limit fixtures are clock-relative and the predicate reads the clock
# itself, so each such observation is bracketed by two clock reads and retried
# while they disagree. See the bracket in the harness below for why.
CLOCK_STABLE_OBSERVATION_ATTEMPTS=8
CLOCK_UNSTABLE_RESULT='CLOCK-UNSTABLE'

fail() {
  printf 'bashrc-brew-cache-self-heal: FAIL -- %s\n' "$*" >&2
  exit 1
}

if [[ "$(uname -s)" != Darwin ]]; then
  printf 'bashrc-brew-cache-self-heal: SKIP (block is darwin-only; host is %s)\n' "$(uname -s)"
  exit 0
fi
command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render dot_bashrc.tmpl\n'
  exit 0
}
[[ -f $TEMPLATE ]] || fail "missing template: $TEMPLATE"
[[ -x $WRITER_SOURCE ]] || fail "missing deployed writer source: $WRITER_SOURCE"

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT
mkdir -p "$sandbox/render-home"

rendered="$(HOME="$sandbox/render-home" CI=1 chezmoi --source "$REPO_ROOT" \
  execute-template --no-tty <"$TEMPLATE")" ||
  fail 'chezmoi failed to render dot_bashrc.tmpl'

# Definitions only: paths and predicates, stopping before the `if` that acts on
# them. Sourcing this gives the real predicates with no side effects.
definitions="$sandbox/self-heal-definitions.sh"
awk '/^[[:space:]]*__brew_shellenv_cache=/{inside = 1}
     /^[[:space:]]*if __brew_shellenv_cache_needs_repair/{inside = 0}
     inside' <<<"$rendered" >"$definitions"
grep -qF '__brew_shellenv_cache_needs_repair()' "$definitions" ||
  fail 'could not slice the self-heal predicate definitions out of the rendered ~/.bashrc'
grep -qF '__brew_shellenv_repair_is_due()' "$definitions" ||
  fail 'could not slice the rate-limit predicate out of the rendered ~/.bashrc'

# ---------------------------------------------------------------------------
# 1 + 2. Drive the real predicates against fixtures.
#
# Every case runs in ONE subshell that sources the definitions once and then
# re-points the block's own variables, so the matrix costs no extra processes.
# Fields are pipe-delimited because a case name must never be split on
# whitespace.
# ---------------------------------------------------------------------------
# `|| true` on the harness: a predicate that dies mid-run (bash aborts a
# non-interactive shell on an arithmetic syntax error, which is exactly what an
# unvalidated stamp value causes) must be reported as a MISSING case below, not
# abort this script through `set -e` with nothing printed.
predicate_report="$sandbox/predicate-report"
env -i PATH="${BASH%/*}:/usr/bin:/bin" HOME="$sandbox/predicate-home" \
  DEFINITIONS="$definitions" FIXTURES="$sandbox/fixtures" \
  CLOCK_STABLE_OBSERVATION_ATTEMPTS="$CLOCK_STABLE_OBSERVATION_ATTEMPTS" \
  CLOCK_UNSTABLE_RESULT="$CLOCK_UNSTABLE_RESULT" \
  "$BASH" --noprofile --norc -s >"$predicate_report" 2>&1 <<'HARNESS' || true
set -uo pipefail
# shellcheck disable=SC1090
. "$DEFINITIONS"

mkdir -p "$FIXTURES"
report() { printf '%s|%s\n' "$1" "$2"; }

# --- __brew_shellenv_cache_needs_repair ------------------------------------
# Rebuild a healthy world, then damage exactly one thing per case. The root is
# published in a global rather than echoed: a command substitution would run the
# setup in a subshell and every variable it points at the fixture would be lost.
fixture_root=''
build_healthy_world() {
  fixture_root="$FIXTURES/$1"
  chmod -R u+rwX "$fixture_root" 2>/dev/null
  rm -rf "$fixture_root"
  mkdir -p "$fixture_root"
  printf 'export CACHED=1\n' >"$fixture_root/cache"
  printf '# generator\n' >"$fixture_root/generator"
  printf '/opt/homebrew/bin\n' >"$fixture_root/paths"
  touch -t 202001010000 "$fixture_root/generator"
  __brew_shellenv_cache="$fixture_root/cache"
  __brew_shellenv_generator="$fixture_root/generator"
  __brew_shellenv_paths_file="$fixture_root/paths"
}
needs_repair_case() {
  local name="$1"
  build_healthy_world "$name"
  shift
  "$@" "$fixture_root"
  if __brew_shellenv_cache_needs_repair; then report "$name" REPAIR; else report "$name" QUIET; fi
}
nothing() { :; }
remove_cache() { rm -f "$1/cache"; }
empty_cache() { : >"$1/cache"; }
unreadable_cache() { chmod 000 "$1/cache"; }
directory_cache() { rm -f "$1/cache" && mkdir "$1/cache"; }
newer_generator() { touch "$1/generator"; }
remove_paths() { rm -f "$1/paths"; }
unreadable_paths() { chmod 000 "$1/paths"; }
remove_generator() { rm -f "$1/generator"; }

needs_repair_case healthy nothing
needs_repair_case cache-absent remove_cache
needs_repair_case cache-empty empty_cache
needs_repair_case cache-unreadable unreadable_cache
needs_repair_case cache-is-a-directory directory_cache
needs_repair_case cache-stale newer_generator
needs_repair_case paths-absent remove_paths
needs_repair_case paths-unreadable unreadable_paths
needs_repair_case generator-absent remove_generator

# --- __brew_shellenv_repair_is_due -----------------------------------------
# The predicate reads EPOCHSECONDS ITSELF, so a stamp written as an absolute
# epoch computed EARLIER is not the age it claims to be: whatever wall clock
# passes between the fixture's read and the predicate's is added to the age the
# predicate measures. That gap spans a fixture rebuild and averaged 8.8ms when
# measured on this tree, which crosses an integer-second boundary about one run
# in a hundred, and the case one second INSIDE the retry interval then answers
# DUE. Green here, red on a slower runner, with nothing wrong in the predicate.
#
# So a clock-relative fixture declares an AGE and its observation is BRACKETED
# by two clock reads: the answer counts only when both land in the same second,
# which makes the age the predicate saw exactly the age the case asked for. A
# clock that will not hold still for a stamp write and a function call is
# REPORTED as such, never asserted through. Fixtures whose stamp is a literal
# do not depend on the clock and need none of this.
interval="$__brew_shellenv_retry_interval_seconds"
stamp_root="$FIXTURES/stamps"
mkdir -p "$stamp_root"

reset_stamp() {
  local stamp="$1"
  chmod -R u+rwX "$stamp" 2>/dev/null
  rm -rf "$stamp"
}

# Answers into a global rather than through a command substitution: a
# substitution forks, and that fork would sit INSIDE the clock bracket below,
# widening the very window the bracket exists to keep shut.
predicate_answer=''
observe_is_due() {
  __brew_shellenv_attempt_stamp="$1"
  if __brew_shellenv_repair_is_due; then predicate_answer=DUE; else predicate_answer=BLOCKED; fi
}

# A stamp holding a literal, for the cases that ask what NON-EPOCH content does.
is_due_case() {
  local name="$1" kind="$2" value="${3:-}"
  local stamp="$stamp_root/$name"
  reset_stamp "$stamp"
  case "$kind" in
    absent) : ;;
    directory) mkdir "$stamp" ;;
    unreadable)
      printf '%s\n' "$value" >"$stamp"
      chmod 000 "$stamp"
      ;;
    empty) : >"$stamp" ;;
    value) printf '%s\n' "$value" >"$stamp" ;;
    *)
      report "$name" "UNKNOWN-KIND-$kind"
      return
      ;;
  esac
  observe_is_due "$stamp"
  report "$name" "$predicate_answer"
}

# A stamp `age_seconds` old, observed under a clock proven not to have ticked
# across the observation. A NEGATIVE age is a stamp dated that many seconds in
# the FUTURE.
is_due_age_case() {
  local name="$1" age_seconds="$2"
  local stamp="$stamp_root/$name" attempt clock_before clock_after
  for ((attempt = 0; attempt < CLOCK_STABLE_OBSERVATION_ATTEMPTS; attempt++)); do
    reset_stamp "$stamp"
    clock_before="$EPOCHSECONDS"
    printf '%s\n' "$((clock_before - age_seconds))" >"$stamp"
    observe_is_due "$stamp"
    clock_after="$EPOCHSECONDS"
    if [[ $clock_before == "$clock_after" ]]; then
      report "$name" "$predicate_answer"
      return
    fi
  done
  report "$name" "$CLOCK_UNSTABLE_RESULT"
}

is_due_case stamp-absent absent
is_due_age_case stamp-now 0
is_due_age_case stamp-just-under-interval "$((interval - 1))"
is_due_age_case stamp-at-interval "$interval"
is_due_age_case stamp-in-the-future "-$interval"
is_due_case stamp-empty empty
is_due_case stamp-not-a-number value 'not-a-number'
is_due_case stamp-leading-zero value '0123456789'
is_due_case stamp-too-many-digits value '12345678901'
is_due_case stamp-is-a-directory directory
is_due_case stamp-unreadable unreadable "$EPOCHSECONDS"
HARNESS

assert_predicate() {
  local case_name="$1" expected="$2" description="$3" actual
  actual="$(awk -F'|' -v want="$case_name" '$1 == want {print $2}' "$predicate_report")"
  [[ -n $actual ]] || {
    printf 'predicate harness output was:\n%s\n' "$(cat "$predicate_report")" >&2
    fail "no result for predicate case '$case_name'"
  }
  # A clock that ticked on every attempt is a HOST condition, not a verdict on
  # the predicate. Say so, rather than letting the case description below blame
  # the code for a fixture that was never the age it declared.
  [[ $actual != "$CLOCK_UNSTABLE_RESULT" ]] ||
    fail "case '$case_name' never observed a stable clock: the wall clock crossed a second boundary on all $CLOCK_STABLE_OBSERVATION_ATTEMPTS attempts, so the stamp was never the age the case asked for"
  [[ $actual == "$expected" ]] ||
    fail "$description (case '$case_name' answered $actual, wanted $expected)"
}

assert_predicate healthy QUIET \
  'the guard wants a repair when the cache, generator and paths file are all healthy'
assert_predicate cache-absent REPAIR \
  'a missing cache does not trigger a repair, so a fresh host never gets one'
assert_predicate cache-empty REPAIR \
  'an empty cache does not trigger a repair, so it stays empty and every shell pays the live cost'
assert_predicate cache-unreadable REPAIR \
  'an unreadable cache does not trigger a repair, so every shell start prints a bash diagnostic'
assert_predicate cache-is-a-directory REPAIR \
  'a directory in the cache path does not trigger a repair, so every shell start prints a diagnostic'
assert_predicate cache-stale REPAIR \
  'a cache older than the Homebrew shellenv generator does not trigger a repair'
assert_predicate paths-absent REPAIR \
  'a missing ${HOMEBREW_PREFIX}/etc/paths does not trigger a repair, so PATH loses Homebrew'
assert_predicate paths-unreadable REPAIR \
  'an UNREADABLE ${HOMEBREW_PREFIX}/etc/paths does not trigger a repair; path_helper fails on it and PATH loses Homebrew exactly as if it were missing'
assert_predicate generator-absent QUIET \
  'a host with no Homebrew at all still asks for a repair from the staleness terms'

assert_predicate stamp-absent DUE \
  'the first repair on a host is blocked because no attempt has been recorded yet'
assert_predicate stamp-now BLOCKED \
  'a repair attempted moments ago is allowed to run again, so ten new panes launch ten regenerations'
assert_predicate stamp-just-under-interval BLOCKED \
  'a repair inside the retry interval is allowed to run again'
assert_predicate stamp-at-interval DUE \
  'a repair is still blocked once the full retry interval has passed'
assert_predicate stamp-in-the-future DUE \
  'a stamp dated in the future blocks every later repair, the one-sided-comparison bug'
assert_predicate stamp-empty DUE \
  'an empty stamp blocks repairs instead of re-arming them'
assert_predicate stamp-not-a-number DUE \
  'a non-numeric stamp blocks repairs instead of re-arming them'
assert_predicate stamp-leading-zero DUE \
  'a zero-padded stamp is fed to the arithmetic, where it parses as octal'
assert_predicate stamp-too-many-digits DUE \
  'an over-long stamp is fed to the arithmetic instead of re-arming'
assert_predicate stamp-is-a-directory DUE \
  'a directory where the stamp belongs blocks repairs instead of re-arming them'
assert_predicate stamp-unreadable DUE \
  'an unreadable stamp blocks repairs instead of re-arming them'

printf 'bashrc-brew-cache-self-heal: OK (20 predicate cases)\n'
