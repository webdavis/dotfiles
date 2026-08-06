# shellcheck shell=bash
# pns presence PROBE: the IMPURE half of the presence gate, deliberately not in
# event.sh, whose whole point is that everything in it is a total function of
# its arguments. The VERDICT still lives there (pns_wants_phone); only the
# reading is here.
#
# ONE DEFINITION, TWO CALLERS: relay.sh, deciding which channels fire, and
# hooks/moshi-gate.sh, deciding whether a phone round trip fires at all. A
# second copy of this probe is how the two would drift into disagreeing about
# where the operator is sitting.

# pns_idle_secs
# Seconds since the last human input, or the EMPTY STRING when that cannot be
# read. Empty is the unknown verdict, and pns_wants_phone reads unknown as
# away: a garbled probe line must never coerce to 0, which reads as "actively
# typing" and silently drops the push. RELAY_IDLE_SECS overrides the probe
# (tests and manual runs); RELAY_IOREG points it at a stub. HIDIdleTime is
# input-idle, which is what works under the never-sleep power policy.
pns_idle_secs() {
  local idle_secs="${RELAY_IDLE_SECS:-}" idle_ns
  if [[ -z $idle_secs ]]; then
    idle_ns="$("${RELAY_IOREG:-/usr/sbin/ioreg}" -c IOHIDSystem 2>/dev/null | grep -m1 HIDIdleTime | awk '{print $NF}' || true)"
    [[ $idle_ns =~ ^[0-9]+$ ]] && idle_secs=$((idle_ns / 1000000000))
  fi
  printf '%s' "$idle_secs"
}
