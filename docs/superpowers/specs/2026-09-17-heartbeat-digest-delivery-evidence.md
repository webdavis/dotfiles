# Heartbeat and digest delivery evidence, 2026-09-17

Ledger task 50a owes the acceptance that tasks 43 and 44 left open: the installed LaunchAgents invoke
the Rust `posture` binary, which proves what is wired and never that a page was delivered. This
document records the delivery evidence for the heartbeat and for the digest separately, from runs that
had already happened.

No job was triggered to produce it. `posture heartbeat`, `posture digest`, `posture poll`,
`posture funnel`, `posture watchdog` and `posture alert` were not run, and neither were `pns doctor`,
`pns nag`, `pns stale`, `pns pulse` or `pns recap agent`. Every fact below comes from state that was
already on disk: the pns delivery ledger at `~/.local/state/pns/pns.db`, the digest spool under
`~/.local/state/osquery-digest-spool/`, the job logs under `~/.local/log/osquery/`, the hermes gateway
log at `~/.hermes/logs/gateway.log`, and `launchctl print` for the two labels.

`~/.hermes/.env` and `~/.config/pns/config.toml` were not read. No channel identifier, secret, token or
webhook address appears here. Route names do. The gateway log's per-delivery correlation tokens are
truncated where they are quoted.

## What the acceptance asked for, and why two thirds of it cannot be satisfied any more

Tasks 43 and 44 were written while posture submitted its pages through pns. Their acceptance names
three artifacts per run: a silent line on the pns-keyed route, a silent desk banner, and a row in the
pns delivery ledger.

The deployed posture configuration selects `mode = "hermes"` in its `[notify]` table, so posture signs
each page with the key for the route that page is going to and posts it itself. Two consequences, both
by design rather than by fault:

- **No pns ledger row.** posture does not submit through pns at all, so no run after that configuration
  landed can appear in `ledger_events` or `ledger_legs`.
- **No banner on a delivered page.** `posture-adapters/src/hermes.rs` raises the local banner only when
  a post could not be delivered, under the titles `Posture page could not be delivered` and
  `Posture page copy could not be posted`. A silent desk banner per successful heartbeat no longer
  exists to observe.

So the acceptance is restated to what the current delivery path can actually evidence: a post that the
gateway accepted and handed to its destination on the untiered route, and, for the digest, a filled
spool claimed and kept rather than restored.

One more thing the exit code cannot carry. `posture-application/src/heartbeat.rs` discards the
submission result (`let _ = self.sink.submit(&Alert { .. })`), deliberately, so that a refusal does not
turn a daily observation into a retry loop or a security page. `last exit code = 0` on either label is
therefore not evidence of delivery, and `~/.local/log/osquery/heartbeat.log` (0 bytes, last written
2026-07-03) and `~/.local/log/osquery/digest.log` (0 bytes, last written 2026-06-21) print nothing
either way.

## The heartbeat

**What is scheduled.** `launchctl print gui/501/com.webdavis.osquery-heartbeat` reads
`program = /Users/stephen/.cargo/bin/posture` with `arguments = { .../posture, heartbeat }`, and one
calendar-interval event trigger with the descriptor `Minute => 0`, `Hour => 9`. A calendar-interval job
carries an event trigger rather than a countdown, so launchd prints no next-fire time for it; the next
fire is 09:00 local on the next day.

**When it last fired.** `runs = 3`, `last exit code = 0`, `job state = exited`. The counter was reset
when the repointed plist was bootstrapped, so the three runs are 2026-09-15, 2026-09-16 and 2026-09-17,
all at 09:00 local. The last fire was 2026-09-17 at 09:00.

**What it delivered, and to which route.** Three gateway deliveries, each within six seconds of its
fire, on route `posture-pages`:

| Fire (local)         | Gateway line                          | Message bytes |
| -------------------- | ------------------------------------- | ------------- |
| 2026-09-15 09:00:04  | `direct-deliver route=posture-pages`  | 389           |
| 2026-09-16 09:00:05  | `direct-deliver route=posture-pages`  | 390           |
| 2026-09-17 09:00:05  | `direct-deliver route=posture-pages`  | 291           |

Each line reads `target=discord`. The 2026-09-17 line carries `delivery=posture-f443e498...`, posture's
own request identifier, truncated here.

These three are separable from the recurring posture traffic on the same route rather than merely
adjacent to it. The 15-minute monitors post at five minutes past each quarter (`:05`, `:20`, `:35`,
`:50`) at a steady 286, 408 or 409 bytes; the heartbeat posts on the hour at a length none of them use.

**What the destination answered.** Delivered. The gateway handed all three to Discord. The route has
answered no 404 (HTTP, hypertext transfer protocol, not found) since this gateway started on 2026-09-15
at 03:58, and the ten `Invalid signature for route posture-pages` warnings in the same window fall at
05:10, 13:53, 14:21 and 17:57 on 09-15, at 19:07, 22:18 and 23:54 on 09-16, and at 01:08, 01:39 and
04:18 on 09-17. None of them is a heartbeat fire.

**Whether the banner leg delivered.** No banner fired, and none should have. The banner is the
failure path, and there was no failure to report.

**Verdict: MET.** Three scheduled fires, three accepted posts on the untiered route, no refusal.

One residual, named rather than glossed: nothing on this machine records the body posture sent. The
gateway logs a length and not a message, and posture in hermes mode keeps no delivery ledger of its own,
so the correlation above is by label, minute and length. It is tight, and it is not the message itself.

## The digest

**What is scheduled.** `launchctl print gui/501/com.webdavis.osquery-digest` reads the same program with
`arguments = { .../posture, digest }` and a calendar-interval trigger of `Minute => 0`, `Hour => 18`.

**When it last fired.** `runs = 2`, `last exit code = 0`, `job state = exited`: 2026-09-15 and
2026-09-16, both at 18:00 local. The last fire was 2026-09-16 at 18:00. Today's has not happened yet.
This supersedes the 2026-09-15 measurement recorded in task 50a, which found `runs = 0` and
`job state = uninitialized`.

**What it delivered, and to which route.** Two gateway deliveries on route `posture-pages`,
`target=discord`: 2026-09-15 at 18:00:05 at 1518 bytes, and 2026-09-16 at 18:00:05 at 1153 bytes. Both
are several times the length of any monitor post on that route, which is what a filled digest body
looks like and an empty one does not.

**The filled spool and the `.last` rotation.** `~/.local/state/osquery-digest-spool/` holds
`digest.ndjson.last`, 76 newline-delimited JSON (JavaScript object notation) rows spanning
2026-09-16T00:48:53Z to 2026-09-16T23:50:28Z, and a live `digest.ndjson` of 111 rows starting
2026-09-17T00:02:04Z. Every one of the 76 rows parses and carries the six expected keys (`timestamp`,
`detector`, `category`, `identity`, `action`, `summary`).

That pair is the digest's own delivery record, and it says Sent. `digest_spool.rs` renames a claimed
batch to the `.last` suffix on two paths only: an accepted submission, and a batch whose every line was
unreadable and is kept for forensics instead of sent. A submission that was not accepted takes the
other path and RESTORES the batch, folding its rows back into the live spool for the next run. The live
spool holds none of those 76 rows, and none of the 76 is unreadable, so the 2026-09-16 batch was
delivered and kept.

**What the destination answered.** Delivered, both days, on the same route and with no signature
refusal at either fire.

**Whether the banner leg delivered.** No banner fired, and none should have, for the same reason as the
heartbeat.

**Verdict: MET.** Two scheduled fires, two accepted posts, and the 2026-09-16 run evidenced end to end:
a filled 76-row spool claimed by rename, a 1153-byte body accepted on `posture-pages`, and the batch
kept as `.last` rather than restored.

The same residual applies: the delivered body is not recorded, and the 2026-09-15 rotation cannot be
re-examined because the 2026-09-16 run overwrote `.last`.

## The eight dead-lettered legs, and what has changed since

The pns delivery ledger holds exactly eight events with `producer = 'posture'`, four heartbeats and four
digests, from the pns-producer era. Every one has two legs: a `macos-banner` leg that delivered, and a
`hermes` leg that dead-lettered on the retired bare `posture` route with `http_status = 404` and
`deadletter_reason = 'permanent'`.

```
title                                            fired (local)      banner   hermes
✅ osquery pipeline healthy · 2026-09-10         2026-09-10 09:00   ok       404 permanent
🗒️ osquery daily digest · 2026-09-11 · 129       2026-09-10 18:00   ok       404 permanent
✅ osquery pipeline healthy · 2026-09-11         2026-09-11 09:00   ok       404 permanent
🗒️ osquery daily digest · 2026-09-12 · 84        2026-09-11 18:00   ok       404 permanent
✅ osquery pipeline healthy · 2026-09-12         2026-09-12 09:00   ok       404 permanent
🗒️ osquery daily digest · 2026-09-13 · 26        2026-09-12 18:00   ok       404 permanent
✅ osquery pipeline healthy · 2026-09-13         2026-09-13 09:00   ok       404 permanent
🗒️ osquery daily digest · 2026-09-14 · 67        2026-09-13 18:00   ok       404 permanent
```

The title dates run one day ahead of the local fire for the digests because the title is stamped in
coordinated universal time and 18:00 local is midnight UTC. Three of the eight banner legs took two
attempts, the first recorded as `banner FAILED (terminal-notifier did not run)` and the second as
`posted the banner`.

**Nothing has changed since task 99 retired that name.** The count is still eight, and the ledger holds
no `producer = 'posture'` row after 2026-09-13 18:00 local. That is not the 404 having been fixed in
place: posture stopped submitting through pns, so the ledger has no opinion on its runs any more. The
only `posture-pages` and `priority` legs in the ledger belong to pns's own `apply-check` route probe,
producer `pns`, and both delivered.

## What is still not proven

- The body of any heartbeat or digest message. Nothing local records it.
- The 2026-09-17 digest, which fires at 18:00 local, after this document.
- Both verdicts rest on a correlation by label, minute and message length, plus the spool's own
  claim-and-keep record for the digest.

## Retired helpers still deployed

`~/.local/libexec/osquery/heartbeat.sh` (7,344 bytes, 2026-08-05) is still on disk and is the trash step
task 43 left to the operator. `digest.sh` is already absent, as task 44 recorded. `allowlist.sh` and
`enrich-finding.sh` are also still deployed and belong to the steps 4.1 and 4.2 reconciliation, not to
either cutover here. Chezmoi never deletes a target, which is why a deleted source outlives its own
change, and this repository builds no removal mechanism, so the operator trashes each by hand.

## What the operator would run to close the residuals

1. Read `#posture-pages` in Discord for the three heartbeat lines at 09:00 on 2026-09-15, 09-16 and
   09-17 and the two digest lines at 18:00 on 09-15 and 09-16. That is the only place a delivered body
   can still be read, and it is what turns a length into a message.
1. After 18:00 local today, confirm the third digest without triggering it:
   `launchctl print gui/$(id -u)/com.webdavis.osquery-digest | grep -E "runs|last exit code"` should read
   `runs = 3`, then `grep "18:00" ~/.hermes/logs/gateway.log | grep posture-pages` should show one
   `direct-deliver route=posture-pages target=discord` line with a body of roughly a thousand bytes, and
   `ls -l ~/.local/state/osquery-digest-spool/` should show `digest.ndjson.last` restamped with today's
   rows and a fresh `digest.ndjson`.
1. Trash the deployed `~/.local/libexec/osquery/heartbeat.sh`.
