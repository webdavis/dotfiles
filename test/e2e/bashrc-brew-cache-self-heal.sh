#!/usr/bin/env bash
#
# End-to-end behavior of the brew-shellenv cache self-heal in ~/.bashrc.
#
# The block is the ONLY automatic writer of the cache (an apply-time regen script
# cannot be: `chezmoi apply --exclude=templates` skips templated scripts), so
# this drives the REAL rendered block against a fixture Homebrew prefix and the
# REAL deployed writer, and pins when it fires, when it stays quiet, and what a
# shell start is allowed to print.
#
# Fires:
#   - cache missing (fresh host): `-nt` is true when its right operand is absent
#   - Homebrew's shellenv generator is newer than the cache
#   - ${HOMEBREW_PREFIX}/etc/paths is missing. `brew shellenv` recreates that
#     file and the cached path_helper line reads it at runtime, so without it
#     ${HOMEBREW_PREFIX}/bin and /sbin drop out of PATH; its disappearance does
#     not change the generator's mtime, so nothing else would notice
# Stays quiet:
#   - everything present and current (the common path, every shell)
#   - Homebrew not installed, even with no paths file to find
#   - the deployed writer is not there to run
# And on every path: nothing on stdout or stderr, and no entry in the shell's job
# table (a tracked background job would print `[N] pid` and a failed-job notice
# at an interactive prompt).
#
# e2e: real subprocesses and a real detached write, so it is timing-bound and
# polls. Darwin-only, matching the template's `{{ if eq .chezmoi.os "darwin" }}`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/dot_bashrc.tmpl"
WRITER_SOURCE="$REPO_ROOT/dot_local/bin/executable_brew-shellenv-cache-refresh.sh"

WRITER_RELATIVE='.local/bin/brew-shellenv-cache-refresh.sh'
LOG_RELATIVE='.local/log/brew-shellenv-selfheal.log'
CACHE_RELATIVE='.cache/brew-shellenv.sh'
GENERATOR_RELATIVE='Library/Homebrew/cmd/shellenv.sh'
PATHS_RELATIVE='etc/paths'
STUB_PAYLOAD='export STUB_REGENERATED=1'
REGENERATION_TIMEOUT_SECONDS=10
SETTLE_SECONDS=1
POLL_INTERVAL_SECONDS=0.05

fail() {
  printf 'bashrc-brew-cache-self-heal(e2e): FAIL -- %s\n' "$*" >&2
  exit 1
}

if [[ "$(uname -s)" != Darwin ]]; then
  printf 'bashrc-brew-cache-self-heal(e2e): SKIP (block is darwin-only; host is %s)\n' "$(uname -s)"
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

# --- the block under test ----------------------------------------------------
render_home="$sandbox/render-home"
mkdir -p "$render_home"
rendered="$(HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$TEMPLATE")" ||
  fail 'chezmoi failed to render dot_bashrc.tmpl'

harness="$sandbox/self-heal-block.sh"
awk '/^[[:space:]]*__be_cache=/,/^[[:space:]]*unset __be_cache/' <<<"$rendered" >"$harness"
[[ -s $harness ]] || fail 'could not slice the self-heal block out of the rendered ~/.bashrc'
# Record this shell's job table right after the block, so a tracked (rather than
# detached) background job is caught deterministically instead of by watching for
# prompt noise. `jobs` is a builtin, so recording it forks nothing.
# shellcheck disable=SC2016 # this string is SOURCE for the harness; it must not expand here
printf 'jobs >"$SELF_HEAL_JOBS_FILE"\n' >>"$harness"

# The cache CONSUMER, sliced separately: the block that sources the cache and
# falls back to a live `eval "$(brew shellenv)"` when it is unusable. On a fresh
# host that fallback is what the FIRST shell takes (paying ~50ms once) while the
# self-heal regenerates in the background, so it carries real weight now rather
# than being a rarely-taken branch.
consumer="$sandbox/cache-consumer-block.sh"
awk '/^[[:space:]]*__brew_shellenv=/,/^[[:space:]]*unset __brew_shellenv/' <<<"$rendered" >"$consumer"
[[ -s $consumer ]] || fail 'could not slice the brew-cache sourcing block out of the rendered ~/.bashrc'
# shellcheck disable=SC2016 # this string is SOURCE for the harness; it must not expand here
printf 'printf "%%s\\n" "${STUB_REGENERATED:-unset}" "${SEEDED_CACHE:-unset}" >"$BREW_ENV_FILE"\n' \
  >>"$consumer"

# --- fixture host ------------------------------------------------------------
# A sandbox macOS host: a HOME, a fixture Homebrew prefix and repository, the
# real deployed writer, and a stub `brew` that records its invocations, emits a
# recognizable payload, and recreates the prefix paths file exactly as Homebrew's
# own cmd/shellenv.sh does.
new_host() {
  local name="$1"
  local host="$sandbox/$name"
  mkdir -p "$host/home/.local/bin" "$host/prefix/bin" "$host/prefix/etc" \
    "$host/repo/${GENERATOR_RELATIVE%/*}"
  printf '# fixture shellenv generator\n' >"$host/repo/$GENERATOR_RELATIVE"
  printf '%s/bin\n%s/sbin\n' "$host/prefix" "$host/prefix" >"$host/prefix/$PATHS_RELATIVE"
  cat >"$host/prefix/bin/brew" <<STUB
#!/usr/bin/env bash
if [[ \${1:-} != shellenv ]]; then
  printf 'stub brew: unexpected subcommand: %s\n' "\${1:-<none>}" >&2
  exit 64
fi
printf 'x' >>"$host/invocations"
if [[ ! -f "$host/prefix/$PATHS_RELATIVE" ]]; then
  printf '%s/bin\n%s/sbin\n' "$host/prefix" "$host/prefix" >"$host/prefix/$PATHS_RELATIVE"
fi
printf '%s\n' '$STUB_PAYLOAD'
STUB
  chmod +x "$host/prefix/bin/brew"
  cp "$WRITER_SOURCE" "$host/home/$WRITER_RELATIVE"
  chmod +x "$host/home/$WRITER_RELATIVE"
  printf '%s\n' "$host"
}

# Seed a cache that is NEWER than the generator, i.e. the steady state where the
# guard must stay quiet.
seed_current_cache() {
  local host="$1"
  mkdir -p "$host/home/${CACHE_RELATIVE%/*}"
  printf 'export SEEDED_CACHE=1\n' >"$host/home/$CACHE_RELATIVE"
  touch -t 202001010000 "$host/repo/$GENERATOR_RELATIVE"
}

# Start one shell over a sliced block, with a SCRUBBED environment so the ambient
# HOME/XDG_CACHE_HOME/Homebrew of whoever runs the suite cannot decide anything.
start_shell_over() {
  local host="$1" block="$2"
  env -i \
    PATH="${BASH%/*}:/usr/bin:/bin" \
    HOME="$host/home" \
    XDG_CACHE_HOME="$host/home/.cache" \
    HOMEBREW_PREFIX="$host/prefix" \
    HOMEBREW_REPOSITORY="$host/repo" \
    SELF_HEAL_JOBS_FILE="$host/jobs" \
    BREW_ENV_FILE="$host/brew-env" \
    "$BASH" --noprofile --norc "$block" >"$host/terminal" 2>&1
}

start_shell() {
  start_shell_over "$1" "$harness"
}

assert_shell_was_silent() {
  local host="$1" label="$2"
  if [[ -s $host/terminal ]]; then
    printf 'terminal output was:\n%s\n' "$(cat "$host/terminal")" >&2
    fail "$label: the shell printed to the terminal; a shell start must be silent"
  fi
  if [[ -s $host/jobs ]]; then
    printf 'job table was:\n%s\n' "$(cat "$host/jobs")" >&2
    fail "$label: the regeneration entered the shell's job table (prompt would show [N] pid)"
  fi
}

invocation_count() {
  local host="$1"
  [[ -f $host/invocations ]] || {
    printf '0\n'
    return 0
  }
  wc -c <"$host/invocations" | tr -d ' '
}

wait_for_regeneration() {
  local host="$1" deadline=$((SECONDS + REGENERATION_TIMEOUT_SECONDS))
  while ((SECONDS < deadline)); do
    if [[ -f $host/invocations ]]; then return 0; fi
    sleep "$POLL_INTERVAL_SECONDS"
  done
  return 1
}

assert_regenerated() {
  local host="$1" label="$2"
  wait_for_regeneration "$host" ||
    fail "$label: the guard never regenerated the cache (brew was never invoked)"
  grep -qF "$STUB_PAYLOAD" "$host/home/$CACHE_RELATIVE" ||
    fail "$label: the cache does not carry the regenerated output"
  local litter
  litter="$(find "$host/home/${CACHE_RELATIVE%/*}" -maxdepth 1 -name "${CACHE_RELATIVE##*/}.*" -print)"
  [[ -z $litter ]] || fail "$label: a temp file was left behind: $litter"
}

assert_did_not_regenerate() {
  local host="$1" label="$2"
  sleep "$SETTLE_SECONDS"
  [[ ! -f $host/invocations ]] ||
    fail "$label: the guard regenerated the cache when nothing was stale"
  # Nothing was launched at all: a quiet guard must short-circuit BEFORE the
  # mkdir that prepares the log, so no log file appears either. This is what
  # separates "launched something that failed quietly into the log" from "decided
  # not to launch".
  [[ ! -e $host/home/$LOG_RELATIVE ]] ||
    fail "$label: the guard launched a regeneration it should have skipped (log: $(cat "$host/home/$LOG_RELATIVE"))"
}

# --- A. control: everything present and current, so nothing happens ----------
host="$(new_host control)"
seed_current_cache "$host"
start_shell "$host"
assert_shell_was_silent "$host" 'control'
assert_did_not_regenerate "$host" 'control'
grep -qF 'SEEDED_CACHE' "$host/home/$CACHE_RELATIVE" ||
  fail 'control: the seeded cache was modified even though nothing was stale'

# --- B. fresh host: no ~/.cache and no ~/.local/log --------------------------
host="$(new_host freshhost)"
touch -t 202001010000 "$host/repo/$GENERATOR_RELATIVE"
if [[ -e $host/home/.cache || -e $host/home/.local/log ]]; then
  fail 'fresh host fixture started with a cache or log dir'
fi
start_shell "$host"
assert_shell_was_silent "$host" 'fresh host'
assert_regenerated "$host" 'fresh host'
[[ -f $host/home/$LOG_RELATIVE ]] || fail 'fresh host: no log file was created'
grep -qF 'Regenerated' "$host/home/$LOG_RELATIVE" ||
  fail "fresh host: the writer's report did not land in the log"

# --- C. Homebrew shipped a newer shellenv generator --------------------------
host="$(new_host staleafterupdate)"
seed_current_cache "$host"
touch "$host/repo/$GENERATOR_RELATIVE"
start_shell "$host"
assert_shell_was_silent "$host" 'stale after brew update'
assert_regenerated "$host" 'stale after brew update'

# --- D. the prefix paths file vanished (cache itself is current) -------------
host="$(new_host missingpathsfile)"
seed_current_cache "$host"
rm "$host/prefix/$PATHS_RELATIVE"
start_shell "$host"
assert_shell_was_silent "$host" 'missing paths file'
assert_regenerated "$host" 'missing paths file'
[[ -f $host/prefix/$PATHS_RELATIVE ]] ||
  fail 'missing paths file: regenerating did not recreate the prefix paths file'
# ...and it converges: the next shell has nothing left to fix.
first_count="$(invocation_count "$host")"
start_shell "$host"
assert_shell_was_silent "$host" 'missing paths file, second shell'
sleep "$SETTLE_SECONDS"
[[ "$(invocation_count "$host")" == "$first_count" ]] ||
  fail 'missing paths file: the guard kept regenerating after the file was restored'

# --- E. the deployed writer is not installed ---------------------------------
host="$(new_host nowriter)"
rm "$host/home/$WRITER_RELATIVE"
start_shell "$host"
assert_shell_was_silent "$host" 'writer absent'
assert_did_not_regenerate "$host" 'writer absent'

# --- F. Homebrew is not installed at all (no generator, no paths file) -------
host="$(new_host nohomebrew)"
rm "$host/repo/$GENERATOR_RELATIVE" "$host/prefix/$PATHS_RELATIVE"
start_shell "$host"
assert_shell_was_silent "$host" 'Homebrew absent'
assert_did_not_regenerate "$host" 'Homebrew absent'

# --- G. cold start: with no cache, the first shell still gets Homebrew's env --
# The self-heal is asynchronous, so the shell that triggers it cannot use its
# output. That shell has to fall back to a live `eval "$(brew shellenv)"`, which
# is now the normal first-boot path rather than a rare branch.
host="$(new_host coldstart)"
if [[ -e $host/home/$CACHE_RELATIVE ]]; then fail 'cold start fixture started with a cache'; fi
start_shell_over "$host" "$consumer"
if [[ -s $host/terminal ]]; then
  fail "cold start: the consumer block printed to the terminal: $(cat "$host/terminal")"
fi
[[ "$(sed -n 1p "$host/brew-env")" == 1 ]] ||
  fail 'cold start: with no cache the shell did not fall back to a live brew shellenv'

# --- H. warm start: an existing cache is sourced, and brew is never spawned ---
host="$(new_host warmstart)"
seed_current_cache "$host"
start_shell_over "$host" "$consumer"
if [[ -s $host/terminal ]]; then
  fail "warm start: the consumer block printed to the terminal: $(cat "$host/terminal")"
fi
[[ "$(sed -n 2p "$host/brew-env")" == 1 ]] ||
  fail 'warm start: the existing cache was not sourced'
[[ "$(invocation_count "$host")" == 0 ]] ||
  fail 'warm start: brew was spawned even though a usable cache was present'

printf 'bashrc-brew-cache-self-heal(e2e): OK (fires on missing cache, new generator, missing paths file; quiet and job-free otherwise; cold start falls back to a live eval)\n'
