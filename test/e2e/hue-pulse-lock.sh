#!/usr/bin/env bash
# hue-pulse-lock: the pulse must be SINGLE-WINNER under the kernel lock (/usr/bin/lockf). Concurrent
# triggers (a Stop hook and the long-command notifier firing at once) must never interleave openhue
# calls; a wedged prior pulse (SIGKILLed mid-run, leaving stale lock state) must never suppress a later
# pulse; and every contender -- the winner and every skipped loser -- must exit 0, because the Stop hook
# execs this script and a non-zero exit breaks the always-exit-0 notification contract.
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
[[ -x /usr/bin/lockf ]] || {
  echo "hue-pulse-lock: skipped (no /usr/bin/lockf; the unlocked non-darwin path is not exercised here)"
  exit 0
}
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Mock openhue: get-room enters a critical section guarded by an atomic mkdir. If a second pulse is
# inside it concurrently, the mkdir fails and we record OVERLAP. A short sleep widens the window so a
# lock that is NOT single-winner is reliably caught. get-light returns [] so hue-pulse exits before the
# real (light-restoring) pulse work.
cat >"$tmp/openhue" <<MOCK
#!/usr/bin/env bash
if [[ "\$*" == *"get room"* ]]; then
  if ! mkdir "$tmp/section" 2>/dev/null; then touch "$tmp/OVERLAP"; fi
  sleep 0.15
  rmdir "$tmp/section" 2>/dev/null || true
  echo '[{"Name":"TEST-ROOM","Id":"room1"}]'
elif [[ "\$*" == *"get light"* ]]; then
  echo '[]'
fi
MOCK
chmod +x "$tmp/openhue"

# Pre-seed STALE lock state left by a pulse that died mid-run: a leftover lock DIRECTORY (the old
# mkdir-protocol shape) holding a DEAD pid. A kernel-lock pulse must ignore this cruft entirely; if a
# stale lock can suppress or race pulses, the concurrent stress rounds below expose it.
sleep 0 &
dead_pid=$!
wait "$dead_pid" 2>/dev/null || true
mkdir -p "$tmp/hue-pulse.lock"
printf '%s\n' "$dead_pid" >"$tmp/hue-pulse.lock/pid"

# Stress: fire N contenders at once, R rounds. Under a single-winner kernel lock exactly one runs the
# critical section per round while the rest skip immediately (exit 0). Any overlap, or any contender
# exiting non-zero, fails.
contenders=6
rounds=15
for ((r = 0; r < rounds; r++)); do
  pids=()
  for ((c = 0; c < contenders; c++)); do
    HUE_PULSE_ROOM=TEST-ROOM TMPDIR="$tmp" PATH="$tmp:$PATH" bash "$hue" 0 &
    pids+=("$!")
  done
  round_rc=0
  for p in "${pids[@]}"; do
    wait "$p" || round_rc=1
  done
  [[ $round_rc -eq 0 ]] || {
    echo "hue-pulse-lock: FAIL -- a contender exited non-zero (round $r); breaks the always-exit-0 contract" >&2
    exit 1
  }
done

[[ -e "$tmp/OVERLAP" ]] && {
  echo "hue-pulse-lock: FAIL -- pulses overlapped (the lock is not single-winner)" >&2
  exit 1
}

# The kernel releases the lock on every exit (normal or crash), so nothing is wedged: prove it by
# acquiring the lock non-blocking now. If a prior contender leaked a held lock this fails.
exec 8>>"$tmp/hue-pulse.lockf"
/usr/bin/lockf -s -t 0 8 || {
  echo "hue-pulse-lock: FAIL -- the lock is still held after every pulse exited (wedged)" >&2
  exit 1
}
exec 8>&-
echo "hue-pulse-lock: OK"
