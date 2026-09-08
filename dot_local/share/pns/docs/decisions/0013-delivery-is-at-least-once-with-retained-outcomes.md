# 0013: Delivery is at-least-once, with retained outcomes

Status: accepted. The storage contract is implemented; dispatch and daemon composition belong to the
submission and delivery use cases.

A logical submission is identified by its original producer and request id, not by hashing its text. Two
events with the same body can both be legitimate. A retry preserves that identity, the rendered event and
the destination instance and named route originally chosen. Reusing an identity with a conflicting body
or route is a refusal, never a replacement of the recorded submission.

The event and every planned leg are recorded in one transaction before dispatch. Planned means that an
outcome is still unknown. A destination's printable silence, successful spawn or configured plan cannot
become an acknowledgement. The transport's actual acknowledgement is recorded separately from whether
that recording succeeded. Delivery-side storage failure remains fail-open and is reported through the
existing non-secret daemon diagnostic when possible.

Each initial leg belongs to the active writer. The retry scan leases only unacknowledged work whose owner
is gone or whose lease has expired. An owned generation token prevents an old worker from settling a
later lease. The caller chooses the finite lease and next retry instant; the store performs no inline
retry. Pending rows are the queue, so the daemon does not need a second job record describing the same
delivery.

An acknowledged leg stays in the ledger as audit history and never returns to the retry queue.
Unconfirmed outcomes retain their failed, unlaunched or unknown distinction. Every leased generation
begins with an unknown outcome. Completion settles only that generation, and older attempts remain
visible with their generation and recording time. A process killed after sending but before recording an
acknowledgement leaves a recoverable pending leg. That choice prevents a silent loss, but it can send the
event twice.

Delivery is therefore at-least-once. Only a destination that durably enforces the original identity can
deduplicate it. A gateway acknowledging a request does not prove the operator saw it. Ordering is
best-effort: a later inline delivery can arrive before a failed earlier leg's retry. The ledger retains
monotonic event sequence and per-leg generations so that the recorded order remains inspectable.

The store uses the version 1 database's explicit transactions, private files and bounded busy timeout.
The ledger adds schema version 2 without changing the existing record tables or their import policy.
Invalid plans, conflicting submissions and invalid lease windows fail before mutation; failed
transactions leave no partial plan, attempt or claim. The dispatcher and daemon remain responsible for
the actual send, outcome mapping, diagnostics and scheduling.

An identical duplicate receives history without dispatch claims, even while the first submission is still
in flight. Unknown history does not mean the owner is dead. Retry eligibility is determined by the active
lease or by a completed attempt's due time. Only a created submission receives initial dispatch claims.
The retry method returns one claimed leg with its original payload and routing facts.

Completing a claim commits its ledger outcome and any retained decision-leg revision together. The claim
supplies the original identity and destination. A pruned decision stays pruned; a malformed retained
decision or failed update rolls back completion and preserves ownership. Silent remains unconfirmed.
Standalone decision revision is available for delivery that could not be queued.
