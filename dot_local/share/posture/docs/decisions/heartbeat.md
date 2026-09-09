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

`PnsProducer` sends one request through `pns submit --json`. The request carries the source event,
occurrence time, and title followed by detail on a new line. A caller-supplied occurrence seed gives the
same `posture-<32 hex>` identity on repeat submission. Without a seed, each call gets a fresh identity.
Only `NeedsAttention` gets the `security` class; heartbeat and digest observations retain their silent
policy input. The constructor accepts the route selected by deployment and supplies no route of its own.

Acceptance requires the original request identity, accepted status and `ledger_committed`. Destination
outcomes cannot substitute for that receipt. Pns returns exit 2 for a normal protocol refusal, so a
correlated rejected result remains a refusal rather than an opaque command failure. Its explicit
`submission_unavailable` diagnostic reports an engine submission failure. A degraded `ledger_unavailable`
result or a missing commitment leaves the caller's state unchanged without raising an engine alarm.

An unavailable, failed, timed-out or unparseable engine triggers one independent banner attempt. The same
`IndependentAlarm` port lets the watchdog report directly without submitting to pns. The banner uses
backslash-first AppleScript escaping and the approved fixed Sosumi sound. It reports command success, not
proof the operator saw the notification. Neither adapter stores or retries a request, and a forged
correlated committed receipt remains undetectable at this boundary.

The command runner writes stdin incrementally while draining stdout under its existing deadline. It
closes stdin after the request, retains total and per-command budget modes, and reaps its owned child
before returning. Banner composition supplies a separate bounded runner so an exhausted submission budget
cannot suppress the independent attempt. The existing heartbeat script, LaunchAgent, cutover-gate
invocation and watchdog helper remain in service until the separate caller and deployment cutover. This
adapter work does not install a route or retire the Bash queue.

The heartbeat command supplies the proposed fixed route name `posture` through the producer's existing
validated route argument. It does not add a protocol dependency to the command crate. This name requires
an operator-created, pns-keyed Hermes binding under specification section 5.5(5); no encrypted
configuration or live route was inspected or changed. The prepared plist changes only its two program
arguments, to `~/.local/libexec/posture/posture heartbeat`, and remains held for that prerequisite. The
existing Bash script and tests are retained, and `canary-freshness.sh` still serves the watchdog.

The five-second submission limit comes from the Bash remote delivery bound. The independent banner uses
the existing ten-second posture command bound, through a separate runner. This preserves its opportunity
to report an engine timeout. The command ignores extra operands and every submission outcome returns
zero; an absent `HOME` is a setup refusal before submission.

`HeartbeatWindow` separates validated seconds from their original display text. Captures exposed Bash's
octal interpretation and its signed arithmetic overflow. Valid literals retain their spelling, including
`020` for sixteen seconds. Invalid octal and values above `u64::MAX` use the existing 1800-second default
with one fixed diagnostic. Valid unsigned values above `i64::MAX` retain their actual bound. The command
does not reproduce negative skew or age caused by Bash overflow, because S200 and S203 require validated
arithmetic and positive unhealthy distances. The acceptance record identifies those changed outputs.

Section 3.11 retires the Bash path environment seams. Heartbeat constructs snapshot and engine paths from
`HOME` once and accepts alternate paths only through the test configuration value. The legacy override
captures remain evidence of this deliberate change, not a compatibility exception. The scalar
`OSQUERY_CANARY_MAX_AGE` setting remains because S200 explicitly names it. A conflicting legacy snapshot
override is supplied in the real command fixture and must not change the selected canary.
