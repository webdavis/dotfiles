# 0014: First-run import keeps legacy bytes recoverable

Status: the internal-record consumers use the transactional repositories for plan 12.1. Plan 11.3
introduces storage, and plan 11.4 adds the delivery ledger. The deployment integration must establish the
quiescence described below before the new consumers run.

## What moves

The five internal rings, return-window edge, quiet expiry, home-staleness episode and lamp records live
in the same database. Their existing codecs and retention remain authoritative. Decisions keep five
lines, missed notifications 25, activity 150, policy-settings audit 20 and presence decisions five. The
lamp records include held fixtures and the working streak because their only consumers are inside pns.
File paths observed by another program, coordination files, daemon heartbeat and configuration remain
files.

Import preserves the original readable decision and journal text that doctor consumes. The replay and
activity readers still decode entries with the existing codec. An unrecognized journal line counts where
doctor counts it but cannot become a fabricated replay event.

## The transaction and recovery boundary

The first import records its rows and per-family completion together in one transaction. An interrupted
transaction leaves neither imported rows nor a completed-import marker. Retrying it cannot duplicate
rows. A completed import never reapplies an old file over newer database state.

Readable files are imported in their existing order, subject to the same retention. An absent file is an
ordinary absent history. An existing empty ring remains a present, empty history. An unreadable file is
recorded as an import failure, while other readable families can still migrate. The old files remain
untouched, including files that could not be read. The import API returns named failures without event
content and retains them for the later diagnostic command. It writes a bounded diagnostic to the existing
daemon log where possible. A successful semantic replacement clears only that family's failure; it never
edits the original file.

A failed import of the return edge costs at most the first recap window. Failed ring imports lose only
the bounded history listed above from the new store's view; the original bytes remain available for
recovery. An unreadable quiet expiry keeps the existing notification fail-open direction. Unreadable lamp
state keeps its existing fail-dark direction. The failed-import record must retain that distinction
instead of treating every unreadable input as an ordinary absence.

## Ownership at cutover

Import must let a live legacy journal or return-window owner finish. Abandoned readable holds transfer
under the same import transaction as their completion record. A completed replay attempt still consumes
its owned batch even when delivery failed; an interrupted attempt remains recoverable. The delivery
ledger owns any later retry or acknowledgement policy.

An old daemon or hook invocation can write files after a snapshot. Every old invocation must finish
before import becomes authoritative, including the daemon that will then start with the new binary. A
daemon restart alone does not establish that quiescence. Root integration owns the startup and rollout
boundary; this adapter does not control a running daemon.

Import refuses while a legacy ring lock is at or inside its five-second stale boundary, or when its clock
is unknown or in the future. The existing return-window predicate and journal owner check decide which
claims are abandoned. Readable abandoned batches retain their original order outside pending ring
retention, and transfer in the same transaction as import completion. No legacy ownership file is
removed. Import does not watch later file changes or start a second file writer.

## Consumer access and diagnostics

`SqliteStore::for_records` constructs an inert repository. Its first access commits the legacy import
before returning records or accepting a mutation. A failed transaction cannot masquerade as an empty
successful store. Completed import checks use reads, so an unrelated writer does not prevent ordinary
history reads. There is no file fallback and no second authoritative writer. `SqliteStore::new` is
reserved for ledger-only composition, whose records have no legacy source.

Doctor may create the database and import retained history on its first read. It still records no
decision, activity or missed event of its own and claims no return batch. Original files remain intact.
After history, it names retained import failures or an unavailable import check without changing the
existing delivery-health exit code. Malformed quiet and lamp records retain their existing diagnostic
text and failure directions. A failed direct news, return-edge or audit write reports a bounded,
non-secret daemon-log entry while the event caller continues.

## One decision per logical event

Schema version 3 adds an optional original producer/request pair to the existing decisions table.
Imported rows keep null identity fields and their original text. A unique pair identifies a retained
logical event. `begin` may record `legs=none` before dispatch, while the delivery ledger retains each
planned leg as Unknown. A completion appends its registered destination and printable verdict; a later
attempt replaces that destination's verdict in place. The codec preserves the original policy and hook
facts, other legs and encounter order. Both reading and updating the line happen inside one transaction,
so concurrent independent completions cannot lose each other.

Repeating an initial record keeps its current outcome. A late retry returns false for an absent or pruned
decision and cannot recreate it. A malformed or duplicate outcome field, or a destination name containing
a field delimiter, is refused without writing. The ledger's acknowledgement rules remain separate: a
printable Silent verdict never establishes delivery. No additional policy snapshot is stored.
