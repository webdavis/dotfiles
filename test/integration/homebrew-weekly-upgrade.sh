#!/usr/bin/env bash
#
# homebrew-weekly-upgrade.sh helper: resilience AND aggregate exit status.
#
#   - Resilient: a failing step is logged but does NOT abort the run; every
#     later step (including cleanup) still runs.
#   - Aggregate exit (Fix 2 / plan-mandated): the helper accumulates step
#     failures and exits NON-zero when any step failed, so an all-steps-failed
#     run cannot exit 0. A fully clean run exits 0.
#
# Integration test: runs the whole helper end to end with brew, mas, tailscaled,
# and (for the refresh-failure case) sudo stubbed at the boundary. No sleeps, no
# timing, no real upgrades -- safe anywhere.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
helper="$REPO_ROOT/dot_local/bin/executable_homebrew-weekly-upgrade.sh"

fail() {
  printf 'homebrew-weekly-upgrade: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -x $helper ]] || fail "helper not found/executable: $helper"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# All seven section headers plus the done marker; every scenario must print them
# (resilience: no failure short-circuits a later step).
sections=(
  "== brew update ==" "== brew outdated ==" "== mas outdated =="
  "== brew upgrade ==" "== tailscaled refresh" "== mas upgrade =="
  "== brew cleanup ==" "=== done"
)

assert_all_sections() {
  local out="$1" name="$2" marker
  for marker in "${sections[@]}"; do
    grep -qF "$marker" <<<"$out" || {
      printf '=== %s output ===\n%s\n' "$name" "$out" >&2
      fail "$name: missing section: $marker"
    }
  done
}

# A brew stub that succeeds on everything except the subcommands named in
# $BREW_FAIL (space separated); a mas stub that always succeeds.
make_brew() {
  cat >"$tmp/brew" <<'MOCK'
#!/usr/bin/env bash
echo "mock brew $*"
for bad in $BREW_FAIL; do
  [[ ${1:-} == "$bad" ]] && exit 1
done
exit 0
MOCK
  cat >"$tmp/mas" <<'MOCK'
#!/usr/bin/env bash
echo "mock mas $*"
exit 0
MOCK
  chmod +x "$tmp/brew" "$tmp/mas"
}
make_brew

# --- Scenario 1: partial failure -> resilient AND non-zero aggregate exit -----
# brew upgrade fails; every later step still runs and the helper exits non-zero.
out="$(BREW_FAIL="upgrade" HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
  HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" HOMEBREW_WEEKLY_LOCKFILE="$tmp/lock1" \
  bash "$helper" 2>&1)"
rc=$?
assert_all_sections "$out" "partial-failure"
grep -qF "FAILED" <<<"$out" || fail "partial-failure: the failed step was not reported"
[[ $rc -ne 0 ]] || {
  printf '=== partial-failure output ===\n%s\n' "$out" >&2
  fail "partial-failure: a failed step must make the helper exit non-zero (got rc=0)"
}

# --- Scenario 2: fully clean run -> exit 0 ------------------------------------
out="$(BREW_FAIL="" HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
  HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" HOMEBREW_WEEKLY_LOCKFILE="$tmp/lock2" \
  bash "$helper" 2>&1)"
rc=$?
assert_all_sections "$out" "all-success"
grep -qF "FAILED" <<<"$out" && fail "all-success: reported a FAILED step though none should fail"
[[ $rc -eq 0 ]] || {
  printf '=== all-success output ===\n%s\n' "$out" >&2
  fail "all-success: a clean run must exit 0 (got rc=$rc)"
}

# --- Scenario 3: missing tool -> steps fail, run completes, non-zero exit -----
# brew binary does not exist: every brew step fails (127) but the run continues
# through mas and the done marker, and the aggregate exit is non-zero.
out="$(HOMEBREW_WEEKLY_BREW="$tmp/does-not-exist-brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
  HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" HOMEBREW_WEEKLY_LOCKFILE="$tmp/lock3" \
  bash "$helper" 2>&1)"
rc=$?
assert_all_sections "$out" "missing-tool"
[[ $rc -ne 0 ]] || fail "missing-tool: a missing brew must make the helper exit non-zero (got rc=0)"

# --- Scenario 4: Tailscale-refresh failure -> logged, later steps run, non-zero
# TS is an executable stub and /usr/local/bin/tailscaled differs, so cmp fails
# and the refresh proceeds to `sudo -n <TS> install-system-daemon`; a stub sudo
# on PATH exits 1, so the refresh step FAILS. Later steps must still run and the
# aggregate exit must be non-zero.
stubdir="$tmp/stub-path"
mkdir -p "$stubdir"
printf '#!/usr/bin/env bash\nexit 1\n' >"$stubdir/sudo"
chmod +x "$stubdir/sudo"
ts_stub="$tmp/tailscaled-stub"
printf '#!/usr/bin/env bash\nexit 0\n' >"$ts_stub"
chmod +x "$ts_stub"
out="$(BREW_FAIL="" PATH="$stubdir:/usr/bin:/bin" \
  HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
  HOMEBREW_WEEKLY_TAILSCALED="$ts_stub" HOMEBREW_WEEKLY_LOCKFILE="$tmp/lock4" \
  bash "$helper" 2>&1)"
rc=$?
assert_all_sections "$out" "tailscale-refresh-failure"
grep -qF "FAILED" <<<"$out" || fail "tailscale-refresh-failure: the failed refresh was not reported"
[[ $rc -ne 0 ]] || {
  printf '=== tailscale-refresh-failure output ===\n%s\n' "$out" >&2
  fail "tailscale-refresh-failure: a failed refresh must make the helper exit non-zero (got rc=0)"
}

printf 'homebrew-weekly-upgrade: OK (resilient; clean run exits 0; any failed step exits non-zero)\n'
