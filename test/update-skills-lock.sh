#!/usr/bin/env bash
# update-skills-lock.sh, the serialize lock must NEVER steal from a live run by
# age; it reclaims ONLY from a provably dead owner, and its EXIT trap removes
# only a lock it still owns. This closes the three-writer race the audit found:
# the old lock removed any dir older than 120 min (killing a long healthy run),
# and its unconditional `trap rm -rf` then deleted a newcomer's lock.
#
# macOS ships neither flock(1) nor lockf(1), so the lock stays a mkdir lock, but
# it now carries an owner token: PID plus the process start time (a boot-stable
# discriminator, so a recycled PID cannot masquerade as the original owner).
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"
LOCKDIR="$HOME/.agents/.update-skills.lock.d"

cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {},
  "clawhubTracked": {},
  "forks": {}
}
EOF

# Offline stubs so a proceeding full run is fast and never hits the network.
stub="$tmp/stub"
mkdir -p "$stub"
cat >"$stub/npx" <<'EOF'
#!/usr/bin/env bash
echo stub
EOF
chmod +x "$stub"/*
export PATH="$stub:$PATH"

proc_start() { ps -o lstart= -p "$1" 2>/dev/null | tr -s ' ' | sed 's/^ *//;s/ *$//'; }

# ── 1) live owner, even with an OLD lock dir → contender must NOT steal ──────
# (The old age-gate would `rm -rf` this lock because its mtime is > 120 min,
# killing a live run. The new lock reclaims only from a dead owner.)
sleep 300 &
live_pid=$!
mkdir -p "$LOCKDIR"
printf '%s\t%s' "$live_pid" "$(proc_start "$live_pid")" >"$LOCKDIR/owner"
touch -t 200001010000 "$LOCKDIR" # make the dir look ancient
out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" || fail "script exited non-zero on contention: $out"
printf '%s\n' "$out" | grep -qi 'another run in progress' ||
  fail "contender did not defer to a live owner (age-steal bug?): $out"
[[ -d $LOCKDIR ]] || fail "contender removed a live owner's lock dir (age-steal)"
[[ "$(cat "$LOCKDIR/owner" 2>/dev/null)" == "$live_pid"* ]] ||
  fail "contender overwrote a live owner's token"
kill "$live_pid" 2>/dev/null || true
wait "$live_pid" 2>/dev/null || true
rm -rf "$LOCKDIR"

# ── 2) dead owner → reclaim and proceed ─────────────────────────────────────
mkdir -p "$LOCKDIR"
printf '%s\t%s' "999999" "Sat Jan  1 00:00:00 2000" >"$LOCKDIR/owner" # pid 999999 is dead
out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" ||
  fail "script exited non-zero reclaiming a dead lock: $out"
printf '%s\n' "$out" | grep -qi 'reclaim' || fail "did not reclaim a dead owner's lock: $out"
printf '%s\n' "$out" | grep -qF '[update-skills] done' || fail "did not proceed after reclaiming: $out"
[[ ! -d $LOCKDIR ]] || fail "a successful run left its own lock behind (trap did not release)"

# ── 3) EXIT trap must not remove a lock it no longer owns (hijack) ───────────
# The npx stub blocks until we release it, so we can rewrite the owner token
# mid-run (simulating a reclaim by a later run). The trap must see the mismatch
# and leave the lock alone.
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
while [[ ! -e "$tmp/go" ]]; do sleep 0.05; done
echo stub
EOF
chmod +x "$stub/npx"
rm -f "$tmp/go"
(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1) &
run_pid=$!
until [[ -f "$LOCKDIR/owner" ]]; do sleep 0.05; done
printf '%s\t%s' "424242" "Hijacked start" >"$LOCKDIR/owner" # a foreign owner
touch "$tmp/go"
wait "$run_pid"
[[ -d $LOCKDIR ]] || fail "the EXIT trap removed a lock it no longer owned (hijacked token)"
[[ "$(cat "$LOCKDIR/owner" 2>/dev/null)" == "424242"* ]] ||
  fail "the hijacked owner token was clobbered by the finishing run"
rm -rf "$LOCKDIR"

# Reset npx to the fast non-blocking stub for the remaining cases.
cat >"$stub/npx" <<'EOF'
#!/usr/bin/env bash
echo stub
EOF
chmod +x "$stub/npx"

# ── 4) ownerless lock (crash between mkdir and token write) → reclaimable ────
# Item 3(c): a lock dir with NO owner token must not wedge every later run
# forever; a run must reclaim it and proceed.
mkdir -p "$LOCKDIR" # no owner file written at all
out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" ||
  fail "script exited non-zero on an ownerless lock: $out"
printf '%s\n' "$out" | grep -qi 'reclaim' ||
  fail "an ownerless lock (no token) was not reclaimed (wedge-forever bug): $out"
printf '%s\n' "$out" | grep -qF '[update-skills] done' ||
  fail "did not proceed after reclaiming an ownerless lock: $out"
[[ ! -d $LOCKDIR ]] || fail "a successful run left its own lock behind after an ownerless reclaim"

# ── 5) live owner but a FAILING ps lookup → treated ALIVE (defer) ────────────
# Item 3(b): kill -0 succeeds, but if ps errors we cannot prove the owner dead,
# so we must NOT steal. A ps stub that always fails simulates an unreadable
# process table for the owner-liveness lookup.
psfail="$tmp/psfail"
mkdir -p "$psfail"
cat >"$psfail/ps" <<'EOF'
#!/usr/bin/env bash
echo "ps: simulated failure" >&2
exit 1
EOF
chmod +x "$psfail/ps"
sleep 300 &
live_pid2=$!
mkdir -p "$LOCKDIR"
printf '%s\t%s' "$live_pid2" "$(proc_start "$live_pid2")" >"$LOCKDIR/owner"
out="$(PATH="$psfail:$PATH" UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" ||
  fail "script exited non-zero on a failed-ps liveness check: $out"
printf '%s\n' "$out" | grep -qi 'another run in progress' ||
  fail "a live owner with a failing ps lookup was stolen (must treat unprovable death as ALIVE): $out"
[[ -d $LOCKDIR ]] || fail "a live owner's lock was removed under a failing ps lookup"
[[ "$(cat "$LOCKDIR/owner" 2>/dev/null)" == "$live_pid2"* ]] ||
  fail "a live owner's token was clobbered under a failing ps lookup"
kill "$live_pid2" 2>/dev/null || true
wait "$live_pid2" 2>/dev/null || true
rm -rf "$LOCKDIR"

# ── 6) two simultaneous reclaimers of a dead lock → EXACTLY one proceeds ─────
# Item 3(a): the reclaim must be a single-winner move-aside, never two runs both
# validating the same dead token and both proceeding. A blocking npx holds the
# winner's lock while the loser resolves.
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
while [[ ! -e "$tmp/go2" ]]; do sleep 0.02; done
echo stub
EOF
chmod +x "$stub/npx"
rm -f "$tmp/go2"
mkdir -p "$LOCKDIR"
printf '%s\t%s' "999999" "Sat Jan  1 00:00:00 2000" >"$LOCKDIR/owner" # dead owner
o1="$tmp/o1.log"
o2="$tmp/o2.log"
(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >"$o1" 2>&1) &
r1=$!
(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >"$o2" 2>&1) &
r2=$!
# Wait until one contender has parked on the blocking npx (it now owns the lock
# with a LIVE token) and the other has resolved. Bounded wait.
for _ in $(seq 1 200); do
  live_token="$(cat "$LOCKDIR/owner" 2>/dev/null || true)"
  live_owner_pid="${live_token%%$'\t'*}"
  [[ $live_owner_pid =~ ^[0-9]+$ ]] && kill -0 "$live_owner_pid" 2>/dev/null && break
  sleep 0.02
done
# Let the parked winner finish.
touch "$tmp/go2"
wait "$r1" 2>/dev/null || true
wait "$r2" 2>/dev/null || true
proceed_count=0
grep -qF '[update-skills] done' "$o1" && proceed_count=$((proceed_count + 1))
grep -qF '[update-skills] done' "$o2" && proceed_count=$((proceed_count + 1))
[[ $proceed_count -eq 1 ]] ||
  fail "two reclaimers of a dead lock: expected exactly one to proceed, got $proceed_count
--- o1 ---
$(cat "$o1")
--- o2 ---
$(cat "$o2")"
defer_count=0
grep -qi 'another run in progress' "$o1" && defer_count=$((defer_count + 1))
grep -qi 'another run in progress' "$o2" && defer_count=$((defer_count + 1))
[[ $defer_count -eq 1 ]] ||
  fail "two reclaimers: expected exactly one to defer with 'another run in progress', got $defer_count"
[[ ! -d $LOCKDIR ]] || fail "the winning reclaimer left its lock behind"

echo "update-skills-lock: OK"
