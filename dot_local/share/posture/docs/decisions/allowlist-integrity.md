# Preserve the two manifest consumers and the allowlist boundary

Source policy was read at main b34374a8d3cc10b2e28e176cd365ff0e0c512bd8. The package dependency is the
signed row 2.2 source at 2522d0b25c4f97dbe51003b05231585c955d3c2b. Both clean-code skills apply, with
clean-code-rust controlling Rust boundaries and size limits.

`allowlist` owns identity comparison, label validation and curation order. It receives decoded entries or
classified source lines. `allowlist_file` will own one-value-per-line JSON parsing, field coercion,
comments and blank-line classification, exact serialization and atomic publication. An object without a
label survives curation. A line starting with # or an empty line is preserved; Bash currently refuses
indented comments and whitespace-only lines. The domain preserves a supplied raw line rather than
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
decision with a constant must change an asserted outcome. No new process, wire format, persistent store,
registry, configuration field or external dependency is introduced.
