#!/usr/bin/env bash
# herdr-plugin-build-timeout.sh: a wedged herdr socket (accepts the connection
# but never replies) would hang `herdr plugin list` / `herdr plugin link` in the
# shared plugin-build partial (.chezmoitemplates/herdr-plugin-build.sh.tmpl)
# forever. That partial runs on every apply through its includers
# (run_onchange_after_55 and run_onchange_after_57), and its registration phase
# runs BEFORE after_58's already-bounded health check can protect the apply, so
# one wedged server would block the apply here. Every herdr call in the partial
# must therefore be bounded by the SAME coreutils timeout the health-check
# partial uses; an expiry counts as a RETRYABLE registration failure (the retry
# marker is left set so the next apply retries), never a hang and never an abort.
#
# This drives BOTH includers through their real render with a cargo stub that
# "builds" successfully (so control reaches the registration phase) and a herdr
# stub that sleeps far past the per-call bound (the "never replies" wedge), the
# whole run wrapped in an OUTER watchdog. A run that returns 124 from the
# watchdog is a hang and fails the test; the fixed partial returns well within
# the bound, exits 0 (never aborts the apply), and leaves the retry marker set.
#
# after_55 has a SECOND unbounded herdr path the registration bound does not
# cover: its seed postamble runs the compiled plugin binary as `<binary> seed`,
# and that binary shells `herdr workspace list` with no timeout of its own. The
# earlier version of this test staged NO plugin binary (the cargo stub just
# exited 0), so `[[ -x $binary ]]` was false and the seed path never ran -- a
# blind spot. Here the cargo stub EMITS an executable fake plugin binary whose
# `seed` reaches the sleeping herdr, so an unbounded seed hangs the whole script
# and trips the watchdog; the fixed after_55 wraps the seed in the same coreutils
# timeout, so it returns within the bound (best-effort: a seed expiry is a
# warn-and-continue, never an abort).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
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
    printf 'SKIP: %s not on PATH; cannot render/run the plugin build scripts\n' "$tool"
    exit 0
  fi
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# A herdr stub that never replies: every subcommand sleeps far past the bound.
make_sleeping_herdr() {
  local dir="$1"
  cat >"$dir/herdr" <<'STUB'
#!/bin/bash
sleep 300
exit 0
STUB
  chmod +x "$dir/herdr"
}

# A cargo stub that "builds" successfully so control reaches the registration
# phase AND the seed postamble. On `build` (CWD is the plugin dir) it emits an
# EXECUTABLE fake plugin binary at target/release/last-workspace -- the path
# after_55's seed invokes -- by copying the pre-written binary at $FAKE_SEED_BIN.
# That binary's `seed` shells the (sleeping) herdr exactly as the real plugin
# does, so an unbounded seed hangs the whole script.
make_cargo_stub() {
  local path="$1"
  mkdir -p "$(dirname "$path")"
  cat >"$path" <<'STUB'
#!/bin/bash
if [[ ${1:-} == build ]]; then
  mkdir -p target/release
  cp "$FAKE_SEED_BIN" target/release/last-workspace
  chmod +x target/release/last-workspace
fi
exit 0
STUB
  chmod +x "$path"
}

# The fake plugin binary the cargo stub installs: its `seed` records that it ran
# (so the test can prove the seed path was exercised) and then shells the
# sleeping herdr, which blocks until a timeout kills it. Any other invocation is
# a fast no-op.
FAKE_SEED_BIN="$work/fake-last-workspace"
cat >"$FAKE_SEED_BIN" <<'BIN'
#!/bin/bash
if [[ ${1:-} == seed ]]; then
  printf 'seed-started\n' >>"$SEED_STARTED_FILE"
  herdr workspace list
fi
exit 0
BIN
chmod +x "$FAKE_SEED_BIN"

# PATH with every dir that carries an executable `cargo` removed, so the stub
# cargo at ~/.cargo/bin is the only one and coreutils/jq stay reachable.
path_without_cargo() {
  local -a dirs=() keep=()
  IFS=: read -ra dirs <<<"$PATH"
  local d
  for d in "${dirs[@]}"; do
    [[ -n $d ]] || continue
    [[ -x $d/cargo ]] && continue
    keep+=("$d")
  done
  local IFS=:
  printf '%s' "${keep[*]}"
}
PATH_NO_CARGO="$(path_without_cargo)"

# run_one <script-basename> <plugin-id> <has-seed>: render the includer, stage
# cargo+herdr stubs (the cargo stub emits the fake plugin binary), run it under
# the outer watchdog with a sleeping herdr, and assert it completed within the
# bound, exited 0, and left the retry marker set (retryable non-success). When
# <has-seed> is `seed`, also assert the seed postamble actually ran (proving the
# newly-covered seed path was exercised and bounded, not merely skipped).
run_one() {
  local script="$1" plugin_id="$2" has_seed="${3:-no}"
  local template="$REPO_ROOT/.chezmoiscripts/$script"
  [[ -f $template ]] || fail "missing template: $template"

  local case_home="$work/$plugin_id/home"
  local bin="$work/$plugin_id/bin"
  local plugin_dir="$case_home/.local/share/herdr/plugins/$plugin_id"
  local marker="$case_home/.cache/herdr-plugin-build/$plugin_id.retry"
  local seed_started="$work/$plugin_id/seed-started"
  mkdir -p "$bin" "$plugin_dir/target/release" "$case_home/.local/bin"
  make_sleeping_herdr "$bin"
  make_cargo_stub "$case_home/.cargo/bin/cargo"

  # Render the darwin-only script (scratch HOME, CI=1). Empty render == non-darwin.
  local rendered="$work/$plugin_id/rendered.sh"
  local render_home="$work/$plugin_id/render-home"
  mkdir -p "$render_home"
  HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$template" >"$rendered" || fail "chezmoi failed to render $template"
  if [[ ! -s $rendered ]]; then
    printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
    exit 0
  fi

  local rc=0
  HOME="$case_home" PATH="$bin:$case_home/.cargo/bin:$PATH_NO_CARGO" \
    FAKE_SEED_BIN="$FAKE_SEED_BIN" SEED_STARTED_FILE="$seed_started" \
    "$watchdog_bin" "$WATCHDOG_SECONDS" bash "$rendered" \
    >"$work/$plugin_id/stdout" 2>"$work/$plugin_id/stderr" || rc=$?
  [[ $rc -ne 124 ]] ||
    fail "$script: a wedged herdr socket hung the plugin build past ${WATCHDOG_SECONDS}s (an unbounded herdr call -- registration or the seed postamble -- was not timeout-wrapped)"
  [[ $rc -eq 0 ]] ||
    fail "$script: script errored under a wedged herdr (rc=$rc; stderr: $(cat "$work/$plugin_id/stderr"))"
  [[ -f $marker ]] ||
    fail "$script: a timed-out registration must leave the retry marker set (trigger un-consumed), but it is absent"
  if [[ $has_seed == seed ]]; then
    [[ -f $seed_started ]] ||
      fail "$script: the seed postamble never ran (the fake plugin binary's seed was not invoked); the seed path is not being exercised, so its bound is untested"
  fi
}

run_one run_onchange_after_55-build-herdr-last-workspace-plugin.sh.tmpl herdr-last-workspace seed
run_one run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl herdr-smart-nav no

printf 'PASS: every herdr call in the plugin-build partial AND the after_55 seed postamble is bounded by a coreutils timeout; a wedged (never-replying) herdr yields a retryable registration failure (marker set, exit 0) and a warn-and-continue seed within the bound instead of hanging the apply\n'
