---
name: loop
description: Start a long-running loop with the pns loop lamp held for its whole duration, and give the lamp back when the work ends. Use when the user asks to run something overnight, in a loop, or unattended, or says "/pns:loop".
---

# Loop

Hold the pns loop lamp for the whole of a long unattended run, so the lamp is a
liveness signal: it breathes while the work is in flight and stops the moment
the loop, pns, or the machine dies.

## Take the lease first

```bash
~/.local/libexec/pns/pns loop begin
```

Run it in the pane the work will run in. It keys the lease to `HERDR_PANE_ID`
from its own environment, and that pane's ordinary hook traffic renews it. With
no `HERDR_PANE_ID` and no `--pane <id>` it refuses rather than guessing; do not
pass `--pane` unless the user names a pane.

If it exits non-zero, say so and stop. Do not start the work with the lamp
unclaimed and do not retry with `--pane` to get around the refusal.

## Then set the work up

Establish the goal and the loop the user asked for, exactly as you would
without the lamp. The lease changes nothing about how the work runs.

## Give the lease back at the end

```bash
~/.local/libexec/pns/pns loop end
```

Run it in the same pane, as the last step of the run, on success and on
failure alike. The lease times out on its own if this never runs, so the lamp
is never stuck for good, but it does hold for the whole timeout.
