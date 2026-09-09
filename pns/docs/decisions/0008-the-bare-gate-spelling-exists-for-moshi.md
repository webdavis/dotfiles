# 0008: `pns <harness>-hook` is a compatibility spelling, because a third-party field holds one pathname

Status: accepted, and not ours to change.

## The constraint

moshi's generated pi and omp extensions call a helper through a field named `helperBinary`. That field
holds ONE pathname. It has no room for a subcommand, so those extensions invoke `pns pi-hook` rather than
`pns gate pi-hook`.

## The rule

The binary answers both spellings, and both end in the same place:

- `pns gate <harness>-hook` is the documented spelling, the one an operator reads.
- `pns <harness>-hook` is the bare spelling moshi is stuck with. `src/hooks.rs:is_harness_subcommand`
  recognises it in `src/main.rs:main`, above the typo refusal.

Both reach `gate_mode`, which REFUSES a word it will not vouch for. That refusal matters: falling through
to the event path instead is how the documented spelling used to fire a notification about an empty
event.

The two spellings differ on one point, and the difference is deliberate. `pns gate <unknown word>` exits
0, because a gate that declines is telling the harness it has no opinion. A bare `pns <unknown word>` is
indistinguishable from a typo at that position, so it takes the typo refusal and exits 2 (see
`docs/decisions/0006-a-word-that-names-no-command-is-a-typo.md`).

## Consequence for the refactor

The bare spelling is a frozen part of the command-line surface. It cannot be deprecated, renamed, or
moved behind a subcommand while moshi generates those extensions, because the caller is not ours to
update. Command decoding in the command-line crate keeps both entry points, and both resolve to one gate
use case.
