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
`gmtime_r`. A pre-epoch or unrepresentable time fails as unknown.

Given `posture heartbeat`, when trailing operands are present, then the command ignores them. Production
paths are constructed once from `HOME`, as section 3.11 requires. Snapshots use
`$HOME/.local/log/osquery/osqueryd.snapshots.log`; the legacy `OSQUERY_SNAPSHOTS_LOG` path override is
ignored. Tests supply alternate paths through the existing configuration value. The command reads no
control state and writes no heartbeat baseline.

Given `OSQUERY_CANARY_MAX_AGE`, when it contains ASCII digits, then a leading zero selects octal and
other literals select decimal. Checked parsing admits the full unsigned 64-bit range and preserves valid
display text. Empty or nonnumeric values quietly use 1800. Invalid octal or overflow uses 1800 and emits
one fixed line: `posture heartbeat: invalid OSQUERY_CANARY_MAX_AGE literal; using 1800 seconds`.
Freshness still uses unsigned distances and the existing two-sided boundary.

The command submits through `$HOME/.local/libexec/pns/pns` on the proposed named route `posture`.
Heartbeat remains an observation without the security class. Submission has a five-second command budget;
an independent runner gives the local failure banner ten seconds. Correlated `ledger_committed`
acceptance is the only durable acknowledgement. Refusal, engine failure and alarm failure all leave the
command's best-effort exit status at zero. Missing `HOME` refuses setup with a fixed diagnostic and exit
1\.

The route name is a deployment prerequisite, not an installed binding. The operator must configure the
pns-keyed Hermes route to preserve the silent daily Discord record and desktop banner before the held
LaunchAgent change can land. The Bash entry point, its 17 tests and the shared canary helper remain
active until that separate caller and operator cutover.
