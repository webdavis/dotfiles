# Heartbeat observation

Given a readable daemon snapshot log, when the heartbeat runs, then it reads the system clock first,
selects the last emitted `heartbeat_canary` timestamp and submits one observation. The domain accepts
only decimal epochs from 0 through 9999999999, with no leading zero except `0`. Freshness includes both
ends of the configured window, 1800 seconds by default. A small future skew renders an age of exactly
zero. Missing, stale and implausibly future timestamps remain distinct messages.

Given a failed clock read, when the heartbeat runs, then it submits the time-unknown message without
reading snapshots. Given an unreadable log, it reports a missing canary. Given any submission refusal, it
returns without retrying or advancing state. Healthy and unhealthy daily messages are observations;
neither carries security attention. The original title and detail bytes describe a recent observation and
assign current liveness checking to the watchdog.

The snapshots adapter reads one size-bounded file window in buffered chunks. It refuses a non-regular
file without waiting for a writer. It skips torn or non-JSON lines, selects by parsed name, prefers
`unixTime`, and falls back to `snapshot[0].unix_time` for missing, null or false envelope values. It
validates after choosing the last emitted line. An invalid last value masks an older valid value; a later
null or torn row emits nothing and leaves the preceding value available.

The native clock reads epoch seconds and formats their Coordinated Universal Time (UTC) date through
`gmtime_r`. A pre-epoch or unrepresentable time fails as unknown. The application receives a numeric
maximum age; command-line environment parsing remains with the later composition owner.

These functions and ports prepare the heartbeat cutover. Pns provides `submit --json`, committed-ledger
acknowledgement and delivery-class policy. The posture observation route is frozen locally but remains
unmerged, and the concrete producer is not wired here. Only durable acknowledgement may implement
`AlertSink` acceptance. The deployed Bash entry point remains until the route preserves the silent daily
Discord record and desktop banner and the producer satisfies that contract.
