# 0008: `pns <harness>-hook` is the only gate spelling, because a third-party field holds one pathname

Status: accepted, and not ours to change.

## The constraint

moshi's generated pi and omp extensions call a helper through a field named `helperBinary`. That field
holds ONE pathname. It has no room for a subcommand, so those extensions invoke `pns pi-hook` rather than
`pns gate pi-hook`.

## The rule

The binary answers the bare spelling and nothing else. `crates/pns/src/invocation.rs` hands every
hook-shaped word (anything ending `-hook`) to `gate_mode`, above the typo refusal, and `gate_mode` is
the ONE place that judges whether the word is one it will vouch for.

`pns gate <harness>-hook` was a second spelling of the same gate, for an operator to read. It is gone:
two spellings of one gate is one too many, and `gate` now names no subcommand, so it takes the typo
refusal and exits 2 like any other unknown word (see
`docs/decisions/0006-a-word-that-names-no-command-is-a-typo.md`).

A hook-shaped word `gate_mode` will not vouch for is REFUSED: exit 2 and a sentence on stderr naming the
word. The retired spelling used to exit 0 for such a word, on the reasoning that a gate which declines is
telling the harness it has no opinion. That was the worst answer available. A hook that exits 0 having
forwarded nothing looks wired for the life of the install, so a word the gate cannot use has to say so.
Exit 0 is kept for the paths that genuinely DECLINE: no moshi, the operator at the desk, a payload that
did not arrive whole.

## Consequence for the refactor

The bare spelling is a frozen part of the command-line surface. It cannot be deprecated, renamed, or
moved behind a subcommand while moshi generates those extensions, because the caller is not ours to
update. Command decoding in the command-line crate keeps that one entry point, resolving to one gate use
case.
