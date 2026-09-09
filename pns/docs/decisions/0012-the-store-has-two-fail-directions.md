# 0012: The store has two fail directions

Status: accepted, recorded after the fact. The rule was followed by the SQLite work in PR 11.3 and by
the recording decorator in PR 9.2; this record states it in one place, which it should have had before
the first SQLite code rather than after.

The store is one file at `~/.local/state/pns/pns.db`, reached through `rusqlite` with the bundled
feature so the build stays `--locked` and offline, in WAL mode behind a bounded busy timeout, with
versioned migrations and explicit transactions.

A single store serves two kinds of caller, and they fail in opposite directions.

## The delivery path is fail-open

A busy, locked, missing or corrupt store never blocks a delivery. The notification is the product; the
record of it is bookkeeping. When the store cannot be read or written on this path, the delivery
proceeds and the miss is recorded wherever recording is still possible: the daemon log, and `pns doctor`
where a reader will look for it.

This is the same rule the recording decorator states as fail-quiet. A sink that cannot write never turns
a `Delivered` into a `Failed`. The failure to record is a separate fact from the failure to deliver, and
conflating them would report a working destination as broken.

## A state mutation is fail-closed

An ownership claim or an acknowledgement write that fails is reported as a failure to the caller that
asked for it. It is never passed off as success.

These writes are what keep at-least-once delivery from becoming at-most-once. A lease that silently
failed to record leaves work owned by nobody and delivered twice, or acknowledged by nobody and
delivered forever. The caller can retry, escalate or refuse; it cannot do any of that if the store
answered `Ok` for a write that did not happen.

## Why the two are not reconciled into one rule

A single direction would be wrong for one of the two callers. Fail-closed on the delivery path turns a
locked database into a missed page, which is the outcome the engine exists to prevent. Fail-open on a
state mutation turns a lost write into a duplicate or a lost event, silently.

The split is therefore a property of the caller rather than of the store, and every new store method
belongs to one side of it before it is written. Related: [0010](0010-a-notification-never-fails-the-work-it-reports-on.md)
carries the same asymmetry one layer out, and [0013](0013-delivery-is-at-least-once-with-retained-outcomes.md)
depends on the fail-closed half holding.
