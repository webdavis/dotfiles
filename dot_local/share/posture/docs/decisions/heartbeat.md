# Heartbeat boundaries

`canary` owns epoch admission and the two-sided freshness decision. `heartbeat_text` owns the existing
message vocabulary. `Heartbeat` orders the consumer-owned `Clock`, `SnapshotsLog` and `AlertSink` ports.
The native clock and snapshots adapters depend inward; no process or serialization dependency enters the
domain or application.

The snapshots reader reuses the allowlist's measured private projection input and decimal formatter. They
move into `legacy_json` because two real readers now need them. Serde still validates grammar and borrows
raw values; the input copy retains measured numeric and surrogate acceptance. The move preserves the
formatter and all 55 tests byte for byte. The allowlist keeps its original publication bytes and its
private path/program renderer. This work adds no public codec or digest grouping policy.

Bash applies `tail -1` before epoch validation. Captures showed that an invalid final timestamp masks a
preceding valid one, null values can leave that preceding value selected, and a timestamp string
containing a newline contributes only its last output line. The adapter preserves those details,
including command substitution's NUL discard. An inert low surrogate is accepted, an unmatched high
surrogate refuses the row, and a numeric exponent keeps its displayed spelling before admission.

The file adapter uses a nonblocking open followed by descriptor metadata validation, so a raced named
pipe cannot hang the observation. It reads the descriptor's measured byte window; concurrent appends
belong to the next run. The native clock supplies seconds and date from one reading, so a midnight
transition cannot put two dates into the same observation. Tests inject clock values and use only owned
files, including the named-pipe refusal fixture.

The shared-delivery and caller portions remain dependent on the posture route and concrete producer. Pns
now supplies `submit --json` and the `ledger_committed` diagnostic alongside accepted status and the
matching request identity. Accepted status alone is insufficient. The observation route is frozen locally
but remains unmerged. The existing heartbeat script, LaunchAgent, cutover-gate invocation and watchdog
helper remain in service until that route and the producer are available. The independent last-resort
banner, occurrence identity and request class are still required parts of that cutover.
