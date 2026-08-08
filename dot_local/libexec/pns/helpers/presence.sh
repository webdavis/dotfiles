# shellcheck shell=bash
# pns presence PROBES: the readings that say where the operator is, deliberately
# not in event.sh, whose whole point is that everything in it is a total
# function of its arguments. The top-level verdict still lives there
# (pns_wants_phone); the readings are here.
#
# THE SPLIT INSIDE THIS FILE MATTERS AS MUCH AS THE ONE BETWEEN THE FILES.
# Every decision below is a total function of its arguments or its stdin
# (pns_attention_band, pns_mosh_rate_active), and the IO sits at the edge in
# the two functions that do the reading (pns_idle_secs, pns_phone_attention).
# That is what lets bats pin the interesting half with fixture bytes instead of
# stubbing a second of live network sampling.
#
# ONE DEFINITION, TWO CALLERS: relay.sh, deciding which channels fire, and
# hooks/moshi-gate.sh, deciding whether a phone round trip fires at all. A
# second copy of these probes is how the two would drift into disagreeing about
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

# pns_attention_band <idle_secs> <desk_idle_secs>
# PURE, and the whole three-tier arbitration in one predicate: 0 when the idle
# reading sits in the ONE band where a phone signal may overrule the Mac.
#
#   idle < PNS_PHYSICAL_FRESH_SECS   the operator just touched this Mac, so
#                                    hands are here. A phone streaming Moshi
#                                    only proves the app is on a screen
#                                    somewhere; a keypress proves where the
#                                    person is (the switching drill's r13 and
#                                    r25 calls, operator-confirmed).
#   fresh <= idle < desk             the band this returns 0 for. The Mac reads
#                                    "at the desk" while the operator may be
#                                    standing in the hallway watching through
#                                    Moshi, so a phone signal decides.
#   idle >= desk                     away, and the ordinary rule already sends
#                                    the push, so there is nothing to overrule.
#
# BOTH CONSUMERS CALL THIS. relay.sh gates its phone channel with it and
# hooks/moshi-gate.sh gates the approval round trip with it; a second copy of
# these bounds is how the two would drift into disagreeing about where the
# operator is. It is also what confines the probes below to the one band where
# they can change an answer, which is what keeps their cost off every other
# notification. An unreadable idle is not in any band: unknown presence already
# fails open into a push, so there is nothing left to overrule.
pns_attention_band() {
  local idle="${1:-}" desk="${2:-}" fresh="${PNS_PHYSICAL_FRESH_SECS:-20}"
  [[ $idle =~ ^[0-9]+$ && $desk =~ ^[0-9]+$ && $fresh =~ ^[0-9]+$ ]] || return 1
  ((idle >= fresh && idle < desk))
}

# pns_mosh_rate_active
# PURE apart from its stdin: a two-sample `nettop -L 2` CSV in, the verdict as
# the exit code out. 0 when any mosh-server session's bytes_in grew by more
# than ${PNS_ATTENTION_FLOOR_BYTES:-100} between the first sample and the last.
#
# BYTES IN, not bytes out. Traffic the CLIENT sent is what proves the phone's
# Moshi app is foregrounded and being read; output alone is this machine
# talking into a session nobody has on screen.
#
# The floor is there because an attached-but-pocketed session still trickles
# keepalives. The drills put a viewed session thousands of bytes clear of a
# pocketed one within a single second, so the separation is not delicate.
#
# Parsing stays here, apart from the probe that feeds it, because this is the
# half worth pinning: bats hands it fixture CSV and never runs nettop. Rows
# that are not a mosh-server line (the repeated header, a truncated sample,
# anything at all) simply match nothing, so empty and garbage read INACTIVE
# rather than crashing.
pns_mosh_rate_active() {
  local floor="${PNS_ATTENTION_FLOOR_BYTES:-100}"
  [[ $floor =~ ^[0-9]+$ ]] || floor=100
  awk -F, -v floor="$floor" '
    $2 ~ /^mosh-server\./ && $5 ~ /^[0-9]+$/ {
      if (!($2 in first)) first[$2] = $5
      last[$2] = $5
    }
    END {
      for (session in first)
        if (last[session] - first[session] > floor) exit 0
      exit 1
    }'
}

# pns_phone_marker_fresh
# 0 when the phone-attention marker was touched within
# ${PNS_PHONE_MARKER_TTL:-300} seconds.
#
# THE DELIBERATE SIGNAL, and the one case no probe can see: Moshi in the
# background on a phone the operator is holding. A double tap on the phone's
# back runs one forced `touch` over SSH against a key that can do nothing else,
# which is the operator saying "I am on my phone" in as many words.
#
# Five minutes, because a tap means "the next few minutes" and is refreshed by
# tapping again. A longer window would resurrect the mid-reading buzzing the
# 120-second idle ruling was made to stop.
#
# This one fails CLOSED, unlike the idle probe: an absent marker and an
# unreadable one both mean the operator said nothing, and inventing a signal
# out of a failed stat would push to a phone in a pocket.
pns_phone_marker_fresh() {
  local marker="${PNS_PHONE_MARKER_FILE:-${HOME:-}/.local/state/pns/phone-attention.marker}"
  local ttl="${PNS_PHONE_MARKER_TTL:-300}" mtime now
  mtime="$(stat -f %m "$marker" 2>/dev/null || true)"
  now="$(date +%s)"
  [[ $mtime =~ ^[0-9]+$ && $now =~ ^[0-9]+$ && $ttl =~ ^[0-9]+$ ]] || return 1
  ((now - mtime < ttl))
}

# pns_phone_attention
# 0 when the operator is demonstrably on their phone. The IMPURE edge: it does
# the reading and hands the deciding to the two functions above.
#
# Three sources, cheapest first, and every one of them optional:
#   1. RELAY_PHONE_ATTENTION forces the verdict outright (1 yes, 0 no). Tests
#      and manual runs live here; the real probe must never run in a test,
#      because it samples the operator's live counters for a full second.
#   2. the Back Tap marker, a stat of one file.
#   3. mosh byte rate, the one-second nettop sample, run only when the first
#      two said nothing.
#
# ANY MISSING PREREQUISITE IS "NO SIGNAL", not an error: no pgrep, no nettop,
# no mosh sessions and flat counters all return 1 and leave the plain idle rule
# standing, which is the behavior this machine had before the override existed.
pns_phone_attention() {
  case "${RELAY_PHONE_ATTENTION:-}" in
    1) return 0 ;;
    0) return 1 ;;
  esac
  pns_phone_marker_fresh && return 0
  command -v pgrep >/dev/null 2>&1 || return 1
  command -v nettop >/dev/null 2>&1 || return 1
  local pid
  local -a pid_args=()
  while IFS= read -r pid; do
    [[ $pid =~ ^[0-9]+$ ]] && pid_args+=(-p "$pid")
  done < <(pgrep -x mosh-server 2>/dev/null || true)
  [[ ${#pid_args[@]} -gt 0 ]] || return 1
  # -P collapses to one row per process, -x prints raw byte counts rather than
  # MiB, -n skips address resolution, and -L 2 is what makes this two samples a
  # second apart in CSV. The -J column list is chosen so bytes_in lands in
  # field 5, which is the shape pns_mosh_rate_active parses.
  nettop -P -L 2 -x -n -J time,interface,state,bytes_in,bytes_out "${pid_args[@]}" 2>/dev/null |
    pns_mosh_rate_active
}
