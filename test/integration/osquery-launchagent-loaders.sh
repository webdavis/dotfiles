#!/usr/bin/env bash
# osquery-launchagent-loaders.sh. Every osquery LaunchAgent must be a coherent
# triple: a plist that renders to a valid property list, a Label of the form
# com.webdavis.osquery-<name>, a ProgramArguments script under ~/.local/bin, and
# a loader script (run_onchange_after_60-load-osquery-<name>-launchagent) whose
# bootstrap target label and PLIST path agree with that plist. A rename or a
# copy-paste slip that desynchronizes any leg (wrong Label, stale script path,
# loader pointing at a different agent) loads nothing or loads the wrong job and
# fails silently on the live host, so this asserts the whole wiring up front.
#
# darwin-only: the plists live under Library/ (ignored on Linux) and the check
# renders them with chezmoi and lints with plutil (a macOS system tool).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 1 && pwd)"
cd "$REPO_ROOT" || exit 1

if [[ "$(uname -s)" != "Darwin" ]]; then
  printf 'SKIP: darwin-only (plists are Library/-gated, plutil is macOS-only)\n'
  exit 0
fi
if ! command -v plutil >/dev/null 2>&1; then
  printf 'SKIP: plutil not found\n'
  exit 0
fi
if ! command -v chezmoi >/dev/null 2>&1; then
  printf 'SKIP: chezmoi not found (run inside the nix dev shell)\n'
  exit 0
fi

# The six deployed osquery agents. Naming is uniform by design: agent <name> ->
# Label com.webdavis.osquery-<name>, script ~/.local/bin/osquery-<name>.sh,
# loader run_onchange_after_60-load-osquery-<name>-launchagent.sh.tmpl.
agents=(
  results-alerter
  firewall-gatekeeper-monitor
  uptime-watchdog
  digest
  heartbeat
  tailscale-monitor
)

render_home="$(mktemp -d)"
trap 'rm -rf "$render_home"' EXIT

fails=0
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  fails=$((fails + 1))
}

render() { # <template-path> -> stdout (rendered)
  HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$1"
}

for name in "${agents[@]}"; do
  label="com.webdavis.osquery-${name}"
  plist_tmpl="Library/LaunchAgents/${label}.plist.tmpl"
  loader_tmpl=".chezmoiscripts/run_onchange_after_60-load-osquery-${name}-launchagent.sh.tmpl"

  [[ -f "$plist_tmpl" ]] || { fail "$name: missing plist template $plist_tmpl"; continue; }
  [[ -f "$loader_tmpl" ]] || { fail "$name: missing loader template $loader_tmpl"; continue; }

  rendered_plist="$render_home/${name}.plist"
  if ! render "$plist_tmpl" >"$rendered_plist" 2>"$render_home/plist.err"; then
    fail "$name: plist render failed: $(cat "$render_home/plist.err")"
    continue
  fi
  if ! plutil -lint "$rendered_plist" >/dev/null 2>&1; then
    fail "$name: rendered plist is not a valid property list"
    plutil -lint "$rendered_plist" >&2 || true
    continue
  fi

  got_label="$(plutil -extract Label raw "$rendered_plist" 2>/dev/null || true)"
  [[ "$got_label" == "$label" ]] || fail "$name: plist Label is '$got_label', expected '$label'"

  # ProgramArguments must invoke ~/.local/bin/osquery-<name>.sh. Assert against the
  # rendered ProgramArguments array as JSON (plutil -extract raw yields the array
  # count for arrays, not the elements).
  # plutil's JSON escapes path slashes (\/), so match the (slash-free) basename.
  prog_json="$(plutil -extract ProgramArguments json -o - "$rendered_plist" 2>/dev/null || true)"
  grep -qF "osquery-${name}.sh" <<<"$prog_json" \
    || fail "$name: plist ProgramArguments lacks osquery-${name}.sh (got: $prog_json)"

  # The loader must bootstrap THIS label and reference THIS plist path. Render it
  # (the darwin guard and the plist-hash include both resolve under --source).
  if ! rendered_loader="$(render "$loader_tmpl" 2>"$render_home/loader.err")"; then
    fail "$name: loader render failed: $(cat "$render_home/loader.err")"
    continue
  fi
  grep -qF "com.webdavis.osquery-${name}" <<<"$rendered_loader" \
    || fail "$name: loader does not reference label com.webdavis.osquery-${name}"
  grep -qF "Library/LaunchAgents/com.webdavis.osquery-${name}.plist" <<<"$rendered_loader" \
    || fail "$name: loader does not reference plist path for ${name}"
done

if ((fails > 0)); then
  printf '%d osquery LaunchAgent wiring assertion(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'PASS: all %d osquery LaunchAgents render, lint, and wire consistently\n' "${#agents[@]}"
