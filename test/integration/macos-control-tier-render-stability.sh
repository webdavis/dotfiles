#!/usr/bin/env bash
# macos-control-tier-render-stability.sh -- the tier field is REQUIRED on every
# record in .chezmoidata/macos_defaults.yaml and
# .chezmoidata/macos_system_setup.yaml, and backfilling `tier: enforce` onto the
# records that existed before the field did is RENDER-NEUTRAL: both runner
# templates render byte-identically to their pre-tier renders.
#
# The properties pinned:
#   1. Completeness, not a count: EVERY record in both real data files declares
#      a tier, and every declared tier is one of enforce/verify/manual. A new
#      record cannot land without one, because the runners abort the render,
#      and this guard reports the gap by name before an apply ever trips it.
#   2. Byte identity: the Tier 1 and Tier 2 runners, rendered against the
#      repo's REAL data, match the goldens below exactly.
#
# The goldens are the renders of both templates at commit 5774ce0 (this slice's
# base, before the tier field existed) against the real data of that commit,
# captured byte for byte including the trailing blank line. They pin that the
# tier MECHANISM changed nothing about what the existing controls execute. A
# later slice that declares NEW records changes the real render legitimately;
# it must re-derive these goldens as part of its own render assertions.
#
# Real chezmoi and yq; nothing is executed, only rendered.
set -euo pipefail

# Scrubbed at SCRIPT scope, before any chezmoi call. Git exports GIT_DIR to
# every hook it runs and this suite runs from the pre-push hook.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TIER1_TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl"
TIER2_TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl"
DEFAULTS_YAML="$REPO_ROOT/.chezmoidata/macos_defaults.yaml"
SETUP_YAML="$REPO_ROOT/.chezmoidata/macos_system_setup.yaml"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

for tool in chezmoi yq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise render stability\n' "$tool"
    exit 0
  }
done
for required_file in "$TIER1_TEMPLATE" "$TIER2_TEMPLATE" "$DEFAULTS_YAML" "$SETUP_YAML"; do
  [[ -f $required_file ]] || fail "missing file: $required_file"
done

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$work"' EXIT

# ---- 1: every real record declares a recognized tier -------------------------

missing_defaults_tiers="$(yq eval -r \
  '[.macos.defaults[] | select(has("tier") | not) | .domain + " " + .key] | join(", ")' \
  "$DEFAULTS_YAML")"
[[ -z $missing_defaults_tiers ]] ||
  fail "every macos_defaults.yaml record must declare a tier; missing on: $missing_defaults_tiers"

invalid_defaults_tiers="$(yq eval -r \
  '[.macos.defaults[] | select(.tier != "enforce" and .tier != "verify" and .tier != "manual") | .domain + " " + .key] | join(", ")' \
  "$DEFAULTS_YAML")"
[[ -z $invalid_defaults_tiers ]] ||
  fail "every macos_defaults.yaml tier must be enforce, verify, or manual; violated on: $invalid_defaults_tiers"

missing_setup_tiers="$(yq eval -r \
  '[.macos.system_setup[] | select(has("tier") | not) | .description] | join(", ")' \
  "$SETUP_YAML")"
[[ -z $missing_setup_tiers ]] ||
  fail "every macos_system_setup.yaml record must declare a tier; missing on: $missing_setup_tiers"

invalid_setup_tiers="$(yq eval -r \
  '[.macos.system_setup[] | select(.tier != "enforce" and .tier != "verify" and .tier != "manual") | .description] | join(", ")' \
  "$SETUP_YAML")"
[[ -z $invalid_setup_tiers ]] ||
  fail "every macos_system_setup.yaml tier must be enforce, verify, or manual; violated on: $invalid_setup_tiers"

# ---- 2: both runners render byte-identically to the pre-tier goldens ---------

# The Tier 1 runner at 5774ce0, rendered against that commit's real data.
tier1_golden="$work/tier1.golden"
cat >"$tier1_golden" <<'GOLDEN_EOF'
#!/bin/bash
# Tier 1, macOS user defaults runner.
# chezmoi hash-gates on the rendered template body; this script re-runs only
# when .chezmoidata/macos_defaults.yaml changes.

set -euo pipefail

# Pre-flight: close System Settings if open. macOS caches plist values inside
# Settings and writes them back on close, silently overwriting our writes.
osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true

# Main loop: one `defaults write` per record.
defaults write 'com.apple.dock' 'mru-spaces' -bool 'false'
defaults write 'com.apple.dock' 'expose-group-apps' -bool 'false'
defaults write 'com.apple.WindowManager' 'GloballyEnabled' -bool 'false'
defaults write 'com.apple.WindowManager' 'EnableStandardClickToShowDesktop' -bool 'false'
defaults write 'com.apple.WindowManager' 'EnableTilingByEdgeDrag' -bool 'false'
defaults write 'com.apple.WindowManager' 'EnableTilingOptionAccelerator' -bool 'false'
defaults write 'com.apple.WindowManager' 'EnableTopTilingByEdgeDrag' -bool 'false'
# Post-loop: restart user-facing processes so changes take effect immediately.
# cfprefsd kill is non-negotiable (caches plist values in memory).
killall 'Dock' 2>/dev/null || true
killall 'Finder' 2>/dev/null || true
killall 'SystemUIServer' 2>/dev/null || true
killall 'cfprefsd' 2>/dev/null || true

GOLDEN_EOF

# The Tier 2 runner at 5774ce0, rendered against that commit's real data.
tier2_golden="$work/tier2.golden"
cat >"$tier2_golden" <<'GOLDEN_EOF'
#!/bin/bash
# Tier 2, macOS sudo system-setup runner.
# chezmoi hash-gates on the rendered template body; this script re-runs only
# when .chezmoidata/macos_system_setup.yaml changes.

set -euo pipefail

# Pre-flight: refresh sudo timestamp upfront. One password prompt at start;
# none during the loop, it covers the generated tailnet-pin commands too.
sudo -v

echo "→ Install self-healing nix-installer repair LaunchDaemon (NixOS fork)"
sudo "$HOME/.local/bin/install-nix-repair-hook.sh"
echo "→ MagicDNS fallback pin: mister.tail2f2430.ts.net (per CLAUDE.md Tailscale DNS section)"
sudo sh -c 'grep -qF "mister.tail2f2430.ts.net" /etc/hosts || printf "100.109.58.54\tmister.tail2f2430.ts.net\tmister\n" >>/etc/hosts'

GOLDEN_EOF

render_home="$work/render-home"
mkdir -p "$render_home"
render_error="$work/render.err"

render_current() { # <template> <out-file>
  HOME="$render_home" chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$1" >"$2" 2>"$render_error"
}

tier1_rendered="$work/tier1.rendered"
render_current "$TIER1_TEMPLATE" "$tier1_rendered" ||
  fail "the Tier 1 runner must render against the real data (stderr: $(cat "$render_error"))"
if [[ -z "$(tr -d '[:space:]' <"$tier1_rendered")" ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi
cmp -s "$tier1_golden" "$tier1_rendered" ||
  fail "the Tier 1 runner must render byte-identically to its pre-tier golden (diff: $(diff "$tier1_golden" "$tier1_rendered" | head -20))"

tier2_rendered="$work/tier2.rendered"
render_current "$TIER2_TEMPLATE" "$tier2_rendered" ||
  fail "the Tier 2 runner must render against the real data (stderr: $(cat "$render_error"))"
cmp -s "$tier2_golden" "$tier2_rendered" ||
  fail "the Tier 2 runner must render byte-identically to its pre-tier golden (diff: $(diff "$tier2_golden" "$tier2_rendered" | head -20))"

printf 'macos-control-tier-render-stability: OK (every real record declares a recognized tier; both runners render byte-identically to their 5774ce0 goldens)\n'
