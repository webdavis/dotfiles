# Poll policy boundaries

Controls admission creates the values the poller can monitor. Invalid later records refuse the whole set.
`Control` keeps its admitted identifier, reader, declaration and sanitized text together; callers cannot
construct it by filling public fields. The existing severity type is reused. The existing
`ProtectionState` only distinguishes off from other, so the poller's validated trio preserves its
additional firewall state without changing that earlier contract.

A monitoring gap and an exposure can happen in the same tick. `PollPlan` therefore returns separate gap
and exposure pages, plus proposed baseline state. The application must submit the gap first and stop if
it cannot be stored, then submit the exposure and publish only after acceptance. A proposed baseline is
never an acknowledgement. With refused controls and a trusted prior, the preservation flag instructs the
state adapter to retain the prior object's other fields before replacing the trio. The domain does not
own those opaque serialized fields.

The orphan Bash poller harness supplies private probes and a page spy. Its introductory comment says the
page observes the new baseline, but the implementation and the new capture show the reverse: two pages
observe no baseline, and the write follows. The implementation is authoritative. These helper functions
do not count as existing test leaves. Their exact names remain in the delivery inventory, including the
process and publication helpers reserved for the later adapters.

The controls count is a typed array length, so there is no noninteger count string in this interface. A
wire error becomes `Malformed`; an admitted zero-length array still refuses. No new size ceiling, process
fallback or dependency is introduced. The producer configuration and operational cutovers remain separate
from this policy payload.
