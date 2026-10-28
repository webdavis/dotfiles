# Native macOS probe evaluation

Status: measurement, written 2026-09-17. No Rust changed, no dependency added, no apply ran, and no
notification was delivered to produce it. Every number below was taken on dresden (Apple M1, macOS
27.0) with the shipped `pns` binary at `~/.cargo/bin/pns` and with a live mosh session present, so
the phone chain ran to completion rather than stopping at its first empty step.

Finding in one line: **the two `ioreg` reads are worth replacing and the rest are not.** The idle
read alone costs 44.5 ms median against 0.01 ms for the IOKit property it is printing, and the lock
read costs 28.8 ms against 0.14 ms. The three process spawns cost 56.4 ms together, but 25.9 ms of
that is the one step whose selection semantics the September 13 investigation already found hard to
reproduce natively, and 4.1 ms of it is a step too cheap to rewrite at all.

## What each probe shells today

Read from `pns/crates/pns-adapters/src/macos/desk.rs` and `.../macos/phone.rs`. Every one of them
goes through `SystemCommandRunner` in `pns/crates/pns-adapters/src/process/bounded.rs`, which spawns
the command under a five second deadline and a one mebibyte output ceiling, owning a process group
through a forked cleanup child so a wedged probe cannot hold a notification open.

| Probe           | Command                          | What it asks for                             |
| --------------- | -------------------------------- | -------------------------------------------- |
| desk idle       | `/usr/sbin/ioreg -c IOHIDSystem` | the first `HIDIdleTime` line, in nanoseconds |
| desk lock       | `/usr/sbin/ioreg -n Root -d1`    | the quoted `"IOConsoleLocked"` aggregate     |
| phone servers   | `/usr/bin/pgrep -x mosh-server`  | the detached server process identifiers      |
| phone clients   | `/usr/bin/pgrep -P <servers>`    | the client children that own the terminals   |
| phone terminals | `/bin/ps -o tty= -p <clients>`   | each client's controlling terminal name      |

The chain's last step is not a command at all: `newest_terminal_atime` joins each terminal name under
`/dev` and takes the access time with `std::fs::metadata`. That step is already native and is not a
candidate for anything.

Two structural facts matter for the payoff arithmetic. The desk pair and the phone chain run on two
spawned threads (`probes/start.rs`), so the stage costs roughly the slower of the two rather than the
sum of all five. And the lock read is gated on the idle read having parsed, so a machine whose idle
read fails pays for one `ioreg`, not two.

## Measured cost of the shell probes

hyperfine 1.19, `--shell=none` so no shell start is counted, 10 warmup runs, 200 timed runs each,
ambient machine. Milliseconds.

| Command                         |     n | median |   min |   max |
| ------------------------------- | ----: | -----: | ----: | ----: |
| `ioreg -c IOHIDSystem`          |   200 |  44.47 | 34.27 | 63.05 |
| `ioreg -n Root -d1`             |   200 |  28.77 | 23.20 | 40.61 |
| `pgrep -x mosh-server`          |   200 |  25.92 | 24.36 | 33.91 |
| `pgrep -P <one server>`         |   200 |  26.34 | 24.03 | 51.53 |
| `ps -o tty= -p <one client>`    |   200 |   4.14 |  2.92 | 10.21 |
| `/usr/bin/true` (spawn floor)   |   200 |   1.62 |  1.21 |  4.61 |
| `pns --version`                 |   200 |   3.75 |  2.67 |  8.86 |

Repeated with eight `yes > /dev/null` spinners saturating the cores, 100 timed runs each:

| Command                         |     n | median |   min |   max |
| ------------------------------- | ----: | -----: | ----: | ----: |
| `ioreg -c IOHIDSystem`          |   100 |  44.43 | 32.08 | 85.97 |
| `ioreg -n Root -d1`             |   100 |  30.49 | 22.82 | 56.65 |
| `pgrep -x mosh-server`          |   100 |  30.49 | 27.29 | 43.74 |
| `pgrep -P <one server>`         |   100 |  29.52 | 25.09 | 38.00 |
| `ps -o tty= -p <one client>`    |   100 |   9.09 |  3.43 | 15.20 |

Added load moves the medians by single digit milliseconds and widens the tails. Nothing here is
load sensitive in the way a network call is.

Derived chain costs from the ambient medians: the desk pair is 73.2 ms serial, the phone chain is
56.4 ms serial, and the parallel stage is bounded by the desk pair at about 73 ms. A bash harness
running the same chains measured 86.1 ms (desk), 88.0 ms (phone) and 101.4 ms (both in parallel) over
100 runs each, which is those figures plus the shell's own subshells and command substitutions, so
treat the bare sums as the pns-side estimate and the harness numbers as an upper bound.

Two costs are worth naming separately. `/usr/bin/true` at 1.62 ms is the irreducible spawn floor:
five spawns carry at least 8 ms of pure process creation before any probe reads anything. And the
bounded runner adds a second fork per spawn for the cleanup child, so the real per probe overhead is
higher than the hyperfine column, not lower.

Output sizes, which is why the idle read is the expensive one: `ioreg -c IOHIDSystem` prints 359,918
bytes and `ioreg -n Root -d1` prints 95,313 bytes. Both are parsed for one line.

## What fraction of a pns run this is

`pns` itself is cheap. `pns --version` measures 2.79 ms median (n=60) and `pns failures`, which opens
the ledger, reads it and prints, measures 4.45 ms median (n=60). Those are the only two subcommands
timed here, because no non-delivering subcommand exercises the desk or phone probes: the probe stage
is reached from the hook and event paths, which deliver, and from `nag`, `stale` and `doctor`, which
this evaluation was told not to run.

So against everything local that pns does apart from probing and delivering, the probe stage is over
nine tenths of the run: about 73 ms of probes against about 4.5 ms of process start, configuration
load, ledger read and printing.

Against a whole process the share is smaller. The September 13 private investigation recorded in the
ledger measured a complete pns process with destination stubs at 239.9 ms median ambient and 270.8 ms
under added load. That measurement was not reproduced here and is quoted, not verified. Taking it as
the denominator, the probe stage is roughly 30 percent of a whole run ambient and roughly 27 percent
under load, and the remainder is delivery: the gateway post, the banner spawn and the ledger writes.

This is the part of the answer that decides the question. A probe worth 2 ms inside a 400 ms run is
not worth a rewrite, and that is exactly the verdict for `ps -o tty=` at 4.1 ms. It is not the
verdict for the idle read at 44.5 ms, which is one sixth of a whole notification on its own and the
single largest local cost pns pays.

## Measured cost of the native alternatives

Measured with a throwaway C program in the scratchpad, compiled unsigned with
`clang -O2 -framework IOKit -framework CoreFoundation -framework ApplicationServices`, timing the
call itself in process over 200 iterations after 10 warmup iterations. Milliseconds per call.

| Native call                                                   |     n | median |    min |    max |
| ------------------------------------------------------------- | ----: | -----: | -----: | -----: |
| `IORegistryEntryCreateCFProperty` for `HIDIdleTime`           |   200 | 0.0100 | 0.0080 | 0.0350 |
| `CGSessionCopyCurrentDictionary` plus one key read            |   200 | 0.1430 | 0.1190 | 0.8030 |
| `libproc` walk: name match, parent match, terminal device     |   200 | 1.0360 | 0.9580 | 2.0340 |

Under the same eight spinner load: 0.0100, 0.1850 and 0.9630 median, with the libproc walk's maximum
stretching to 15.56 ms. The native calls are three to four orders of magnitude cheaper than the
spawns they would replace, which is not surprising: the spawn floor alone is 160 times the IOKit
property read.

Behaviour was checked against the shipped commands in the same minute:

- `HIDIdleTime` read natively returned 11,491,902,694,000 while `ioreg -c IOHIDSystem` printed
  11,502,662,460,125 moments later. Same property, same units, same magnitude.
- `CGSSessionScreenIsLocked` read `true` while `ioreg -n Root -d1` printed `"IOConsoleLocked" = Yes`.
  Agreement on the one state observable at the time.
- The `libproc` walk found one `mosh-server` by name, one child of it, and terminal device 268435472,
  which `devname` resolves to `ttys016`, exactly the name `ps -o tty=` printed.

## What adopting each one would cost

**The idle read, through IOKit.** `IOServiceGetMatchingService` for `IOHIDSystem` then
`IORegistryEntryCreateCFProperty` for `HIDIdleTime`, then one `CFNumberGetValue`. Needs two new
crates, an IOKit binding and a Core Foundation binding, which the September 13 prototype took as
maintained `objc2-io-kit` and Core Foundation crates. Needs one unsafe block per call and manual
release of the returned property and the service object. No entitlement, no code signing requirement
and no privacy consent prompt: the measurement above ran from an ad hoc unsigned binary with no
special permission. No Linux consequence, because the module is already `src/macos/` and the trait
implementation is already macOS only. Behaviour is identical rather than close: it reads the same
registry property that `ioreg` is printing.

**The lock read. Take it through IOKit as well, not through CoreGraphics.**
`CGSessionCopyCurrentDictionary` is the obvious native call and it is the wrong one. It answers for
the window session the calling process belongs to, it returns a null dictionary in a session that has
none, and its key is the per session `CGSSessionScreenIsLocked` rather than the kernel aggregate
`IOConsoleLocked` that `desk.rs` deliberately reads. The comment in `parse_screen_locked` already
says why the aggregate was chosen: the per session flag would require picking the console session
first. It also pulls a third framework binding in. The registry Root node carries `IOConsoleLocked`
and is reachable with the same two crates the idle read already needs, so the lock read is free once
the idle read is paid for and it keeps the exact semantics the decision layer was written against.
CoreGraphics adds no entitlement requirement either, and it did resolve a dictionary in this
measurement even under `env -i`, but the mosh and remote shell case was not testable here (key only
authentication refused a loopback session), which leaves a documented null return on an untested
path. Given the fail direction in `parse_screen_locked`, where only `Some(true)` locks, a null
reading would silently stop locking rather than fail loudly.

**The phone chain, through `libproc`.** No new dependency at all: `libc` 0.2.189, already a direct
dependency of `pns-adapters`, declares `proc_listpids`, `proc_pidinfo`, `proc_name`,
`proc_bsdinfo` and `PROC_PIDTBSDINFO`. Two symbols are missing and are one line each to declare:
the `PROC_ALL_PIDS` constant and `devname`, needed to turn `e_tdev` into a name under `/dev`. Costs
one unsafe block, a heap buffer sized from a first `proc_listpids` call, and a re-read discipline for
the window where the process table changes between the sizing call and the filling call. No
entitlement and no signing requirement, and no Linux consequence for the same reason as above.

Behaviour here is close rather than identical, and the ledger already recorded the mismatch. macOS
`pgrep` without `-f` matches against process names, and `-x` requires an exact match of that name.
The `libproc` side offers two names for the same process: `pbi_comm`, sixteen bytes, and `pbi_name`,
thirty two, and `proc_name` prefers the longer one. A process that renamed its own argument zero, or
one whose name is longer than sixteen bytes, can therefore be selected by one and not the other.
On dresden the two agree for `mosh-server`, whose `ps -o ucomm=` prints `mosh-server` and whose
`proc_name` returned the same, so this is a latent divergence rather than an observed failure. The
September 13 hybrid resolved it by keeping `pgrep -x` for selection and going native only for the
rest, which the measurements below support for a second reason.

## Recommendation per probe

Ranked by measured payoff.

1. **Desk idle, `ioreg -c IOHIDSystem`: adopt the native call.** 44.5 ms to 0.01 ms, the largest
   single local cost in a pns run, one property read with identical semantics, no entitlement and no
   Linux fork in the code. This is the whole payoff of the exercise; a change that took only this one
   would capture 60 percent of the available saving.
1. **Desk lock, `ioreg -n Root -d1`: adopt the native call, reading `IOConsoleLocked` from the
   registry Root node.** 28.8 ms to well under 1 ms, and the dependency is already paid by the idle
   read. Do not adopt `CGSessionCopyCurrentDictionary` for it: different source of truth, a null
   return on session types not tested here, and a fail direction that turns that null into silently
   never locking.
1. **Phone clients, `pgrep -P <servers>`: adopt the native call, as part of the same `libproc` walk
   that answers the terminal question.** 26.3 ms to about 1 ms, no new dependency, and the parent
   identifier is a plain integer comparison with no naming ambiguity at all, so this step carries
   none of the selection risk that its sibling does.
1. **Phone servers, `pgrep -x mosh-server`: keep the shell.** 25.9 ms is real money, but this is the
   one step where the native answer is not the same answer, and the September 13 investigation
   already found that out the hard way. Keeping it costs one spawn and buys exact, unchanged
   selection semantics. Revisit only with a name predicate pinned to whichever field macOS `pgrep`
   itself reads, proven against a process that renames its argument zero.
1. **Phone terminals, `ps -o tty= -p <clients>`: keep the shell, unless the client step goes native
   first.** 4.1 ms is not worth an unsafe block and a `devname` declaration on its own. If the client
   step above is adopted, the terminal device number is already in hand from the same
   `PROC_PIDTBSDINFO` record and this spawn disappears for free rather than for a rewrite.

Caching is not recommended for any of them. Each pns invocation is a fresh process, so a cache means
a state file, and a state file write costs more than the 4.1 ms probe it would spare while adding a
freshness question to a reading whose entire purpose is freshness. The desk readings are already
cached within a run by the `OnceCell` fields on `SystemProbes`.

Expected result if the three adoptions land: the parallel probe stage falls from about 73 ms to about
27 ms, bounded now by the one retained `pgrep`, and the whole run falls by roughly 46 ms, which is
close to a fifth of the 239.9 ms whole process figure the ledger records.

## What is still not measured, and the costs that are not milliseconds

- **The interruptible deadline goes away.** Every shell probe today runs inside a five second
  deadline enforced by a forked cleanup child that owns the process group. A native call in process
  has no such bound: a wedged IOKit registry read or a `libproc` call against a stuck process cannot
  be killed by that mechanism, and the notification path would block on it. The ledger already flags
  this as the direct call variant's loss. It is the one genuine cost of adoption, and it is not a
  cost the measurements above can price.
- **Locked state transitions were not exercised.** The machine reported locked throughout, so the
  parity check covers one state, not the transition.
- **Unreadable and multi-user cases were not exercised.** No device refused a reading during the run,
  and only one login session existed.
- **A whole pns run was never timed here.** No non-delivering subcommand reaches these probes, so the
  denominator is the ledger's own earlier private figure rather than a measurement of this session.
- **The mosh and remote shell session case for `CGSessionCopyCurrentDictionary` was not testable.**
  Loopback shell access refused password authentication, and the process ancestry of the measuring
  shell was a graphical terminal, not the mosh session.

## Reproducing

The timings above came from `hyperfine -N --warmup 10 --runs 200` over the five commands and the two
baselines, with the pid arguments resolved once from the live mosh session, and from a throwaway C
program compiled in the scratchpad. Nothing was written into the repository beyond this document, and
no `pns` subcommand other than `--version` and `failures` was run.
