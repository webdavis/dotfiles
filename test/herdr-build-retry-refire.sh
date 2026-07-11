#!/usr/bin/env bash
# herdr-build-retry-refire.sh — the retry marker must actually RE-FIRE the
# run_onchange trigger. chezmoi re-runs a run_onchange script only when its
# rendered content changes; a retryable non-success (missing cargo, unverified
# registration) bumps a marker file whose contents the build partial interpolates
# into the rendered trigger. This test proves that bumping the marker changes the
# render — so the next apply genuinely re-runs the build — and that a distinct
# render appears for each successive attempt count (a monotonic counter, not a
# boolean that would stop re-firing after the first retry), and that clearing the
# marker returns the render to its settled form.
#
# It renders the REAL after_55 template with the host chezmoi against a scratch
# HOME, planting the marker at the exact path the partial reads
# ($HOME/.cache/herdr-plugin-build/<id>.retry). Companion to
# herdr-build-scripts-resilience.sh, which proves the SCRIPT writes the marker;
# this proves the TEMPLATE reacts to it.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_onchange_after_55-build-herdr-last-workspace-plugin.sh.tmpl"
PLUGIN_ID="herdr-last-workspace"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if ! command -v chezmoi >/dev/null 2>&1; then
  printf 'SKIP: chezmoi not on PATH; cannot render the plugin build template\n'
  exit 0
fi
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

render_home="$(mktemp -d)"
trap 'rm -rf "$render_home"' EXIT
marker="$render_home/.cache/herdr-plugin-build/$PLUGIN_ID.retry"
mkdir -p "$(dirname "$marker")"

render() {
  HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$SCRIPT"
}

# Settled (no marker): the trigger's baseline form.
rm -f "$marker"
settled="$(render)" || fail "render failed (no marker)"
[[ -n $settled ]] || {
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
}

# Attempt 1.
printf '1\n' >"$marker"
attempt1="$(render)" || fail "render failed (marker=1)"

# Attempt 2.
printf '2\n' >"$marker"
attempt2="$(render)" || fail "render failed (marker=2)"

# Cleared again -> back to settled.
rm -f "$marker"
resettled="$(render)" || fail "render failed (marker cleared)"

[[ $settled != "$attempt1" ]] ||
  fail "bumping the retry marker (0 -> 1) did not change the rendered trigger; chezmoi would not re-run the build"
[[ $attempt1 != "$attempt2" ]] ||
  fail "a second retry (1 -> 2) did not change the render; a boolean marker would stall retries after the first"
[[ $settled == "$resettled" ]] ||
  fail "clearing the marker did not return the render to its settled form (the trigger would never settle)"

printf 'PASS: the retry marker re-fires the run_onchange trigger — each attempt count renders distinctly and clearing settles it\n'
