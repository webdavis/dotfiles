#!/usr/bin/env bash
#
# The deployed brew-shellenv cache writer
# (dot_local/libexec/executable_brew-shellenv-cache-refresh.sh).
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
#   3. writes ATOMICALLY, which is two properties, pinned separately:
#      a. a failing `brew shellenv` leaves the previous cache intact instead of
#         a truncated file that ~/.bashrc would source
#      b. the publish is a RENAME of the file brew wrote, never a copy into the
#         live cache. A copy passes (a) while writing the destination in place,
#         where a shell starting mid-write sources a half-written cache
#   4. leaves no temp file behind, on success or on failure
#   5. refuses to run when Homebrew is absent, instead of writing an empty cache
#   6. rejects unknown arguments loudly
#   7. refuses an EMPTY `brew shellenv` that exited 0. Homebrew's shellenv
#      returns early and prints nothing when PATH already begins with its bin and
#      sbin pair, and an empty cache is the one state ~/.bashrc's self-heal
#      cannot detect as stale, so it would stick
#   8. refuses to write when something that is not a regular file occupies the
#      cache path, because `mv file dir` moves the file INSIDE the directory
#      instead of replacing it
#
# Unit test: the real script against a STUB brew in a sandbox HOME. No flows, no
# sleeps.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WRITER="$REPO_ROOT/dot_local/libexec/executable_brew-shellenv-cache-refresh.sh"
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
#
# With a fourth argument, the stub also records the INODE of the file its own
# stdout is connected to. The writer runs it as `brew shellenv >$temp`, so that
# is the temp file, and comparing the recording with the published cache
# afterwards is how the publish is held to a rename (see case 3b).
make_stub_brew() {
  local stub_path="$1" payload_file="$2" exit_code="${3:-0}" stdout_inode_record="${4:-}"
  cat >"$stub_path" <<STUB
#!/usr/bin/env bash
if [[ \${1:-} != shellenv ]]; then
  printf 'stub brew: unexpected subcommand: %s\n' "\${1:-<none>}" >&2
  exit 64
fi
printf 'x' >>"\${0%/*}/invocations"
stdout_inode_record='$stdout_inode_record'
if [[ -n \$stdout_inode_record ]]; then
  # fd 3 keeps hold of this stub's stdout while stat writes to the record file,
  # so /dev/fd/3 still names the writer's temp; /dev/fd/1 would name the record.
  # GNU form first, BSD form as the fallback, per the suite's stat-order guard.
  exec 3>&1
  stat -c '%i' /dev/fd/3 >"\$stdout_inode_record" 2>/dev/null ||
    stat -f '%i' /dev/fd/3 >"\$stdout_inode_record"
  exec 3>&-
fi
cat "$payload_file"
exit $exit_code
STUB
  chmod +x "$stub_path"
}

# The inode of a path. GNU form first, BSD form as the fallback (macOS), per the
# suite's stat-order guard.
inode_of() {
  stat -c '%i' "$1" 2>/dev/null || stat -f '%i' "$1"
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

# --- 3b: the publish is a RENAME of the file brew wrote ----------------------
# The case above pins the SUCCESS GATE, not the rename: `cp "$temp" "$cache"`
# followed by `rm -f "$temp"` passes it, since a failing brew still never reaches
# the copy. It fails the property that matters at shell startup, though. A copy
# writes the destination IN PLACE, so a shell that sources the cache mid-write
# gets a truncated file, which is the same prompt-noise class ~/.bashrc's
# usability guard exists to prevent, and this writer is now the only
# implementation of the write, so nothing else would catch it. Ask the property
# directly: after a rename the published cache IS the file brew wrote into;
# after any copy-then-delete it is a different file. The destination is seeded
# first, both because that is the shape the real cache has after its first run
# and because it gives a copy an existing inode to write through.
case_dir="$(new_case_dir atomicpublish)"
mkdir -p "$case_dir/home/.cache"
cache="$case_dir/home/.cache/$CACHE_FILE_NAME"
printf 'export PREVIOUS_GOOD_CACHE=1\n' >"$cache"
printf 'export REGENERATED=1\n' >"$case_dir/payload"
make_stub_brew "$case_dir/bin/brew" "$case_dir/payload" 0 "$case_dir/generated-inode"
run_writer "$case_dir" >"$case_dir/stdout" 2>"$case_dir/stderr" ||
  fail "writer failed while replacing an existing cache: $(cat "$case_dir/stderr")"

generated_inode="$(cat "$case_dir/generated-inode")"
[[ -n $generated_inode ]] ||
  fail 'the stub could not record which file brew shellenv wrote into, so the publish was never observed'
published_inode="$(inode_of "$cache")"
[[ $published_inode == "$generated_inode" ]] ||
  fail "the cache was published by copying into the destination (brew wrote inode $generated_inode, the published cache is inode $published_inode), so a shell can source it half-written; the publish must be a rename"
cmp -s "$case_dir/payload" "$cache" ||
  fail 'the renamed cache does not carry the regenerated output'

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

# --- 7a: brew exits 0 but prints nothing, previous cache survives ------------
# Homebrew's cmd/shellenv.sh returns early and prints NOTHING (exit 0) when PATH
# already starts with "${HOMEBREW_PREFIX}/bin:${HOMEBREW_PREFIX}/sbin". Caching
# that would publish a cache that sets no environment, and ~/.bashrc's self-heal
# reads such a cache as current, so it would never regenerate it.
case_dir="$(new_case_dir emptyshellenv)"
mkdir -p "$case_dir/home/.cache"
cache="$case_dir/home/.cache/$CACHE_FILE_NAME"
printf 'export PREVIOUS_GOOD_CACHE=1\n' >"$cache"
: >"$case_dir/payload"
make_stub_brew "$case_dir/bin/brew" "$case_dir/payload"

if run_writer "$case_dir" >"$case_dir/stdout" 2>"$case_dir/stderr"; then
  fail 'writer exited 0 after brew shellenv printed nothing'
fi
assert_contains "$(cat "$cache")" 'PREVIOUS_GOOD_CACHE' \
  'an empty brew shellenv replaced the previous good cache with an empty one'
assert_contains "$(cat "$case_dir/stderr")" 'printed nothing' \
  'writer did not say that brew shellenv produced no output'
litter="$(temp_siblings "$case_dir/home/.cache")"
[[ -z $litter ]] || fail "writer left a temp file behind after an empty brew shellenv: $litter"

# --- 7b: an empty brew shellenv on a fresh host leaves NO cache --------------
case_dir="$(new_case_dir emptyshellenvfresh)"
: >"$case_dir/payload"
make_stub_brew "$case_dir/bin/brew" "$case_dir/payload"
if run_writer "$case_dir" >"$case_dir/stdout" 2>"$case_dir/stderr"; then
  fail 'writer exited 0 after brew shellenv printed nothing on a fresh host'
fi
if [[ -e $case_dir/home/.cache/$CACHE_FILE_NAME ]]; then
  fail 'writer published an empty cache on a fresh host'
fi

# --- 8: a directory sitting on the cache path is refused, not filled ---------
# `mv "$temp" "$cache"` where $cache is a directory moves the temp file INSIDE
# it. The writer must notice before generating anything, and must not delete a
# directory it did not create.
case_dir="$(new_case_dir cacheisdirectory)"
mkdir -p "$case_dir/home/.cache/$CACHE_FILE_NAME"
printf 'export STUB=1\n' >"$case_dir/payload"
make_stub_brew "$case_dir/bin/brew" "$case_dir/payload"

if run_writer "$case_dir" >"$case_dir/stdout" 2>"$case_dir/stderr"; then
  fail 'writer exited 0 with a directory sitting on the cache path'
fi
assert_contains "$(cat "$case_dir/stderr")" 'not a regular file' \
  'writer did not report what is occupying the cache path'
[[ -d $case_dir/home/.cache/$CACHE_FILE_NAME ]] ||
  fail 'writer removed a directory it did not create'
swallowed="$(find "$case_dir/home/.cache/$CACHE_FILE_NAME" -mindepth 1 -print)"
[[ -z $swallowed ]] || fail "writer moved its temp file inside the directory: $swallowed"
if [[ -e $case_dir/bin/invocations ]]; then
  fail 'writer spawned brew before checking that the destination is replaceable'
fi

# --- 9: destination follows HOME when XDG_CACHE_HOME is unset ----------------
case_dir="$(new_case_dir noxdg)"
printf 'export STUB=1\n' >"$case_dir/payload"
make_stub_brew "$case_dir/bin/brew" "$case_dir/payload"
env -i PATH="${BASH%/*}:/usr/bin:/bin" HOME="$case_dir/home" \
  BREW_SHELLENV_CACHE_BREW="$case_dir/bin/brew" \
  "$BASH" "$WRITER" >"$case_dir/stdout" 2>"$case_dir/stderr" ||
  fail "writer failed with XDG_CACHE_HOME unset: $(cat "$case_dir/stderr")"
[[ -f $case_dir/home/.cache/$CACHE_FILE_NAME ]] ||
  fail 'with XDG_CACHE_HOME unset the writer did not fall back to the HOME cache dir'

printf 'brew-shellenv-cache-writer: OK (verbatim copy; creates its dir; publishes by rename, success-gated; no litter; refuses empty output, a non-regular destination, a missing brew and bad args)\n'
