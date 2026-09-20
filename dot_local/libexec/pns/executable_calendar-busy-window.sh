#!/usr/bin/env bash
#
# calendar-busy-window.sh, the reference implementer of pns's `[quiet.calendar]`
# `command`. Run read-only on the poll interval, handed no input, it lists the
# primary calendar's events (whichever account gog's own config already
# selects; this script names no account and no calendar) for the next hour and
# answers on stdout with:
#
#   {"events": [{"start": <epoch>, "end": <epoch>, "busy": <true|false>}]}
#
# `start`/`end` come from each event's RFC 3339 `dateTime` (offset preserved,
# not just "Z"), converted to epoch seconds by jq alone: `fromdate` only
# accepts a trailing "Z", so the offset is captured, the wall-clock digits are
# parsed as if they were UTC via `strptime`/`mktime`, and the real offset is
# subtracted back out. An all-day event (`start.date`, no `dateTime`) is
# skipped, since a day marked off is not an hour the operator is in a meeting.
# `busy` is true unless `transparency` is "transparent" or `status` is
# "cancelled". Nothing else about an event (summary, attendees, id) reaches
# stdout.
#
# On any failure this prints one line to stderr naming the step, never the
# event data, and exits non-zero; pns then leaves the mute as it was.

set -euo pipefail

# The daemon's own PATH already carries these (see
# Library/LaunchAgents/com.webdavis.pns-daemon.plist.tmpl), appended rather
# than prepended so a test's stubbed PATH entry still wins.
PATH="$PATH:$HOME/.local/bin:/opt/homebrew/bin"

fail() {
  printf 'calendar-busy-window: %s\n' "$1" >&2
  exit 1
}

command -v gog >/dev/null 2>&1 || fail "gog is not on PATH"
command -v jq >/dev/null 2>&1 || fail "jq is not on PATH"

from="$(date -u +%Y-%m-%dT%H:%M:%SZ)" || fail "could not read the clock"
to="$(date -u -v+1H +%Y-%m-%dT%H:%M:%SZ)" || fail "could not read the clock"

raw=""
if ! raw="$(gog calendar events --json --readonly --results-only --from "$from" --to "$to" --max 50)"; then
  fail "gog calendar events exited non-zero"
fi

# jq's own stderr is discarded: a parse error on bad input echoes the input
# back, which is exactly the event data this script must never quote.
#
# capture/strptime/mktime rather than fromdate: fromdate demands a literal
# "Z" offset and every non-UTC event would fail it.
document="$(jq -c '
  def offset_seconds:
    if . == "Z" or . == "" then 0
    else ((.[1:3] | tonumber) * 3600 + (.[4:6] | tonumber) * 60)
         * (if .[0:1] == "-" then -1 else 1 end)
    end;
  def to_epoch:
    capture("^(?<dt>[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2})(?<off>Z|[+-][0-9]{2}:[0-9]{2})$")
    | ((.dt + "Z") | strptime("%Y-%m-%dT%H:%M:%SZ") | mktime) - (.off | offset_seconds);
  {
    events: (
      [ .[]
        | select(.start.dateTime != null and .end.dateTime != null)
        | {
            start: (.start.dateTime | to_epoch),
            end: (.end.dateTime | to_epoch),
            busy: ((.transparency != "transparent") and (.status != "cancelled"))
          }
      ] | sort_by(.start)
    )
  }' <<<"$raw" 2>/dev/null)" || fail "gog answered something jq could not read as calendar events"

printf '%s\n' "$document"
