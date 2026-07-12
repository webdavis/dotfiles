#!/usr/bin/env bash
# hue-pulse-lock: two pulses fired at once must serialize (queue) through the lock, never overlap their
# openhue calls. A mock openhue flags any overlap atomically (mkdir); the lock must prevent it and release.
set -uo pipefail
hue="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/dot_local/bin/executable_hue-pulse.sh"
[[ -x $hue ]] || {
  echo "hue-pulse-lock: FAIL -- not executable: $hue" >&2
  exit 1
}
command -v jq >/dev/null 2>&1 || {
  echo "hue-pulse-lock: skipped (no jq)"
  exit 0
}
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Mock openhue: only the get-room + get-light path is exercised (get-light returns [] so hue-pulse exits
# before the real pulse). get-room enters a critical section guarded by an atomic mkdir; if a second pulse
# is inside it at the same time, the mkdir fails and we record OVERLAP.
cat >"$tmp/openhue" <<MOCK
#!/usr/bin/env bash
if [[ "\$*" == *"get room"* ]]; then
  mkdir "$tmp/section" 2>/dev/null || touch "$tmp/OVERLAP"
  sleep 0.4
  rmdir "$tmp/section" 2>/dev/null || true
  echo '[{"Name":"TEST-ROOM","Id":"room1"}]'
elif [[ "\$*" == *"get light"* ]]; then
  echo '[]'
fi
MOCK
chmod +x "$tmp/openhue"

# Fire two pulses simultaneously (children of THIS shell, so wait actually waits for them).
HUE_PULSE_ROOM=TEST-ROOM TMPDIR="$tmp" PATH="$tmp:$PATH" bash "$hue" 0 &
HUE_PULSE_ROOM=TEST-ROOM TMPDIR="$tmp" PATH="$tmp:$PATH" bash "$hue" 0 &
wait

[[ -e "$tmp/OVERLAP" ]] && {
  echo "hue-pulse-lock: FAIL -- two pulses ran concurrently (lock did not queue them)" >&2
  exit 1
}
[[ -e "$tmp/hue-pulse.lock" ]] && {
  echo "hue-pulse-lock: FAIL -- lock dir leaked (not released on exit)" >&2
  exit 1
}

# FIX 3 (stale lock recovery): a wedged lock from a process that died mid-pulse (e.g. SIGKILL)
# must not suppress every later pulse forever. Pre-seed the lock dir with a DEAD pid; the next
# pulse must detect the dead holder, take the lock over, proceed to openhue, and release it.
: >"$tmp/GOTROOM"
rm -f "$tmp/GOTROOM"
cat >"$tmp/openhue" <<MOCK
#!/usr/bin/env bash
if [[ "\$*" == *"get room"* ]]; then
  touch "$tmp/GOTROOM"
  echo '[{"Name":"TEST-ROOM","Id":"room1"}]'
elif [[ "\$*" == *"get light"* ]]; then
  echo '[]'
fi
MOCK
chmod +x "$tmp/openhue"
# A guaranteed-dead PID: fork a child, wait for it to exit, then reuse its (now free) pid.
sleep 0 &
dead_pid=$!
wait "$dead_pid" 2>/dev/null || true
mkdir -p "$tmp/hue-pulse.lock"
printf '%s\n' "$dead_pid" >"$tmp/hue-pulse.lock/pid"
HUE_PULSE_ROOM=TEST-ROOM TMPDIR="$tmp" PATH="$tmp:$PATH" bash "$hue" 0
[[ -e "$tmp/GOTROOM" ]] || {
  echo "hue-pulse-lock: FAIL -- a stale (dead-PID) lock suppressed the pulse (never reached openhue)" >&2
  exit 1
}
[[ -e "$tmp/hue-pulse.lock" ]] && {
  echo "hue-pulse-lock: FAIL -- stale lock not replaced/released after takeover" >&2
  exit 1
}
echo "hue-pulse-lock: OK"
