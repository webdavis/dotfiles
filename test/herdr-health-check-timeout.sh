#!/usr/bin/env bash
# herdr-health-check-timeout.sh: a wedged herdr socket (accepts the connection
# but never replies) would hang `herdr plugin list` / `herdr session list` /
# `herdr server reload-config` forever. The shared health-check partial
# (.chezmoitemplates/herdr-health-check.sh.tmpl) runs on every apply through
# its includers, so one wedged server would block ALL future applies. Every
# herdr call in the predicate must therefore be bounded by a coreutils timeout;
# an expiry counts as unhealthy (not-verified), never as a hang.
#
# This drives the predicate through BOTH real include sites with a herdr stub
# that sleeps far past the per-call bound (the "never replies" wedge), each run
# wrapped in an OUTER watchdog. A run that returns 124 from the watchdog is a
# hang and fails the test; the fixed predicate returns well within the bound
# and yields the not-verified outcome (before_10 defers --cleanup; after_58
# writes no stamp).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BEFORE_10="$REPO_ROOT/.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl"
AFTER_58="$REPO_ROOT/.chezmoiscripts/run_after_58-herdr-migration-verify.sh.tmpl"
STAMP_REL=".cache/herdr-migration/verified"
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
    printf 'SKIP: %s not on PATH; cannot render/run the health-check includers\n' "$tool"
    exit 0
  fi
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

render_into() {
  local script="$1" home="$2" out="$3"
  HOME="$home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$script" >"$out" || fail "chezmoi failed to render $script"
}

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

# Stub Homebrew prefix for before_10: brew list reports tmux installed so the
# script reaches the multiplexer branch; bundle records its argv.
build_stub_prefix() {
  local prefix="$1"
  mkdir -p "$prefix/bin"
  cat >"$prefix/bin/brew" <<'STUB'
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
  printf '#!/bin/bash\nexit 0\n' >"$prefix/bin/uv"
  printf '#!/bin/bash\nexit 0\n' >"$prefix/bin/volta"
  chmod +x "$prefix/bin/brew" "$prefix/bin/uv" "$prefix/bin/volta"
}

# --- before_10 include site ------------------------------------------------
# tmux installed + stamp present drives the current before_10 into its live
# health check. A wedged herdr must not hang it: it must return within the
# bound and defer --cleanup (never remove tmux while herdr is unproven).
b10_home="$work/b10/home"
b10_prefix="$work/b10/prefix"
b10_bin="$work/b10/bin"
b10_bundle="$work/b10/bundle-argv"
mkdir -p "$b10_home/.config/herdr" "$b10_bin"
build_stub_prefix "$b10_prefix"
make_sleeping_herdr "$b10_bin"
printf '#!/bin/bash\nexit 0\n' >"$b10_bin/npm"
chmod +x "$b10_bin/npm"
printf 'x = 1\n' >"$b10_home/.config/herdr/config.toml"
mkdir -p "$b10_home/$(dirname "$STAMP_REL")"
: >"$b10_home/$STAMP_REL"
b10_rendered="$work/b10-rendered.sh"
render_into "$BEFORE_10" "$b10_home" "$b10_rendered"
if [[ ! -s $b10_rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi
b10_rc=0
HOME="$b10_home" HOMEBREW_PREFIX="$b10_prefix" PATH="$b10_bin:$PATH" \
  INSTALLED_PKGS="tmux" BUNDLE_RECORD="$b10_bundle" \
  "$watchdog_bin" "$WATCHDOG_SECONDS" bash "$b10_rendered" >/dev/null 2>"$work/b10-stderr" || b10_rc=$?
[[ $b10_rc -ne 124 ]] ||
  fail "before_10: a wedged herdr socket hung the health check past ${WATCHDOG_SECONDS}s (no bounded timeout on the herdr calls)"
[[ $b10_rc -eq 0 ]] ||
  fail "before_10: script errored under a wedged herdr (rc=$b10_rc; stderr: $(cat "$work/b10-stderr"))"
grep -qw -- '--cleanup' "$b10_bundle" &&
  fail "before_10: --cleanup passed though the live health check could not prove herdr (wedged socket)"

# --- after_58 include site -------------------------------------------------
# after_58 runs the health check on every apply. A wedged herdr must not hang
# it: it must return within the bound and write no stamp.
a58_home="$work/a58/home"
a58_bin="$work/a58/bin"
mkdir -p "$a58_home/.config/herdr" "$a58_bin"
make_sleeping_herdr "$a58_bin"
printf 'x = 1\n' >"$a58_home/.config/herdr/config.toml"
a58_rendered="$work/a58-rendered.sh"
render_into "$AFTER_58" "$a58_home" "$a58_rendered"
a58_rc=0
HOME="$a58_home" PATH="$a58_bin:$PATH" \
  "$watchdog_bin" "$WATCHDOG_SECONDS" bash "$a58_rendered" >/dev/null 2>"$work/a58-stderr" || a58_rc=$?
[[ $a58_rc -ne 124 ]] ||
  fail "after_58: a wedged herdr socket hung the health check past ${WATCHDOG_SECONDS}s (no bounded timeout on the herdr calls)"
[[ $a58_rc -eq 0 ]] ||
  fail "after_58: script errored under a wedged herdr (rc=$a58_rc; stderr: $(cat "$work/a58-stderr"))"
[[ ! -f $a58_home/$STAMP_REL ]] ||
  fail "after_58: the verified stamp was written though herdr never replied (wedged socket)"

printf 'PASS: every herdr call in the shared predicate is bounded by a coreutils timeout; a wedged (never-replying) herdr yields not-verified within the bound at BOTH include sites (before_10 defers --cleanup, after_58 writes no stamp) instead of hanging the apply\n'
