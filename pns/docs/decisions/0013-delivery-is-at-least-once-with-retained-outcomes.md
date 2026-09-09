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
later lease. The caller chooses the finite lease and configured retry policy. The store computes each
completed retry's due time from its owned generation and performs no inline retry. Pending rows are the
queue, so the daemon does not need a second job record describing the same delivery.

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

Return summaries use the same ledger and submission path. The return claim stores one request key and its
original window before the summary is composed. An adopted claim keeps those values and its original
journal rows; later arrivals remain a separate batch. An empty journal can still own a digest card. Once
the ledger contains that replay, adoption completes the journal handoff without composing or publishing
another summary. A refused or unpersisted submission leaves the claimed journal recoverable. The shared
delivery runtime supplies the same finite lease and retry policy used by original events. The aggregate
keeps its legacy default route and creates no additional logical decision record.

Schema version 4 adds nullable original producer/request keys to journal rows and the saved replay key
and window to return claims. Legacy journal rows remain readable without an identity. An acknowledged
decorative leg removes only its matching original miss in the existing completion transaction. A failed
removal rolls back the ledger completion too. A late event tail checks those retained acknowledgements
before appending, so it cannot recreate an already delivered miss. Durable-log acknowledgement alone,
failed delivery and printable silence do not clear the missed event. This linkage complements the
outcome-based missed predicate owned by the original submission tail.

Schema version 5 retains the original producer request as one nullable encoded value on its existing
ledger event. The protocol boundary owns encoding and bounds; the application has no JSON dependency.
Including that value in submission equality prevents changed source metadata from reusing an identity
merely because its rendered notification is the same. Older rows and aggregate returns have no such
request and stay null. Retry claims leave the value intact and use the stored rendering and route.

The optional request class is part of that canonical producer value. Its configured exception applies
only when planning the original banner and phone delivery. Existing mute and Focus inputs remain true, so
pulse and unmarked return summaries stay quiet. Missing class metadata preserves the prior canonical
bytes, and retry follows the stored legs rather than applying a later class configuration. The renderer
writes the default security class explicitly, making the exception visible to the operator.

Schema version 7 retains a terminal HTTP status on the affected leg and attempt. That completion, the
dead-letter marker and its pending local alarm commit together. The record, original request metadata,
route and earlier attempts remain inspectable; terminal failure never acknowledges delivery or clears a
missed event. The initial send remains pending for its first daemon retry.

Schema version 8 widens what that record can hold and moves the decision behind one rule. The terminal
set was four statuses named in the Hermes channel; it is now the permanent class in `pns_domain::retry`,
so a 404 on a route the gateway does not serve stops at its first queued retry rather than spending
twenty attempts over seven days on the schedule meant for a gateway that is down. A channel reports the
status it received and judges nothing, which is what keeps a second destination from disagreeing with the
first about the same code. The dead-letter reason gains `permanent` beside `attempts` and `age`, so a
report can say the gateway refused this rather than that it ran out of tries.

The attempt's status column widens to any real HTTP status and now records what THAT attempt received,
whether or not the leg was given up on: a 503 that will be tried again used to leave its code nowhere.
The leg's own status keeps its narrower meaning, the status the leg died of. A stored status therefore no
longer implies a terminal outcome, and the three facts the write path used, a failed outcome, a permanent
class, and a generation past the initial send, are what reconstruct one on the way back out.

The two migration steps that wrote the narrow constraints now write the wide ones directly, and step 8
copies a column only for a database that already holds a narrow one. A fresh database does no copying at
all, which matters because creating one is what every test and every new machine does.

Queued failures use the completion time plus `retry_base_secs` times the retry count, and nothing else.
The base defaults to 60 seconds. There is no random offset: the jitter this record originally described
was removed on 2026-09-09, because random spread exists to stop many clients retrying in one instant and
this is a single local daemon draining one queue against a loopback gateway. The retired
`retry_random_secs` key is refused by name rather than ignored, so an operator who still has it learns it
is gone. Initial sends consume no retry count or delay, while interrupted retry claims still consume a
generation. Arithmetic saturates at the unsigned timestamp ceiling. Lease expiry and retry delay are
separate policies.

The same change added the permanent-versus-temporary split in `pns_domain::retry`. A refused request
dead-letters on its FIRST failure with `DeadletterReason::Permanent`, rather than consuming the twenty
attempts a recoverable one is allowed. The rule is one function over a `DeliveryOutcome`, so every
destination classifies identically and only the operator-facing wording varies.
