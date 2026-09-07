# Enrichment boundaries

The domain owns signing classification and interpreter membership. The application owns inspection order
through `EnrichmentInspection`. Concrete file and command inspection belongs to adapters; the
command-line crate constructs those adapters and writes the result. No serialization dependency enters
the domain or application, and no persistence protocol changes.

One `CommandRunner` accepts separate arguments, a fixed executable path and the required output channel
policy. Signing merges stdout and stderr into the same pipe, preserving write order. Other inspections
discard command stderr as Bash did. The adapter removes command-substitution null bytes and trailing
newlines before passing readings inward. Diagnostic wording identifies posture instead of a retired Bash
source line.

The composition root supplies one ten-second budget shared by its spawned inspections. The previous
enricher had no bound; the approved process-adapter architecture requires one. Tests inject short
budgets. Every read and wait uses the same absolute deadline. The runner observes exit with `waitid` and
`WNOWAIT`, retaining the child identifier until process-group termination and reaping. This avoids
reusing a reaped child's identifier during cleanup. Nonblocking reads also keep a closed-output child or
a descendant holding the pipe inside the deadline.

Metadata uses native calls instead of spawning stat. The owner-name lookup, symbolic mode and local
calendar preserve the existing format. macOS stat uses local time even though the chosen format ends with
a literal `Z`; this port preserves that wording instead of silently changing timestamps. The interpreter
exception and acceptance of an unrecognized named authority are existing policy, not new claims that a
script or signing certificate is safe.

Only the adapter crate adds libc, already named by the approved design. The builder's locked build line,
installed binary path, file-integrity record and justfile coverage already support this workspace. The
real router, three executable doubles and the retired Bash source change together. The watchdog's
arbitrary second manifested-file fixture is retained; it never invokes enrichment.
