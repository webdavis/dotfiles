#!/usr/bin/env bash
# herdr-health-check-timeout.sh: a wedged herdr socket (accepts the connection
# but never replies) would hang `herdr plugin list` / `herdr session list` /
# `herdr server reload-config` forever. The shared health-check partial
# (.chezmoitemplates/herdr-health-check.sh.tmpl) runs on every apply through its
# includer (run_after_58), so one wedged server would block ALL future applies.
# Every herdr call in the predicate must therefore be bounded by a coreutils
# timeout; an expiry counts as unhealthy (not-verified), never as a hang.
#
# This drives the predicate through its real include site (after_58) with a
# herdr stub that sleeps far past the per-call bound (the "never replies" wedge)
# and a legacy multiplexer installed so the health check is actually reached,
# the whole run wrapped in an OUTER watchdog. A run that returns 124 from the
# watchdog is a hang and fails the test; the fixed predicate returns well within
# the bound and yields the not-verified outcome (no cleanup is performed).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
AFTER_58="$REPO_ROOT/.chezmoiscripts/run_after_58-herdr-migration-verify.sh.tmpl"
WATCHDOG_SECONDS=30

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# The outer watchdog needs a coreutils timeout binary too; the whole point is
# that one exists (coreutils is a declared formula). If neither is present the
# test cannot assert bounded completion, so skip rather than hang.
watchdog_bin=""
if command -v timeout >/dev/null 2>&1; then
  watchdog_bin="timeout"
elif command -v gtimeout >/dev/null 2>&1; then
  watchdog_bin="gtimeout"
else
  printf 'SKIP: no coreutils timeout/gtimeout on PATH; cannot run the watchdog\n'
  exit 0
fi

for tool in chezmoi jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'SKIP: %s not on PATH; cannot render/run the health-check includer\n' "$tool"
    exit 0
  fi
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# A herdr stub that never replies: every subcommand sleeps far past the bound.
# --version sleeps too, so the FIRST bounded call already proves the timeout.
make_sleeping_herdr() {
  local dir="$1"
  cat >"$dir/herdr" <<'STUB'
#!/bin/bash
sleep 300
exit 0
STUB
  chmod +x "$dir/herdr"
}

# Stub brew so after_58 sees tmux installed (reaching the health check) and can
# record any bundle cleanup argv. A wedged herdr must never let cleanup run.
make_brew_stub() {
  local dir="$1"
  cat >"$dir/brew" <<'STUB'
#!/bin/bash
case "$1" in
  list)
    [[ $2 == tmux ]] && exit 0
    exit 1 ;;
  bundle)
    printf '%s\n' "$*" >>"$BUNDLE_RECORD"
    cat >/dev/null
    exit 0 ;;
esac
exit 0
STUB
  chmod +x "$dir/brew"
}

# after_58 runs the health check on every apply once a legacy multiplexer is
# installed. A wedged herdr must not hang it: it must return within the bound
# and perform no cleanup.
a58_home="$work/a58/home"
a58_prefix="$work/a58/prefix"
a58_bin="$work/a58/bin"
a58_bundle="$work/a58/bundle-argv"
mkdir -p "$a58_home/.config/herdr" "$a58_prefix/bin" "$a58_bin"
make_sleeping_herdr "$a58_bin"
make_brew_stub "$a58_prefix/bin"
printf 'x = 1\n' >"$a58_home/.config/herdr/config.toml"
a58_rendered="$work/a58-rendered.sh"
HOME="$a58_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$AFTER_58" >"$a58_rendered" || fail "chezmoi failed to render $AFTER_58"
if [[ ! -s $a58_rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi
a58_rc=0
HOME="$a58_home" HOMEBREW_PREFIX="$a58_prefix" PATH="$a58_bin:$PATH" \
  INSTALLED_PKGS="tmux" BUNDLE_RECORD="$a58_bundle" \
  "$watchdog_bin" "$WATCHDOG_SECONDS" bash "$a58_rendered" >/dev/null 2>"$work/a58-stderr" || a58_rc=$?
[[ $a58_rc -ne 124 ]] ||
  fail "after_58: a wedged herdr socket hung the health check past ${WATCHDOG_SECONDS}s (no bounded timeout on the herdr calls)"
[[ $a58_rc -eq 0 ]] ||
  fail "after_58: script errored under a wedged herdr (rc=$a58_rc; stderr: $(cat "$work/a58-stderr"))"
if [[ -f $a58_bundle ]] && grep -q 'cleanup' "$a58_bundle"; then
  fail "after_58: brew bundle cleanup ran though herdr never replied (wedged socket)"
fi

printf 'PASS: every herdr call in the shared predicate is bounded by a coreutils timeout; a wedged (never-replying) herdr yields not-verified within the bound at the after_58 include site (no cleanup performed) instead of hanging the apply\n'
