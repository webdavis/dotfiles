#!/usr/bin/env bash
#
# homebrew-weekly-lock.sh (Fix 2): the weekly-upgrade helper serializes itself
# with a KERNEL lock via macOS /usr/bin/lockf, so the Monday-noon LaunchAgent and
# an ad-hoc `just brew-upgrade` can never run concurrent brew/mas/cleanup/
# tailscaled operations. While one run holds the lock a second run must exit
# quickly and loudly (retryable exit 75, EX_TEMPFAIL) rather than proceed.
#
# e2e: two REAL concurrent processes drive the actual lockf acquisition (the
# first parks mid-run holding the lock via a blocking brew stub); timing-bound,
# so it lives in the e2e camp. Darwin-only (the LaunchAgent that creates
# contention is darwin-only, and /usr/bin/lockf is the darwin lock primitive).
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
helper="$REPO_ROOT/dot_local/bin/executable_homebrew-weekly-upgrade.sh"

fail() {
  printf 'homebrew-weekly-lock: FAIL -- %s\n' "$*" >&2
  exit 1
}

if [[ "$(uname -s)" != "Darwin" || ! -x /usr/bin/lockf ]]; then
  echo "homebrew-weekly-lock: SKIP (no /usr/bin/lockf on this host)"
  exit 0
fi
[[ -x $helper ]] || fail "helper not found/executable: $helper"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

lockfile="$tmp/weekly.lock"
parked="$tmp/parked"
go="$tmp/go"

# Blocking brew stub: on `update` it signals it has parked (holding the lock,
# which the helper acquires before any step) and then blocks until $go appears.
cat >"$tmp/brew" <<MOCK
#!/usr/bin/env bash
if [[ \${1:-} == update ]]; then
  : >"$parked"
  while [[ ! -e "$go" ]]; do sleep 0.02; done
fi
exit 0
MOCK
printf '#!/usr/bin/env bash\nexit 0\n' >"$tmp/mas"
chmod +x "$tmp/brew" "$tmp/mas"

# --- run 1: parks mid-run holding the lock ------------------------------------
o1="$tmp/o1.log"
HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
  HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" HOMEBREW_WEEKLY_LOCKFILE="$lockfile" \
  bash "$helper" >"$o1" 2>&1 &
run1_pid=$!
for _ in $(seq 1 300); do
  [[ -e $parked ]] && break
  sleep 0.02
done
[[ -e $parked ]] || {
  : >"$go"
  fail "run 1 never parked (setup): $(cat "$o1" 2>/dev/null)"
}

# --- run 2: contends for the held lock -> must defer with exit 75 -------------
set +e
out2="$(HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
  HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" HOMEBREW_WEEKLY_LOCKFILE="$lockfile" \
  bash "$helper" 2>&1)"
rc2=$?
set -e
[[ $rc2 -eq 75 ]] || {
  : >"$go"
  fail "a concurrent run did not defer with the retryable exit 75 (got $rc2): $out2"
}
grep -qiE 'another run holds the lock|deferring' <<<"$out2" || {
  : >"$go"
  fail "a concurrent run did not announce the deferral: $out2"
}

# --- release: run 1 completes -------------------------------------------------
: >"$go"
wait "$run1_pid" 2>/dev/null || true
grep -qF "=== done" "$o1" || fail "the parked run 1 did not finish: $(cat "$o1")"

printf 'homebrew-weekly-lock: OK (second concurrent run defers with exit 75 while the first holds the lock)\n'
