# pns delivery failure reporting

Status: design, approved by the operator on 2026-09-08. Not yet built.

## Why this exists

posture is being ported to Rust and stops being its own notifier. It becomes a producer that hands
findings to pns, which already owns delivery (banner, Discord through the hermes gateway, phone
through moshi, lights). Its pages are security pages, so a page that does not arrive is the worst
outcome in the system.

Today a page that cannot be delivered fails quietly and slowly. This design makes a delivery failure
loud, specific, and quick to act on.

### The defect that prompted it

Every producer names a hermes route with `--channel <route>`. pns validates that the name is safe as
a URL (uniform resource locator) path segment and nothing more: `route_name_is_usable` accepts any
run of ASCII letters, digits, `-` and `_`. A well formed name that names no route on the gateway is
built into a URL and posted.

The gateway answers 404. pns classifies that as `PostOutcome::Status(404)`, and `delivered()` is true
only for 200 to 299, so the post failed. Nothing else happens. There is no permanent-status rule, so
a 404 is retried on the same schedule as an unreachable gateway: `RetryLimits` allows 20 attempts or
seven days, whichever comes first, and then the leg is dead-lettered.

So a route that will never exist consumes 20 attempts before it is abandoned, and the operator's only
signal is an aggregate banner that names neither the route nor the status:

```
N undelivered delivery leg(s), M deadlettered; backlog growth streak X. The delivery pipeline needs
attention.
```

The HTTP (hypertext transfer protocol) status is known at the moment of the post and is printed in
sync mode as `pns: post FAILED HTTP 404`, but it is not persisted anywhere and never reaches a
background caller. A typo'd route and a dead gateway are indistinguishable from the outside.

## Failure classification

Delivery failures split in two, and the split decides how long pns keeps trying.

**Temporary.** The destination may recover: no response at all, any 5xx, 408, and 429. These retry
under the backoff below and dead-letter only when the existing attempt and age limits are exhausted.

**Permanent.** The request was understood and refused, or was never sendable, and repeating it cannot
help: 400, 401, 403, 404, 405, 410, 422, and a malformed URL that never reached the wire. These
dead-letter on the first failure, carrying the status and the reason.

A malformed URL is permanent even though it produced no status. `PostOutcome::NoStatus` means ureq
refused the URI or a header, and neither heals with time.

The distinction is written once, in `pns-domain`, so the retry loop and the reporting read the same
rule. `DeadletterReason` gains a variant for a permanent refusal alongside `Attempts` and `Age`.

**The rule applies to every destination, not only hermes.** A refusal will not fix itself whatever
answered it, so moshi and any later HTTP destination classify the same way. What varies per
destination is the wording of `meaning`, never the classification.

### Backoff

The stranded branch `fix/pns-retry-backoff` adds `RetryBackoff` with `base_secs: 60` and
`random_secs: 60`, scheduling the next attempt at `now + base_secs * retries + jitter`. Backoff is
correct and is kept: without it a failing leg retries in a tight loop against a restarting gateway.

The jitter is not kept. Random spread exists to stop many clients retrying in the same instant, and
this is one local daemon draining one queue against a loopback gateway. There is no herd to spread,
and the randomness makes tests nondeterministic for no gain.

Backoff and the permanent rule must land together. Backoff alone makes the misconfiguration case
worse, stretching a hopeless 20 attempts from minutes to hours.

## The failure message

One record, rendered two ways. Both carry the same fields in the same order, and neither is a summary
of the other.

### Fields

| Field | Carries |
| --- | --- |
| `status` | the code and its standard name, for example `HTTP 404 (Not Found)` |
| `meaning` | what the code means here, in plain words, so the reader needs no HTTP knowledge |
| `webhook route` | the route the producer named |
| `failed command` | the routing flags pns was given, as the string to search for |
| `fix` | what to do next |

`status` pairs the number with its registered name because a reader may know one and not the other.
`meaning` is pns explaining the code in this context, which is the part that teaches.

### Every meaning names its concrete subject

A meaning line never says "the route", "the key" or "the payload" in the abstract. It substitutes the
real value, because a reader holding a phone cannot resolve a pronoun against a system they are not
looking at. The route name, the config key, and the address are all short, so precision costs almost
nothing against the character budget.

Meanings are keyed by destination and code together. A 401 from hermes and a 401 from moshi name
different secrets, so one table per destination.

**hermes**, for a post to route `testpath` at `127.0.0.1:8644`:

| Code | `meaning` | Class |
| --- | --- | --- |
| 400 | hermes could not parse the body pns posted to `testpath` | permanent |
| 401 | the `[plugins.hermes]` key is wrong or missing | permanent |
| 403 | the `[plugins.hermes]` key may not post to the `testpath` route | permanent |
| 404 | the hermes gateway has no route named `testpath` | permanent |
| 405 | the `testpath` route exists but refuses a POST | permanent |
| 410 | the `testpath` route existed once and has been removed | permanent |
| 422 | hermes parsed the body but rejected its fields | permanent |
| bad URL | the URL pns built for `testpath` is malformed, nothing was sent | permanent |
| 408 | hermes did not finish reading the request before its own timeout | temporary |
| 429 | hermes is rate limiting and refused this one for now | temporary |
| 500 | hermes accepted the request then failed while handling it | temporary |
| 502 | a proxy in front of hermes could not reach it at `127.0.0.1:8644` | temporary |
| 503 | hermes is running but refusing work, which usually means a restart | temporary |
| 504 | a proxy in front of hermes gave up waiting for it | temporary |
| no response | nothing answered at `127.0.0.1:8644` | temporary |

**moshi:**

| Code | `meaning` | Class |
| --- | --- | --- |
| 401 | the `[plugins.mobile]` token is wrong or expired | permanent |
| 404 | moshi has no endpoint at the configured URL | permanent |
| no response | nothing answered at the moshi URL | temporary |

Two of those rows are deliberately surprising. A loopback gateway normally has no proxy in front of
it, so naming one in 502 and 504 makes an odd situation visible instead of hiding it behind "something
went wrong".

Naming a config key inside an error string is a commitment: rename `[plugins.hermes] key` and these
messages go stale silently. A test asserts the strings match the live config key names, so a rename
breaks the build rather than the message.

`failed command` is past tense on purpose. An earlier draft labelled it `command`, which reads as an
instruction to run it, and it is the opposite: the command that already ran and failed. It carries
only the routing flags. `--detail` holds arbitrary producer text and never appears in an error.

### Full form

For surfaces with no length limit and a monospace face: the terminal, `pns doctor`, `pns failures`,
the log, and Discord inside a fenced code block.

```
pns: delivery failed
  status:          HTTP 404 (Not Found)
  meaning:         the hermes gateway has no webhook route by that name
  webhook route:   testpath
  sent by:         posture heartbeat
  failed command:  pns --agent posture --channel testpath
  fix:             run `pns doctor` to see which routes the gateway accepts,
                   then add "testpath" to ~/.hermes/config.yaml
```

### Notification form

For the desktop banner and the phone card.

**The budget is 256 characters, counting newlines. The header does not count against it.** Measured on
this machine on 2026-09-08 by bisection: 256 arrives whole, 257 is truncated, and two probes of
different lengths cut at the same absolute character position, which is what identified the number
rather than a range.

It is a total budget, not a per-line one. Rewrapping a long line does not buy anything back; only
removing content does. Notification bodies target 240 to leave room for a long route or agent name.

Two traps that cost several rounds of measurement, recorded so nobody repeats them:

- **Synthetic padding lies.** A probe padded with an unbroken run of characters gets dropped rather
  than wrapped, because no renderer can break a 200 character word. Probes must look like real
  messages, with spaces and newlines.
- **Notifications group.** Several sent within a few seconds collapse into a stacked preview showing
  a few words of the group, which reads exactly like truncation and is not. Probes need long gaps.

The desktop variant, at 230 characters:

```
status: HTTP 404 (Not Found)
meaning: the hermes gateway has no webhook route by that name
webhook route: testpath
failed command: pns --agent posture --channel testpath
fix: run `pns failures` for the full error and how to fix it
```

The phone variant differs only in its last line, which points at the reader's surface rather than at
a terminal they are not sitting at. See below.

Two differences from the full form, both forced by the budget:

- No column padding. Banners and cards render proportionally, so padding produces ragged text rather
  than alignment.
- `sent by` is dropped. `failed command` already contains `--agent posture`, so the two say the same
  thing, and the notification form cannot afford to say anything twice.

The gateway is named once, in `meaning`, which is why `webhook route` carries the bare name.

### The fix line points at the reader's surface

`fix` names something the reader can act on from where they are standing. At the machine that is a
command; on a phone it is not, because there is no terminal in reach and no supported way to open one
from a card.

| Surface | `fix` says | Because |
| --- | --- | --- |
| terminal, `pns doctor`, log | the repair, naming the file to edit | the reader is already there |
| desktop banner | `run pns failures for the full error` | a terminal is one keystroke away |
| phone, `serve = true` | `open moshi's servers list, pick pns :8646` | the page is the whole record |
| phone, `serve = false`, hermes ok | `full error in Discord, #priority` | the full form is there |
| phone, `serve = false`, hermes failed | `run pns failures on dresden` | nothing else is reachable |

Because moshi supports no deep link, the phone rows must **name the steps**, not just the destination.
"Open moshi's servers list and pick pns" is a two-tap instruction someone can follow while holding the
phone; a bare mention of a page they cannot open is worse than saying nothing.

`serve` is what pns branches on. It cannot detect a moshi Pro subscription, and it does not need to:
an operator who cannot use browser preview sets `serve = false`, and that is the same switch that
decides which pointer the phone gets.

The last row is the honest floor. When the gateway leg is what broke and there is no page, a phone
reader cannot be given the full record by any route, so the message says where it will be rather than
pretending otherwise. That is the rule about never reporting a failure through the destination that
failed, applied to the pointer rather than to the message.

## Where a failure surfaces

Every rung stands on its own. A reader who stops at the first one still knows what broke and what to
do.

1. **The notification.** Banner always; phone under the existing presence rules. Self-sufficient by
   design, because there is no supported way to open more from a tap (see the local page below).
2. **Discord**, in a fenced code block, which preserves the full form's alignment. Only when the
   hermes leg itself succeeded.
3. **The log** at `~/.local/log/`, always written, complete, and independent of every destination.
   uu already rotates it.
4. **`pns failures`**, the detail view, and `pns doctor`, which reports the count and points at it.
5. **A non-zero exit code** for a synchronous caller, so a producer such as posture can tell that its
   own page did not land. Today it cannot.

One rule governs all of them: **never report a delivery failure through the destination that failed.**
A 404 on a hermes route means the message is not in Discord, so pointing the reader at Discord sends
them to an empty channel.

## Two failures can silently replace each other

`agent · state · project` is not only the header shown to the operator. `identifiers.rs` states it is
"used as a key", and the measurement runs proved the consequence: four probes sharing one triple
produced one notification, while the same four with distinct project names produced four.

Every posture delivery failure would otherwise carry the same triple, `posture · failed · <project>`,
so a second failure could quietly displace the first and the operator would never learn there were
two. For security pages that is a lost page, which is the outcome this whole design exists to prevent.

The failure notification therefore varies its triple per failure rather than per producer. The
failure id is the natural discriminator, since it is unique by construction and already appears in
the message.

## The temporary variant

A temporary failure has nothing for the reader to repair, so it does not borrow the permanent form's
`fix` line. It says to stand down:

```
fix: nothing to do, pns will retry (3 of 20 attempts used)
```

That single line is what makes the two classes distinguishable at a glance, which is the point of
splitting them at all. Permanent says go and do something; temporary says pns has it.

## `pns failures` and `pns doctor`

`pns failures` is the detail view over the stored records: recent failures, and any one of them in
full. `pns doctor` stays a summary and routes to it by name, so there is one command to discover and
one command to use.

The default listing is the last twenty, newest first, one line each:

```
  id  when              status        route      sent by
  47  2026-09-08 14:03  HTTP 404      testpath   posture
  46  2026-09-08 13:58  no response   priority   posture

run `pns failures <id>` for one in full
```

Twenty covers a bad night without paging, and carrying the status and route on every line means most
failures are diagnosed from the list without opening one.

### The failure id

A plain counting number, 1, 2, 3, shown everywhere as the id. Short enough to read off a phone and
type at a terminal, and stable for the life of the record.

**An id is never reused.** SQLite recycles a rowid after a delete unless the column is declared
`AUTOINCREMENT`, and a recycled id would make a banner click from three days ago open somebody else's
failure. The monotonic form is required, not preferred.

### Retention

Failure records inherit the legacy pipeline's rule: they are not aged out and nothing drains them
automatically. The operator inspects a record, records its outcome or acknowledges it as undelivered,
and only then is it removable. A page nobody has read is not garbage.

### The ledger columns

Persisting this is a schema change to the delivery ledger, and it is the only part of this design
with real weight. Everything else reads what it stores.

| Column | Why |
| --- | --- |
| `last_status` | the HTTP code, null when there was not one |
| `last_outcome` | status, no-status or no-response, so a null code is not ambiguous |
| `route` | the route named, so a listing can show it |
| `failed_at` | epoch of the most recent failure |
| `deadletter_reason` | the existing `Attempts` and `Age`, plus the permanent case |

All nullable with defaults, so rows written before this change survive without a data migration.

### The route check

`pns doctor` asks the gateway whether each configured route exists, so a misconfiguration surfaces
when it is introduced rather than when a page is lost. An unreachable gateway reports **unknown**, not
a failure: a gateway that is down is a different problem from a route that is missing, and reporting
it as a missing route would be a false alarm during every restart.

`pns doctor` also gains a route check: it asks the gateway whether each configured route exists, so a
misconfiguration is found when it is introduced rather than when a page is lost.

## Interaction model

`pns failures` may present a list to choose from. It must never block a script.

| Situation | Behavior |
| --- | --- |
| Default | infer from the tty: a terminal is interactive, a pipe is not |
| `--interactive` | force on |
| `--non-interactive` | force off |
| `PNS_NON_INTERACTIVE=1` | force off, for launchd and hooks where nobody passes flags |

Flag beats environment, environment beats inference. `--interactive` with no terminal attached is an
error with a non-zero exit, not a hang. `pns setup` already refuses a non-tty, so this is the
established behavior rather than a new one.

## The clickable banner

`BannerChannel` posts through `terminal-notifier` with `-activate` (raise the terminal) and
`-execute` (run a shell string). Clicking does both. For an ordinary event the string focuses the
herdr workspace and pane the event came from; for an event with no pane, which is every delivery
failure, `click_command` returns the no-op `:`. That click is currently doing nothing, and it is the
slot this design uses.

The string pns writes is fixed:

```
/Users/stephen/.local/libexec/pns/pns click <failure-id>
```

pns stays in the loop rather than handing the user's command to `terminal-notifier`. Two reasons. The
notifier's `-execute` takes a shell string that this crate already escapes through
`verbatim_argument`, and passing a user template through that is a quoting hazard with no upside. And
a command spawned by the notifier is unobserved: pns has exited, so nothing can report that the click
failed.

`pns click` reads the configuration and runs the chosen view:

```toml
[banner.click]
type = "herdr"                          # herdr | window | command
command = "kitty -e pns failures {id}"  # type = "command" only
```

| type | What it opens |
| --- | --- |
| `herdr` | a pane in the running session, persistent, where the operator already works |
| `window` | a new terminal window, `open -na Ghostty.app --args -e ...` |
| `command` | whatever the operator names, with `{id}` substituted |

The default is inferred: `herdr` when herdr is present, `window` otherwise. Nobody configures
anything to get sensible behavior, and anyone on another terminal or another workflow can override
it.

`window` cannot be spelled `ghostty -e ...`. Ghostty's own help states that on macOS "launching the
terminal emulator from the CLI (command-line interface) is not supported and only actions are
supported", and directs callers to `open -na Ghostty.app`.

A click runs in a bare launchd context with no PATH and no shell profile, so a configured command
needs absolute paths. That is documented at the configuration key, because the failure is otherwise
a click that silently does nothing.

When the configured view fails, pns records it in the log with the command it tried, falls back to
`window` so the operator still gets what they asked for, and only if that also fails raises a banner
saying so. **That banner carries no click**, which is what stops the loop.

## The local page

moshi's browser preview can view a local server from the phone, so pns serves the failure record as a
page.

What the documentation establishes: `moshi-hook` probes local listeners automatically and remembers
those answering with an HTTP header, so nothing registers itself. The phone reaches the server over a
per-session SSH (secure shell) local-forward through the existing session, never over a public
interface. The scanned set is configurable with `moshi-hook set scan-ports <list>` or
`scan-ports all`.

```toml
[failures]
serve = true
port = 8646
```

Both keys ship uncommented at their defaults, because they are defaulted keys rather than an opt-in
feature. The listener binds `127.0.0.1`: the SSH forward is the trust boundary, so nothing needs to
be exposed on any other interface. The port is fixed and documented so discovery is predictable and
so an operator with a narrowed `scan-ports` list knows what to add.

The page is a thin render of the same stored record the terminal view reads. There must not be two
formatters that can drift.

**Two limits the operator should know.** Browser preview is gated on a moshi Pro subscription, and
the tunnel exists only while a terminal session is open, so the page is unreachable when no session
is running. Neither is a defect in pns and neither can be detected by it.

`serve = false` is the fallback for anyone without Pro. Nothing else changes: the notification is
self-sufficient, Discord carries the full form whenever the hermes leg worked, and the terminal view
is unaffected. The page is an extra rung, never a dependency.

## Out of scope

- **A notification deep-link.** moshi's documentation describes no URL scheme or card format for
  opening a specific page, so a tapped card cannot land on a failure. This is why the notification
  form is designed to stand alone.
- **Image cards.** Rendering the full form to an image would defeat the character budget, but the
  result cannot be searched, copied, or read by a screen reader.
- **Reporting a failure over a route that still works.** When one route is misconfigured the default
  route is almost certainly healthy, so the failure could still reach Discord. It is deferred because
  it is the piece most likely to misfire, and it should land after the rest is proven.
- **Draining the legacy queue safely.** The posture plan's mixed-producer route transition exists to
  protect alerts already queued under the old key. The operator has ruled that nothing has launched
  and no such queue is worth preserving, so that machinery is not a constraint on this design.

## Verification

- The permanent rule: a 404 dead-letters on the first attempt, not the twentieth, with the status and
  route on the record. The control is the same post against a route that exists.
- The budget: a 240 character notification body arrives whole on both surfaces. This was measured by
  hand on 2026-09-08 and should be re-measured if moshi or macOS changes.
- The click: a configured command that does not exist produces a log line, a fallback window, and no
  second click.
- The page: a failure written by the ledger is readable at `127.0.0.1:8646` and matches the terminal
  view byte for byte in its field values.
