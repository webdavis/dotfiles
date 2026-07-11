#!/usr/bin/env bash
# update-skills-launchagent-path.sh — the weekly LaunchAgent must run update-skills.sh
# with a PATH that resolves every tool the script invokes by bare name.
#
# launchd gives a job a minimal PATH, not the interactive shell's — so a tool in
# ~/.local/bin (hermes) is invisible unless the plist adds that dir. This test
# renders the plist and asserts its PATH covers both tool homes the script needs:
#   - ~/.local/bin  (hermes, invoked bare by the weekly hermes registry-update phase)
#   - /opt/homebrew/bin  (npx, jq, git, perl under launchd)
# It exists because a missing ~/.local/bin silently turned the whole hermes phase
# into a logged no-op under automation — caught in review, gated here forever.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLIST="$REPO_ROOT/Library/LaunchAgents/com.webdavis.update-skills.plist.tmpl"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $PLIST ]] || fail "plist template not found: $PLIST"

# Render the template so {{ .chezmoi.homeDir }} etc. resolve exactly as at apply time.
# --source pins the render to THIS checkout (hermetic): bare chezmoi reads the machine's
# configured sourceDir, so the test would break whenever that live checkout is on a
# different branch or holds stray state (mirrors treefmt.nix's rendered-template calls).
rendered="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$PLIST")" ||
  fail "chezmoi execute-template failed on the plist"

# Pull the PATH string: the <string> immediately after the <key>PATH</key> line.
path_value="$(printf '%s\n' "$rendered" |
  awk '/<key>PATH<\/key>/{getline; gsub(/^[[:space:]]*<string>/,""); gsub(/<\/string>[[:space:]]*$/,""); print; exit}')"
[[ -n $path_value ]] || fail "no PATH EnvironmentVariable found in the rendered plist"

home_dir="$(chezmoi --source "$REPO_ROOT" execute-template --no-tty <<<'{{ .chezmoi.homeDir }}')"

# Every dir the script's bare-name tools live in must be on the launchd PATH.
for required_dir in "$home_dir/.local/bin" "/opt/homebrew/bin"; do
  case ":$path_value:" in
    *":$required_dir:"*) : ;;
    *) fail "launchd PATH is missing $required_dir (hermes/npx would not resolve): $path_value" ;;
  esac
done

# Stronger check where the tool is actually installed on this host: hermes must
# resolve under the plist's PATH, not just the interactive shell's.
if command -v hermes >/dev/null 2>&1; then
  PATH="$path_value" command -v hermes >/dev/null 2>&1 ||
    fail "hermes resolves in this shell but NOT under the plist PATH: $path_value"
fi

echo "update-skills-launchagent-path: OK"
