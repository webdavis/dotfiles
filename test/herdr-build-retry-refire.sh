#!/usr/bin/env bash
# herdr-build-retry-refire.sh: the retry marker must actually RE-FIRE the
# run_onchange trigger. chezmoi re-runs a run_onchange script only when its
# rendered content changes; a retryable non-success (missing cargo, unverified
# registration) bumps a marker file whose contents the build partial interpolates
# into the rendered trigger. This test proves that bumping the marker changes the
# render (so the next apply genuinely re-runs the build) and that a distinct
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

# --- render-interpolation sanitization (digits only, never raw bytes) -------
# A hostile multiline marker must not splice raw bytes into the script:
# '1\nfalse\n' would render a live `false` line that runs at apply time and,
# under set -e, aborts the whole apply. Only the leading digit run may reach
# the render: the hostile marker must render byte-identically to a plain '1'.
printf '1\n' >"$marker"
plain1="$(render)" || fail "render failed (marker=1, re-render)"
printf '1\nfalse\n' >"$marker"
hostile="$(render)" || fail "render failed (hostile multiline marker)"
[[ $hostile == "$plain1" ]] ||
  fail "a hostile multiline marker ('1\\nfalse\\n') changed the render beyond the digit run; raw marker bytes are being spliced into the script (live-shell injection at apply time)"
bash -n <<<"$hostile" || fail "hostile-marker render does not parse"

# An empty marker must render safely (and parse).
: >"$marker"
empty_render="$(render)" || fail "render failed (empty marker)"
bash -n <<<"$empty_render" || fail "empty-marker render does not parse"

# A garbage (non-numeric) marker: no marker bytes reach the render, and it
# still parses. (The run-time reset of a garbage COUNT is covered in
# herdr-build-scripts-resilience.sh.)
printf 'garbage\n' >"$marker"
garbage_render="$(render)" || fail "render failed (garbage marker)"
bash -n <<<"$garbage_render" || fail "garbage-marker render does not parse"
grep -q 'garbage' <<<"$garbage_render" &&
  fail "garbage marker bytes were spliced into the render"

# --- two consecutive failures progress the marker against the SAME home -----
# Run the RENDERED script twice with no cargo in the scratch home: each
# retryable non-success must bump the marker (1 then 2) and each bump must
# produce a distinct render. This kills both the constant-counter mutant and
# the delete-the-count-read mutant (either would stall the retry loop after
# the first failure).
rm -f "$marker"
run_rendered() {
  local script="$render_home/rendered-run.sh"
  render >"$script" || fail "render failed (for execution)"
  HOME="$render_home" bash "$script" >/dev/null 2>&1 ||
    fail "rendered script failed on the missing-cargo path (must exit 0, never abort the apply)"
}
run_rendered
[[ -f $marker ]] || fail "first missing-cargo run did not write the retry marker"
[[ "$(cat "$marker")" == "1" ]] ||
  fail "first missing-cargo run wrote marker '$(cat "$marker")', expected 1"
render_at_1="$(render)" || fail "render failed (marker=1 after run)"
run_rendered
[[ "$(cat "$marker")" == "2" ]] ||
  fail "second missing-cargo run wrote marker '$(cat "$marker")', expected 2 (the counter must be monotonic, not constant)"
render_at_2="$(render)" || fail "render failed (marker=2 after run)"
[[ $render_at_1 != "$render_at_2" ]] ||
  fail "consecutive failures produced identical renders; the second retry would never re-fire"

printf 'PASS: the retry marker re-fires the run_onchange trigger, interpolates digits only (hostile/empty/garbage markers render safely), and consecutive failures against one home progress the marker 1 -> 2 with distinct renders\n'
