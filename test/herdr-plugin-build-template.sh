#!/usr/bin/env bash
# herdr-plugin-build-template.sh — the two herdr plugin build chezmoiscripts
# (run_onchange_after_55/57) must share the .chezmoitemplates partial and link
# with an ANCHORED plugin-list match.
#
# Why anchored: `herdr plugin list` prints one line per plugin shaped like
#   - <plugin-id> (<Name>) enabled [local:<path>]
# An unanchored `grep -q "$plugin_id"` is a substring match — a plugin id that
# is a substring of another id, or of any plugin's local PATH (every id appears
# inside its own path suffix and could appear inside another's), false-positives
# and silently skips linking. Pinning the id to its list line ("- <id> ") makes
# the check exact.
#
# Renders both templates with the host chezmoi (same mechanics as the
# rendered-template lint in treefmt.nix: scratch HOME, CI=1) and asserts:
#   1. each render carries the anchored grep: grep -q "^- ${plugin_id} "
#   2. each render carries the shared-partial marker (both scripts consume
#      .chezmoitemplates/herdr-plugin-build.sh.tmpl)
#   3. each render still parses standalone (bash -n)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

SCRIPTS=(
  "$REPO_ROOT/.chezmoiscripts/run_onchange_after_55-build-herdr-last-workspace-plugin.sh.tmpl"
  "$REPO_ROOT/.chezmoiscripts/run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl"
)

# shellcheck disable=SC2016  # the non-expanding ${plugin_id} literal is the point
ANCHORED_GREP='grep -q "^- ${plugin_id} "'
PARTIAL_MARKER='.chezmoitemplates/herdr-plugin-build.sh.tmpl'

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Host-tool guard: plain test/*.sh scripts run outside the Nix shell, so skip
# cleanly when chezmoi is absent (fresh machine before the first apply).
if ! command -v chezmoi >/dev/null 2>&1; then
  printf 'SKIP: chezmoi not on PATH; cannot render the plugin build templates\n'
  exit 0
fi

# chezmoi needs a writable HOME; keep the user's real config out of the render.
scratch_home="$(mktemp -d)"
trap 'rm -rf "$scratch_home"' EXIT

for script in "${SCRIPTS[@]}"; do
  [[ -f $script ]] || fail "missing template: $script"

  rendered="$(HOME="$scratch_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$script")" ||
    fail "chezmoi failed to render $script"
  [[ -n $rendered ]] || fail "empty render (non-darwin?): $script"

  # 1) anchored plugin-list match
  grep -qF "$ANCHORED_GREP" <<<"$rendered" ||
    fail "$(basename "$script"): plugin-link check is not the anchored match ($ANCHORED_GREP)"

  # 2) shared partial marker
  grep -qF "$PARTIAL_MARKER" <<<"$rendered" ||
    fail "$(basename "$script"): render does not carry the shared partial marker ($PARTIAL_MARKER)"

  # 3) render is standalone-valid bash
  bash -n <<<"$rendered" || fail "$(basename "$script"): rendered script does not parse"
done

printf 'PASS: both plugin build scripts share the partial and use the anchored plugin-list match\n'
