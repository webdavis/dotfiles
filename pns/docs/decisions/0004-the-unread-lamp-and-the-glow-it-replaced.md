# 0004: The lamp state is `unread`, and the legacy `glow` state is deleted rather than migrated

Status: accepted, by an operator ruling on 2026-08-31.

## The rename

The steady lamp state that says there is news the operator has not seen is `unread`. The rename landed in
the types: `src/config.rs:Behaviour::Unread`, `src/lights.rs:Unread` with its two flavours `Success` and
`Failure`, `src/lights.rs:Held::UnreadSuccess` and `Held::UnreadFailure`, and the colour constants
`UNREAD_SUCCESS_COLOR` and `FAILURE_COLOR` in `src/pulse.rs`.

The two flavours share one routable configuration word, `unread`, and are told apart only by colour
(`src/lights.rs:Held`).

## What `glow` still names, and what it does not

`glow` was not eliminated from the source. It survives in comment prose in `src/main.rs`, in several test
names in `tests/dispatch.rs`, and as the legacy state entry `lights-glow`.

Two things commonly said about `lights-glow` are wrong, and both were checked against the code on
2026-09-02:

- It is a **file**, not a directory.
- The migration **deletes** it. It does not read it.

`src/main.rs:sweep_legacy_state` calls `std::fs::remove_file` on `lights-glow` and on
`lights-working-since`, and `std::fs::remove_dir_all` on the `lights-needs` directory. No read of any of
them exists. The test `src/main.rs:tests::the_first_tick_sweeps_the_state_the_old_names_held` pins that,
and the reason is in its comment: the old held record "names lamps only the binary that is gone knew how
to put out". Migrating a record of lamp identifiers that the current binary cannot act on would be worse
than discarding it, because it would leave the current run holding a state it cannot clear.

The record that replaced it is `lights-held` (`src/main.rs:LIGHTS_HELD`).

The sweep is unconditional and cheap: removing a name that is not there is one failed system call, so the
deletion happens exactly once and every tick after it pays three failed calls rather than a fourth state
file recording that the migration already ran.

## Consequence for the refactor

New code and new prose say `unread`. Renaming the surviving comments and test names is a separate change
and is not required for correctness. The three legacy names inside `sweep_legacy_state` must not be
renamed at all while that sweep is deployed, because the string in the source is the only thing naming
the file to delete.
