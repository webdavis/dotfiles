#!/usr/bin/env bash
# Claude Code Stop hook: pulse Hue green when a TURN of 5+ minutes ends. Per
# turn, not per session: the marker is written at the prompt and removed here,
# so the elapsed time below measures the turn that just finished.
#
# Hook input: JSON on stdin with { session_id, transcript_path, cwd,
# permission_mode, hook_event_name }. Env vars do NOT carry the session ID.
#
# Paired with claude-user-prompt-start.sh which writes the turn start marker.

set -euo pipefail
# shellcheck source=dot_local/libexec/pns/helpers/event.sh
source "${PNS_HELPERS_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/helpers}/event.sh"

input=$(cat)
session_id=$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null || true)
[[ -z $session_id ]] && exit 0

# The marker lives under $HOME, not in shared /tmp: another local account can
# pre-create a predictable /tmp path, and a planted timestamp there would flash
# the room on demand. The session id is validated before it becomes a filename,
# because it arrives in the harness payload and `..` in it would escape.
state_dir="${PNS_STATE_DIR:-$HOME/.local/state/pns}"
pns_session_id_is_safe "$session_id" || exit 0
start_file="$state_dir/session-${session_id}.start"
[[ -f $start_file ]] || exit 0

# The marker is VALIDATED before it reaches arithmetic. A corrupt one (a
# truncated write, a hand edit) otherwise aborts the hook outright: under
# `set -u` bash reads `not-a-timestamp` as three unbound variables and exits
# non-zero, which is a crash rather than a decision, and a hook that exits
# non-zero is noise the harness reports. Found by mutation testing, which
# showed the "corrupt marker" test passing because of the crash rather than
# because of the guard below.
started="$(cat "$start_file" 2>/dev/null || true)"
rm -f "$start_file"
[[ $started =~ ^[0-9]+$ ]] || exit 0
elapsed=$(($(date +%s) - started))

pns_session_was_long "$elapsed" "${PNS_PULSE_THRESHOLD_SECS:-300}" || exit 0

# The engine's pulse mode, behind the CONFIG: without an enabled hue table it
# would return silently, so no config means no pulse, said by doing nothing.
# PNS_ENGINE_BIN and PNS_CONFIG_FILE are the test seams.
engine="${PNS_ENGINE_BIN:-$HOME/.local/libexec/pns/pns}"
config="${PNS_CONFIG_FILE:-$HOME/.config/pns/config.toml}"
[[ -f $engine && -x $engine && -f $config ]] || exit 0
exec "$engine" pulse 0
