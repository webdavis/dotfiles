# 0007: Passing both delivery-scope flags is refused at the legacy adapter, and never becomes a domain state

Status: superseded 2026-09-17 by the retirement of both flags. The typed domain value this decision
asked for is what survived: one `--scope automatic|local_only|remote_only` flag now states it, so the
combination this refusal existed to catch cannot be typed and the refusal and its wording are gone.

## Today

`--local-only` and `--remote-only` are two independent booleans in the legacy producer surface. Passing
both is a tested contract: nothing is delivered, and the refusal says so on standard output while the
process still exits 0.

```
pns: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent
```

The sentence is at `crates/pns/src/legacy.rs`, on the dispatch path. The pane-scrub warning is deliberately withheld
in this case, because no destination would have received it.

## The rule for the refactor

Delivery scope becomes ONE typed value in the domain:

```
Automatic
LocalOnly
RemoteOnly
```

The enumeration gains no fourth value for "both". Two independent booleans are not carried into the
domain, because the combination they can express is not a delivery scope, it is an argument error.

The refusal is translated at the legacy adapter boundary, with the wording above preserved, and the
domain never sees the invalid combination at all.

## Why this is stated as a decision rather than left to taste

The general rule for the refactor is that invalid states are made unrepresentable. Applied carelessly
here, that rule produces the wrong answer twice: either a fourth enumeration value that only exists to
carry an argument error into the domain, or a silent collapse of "both" into one of the two scopes, which
would deliver something where the tested behavior is to deliver nothing. Naming the translation point is
what avoids both.

## Consequence for the refactor

The legacy command-line adapter owns this translation and its exact wording. Any new producer protocol
carries one typed delivery scope and cannot express the combination, so it needs no equivalent refusal.
