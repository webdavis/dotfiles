#!/usr/bin/env bash
#
# The deployed brew-shellenv cache writer
# (dot_local/bin/executable_brew-shellenv-cache-refresh.sh).
#
# This script is the ONE implementation of the cache write. Both callers use it:
# ~/.bashrc's self-heal guard (the only automatic writer, since
# `chezmoi apply --exclude=templates` skips every templated script) and
# `just brew-cache-refresh`. Everything ~/.bashrc sources at startup comes out of
# here, so its failure modes are pinned individually:
#
#   1. writes `brew shellenv` stdout BYTE-for-byte to
#      ${XDG_CACHE_HOME:-~/.cache}/brew-shellenv.sh (test/e2e/
#      brew-shellenv-cache-drift.sh asserts that same byte-identity against the
#      live generator, and it only holds if the writer copies verbatim)
#   2. creates the cache directory itself, so a fresh host with no ~/.cache works
#   3. writes ATOMICALLY: a failing `brew shellenv` leaves the previous cache
#      intact instead of a truncated file that ~/.bashrc would source
#   4. leaves no temp file behind, on success or on failure
#   5. refuses to run when Homebrew is absent, instead of writing an empty cache
#   6. rejects unknown arguments loudly
#
# Unit test: the real script against a STUB brew in a sandbox HOME. No flows, no
# sleeps.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WRITER="$REPO_ROOT/dot_local/bin/executable_brew-shellenv-cache-refresh.sh"
CACHE_FILE_NAME='brew-shellenv.sh'

fail() {
  printf 'brew-shellenv-cache-writer: FAIL -- %s\n' "$*" >&2
  exit 1
}

# Refute helper. A bare `! grep` cannot fail a test under `set -e` unless it sits
# in final position, so negative assertions go through this instead.
refute_contains() {
  local haystack="$1" needle="$2" description="$3"
  if grep -qF -- "$needle" <<<"$haystack"; then
    fail "$description (found '$needle')"
  fi
}

assert_contains() {
  local haystack="$1" needle="$2" description="$3"
  grep -qF -- "$needle" <<<"$haystack" || fail "$description (missing '$needle')"
}

[[ -f $WRITER ]] || fail "missing writer script: $WRITER"
[[ -x $WRITER ]] || fail "writer script is not executable: $WRITER"

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT

# Stub `brew`: prints a FIXED payload file (so byte-identity is checkable against
# a known fixture) and records each invocation, so a test can tell "regenerated"
# from "left alone". Any subcommand other than `shellenv` is an error, which pins
# what the writer asks Homebrew for.
make_stub_brew() {
  local stub_path="$1" payload_file="$2" exit_code="${3:-0}"
  cat >"$stub_path" <<STUB
#!/usr/bin/env bash
if [[ \${1:-} != shellenv ]]; then
  printf 'stub brew: unexpected subcommand: %s\n' "\${1:-<none>}" >&2
  exit 64
fi
printf 'x' >>"\${0%/*}/invocations"
cat "$payload_file"
exit $exit_code
STUB
  chmod +x "$stub_path"
}

# Each case gets its own HOME so nothing leaks between them.
new_case_dir() {
  local name="$1"
  local case_dir="$sandbox/$name"
  mkdir -p "$case_dir/bin"
  printf '%s\n' "$case_dir"
}

# Run the writer with a SCRUBBED environment: the ambient XDG_CACHE_HOME/HOME of
# whoever runs the suite must never decide where the cache lands. PATH carries the
# running bash so the stub's `#!/usr/bin/env bash` resolves the same interpreter
# the deployed script gets.
run_writer() {
  local case_dir="$1"
  shift
  env -i \
    PATH="${BASH%/*}:/usr/bin:/bin" \
    HOME="$case_dir/home" \
    XDG_CACHE_HOME="$case_dir/home/.cache" \
    BREW_SHELLENV_CACHE_BREW="$case_dir/bin/brew" \
    "$BASH" "$WRITER" "$@"
}

temp_siblings() {
  local cache_dir="$1"
  [[ -d $cache_dir ]] || return 0
  find "$cache_dir" -maxdepth 1 -name "$CACHE_FILE_NAME.*" -print
}

# --- 1 + 2 + 4: fresh host, no cache dir, writes brew stdout verbatim ---------
case_dir="$(new_case_dir fresh)"
printf 'export HOMEBREW_PREFIX="/stub/prefix";\nexport HOMEBREW_CELLAR="/stub/prefix/Cellar";\n' \
  >"$case_dir/payload"
make_stub_brew "$case_dir/bin/brew" "$case_dir/payload"
if [[ -d $case_dir/home/.cache ]]; then fail 'fresh case started with a cache dir'; fi
run_writer "$case_dir" >"$case_dir/stdout" 2>"$case_dir/stderr" ||
  fail "writer failed on a fresh host: $(cat "$case_dir/stderr")"

cache="$case_dir/home/.cache/$CACHE_FILE_NAME"
[[ -f $cache ]] || fail "writer did not create the cache at $cache"
cmp -s "$case_dir/payload" "$cache" ||
  fail 'cache is not a byte-identical copy of brew shellenv stdout'
[[ -e $case_dir/bin/invocations ]] || fail 'writer never invoked brew'

litter="$(temp_siblings "$case_dir/home/.cache")"
[[ -z $litter ]] || fail "writer left a temp file behind on success: $litter"

# --- 3 + 4: a failing brew must not replace the previous cache ----------------
case_dir="$(new_case_dir brewfails)"
mkdir -p "$case_dir/home/.cache"
cache="$case_dir/home/.cache/$CACHE_FILE_NAME"
printf 'export PREVIOUS_GOOD_CACHE=1\n' >"$cache"
# Emits output and THEN fails, which is the case a non-atomic writer truncates.
printf 'export HALF_WRITTEN=1\n' >"$case_dir/payload"
make_stub_brew "$case_dir/bin/brew" "$case_dir/payload" 1

if run_writer "$case_dir" >"$case_dir/stdout" 2>"$case_dir/stderr"; then
  fail 'writer exited 0 even though brew shellenv failed'
fi
assert_contains "$(cat "$cache")" 'PREVIOUS_GOOD_CACHE' \
  'a failing brew replaced the previous good cache'
refute_contains "$(cat "$cache")" 'HALF_WRITTEN' \
  'a failing brew leaked partial output into the live cache'
litter="$(temp_siblings "$case_dir/home/.cache")"
[[ -z $litter ]] || fail "writer left a temp file behind after a brew failure: $litter"

# --- 5a: Homebrew absent, existing cache survives ----------------------------
case_dir="$(new_case_dir nobrew)"
mkdir -p "$case_dir/home/.cache"
cache="$case_dir/home/.cache/$CACHE_FILE_NAME"
printf 'export PREVIOUS_GOOD_CACHE=1\n' >"$cache"
# No stub written, so $case_dir/bin/brew does not exist.
if run_writer "$case_dir" >"$case_dir/stdout" 2>"$case_dir/stderr"; then
  fail 'writer exited 0 with no brew executable present'
fi
assert_contains "$(cat "$cache")" 'PREVIOUS_GOOD_CACHE' \
  'a missing brew clobbered the existing cache'

# --- 5b: Homebrew absent on a fresh host, no scaffolding, own diagnostic -----
# The writer must DECIDE it has nothing to do rather than discover it by failing
# to exec: a machine without Homebrew gets a message naming the missing binary
# and no leftover cache directory. Reaching mkdir/mktemp first would leave the
# directory behind and report a bare ENOENT from the shell.
case_dir="$(new_case_dir nobrewfresh)"
if run_writer "$case_dir" >"$case_dir/stdout" 2>"$case_dir/stderr"; then
  fail 'writer exited 0 with no brew executable present on a fresh host'
fi
assert_contains "$(cat "$case_dir/stderr")" 'nothing to regenerate' \
  "writer did not report a missing Homebrew in its own words"
assert_contains "$(cat "$case_dir/stderr")" "$case_dir/bin/brew" \
  'writer did not name the Homebrew executable it looked for'
if [[ -e $case_dir/home/.cache ]]; then
  fail 'writer created a cache directory on a host with no Homebrew'
fi

# --- 6: unknown argument is an error, not a silent no-op ---------------------
case_dir="$(new_case_dir badarg)"
printf 'export STUB=1\n' >"$case_dir/payload"
make_stub_brew "$case_dir/bin/brew" "$case_dir/payload"
status=0
run_writer "$case_dir" --wat >"$case_dir/stdout" 2>"$case_dir/stderr" || status=$?
((status != 0)) || fail 'writer accepted an unknown argument and exited 0'
assert_contains "$(cat "$case_dir/stderr")" 'Usage' \
  'writer did not print usage to stderr for an unknown argument'
if [[ -e $case_dir/home/.cache/$CACHE_FILE_NAME ]]; then
  fail 'writer wrote a cache while rejecting an unknown argument'
fi

# --- 7: destination follows HOME when XDG_CACHE_HOME is unset ----------------
case_dir="$(new_case_dir noxdg)"
printf 'export STUB=1\n' >"$case_dir/payload"
make_stub_brew "$case_dir/bin/brew" "$case_dir/payload"
env -i PATH="${BASH%/*}:/usr/bin:/bin" HOME="$case_dir/home" \
  BREW_SHELLENV_CACHE_BREW="$case_dir/bin/brew" \
  "$BASH" "$WRITER" >"$case_dir/stdout" 2>"$case_dir/stderr" ||
  fail "writer failed with XDG_CACHE_HOME unset: $(cat "$case_dir/stderr")"
[[ -f $case_dir/home/.cache/$CACHE_FILE_NAME ]] ||
  fail 'with XDG_CACHE_HOME unset the writer did not fall back to the HOME cache dir'

printf 'brew-shellenv-cache-writer: OK (verbatim copy; creates its dir; atomic; no litter; guards brew and args)\n'
