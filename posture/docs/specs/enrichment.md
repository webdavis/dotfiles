# Enrichment

`posture enrich <path>` supplies the existing router with the same fact bytes and trust status as
`enrich-finding.sh`. This batch implements statements S134 through S141. It does not deliver an alert.

| Given                                                   | When                                         | Then                                                                                                                                                                                                   |
| ------------------------------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| No path, an empty path, or a missing ordinary file      | Enrichment runs                              | Exit 0 with no fact. Later operands are ignored.                                                                                                                                                       |
| A bundle suffix, or a regular file classified as Mach-O | Signing is inspected                         | Failed inspection or `not signed` produces `UNSIGNED`, exit 10. `adhoc` produces its existing untrusted label, exit 10.                                                                                |
| Successful signing output                               | Its first `Authority=` line is read          | Preserve awk field 2. Empty authority is untrusted. Apple and Software Signing get the Apple label; Developer ID Application loses that prefix; every other named authority stays trusted and visible. |
| A plist                                                 | Its program is resolved                      | Prefer nonempty `Program`, then `ProgramArguments.0`; unresolved code exits 10.                                                                                                                        |
| One of the fourteen interpreter basenames               | Arguments 1 through 5 are inspected in order | Use the first existing absolute regular-file argument. Report its basename and interpreter without promotion. Arguments outside that range do not select a payload.                                    |
| Selected code or script                                 | Quarantine is read                           | Append `, downloaded` only for a successful nonempty attribute. A failed read remains best effort.                                                                                                     |
| An existing non-code path                               | Metadata is read                             | Report owner, symbolic mode and modified time without a trailing newline. A live symlink gets its own metadata; a broken symlink gets no fact.                                                         |
| An inspection child stalls                              | The shared deadline expires                  | Terminate its owned process group and reap the direct child. Signing becomes untrusted; plist resolution, quarantine and file classification keep their existing failure directions.                   |

The router checks one executable filename and invokes it with two separate arguments, `enrich` and the
complete finding path. Exit 10 alone promotes NOTICE to CRIT. Exit 0 preserves NOTICE; exit 5 with stdout
keeps its fact without promotion. Missing or nonexecutable enrichment stays skipped.

Command-substitution normalization removes null bytes and trailing newlines before classification. The
Bash warning about an ignored null byte becomes
`posture: warning: ignored null byte in inspection output`; the removed shell source pathname and line
number are implementation details.
