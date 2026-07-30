#!/usr/bin/env bash
#
# The brew-shellenv cache self-heal in the rendered ~/.bashrc.
#
# This block is the ONLY automatic writer of the cache that ~/.bashrc sources for
# Homebrew's PATH. `chezmoi apply --exclude=templates` (what `just a` runs, and
# what CLAUDE.md requires of automation) does not run templated scripts, so the
# old apply-time regen script only fired on a rare full interactive apply and was
# removed. Deleting or weakening this block therefore has no backstop.
#
# Two layers here. The first DRIVES the block's two named predicates against
# fixture files, which is the only layer that can catch a predicate that is
# spelled plausibly and answers the wrong question (`! -f` on the paths file read
# a mode-000 file as fine while path_helper was failing on it). The second is
# structural: the shape of the launch, which has no runtime value to assert.
#
#   1. __brew_shellenv_cache_needs_repair is true for every unusable cache state
#      (absent, empty, unreadable, a directory), for a stale cache, and for a
#      paths file that is missing OR present-but-unreadable; false when all three
#      inputs are healthy
#   2. __brew_shellenv_repair_is_due bounds the retries on BOTH sides and treats
#      every unusable stamp as a re-arm
#   3. the cache-usability question the self-heal asks is the SAME one the
#      consumer half of ~/.bashrc asks before sourcing the file
#   4. the guard forks NOTHING: it is stat tests and builtins, on every
#      interactive shell
#   5. regeneration runs the DEPLOYED writer, not a second copy of the write
#   6. regeneration is detached (`( ... & )`), logged, and gated on the stamp
#      write, so a fresh host sees no job-control noise and a host that cannot
#      record an attempt launches nothing
#   7. the block sits under the interactive gate, which is the documented reason
#      a host driven only through `ssh host cmd` never self-heals
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
WRITER_SOURCE="$REPO_ROOT/dot_local/bin/executable_brew-shellenv-cache-refresh.sh"
WRITER_TARGET_PATH='.local/bin/brew-shellenv-cache-refresh.sh'
CACHE_PATH_EXPRESSION='${XDG_CACHE_HOME:-$HOME/.cache}/brew-shellenv.sh'
PATHS_FILE_SUFFIX='/etc/paths'
DELETED_APPLY_TIME_SCRIPT='run_after_44-cache-brew-shellenv'
CONSUMER_CACHE_VARIABLE='__brew_shellenv'
SELF_HEAL_CACHE_VARIABLE='__brew_shellenv_cache'
USABILITY_OPERATORS='-f -r -s'
INTERACTIVE_GATE='if [[ $- == *i* ]]; then'

fail() {
  printf 'bashrc-brew-cache-self-heal: FAIL -- %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local haystack="$1" needle="$2" description="$3"
  grep -qF -- "$needle" <<<"$haystack" || fail "$description (missing '$needle')"
}

# Refute helper: a bare `! grep` only decides a test in final position under
# `set -e`, so negative assertions go through this.
refute_contains() {
  local haystack="$1" needle="$2" description="$3"
  if grep -qF -- "$needle" <<<"$haystack"; then
    fail "$description (found '$needle')"
  fi
}

# The unary test operators applied to one named variable, in source order, e.g.
# "-f -r -s". Comparing these strings is how the consumer's usability test and
# the self-heal's are held to the same QUESTION rather than to merely both
# existing.
usability_operators_for() {
  local text="$1" variable="$2" operators=''
  while [[ $text =~ (-[a-z])\ \$$variable ]]; do
    operators+="${BASH_REMATCH[1]} "
    text="${text#*"${BASH_REMATCH[0]}"}"
  done
  printf '%s' "${operators% }"
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

# The block, sliced between its first assignment and its closing `unset -f`.
block="$(awk '/^[[:space:]]*__brew_shellenv_cache=/,/^[[:space:]]*unset -f __brew_shellenv_cache_needs_repair/' <<<"$rendered")"
[[ -n ${block//[[:space:]]/} ]] ||
  fail 'the brew-shellenv cache self-heal block is gone from the rendered ~/.bashrc'

# Assert on CODE, not on the comments that explain it.
block_code="$(grep -vE '^[[:space:]]*#' <<<"$block" || true)"

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
interval="$__brew_shellenv_retry_interval_seconds"
stamp_root="$FIXTURES/stamps"
mkdir -p "$stamp_root"
is_due_case() {
  local name="$1" kind="$2" value="${3:-}"
  local stamp="$stamp_root/$name"
  chmod -R u+rwX "$stamp" 2>/dev/null
  rm -rf "$stamp"
  case "$kind" in
    absent) : ;;
    directory) mkdir "$stamp" ;;
    unreadable)
      printf '%s\n' "$value" >"$stamp"
      chmod 000 "$stamp"
      ;;
    empty) : >"$stamp" ;;
    *) printf '%s\n' "$value" >"$stamp" ;;
  esac
  __brew_shellenv_attempt_stamp="$stamp"
  if __brew_shellenv_repair_is_due; then report "$name" DUE; else report "$name" BLOCKED; fi
}

is_due_case stamp-absent absent
is_due_case stamp-now value "$EPOCHSECONDS"
is_due_case stamp-just-under-interval value "$((EPOCHSECONDS - interval + 1))"
is_due_case stamp-at-interval value "$((EPOCHSECONDS - interval))"
is_due_case stamp-in-the-future value "$((EPOCHSECONDS + interval))"
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

# ---------------------------------------------------------------------------
# 3. The reader and the writer ask the same usability question.
# ---------------------------------------------------------------------------
# `|| true` on every extraction: a bare `grep` inside a command substitution that
# matches nothing makes the ASSIGNMENT non-zero, and under `set -e` that aborts
# the run silently, turning a real regression into an exit code with no message.
consumer_guard="$(grep -F "if [[ -f \$$CONSUMER_CACHE_VARIABLE " <<<"$rendered" || true)"
[[ -n $consumer_guard ]] ||
  fail 'the cache CONSUMER in ~/.bashrc no longer guards the source with a usability test'
self_heal_guard="$(grep -F "[[ -f \$$SELF_HEAL_CACHE_VARIABLE " <<<"$block_code" || true)"
[[ -n $self_heal_guard ]] ||
  fail 'the self-heal no longer tests whether the cache is usable, only whether it is stale'

consumer_operators="$(usability_operators_for "$consumer_guard" "$CONSUMER_CACHE_VARIABLE")"
self_heal_operators="$(usability_operators_for "$self_heal_guard" "$SELF_HEAL_CACHE_VARIABLE")"
[[ $consumer_operators == "$USABILITY_OPERATORS" ]] ||
  fail "the cache consumer's usability test is '$consumer_operators', wanted '$USABILITY_OPERATORS'"
[[ $self_heal_operators == "$USABILITY_OPERATORS" ]] ||
  fail "the self-heal's usability test is '$self_heal_operators', wanted '$USABILITY_OPERATORS'"

# ...and both name the same file.
assert_contains "$block_code" "$SELF_HEAL_CACHE_VARIABLE=\"$CACHE_PATH_EXPRESSION\"" \
  'self-heal does not point at the cache path ~/.bashrc sources'
assert_contains "$rendered" "$CONSUMER_CACHE_VARIABLE=\"$CACHE_PATH_EXPRESSION\"" \
  'the sourced brew cache path and the self-heal cache path disagree'

# The paths file is tracked, and tested for READABILITY rather than existence.
assert_contains "$block_code" '__brew_shellenv_paths_file=' \
  'self-heal does not track ${HOMEBREW_PREFIX}/etc/paths at all'
assert_contains "$block_code" "$PATHS_FILE_SUFFIX\"" \
  "the tracked paths file is not ${PATHS_FILE_SUFFIX}"
refute_contains "$block_code" '! -f $__brew_shellenv_paths_file' \
  'the paths term tests existence again; -f reports a mode-000 paths file as fine while path_helper fails on it'

# ---------------------------------------------------------------------------
# 4. No fork anywhere in the block that a healthy shell evaluates.
# ---------------------------------------------------------------------------
refute_contains "$block_code" '$(' \
  'the self-heal block contains a command substitution, so it forks on every interactive shell'
refute_contains "$block_code" '`' \
  'the self-heal block contains a backtick substitution, so it forks on every interactive shell'

# ---------------------------------------------------------------------------
# 5 + 6. The launch: deployed writer, detached, logged, stamp-gated.
# ---------------------------------------------------------------------------
assert_contains "$block_code" "\$HOME/$WRITER_TARGET_PATH" \
  'self-heal does not run the deployed brew-shellenv cache writer'
assert_contains "$block_code" '"$__brew_shellenv_writer"' \
  'self-heal never invokes the writer it resolved'
for inlined in 'mktemp' 'shellenv >' 'command mv'; do
  refute_contains "$block_code" "$inlined" \
    'self-heal still inlines its own copy of the atomic write'
done

assert_contains "$block_code" '&)' \
  'regeneration is not launched in a detached ( ... & ) subshell, so the prompt shows job noise'
assert_contains "$block_code" '>>"$__brew_shellenv_log" 2>&1' \
  'regeneration output is not redirected to the log, so errors reach the terminal'
assert_contains "$block_code" 'mkdir -p "${__brew_shellenv_log%/*}"' \
  'the guard does not create the log dir before the redirect that needs it'
assert_contains "$block_code" '2>/dev/null >"$__brew_shellenv_attempt_stamp"' \
  'the guard does not record the attempt before launching, so nothing bounds the retries'

# The stamp write is a TERM OF THE GATE, not a statement inside it: it sits in
# the `if` condition, above the `; then`. That placement is what makes a host
# which cannot record an attempt launch NOTHING, instead of forking a brew on
# every shell with no rate limit left to bound it. Moving the write into the gate
# body keeps it textually before the launch and still deletes the guarantee, so
# the check is against the gate, not against the launch line. What the ordering
# buys is narrow and worth stating exactly: a shell that starts while an earlier
# regeneration is still running already sees the attempt. It is not a lock, so
# shells that start at the same instant all read a stamp-free host and all
# launch. The behavior behind both halves is pinned in
# test/e2e/bashrc-brew-cache-self-heal.sh; this is the structural check the
# commit gate can afford.
launch_line_number="$(grep -nF '"$__brew_shellenv_writer" >>' <<<"$block_code" | cut -d: -f1 || true)"
stamp_line_number="$(grep -nF '>"$__brew_shellenv_attempt_stamp"' <<<"$block_code" | cut -d: -f1 || true)"
[[ -n $launch_line_number && -n $stamp_line_number ]] ||
  fail 'could not locate the launch and the stamp write in the self-heal block'
mapfile -t gate_end_line_numbers < <(grep -nF '; then' <<<"$block_code" | cut -d: -f1)
((${#gate_end_line_numbers[@]} == 1)) ||
  fail "the self-heal block no longer has exactly one gate (found ${#gate_end_line_numbers[@]} '; then' lines), so which condition guards the launch is ambiguous"
# At or before the closing `; then`, i.e. inside the condition. The write is
# currently the LAST term, so it shares that line; anything in the gate body
# lands after it.
((stamp_line_number <= gate_end_line_numbers[0])) ||
  fail 'the attempt is recorded inside the gate body instead of as a term of the gate, so a host that cannot record one launches a regeneration on every shell'
((stamp_line_number < launch_line_number)) ||
  fail 'the attempt is recorded after the launch, so a shell starting while a regeneration runs sees no attempt and launches another'

# ---------------------------------------------------------------------------
# 7. The block is interactive-only, which is a documented limitation.
# ---------------------------------------------------------------------------
gate_line_number="$(grep -nF "$INTERACTIVE_GATE" <<<"$rendered" | head -1 | cut -d: -f1 || true)"
block_line_number="$(grep -nF "$SELF_HEAL_CACHE_VARIABLE=\"" <<<"$rendered" | head -1 | cut -d: -f1 || true)"
[[ -n $gate_line_number && -n $block_line_number ]] ||
  fail 'could not locate the interactive gate and the self-heal block in the rendered ~/.bashrc'
((block_line_number > gate_line_number)) ||
  fail 'the self-heal moved out of the interactive gate; a detached writer launched from every `ssh host cmd` can hold the ssh channel open'
assert_contains "$rendered" 'only INTERACTIVE shells self-heal' \
  'the rendered ~/.bashrc no longer records that non-interactive-only hosts never write the cache'

# The apply-time regen script is gone; nothing may still point at it.
refute_contains "$rendered" "$DELETED_APPLY_TIME_SCRIPT" \
  'rendered ~/.bashrc still references the deleted apply-time regen script'

printf 'bashrc-brew-cache-self-heal: OK (20 predicate cases; shared usability question; fork-free, stamp-gated, detached, deployed writer)\n'
