# 0001: A file protocol owns a file by rename, never by removal

Status: accepted, and load bearing for every filesystem protocol in the crate.

## The measurement

`unlink` does not arbitrate between racing processes on this machine's filesystem, which is APFS (Apple
File System). It reports success to every caller. This was measured directly: eight racers each removed
the same path and all eight were told they had succeeded.

That is the opposite of the assumption a removal-based protocol rests on. A protocol that says "whoever
removes the file owns the work" gives the work to every racer at once.

## The rule

Ownership is taken by `rename`, or by creating a file with the exclusive-creation flag. Never by removing
one, and never by reading a file and then removing it.

Two consequences that follow, and that the code depends on:

1. **Rename first, read second.** A sweep that read an expired epoch and then unlinked could delete a
   FRESH file a racing process published in between, with both processes believing they had removed the
   old one. Taking the file by rename first means what a process removes is what that process took, and
   the value is read again off the claim so a file that turned out to be live is put back rather than
   destroyed.
1. **A claim carries the claimant.** The rename target embeds the claiming process id, so an abandoned
   claim can be told from a live one by asking whether that process still exists, rather than by a
   timeout alone.

## Where the rule is implemented

| Site                                        | What it owns                                                         |
| ------------------------------------------- | -------------------------------------------------------------------- |
| `src/nag.rs:claim_path`                     | One approval record, taken by a fire before it is read for anything  |
| `src/main.rs:claim_by_rename`, `take_claim` | The missed-notification journal                                      |
| `src/main.rs:claim_record`, `claim_fire`    | A nag record and the fire lock                                       |
| `src/main.rs:sweep_markers`                 | Expired wait and lease markers, taken before removal                 |
| `src/main.rs:claim_ring_lock`               | The ring append lock, taken by exclusive creation rather than rename |

## What the rule does NOT fix, stated so nobody re-derives it

One file per session carries no generation. In `src/main.rs:update_blocked_marker`, a blocked event that
publishes a new wait while a previous Stop is still condensing loses that wait when the Stop reaches its
removal. Telling the two apart would need a generation inside the marker and a compare-and-swap publish
over it. The damage is bounded by the configured backstop and closed by the session's next event, which
re-publishes the wait it is still in. This is an accepted limit, not an oversight.

## Consequence for the refactor

The persistence work replaces multi-record durable state with a transactional store. Where a filesystem
protocol survives that change, it survives because the path, name, mode or existence is itself the
interface to something outside pns. Any surviving protocol keeps this rule, and its race behavior is
tested rather than assumed.
