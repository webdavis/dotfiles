#!/usr/bin/env bash
# herdr-plugin-build-template.sh: the two herdr plugin build chezmoiscripts
# (run_onchange_after_55/57) must share the .chezmoitemplates partial and verify
# registration with an EXACT plugin-id match.
#
# Why exact: registration is confirmed by querying the plugin by id over the
# JSON API (`plugin list --plugin <id> --json`, invoked through the bounded
# `"${herdr_cli[@]}"` timeout wrapper) and asserting the result contains an entry
# whose plugin_id EQUALS the id (jq `select(.plugin_id == $id)`, equality, not a
# substring). A substring/any-plugin check (e.g. a bare `grep -q "$plugin_id"`
# against the human list, where every id also appears inside its own local path)
# would false-positive against another plugin's line and silently skip linking.
# Equality on the parsed id cannot.
#
# Renders both templates with the host chezmoi (same mechanics as the
# rendered-template lint in treefmt.nix: scratch HOME, CI=1) and asserts:
#   1. each render queries the exact plugin id: plugin list --plugin "$plugin_id" --json
#   2. each render matches the id by equality: jq select(.plugin_id == $id)
#   3. each render carries the shared-partial marker (both scripts consume
#      .chezmoitemplates/herdr-plugin-build.sh.tmpl)
#   4. each render still parses standalone (bash -n)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

SCRIPTS=(
  "$REPO_ROOT/.chezmoiscripts/run_onchange_after_55-build-herdr-last-workspace-plugin.sh.tmpl"
  "$REPO_ROOT/.chezmoiscripts/run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl"
)

# shellcheck disable=SC2016  # the non-expanding ${plugin_id} literal is the point
EXACT_QUERY='plugin list --plugin "$plugin_id" --json'
# shellcheck disable=SC2016  # the literal jq program is the point
EXACT_MATCH='select(.plugin_id == $id)'
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

  # 1) exact plugin-id query over the JSON API
  grep -qF "$EXACT_QUERY" <<<"$rendered" ||
    fail "$(basename "$script"): registration does not query the exact plugin id ($EXACT_QUERY)"

  # 2) equality match on the parsed id (never a substring)
  grep -qF "$EXACT_MATCH" <<<"$rendered" ||
    fail "$(basename "$script"): registration does not match the id by equality ($EXACT_MATCH)"

  # 3) shared partial marker
  grep -qF "$PARTIAL_MARKER" <<<"$rendered" ||
    fail "$(basename "$script"): render does not carry the shared partial marker ($PARTIAL_MARKER)"

  # 4) render is standalone-valid bash
  bash -n <<<"$rendered" || fail "$(basename "$script"): rendered script does not parse"
done

printf 'PASS: both plugin build scripts share the partial and verify registration by exact plugin id\n'
