#!/usr/bin/env bash
# macos-control-tier-render-stability.sh -- the tier field is REQUIRED on every
# record in .chezmoidata/macos_defaults.yaml and
# .chezmoidata/macos_system_setup.yaml, and backfilling `tier: enforce` onto the
# records that existed before the field did is RENDER-NEUTRAL: both runner
# templates render byte-identically to their goldens below.
#
# The properties pinned:
#   1. Completeness, not a count: EVERY record in both real data files declares
#      a tier, and every declared tier is one of enforce/verify/manual. A new
#      record cannot land without one, because the runners abort the render,
#      and this guard reports the gap by name before an apply ever trips it.
#   2. Byte identity: the Tier 1 and Tier 2 runners, rendered against the
#      repo's REAL data, match the goldens below exactly.
#
# The Tier 1 golden was re-derived by the security-defaults slice after it
# declared the SoftwareUpdate and Safari records: it pins the 5774ce0 render
# (the tier slice's base, before the tier field existed) PLUS the sudo -v
# prelude the system-scope record triggers and exactly those two records'
# write lines, nothing else, byte for byte including the trailing blank
# line. The Tier 2 golden was re-derived by the firewall-baseline slice
# after it declared the firewall records, so it pins the 5774ce0 render PLUS
# exactly those records' lines and nothing else. A later slice that declares
# NEW records changes the real render legitimately; it must re-derive these
# goldens as part of its own render assertions.
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

# Validated BEFORE any trap is armed and before any cd: on bash 3.2 `cd ""`
# succeeds without moving, so an unguarded `cd "$(mktemp -d)"` after a failed
# mktemp would leave the suite in the worktree with an `rm -rf` trap aimed at
# it. The second assignment canonicalizes away macOS's /var -> /private/var
# symlink.
work="$(mktemp -d)"
[[ -n $work && -d $work ]] ||
  fail "mktemp -d produced no usable work directory (got '$work')"
work="$(cd "$work" && pwd -P)"
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

# ---- 2: both runners render byte-identically to their goldens ----------------

# The Tier 1 runner's 5774ce0 render plus the security-defaults records (the
# sudo -v prelude, the sudo-routed system-scope SoftwareUpdate write, and the
# user-scope Safari write). The six LuLu policy records the LuLu-posture
# slice declared are tier: verify (LuLu's extension loads its preferences
# once at start and writes them back from memory, so an external write is
# unobserved and clobbered) and render NO line here at all.
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

# Sudo prelude: at least one record targets a root-owned system plist, so
# validate sudo once, up front, before any write.
sudo -v
# system_defaults_write <plist_path> <key> <-type> <value>: one system-scope
# write, then a root:wheel 0644 repair of the file `defaults` just replaced.
# `defaults write` recreates its target as a root-owned 0600 binary plist
# (verified on a copy, 2026-07-27), and an unreadable plist blinds the
# unprivileged drift reader (`just D`) on every later run, so an unrepaired
# write defeats the very drift check that verifies it. The repair is PER
# WRITE, never a trailing chmod: under set -e a failed later write ends this
# run before any trailing cleanup, leaving the writes that DID land
# unreadable. The write's own failure is re-raised AFTER the repair; a failed
# write that left no file behind skips the repair rather than failing on the
# missing path. Mirrors system_defaults_write in macos-defaults-lib.sh, which
# repairs the apply tool's writes the same way.
system_defaults_write() {
  local plist_path="$1" key="$2" type_option="$3" value="$4"
  local write_status=0 written_file="$plist_path"
  [[ $written_file == *.plist ]] || written_file="$written_file.plist"
  sudo defaults write "$plist_path" "$key" "$type_option" "$value" || write_status=$?
  if [[ $write_status -eq 0 || -e $written_file ]]; then
    sudo chown root:wheel "$written_file"
    sudo chmod 644 "$written_file"
  fi
  return "$write_status"
}
# Main loop: one `defaults write` per record.
defaults write 'com.apple.dock' 'mru-spaces' -bool 'false'
defaults write 'com.apple.dock' 'expose-group-apps' -bool 'false'
defaults write 'com.apple.WindowManager' 'GloballyEnabled' -bool 'false'
defaults write 'com.apple.WindowManager' 'EnableStandardClickToShowDesktop' -bool 'false'
defaults write 'com.apple.WindowManager' 'EnableTilingByEdgeDrag' -bool 'false'
defaults write 'com.apple.WindowManager' 'EnableTilingOptionAccelerator' -bool 'false'
defaults write 'com.apple.WindowManager' 'EnableTopTilingByEdgeDrag' -bool 'false'
system_defaults_write '/Library/Preferences/com.apple.SoftwareUpdate' 'AutomaticCheckEnabled' -bool 'true'
defaults write 'com.apple.Safari' 'AutoOpenSafeDownloads' -bool 'false'
# Post-loop: restart user-facing processes so changes take effect immediately.
# cfprefsd kill is non-negotiable (caches plist values in memory).
killall 'Dock' 2>/dev/null || true
killall 'Finder' 2>/dev/null || true
killall 'SystemUIServer' 2>/dev/null || true
killall 'cfprefsd' 2>/dev/null || true

GOLDEN_EOF

# The Tier 2 runner's 5774ce0 render plus the firewall-baseline records
# (global state first; stealth and both signed-software policies after it),
# plus the SSH drop-in record the ssh-configuration slice declared (no sudo
# prefix by design: the script escalates per operation itself), plus the
# MANUAL pointer for OverSight's Notification Center delivery, its only
# output channel (a runbook echo and no command, the OverSight-posture
# slice; deliberately NOT a microphone or camera grant, which macOS never
# presents for OverSight: no usage-description keys, no entitlements), plus
# the two MANUAL LuLu pointers the LuLu-posture slice declared (the
# system-extension approval and interactive-only rule creation; a runbook
# echo each and no command).
# No manual logging record: firewall logging on this macOS version is on by
# default and cannot be enabled by hand, so nothing renders for it.
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
echo "→ Firewall: enable the application firewall (must stay before the records below so a partial run leaves the firewall on, never policies stored with the firewall off)"
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --setglobalstate on
echo "→ Firewall: enable stealth mode (drop unsolicited probes silently)"
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --setstealthmode on
echo "→ Firewall: auto-allow incoming connections for built-in signed software"
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --setallowsigned on
echo "→ Firewall: auto-allow incoming connections for downloaded signed software"
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --setallowsignedapp on
echo "→ SSH: install the public-key-only sshd drop-in (000-ssh-hardening.conf) and verify the effective configuration"
"$HOME/.local/bin/ssh-hardening.sh"
echo '→ MANUAL OverSight: allow its Notification Center alerts (its only output channel): see the runbook section OverSight notification delivery'
echo '→ MANUAL LuLu: approve its system extension (a one-time macOS security consent): see the runbook section LuLu system extension approval'
echo '→ MANUAL LuLu: create the required outbound allow rules by answering its prompts: see the runbook section LuLu rule creation'
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
# The skip is gated on the ACTUAL host OS, never on the render coming out
# empty: emptiness conflates "non-darwin host" (skip, by design) with "the
# template's OS guard is broken on darwin" (a failure this test exists to
# catch). An empty render on darwin must fail loudly, not skip.
[[ "$(uname)" == Darwin ]] || {
  printf 'SKIP: non-darwin host; the runners render empty by design off darwin\n'
  exit 0
}
[[ -n "$(tr -d '[:space:]' <"$tier1_rendered")" ]] ||
  fail "the Tier 1 runner rendered EMPTY on a darwin host; its OS guard is broken (template: $TIER1_TEMPLATE; stderr: $(cat "$render_error"))"
cmp -s "$tier1_golden" "$tier1_rendered" ||
  fail "the Tier 1 runner must render byte-identically to its pre-tier golden (diff: $(diff "$tier1_golden" "$tier1_rendered" | head -20))"

tier2_rendered="$work/tier2.rendered"
render_current "$TIER2_TEMPLATE" "$tier2_rendered" ||
  fail "the Tier 2 runner must render against the real data (stderr: $(cat "$render_error"))"
[[ -n "$(tr -d '[:space:]' <"$tier2_rendered")" ]] ||
  fail "the Tier 2 runner rendered EMPTY on a darwin host; its OS guard is broken (template: $TIER2_TEMPLATE; stderr: $(cat "$render_error"))"
cmp -s "$tier2_golden" "$tier2_rendered" ||
  fail "the Tier 2 runner must render byte-identically to its golden (diff: $(diff "$tier2_golden" "$tier2_rendered" | head -20))"

printf 'macos-control-tier-render-stability: OK (every real record declares a recognized tier; both runners render byte-identically to their goldens)\n'
