#!/usr/bin/env bash
# The weekly jobs post their records through the engine, and they run under
# launchd where nothing is on PATH and nobody is watching. Their resolution is
# the same transitional rule the hooks use, held once in log-entries.sh, so
# this pins the rule and the three jobs that read it.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

# A home with a space, because these paths are interpolated in several places.
HOME="$scratch/home dir"
export HOME
mkdir -p "$HOME/.local/libexec/pns/helpers"
cp "$REPO_ROOT/dot_local/libexec/pns/helpers/engine-path.sh" \
  "$HOME/.local/libexec/pns/helpers/"

binary="$HOME/.local/libexec/pns/pns"
relay="$HOME/.local/libexec/pns/relay.sh"

fail() {
  echo "$1" >&2
  exit 1
}

# shellcheck source=dot_local/libexec/unattended-upgrades/helpers/log-entries.sh
source "$REPO_ROOT/dot_local/libexec/unattended-upgrades/helpers/log-entries.sh"

# --- the rule --------------------------------------------------------------
printf '#!/usr/bin/env bash\n' >"$relay"
chmod +x "$relay"
[[ "$(unattended_engine)" == "$relay" ]] ||
  fail "before the binary exists the weekly jobs keep the bash engine, got: $(unattended_engine)"

printf '#!/usr/bin/env bash\n' >"$binary"
chmod +x "$binary"
[[ "$(unattended_engine)" == "$binary" ]] ||
  fail "once installed the binary carries the weekly records, got: $(unattended_engine)"

[[ "$(UNATTENDED_LOG_RELAY=/custom/engine unattended_engine)" == /custom/engine ]] ||
  fail "an explicit override must win outright"

# A machine without the pns tree at all: the jobs still resolve something and
# report it, rather than aborting mid-record.
(
  PNS_HELPERS_DIR="$scratch/absent"
  export PNS_HELPERS_DIR
  [[ "$(unattended_engine)" == "$relay" ]] ||
    fail "a missing resolver degrades to the bash engine, got: $(unattended_engine)"
)

# --- the record actually goes to the resolved engine -----------------------
# unattended_log_post is what every weekly job calls; it must hand the record
# to the binary once that exists.
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$@" >"%s/posted"\nprintf "relay: posted HTTP 200\\n"\n' \
  "$scratch" >"$binary"
chmod +x "$binary"
UNATTENDED_LOG_STATE_DIR="$scratch/state" unattended_log_post \
  weekly-test 'done' project 'a detail' >/dev/null 2>&1 || true
[[ -f "$scratch/posted" ]] ||
  fail "the record must reach the resolved engine"
grep -qx -- '--remote-only' "$scratch/posted" ||
  fail "the weekly record is the durable log path, so it must stay --remote-only"

# --- the three jobs read the rule, none keeps its own default --------------
for job in \
  "dot_local/libexec/unattended-upgrades/executable_homebrew-weekly-upgrade.sh" \
  "dot_local/libexec/unattended-upgrades/claude/executable_report-plugin-updates.sh" \
  "dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh"; do
  # Any non-comment mention of the bash engine path is a default of its own.
  if grep -v '^[[:space:]]*#' "$REPO_ROOT/$job" | grep -q 'libexec/pns/relay\.sh'; then
    fail "$job still names the bash engine directly instead of the shared rule"
  fi
done

exit 0
