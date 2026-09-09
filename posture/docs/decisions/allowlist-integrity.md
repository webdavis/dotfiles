# Preserve the two manifest consumers and the allowlist boundary

Source policy was read at main b34374a8d3cc10b2e28e176cd365ff0e0c512bd8. The package dependency is the
signed row 2.2 source at 2522d0b25c4f97dbe51003b05231585c955d3c2b. Both clean-code skills apply, with
clean-code-rust controlling Rust boundaries and size limits.

`allowlist` owns identity comparison, label validation and curation order. It receives decoded entries or
classified source lines. `allowlist_file` owns one-value-per-line JSON parsing and source reads through a
private projection. `publisher` owns exact serialization, source replacement and publication. An object
without a label survives curation. A line starting with # or an empty line is preserved; Bash currently
refuses indented comments and whitespace-only lines. The domain preserves a supplied raw line rather than
rewriting its bytes.

The consumer expands every ~/ token, including embedded tokens in program arguments. The writer
relativizes only a leading HOME in the plist path but every HOME occurrence in the program. These are
measured shell expressions, broader than the specification's shorthand about leading paths. The pinned
plist digest compares case-sensitively, while manifest tuple digests compare without case.

The final list-file vouch runs only after the identity and plist evidence agree. A miss or reused label
already pages and must not spend a hash, stat or settle budget to reach the same answer. An unpinned
plist requires its own vouch first. Signing promotion remains in the gate, so an allowed identity with an
untrusted program still pages.

`known_good` preserves two parsing responsibilities. Its audit line grammar is strict, including row
1.2's explicit unbuilt variant. The existing event consumer splits whitespace into four columns with the
path last. Its future adapter must preserve that accepted set rather than sending every line through the
strict audit parser. Tuple comparison receives those columns directly and rejects empty observed fields
and malformed digests. The managed-bin membership scan also remains separate from tuple validity: a
nonempty trusted file containing no parsed path is a membership miss, while missing, unreadable,
zero-byte or untrustworthy files track every bin neighbor.

Manifest selection precedes comparison. Pipeline and posture files use the pipeline manifest; other local
bin and libexec paths use the managed-bin manifest. The two lists cannot vouch for one another. Own
osquery plists are anchored under HOME/Library/LaunchAgents, and the page allowlist is one exact file.
These are decisions tested through path inputs, not declaration-parity tests.

Root ownership and protected mode raise the bar against user-level writes; they do not protect against an
attacker who can use passwordless sudo. This port retains that limitation and does not change privileged
publication.

`integrity` distinguishes an immediate rehash from the atomic-rename delay request. Its callback receives
that request only for a regular tracked file that was not deleted. The shared deployed state verdict
rejects unreadable, symlink and non-regular readings. File-kind checks before hashing and on each settle
retry remain mandatory adapter/application work; these pure functions cannot guarantee how their inputs
were read. The event digest selects timing only.

All old shell tests remain until cutover. The new domain composition tests join the allowlist verdict to
the existing gate and join tracking, current-state comparison and integrity. Replacing one complete
decision with a constant must change an asserted outcome. Row 2.3 introduced no process, wire format,
persistent store, registry, configuration field or external dependency.

## Bounded interactive publication

The writer cutover is behavior work over the unchanged curation policy. Inert captures of the old
functions established inherited pipe and terminal input for both apply and the manifest runner, separate
forwarded output/error streams, and no inherited lock descriptor. The plan requires bounded children. The
selected composition value is one fifteen-minute total publication budget, sufficient for ordinary
operator password prompts while retaining an explicit termination point. Tests inject short durations.
Publication uses an owned process group, terminal foreground transfer where needed, group termination and
reaping. It never retries an apply or manifest invocation.

The old apply-failure message asserted that nothing was deployed. A private partial-apply fixture proved
that source rollback cannot establish this claim. The new diagnostic reports restored source and possible
partial deployment. Missing original source retains the captured empty-file rollback behavior. Failed
manifest refresh cannot undo a successful apply, so it leaves source and deployed bytes in place and
keeps the existing stale-manifest warning.

Raw entry bytes are separate from their projected label. This preserves invalid UTF-8 bytes and inert
fields during an unrelated rewrite. The existing curation types accept a copied raw payload without
changing label policy, ordering or their seven existing test cases.

## Writer ownership and compatibility projection

`CurateAllowlist` consumes `LaunchdTable`, `SourceAllowlist`, `Publisher` and `WriteLock`. The existing
seven domain curation cases keep their policy. The raw-line carrier is generic so applications can
preserve byte slices, including invalid text, without putting filesystem or JSON types in the domain. The
command module composes native adapters and translates typed outcomes into the operator messages.

The one-object source reader and the query's path/program fields need a private projection because the
old jq reader accepts numeric spellings and surrogate input that a direct serde parse would change. Serde
remains the grammar and borrowed-value owner. A lexical input copy normalizes only those tokens, retains
offsets for the existing decimal formatter, and repairs isolated low surrogates. The original line is the
publication payload. An iterative renderer handles only selected path/program values; it preserves object
key order and the selected-row numeric round trip. This does not add a public codec or implement the
paused digest group-order decision.

`sha2` computes the pinned plist digest directly. The regular-file check follows a symlink to a regular
plist, as the old writer did; a nonblocking open and descriptor metadata check prevent a raced named pipe
from hanging hash capture. The query and source-resolution adapters start their ten-second budgets when
called, after the blocking write lock has been acquired. The fifteen-minute publication budget starts at
publication and is shared by every child it invokes.

The Bash writer and its orphan fixture retire together. The fixture supplied no runnable tests and used
live apply commands, so owned command doubles replace its setup. The seed header and two existing reader
comments name the new command. Allowlist data, the manifest runner, builder paths, notification protocol
and reader policy keep their existing owners. Both clean-code skills govern this batch; Rust controls the
size and verification mechanisms. Logical behavior changes are the command spelling, mandatory native
locking, bounded publication and the corrected partial-deployment diagnostic.
