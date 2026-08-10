#!/usr/bin/env bash
# Claude Code UserPromptSubmit hook: record session start time on first prompt.
#
# Hook input: JSON on stdin with { session_id, transcript_path, cwd,
# permission_mode, hook_event_name, prompt }.
#
# Paired with claude-stop-pulse.sh which reads the marker to decide whether
# to fire a Hue pulse.

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
mkdir -p "$state_dir" 2>/dev/null || exit 0
[[ -f $start_file ]] || date +%s >"$start_file"
exit 0
