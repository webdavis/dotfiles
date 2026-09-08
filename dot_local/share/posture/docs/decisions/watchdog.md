# Watchdog policy boundaries

The audit judges manifest lines and per-path readings. It reuses `KnownGoodTuple` for the existing
four-column grammar and `CanaryEpoch` for liveness. Its scan receives one start time and derives a shared
deadline from the configured seconds. A caller cannot accidentally make that configured bound inert by
supplying an unrelated deadline. The filesystem adapter must enforce the same deadline before performing
each bounded read; evaluating already collected readings does not bound those reads.

Bash streams findings as it scans. A later malformed entry in either manifest leaves earlier lines on
stdout before the refusal token. Captures preserve that output, and the domain's report preserves its
order. The watchdog classifies the mixed text as unknown before applying its usual two-tick confirmation.
Buffering away the findings would change both the fingerprint and the diagnosis.

The old crash-loop comment says the runs counter must advance, but the branch compares equality. Captured
counter resets therefore count as new runs. The domain keeps that behavior. It retains only the validated
numeric prefix of an exit field and selects agent labels from the fixed roster. Counter arithmetic cannot
wrap a failing streak into a healthy result; admission and serialization of old numeric state fields
remain the state adapter's responsibility.

The audit fingerprint covers which paths and columns disagree, not the changed file bytes. Retampering an
already reported path in the same way therefore does not page again. Sorting keeps report order out of
the identity while retaining duplicate columns. The hash operation belongs to the adapter. The
page-rendering branch receives the same report but selects only fixed labels; affected paths are never
rendered. The proposed paged marker is not proof of delivery: the use case may publish it only after
durable acceptance.

This row introduces pure policy under the existing domain crate. It adds no dependency, protocol, process
adapter or producer configuration. The Bash entrypoints remain deployed until the planned watchdog
cutover supplies real probes, state transactions and the independent alarm path. The heartbeat successor
is the source predecessor; its pending transport is not wired by this row.
