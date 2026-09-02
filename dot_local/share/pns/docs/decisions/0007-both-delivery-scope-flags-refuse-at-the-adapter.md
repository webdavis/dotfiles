# 0007: Passing both delivery-scope flags is refused at the legacy adapter, and never becomes a domain state

Status: accepted. The refusal wording is pinned by test.

## Today

`--local-only` and `--remote-only` are two independent booleans in the legacy producer surface. Passing
both is a tested contract: nothing is delivered, and the refusal says so on standard output while the
process still exits 0.

```
pns: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent
```

The sentence is at `src/main.rs`, on the dispatch path. The pane-scrub warning is deliberately withheld
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
