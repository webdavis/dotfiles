# pns profiles design

Date: 2026-09-20. Status: the design is the operator's ruling of 2026-09-17, restated. Every gap that
ruling left is filled below and marked `GAP DECISION`, which means the agent decided it on the
operator's behalf and it stands until the operator changes it.

Source: ledger task 139 in `docs/remaining-work.md`. Implementation plan:
`docs/superpowers/plans/2026-09-20-pns-profiles-plan.md`.

## Purpose

A profile is a named bundle of the settings that decide what reaches the operator: quiet on or off,
which of the four surfaces are on (banner, Discord, phone, lights), and which pages still get through
a surface that is otherwise off. It replaces nothing in the event pipeline. The hooks still record
every event, the durable store still fills and the delivery ledger still commits; a profile changes
DELIVERY only.

The question it answers is the one an operator cannot answer today without reading four tables: why is
it quiet right now, and what would it take to make it loud. `pns profile` answers that in three lines.

## The profile model

A profile has five settings.

| Key | Type | What it decides |
| --- | --- | --- |
| `quiet` | boolean | Whether this profile asserts the same hush `pns mute` asserts by hand. |
| `banner` | `"all"`, `"priority"`, `"none"` | This machine's own screen. |
| `discord` | `"all"`, `"priority"`, `"none"` | The durable log destination. |
| `phone` | `"all"`, `"priority"`, `"none"` | The phone card. |
| `lights` | `"all"`, `"priority"`, `"none"` | The lamp pulse. |

The three words mean: `"all"` is every event this surface would otherwise have reached, `"priority"` is
priority pages and nothing else, `"none"` is nothing at all.

A surface word is not a second presence gate. The engine's own decision runs first and decides where
the operator is and which surfaces an event would reach; the profile then SUBTRACTS from that plan. A
profile can never add a surface the presence decision did not choose, so `phone = "all"` under a
profile does not card a phone the operator is not near.

GAP DECISION (the three words). The ruling names two states per surface, on and "priority only". The
third, `"none"`, is what `work`'s "no Discord" and `night`'s "lights off" need, so the vocabulary is
three words rather than a boolean plus a special case.

GAP DECISION (subtractive only). The ruling says a profile changes delivery; it does not say in which
direction. Subtractive is the direction that cannot surprise: no profile can make the machine louder
than the presence decision already chose, so a mistyped profile costs notifications rather than
raising a card at a phone nobody is holding.

### What a priority page is

An event whose resolved route is the one `[routes] urgent` names. On this machine that is `priority`,
which the `health` and `security` delivery classes select and nothing else does (operator ruling
2026-09-14: health and security only). The route is settled once, in
`pns/crates/pns/src/event_flow/execution.rs`, where the event is routed against its class, so the
profile reads the routed event and never re-derives the class.

posture posts its own pages through its own client and is not touched by pns profiles at all, by
design.

## The three shipped profiles

Exactly as they ship in `dot_config/pns/config-values.toml`, every key visible at its value (operator
ruling 2026-08-31: a defaulted key ships uncommented at its default).

```toml
# What reaches you, bundled by name. One profile is active at a time: the
# first matching rule below chooses it, or `pns profile <name>` does by hand.
# Each surface takes "all" (everything that surface would have shown),
# "priority" (pages on the [routes] urgent route and nothing else) or "none".
# A profile only ever SUBTRACTS from what presence already decided, so it can
# never card a phone you are not near.
[profiles.default]
quiet = false
banner = "all"
discord = "all"
phone = "all"
lights = "all"

# At the desk on somebody else's clock: the screen in front of you still
# talks, the phone and Discord only for a page, and nothing else posts or
# pulses.
[profiles.work]
quiet = true
banner = "all"
discord = "priority"
phone = "priority"
lights = "none"

# Asleep. Only a page gets through, on the phone and in Discord, and
# everything else is held and delivered as one roll-up when the profile next
# admits it.
[profiles.night]
quiet = true
banner = "none"
discord = "priority"
phone = "priority"
lights = "none"
```

`default` is today's behaviour: every surface at `"all"` and no hush, which is exactly what a machine
with no `[profiles]` table has.

A `meeting` profile is the operator's to add. Task 126's `calendar_busy` input is what would select it.

## The priority floor

A profile chooses HOW a priority page arrives and never WHETHER. Three rules carry that, in this order.

1. A profile's `quiet` never applies to a priority page. A page crosses a profile's hush the way a
   `bypass_mute = true` class crosses a timed mute today.
2. A surface at `"priority"` admits a priority page. A surface at `"none"` does not, which is how a
   profile chooses phone over banner or the other way round.
3. `discord` may never be `"none"`, which is a load error naming the profile. This is the floor made
   structural, and it names `discord` rather than "any one of the four" because banner and phone are
   already conditioned on presence before a profile ever sees them (`pns_domain::surface::plan`):
   banner fires only at the desk and only when the origin pane is not already the one on screen, and
   the phone card never fires at the desk at all. Neither can promise a page reaches anything wherever
   the operator happens to be, so a profile whose floor rested on them could still silence a page in
   practice while passing a load check that only looked at the profile in isolation. `discord` is the
   one leg `channel_plan` never masks by presence (it is the durable log every event reaches), so it is
   the one surface no profile may turn off.

GAP DECISION (the operator's own mute is sovereign). The floor binds profiles, rules and
`pns profile <name>`. It does not bind `pns mute`, which is the operator typing a hush by hand with an
expiry they chose, and it does not bind `[delivery_class.<name>] bypass_mute`, which is the existing
per-class answer to the same question. So a page still obeys a `pns mute` exactly as it does today, and
nothing about profiles changes which classes cross it.

## How one is chosen

`[[profiles.rules]]` is an ordered list. Each rule names a profile and any of five inputs, and the
FIRST rule whose named inputs all match wins. A rule naming no input at all matches always, so it is
the last useful row in the list. No rule matching means `default`.

| Input | Written as | Matches when |
| --- | --- | --- |
| `days` | `["Mon", "Tue"]` | The local weekday is in the list. |
| `hours` | `"22:00-06:00"` | The local time is inside the window; a window that crosses midnight is written start-first and is the two ends of the day joined. |
| `location` | `"home"` | The machine is on the network `[profiles.locations]` gives that name. |
| `focus` | `"Work"` | A macOS Focus of that name is asserted right now. |
| `calendar_busy` | `true` or `false` | The calendar input says the operator is (or is not) inside a busy event. |

An input the machine could not read matches NOTHING. An unknown network matches no `location` rule, an
unreadable Focus store matches no `focus` rule, a calendar that did not answer matches no
`calendar_busy` rule, and an unreadable clock matches no `days` or `hours` rule. The direction is
deliberate and is the same one the rest of pns takes on an unreadable probe: not knowing costs the
narrowing and never the notification, so the fall-through is `default`, the loudest profile.

Overlapping rules are fine; order decides. A rule that names a profile no `[profiles.<name>]` table
defines is a LOAD ERROR that names the rule by its one-based index and the profile it named, never a
silent fall-through.

The rules this machine ships with:

```toml
# Which profile is active, first match wins, evaluated on the daemon's clock.
# An input the machine cannot read matches nothing, so an unreadable probe
# falls through to `default` rather than silencing anything. A rule naming no
# input matches always.
[[profiles.rules]]
profile = "night"
hours = "22:00-06:00"

[[profiles.rules]]
profile = "work"
days = ["Mon", "Tue", "Wed", "Thu", "Fri"]
hours = "09:00-17:00"
```

GAP DECISION (which rules ship). The ruling names the three profiles and the rule vocabulary, not the
rows. These two are the minimum that exercises `days` and `hours` and leaves `location`, `focus` and
`calendar_busy` for the operator, whose values are this machine's own network names and Focus names
and cannot be guessed. `pns profile learn` is how the first `location` row gets written.

GAP DECISION (`days` spelling). Three-letter English abbreviations, `Mon` through `Sun`, matched
case-insensitively. A full name is refused by name with the seven words listed, because two accepted
spellings of one weekday is two things to test and one of them to get wrong.

## The manual override

```
pns profile                     print the active profile and what chose it
pns profile <name>              select it until cleared
pns profile <name> --for 2h     select it for a duration
pns profile <name> --until 17:30  select it until a local time today (or tomorrow if it has passed)
pns profile clear               go back to the rules
pns profile learn <name>        print the TOML row for the network this machine is on now
```

An override wins over every rule while it stands. `--for` takes the same duration spelling and the same
one-second-to-twenty-four-hour range `pns mute` takes (`pns_domain::mute::MUTE_RANGE`), because one
spelling of "how long" cannot have two sets of bounds. `--until` takes `HH:MM` and resolves to the next
occurrence of that local minute.

GAP DECISION (`--until` past today's clock). `--until 09:00` typed at 17:00 means nine tomorrow
morning, not a moment eight hours in the past. The alternative, refusing it, makes the operator do
date arithmetic to say something unambiguous.

GAP DECISION (an unbounded override is allowed). `pns mute` deliberately has no untimed form, because a
forgotten mute is a notification system that has silently stopped working. A forgotten PROFILE is not:
it is visible in `pns profile`, it is one line in the ledger, and its own priority floor guarantees
that a page still lands. So `pns profile night` with no bound stands until cleared.

### Storage

One row, in the same SQLite store and on the `Scalar` model the quiet expiry already uses
(`pns/crates/pns-adapters/src/persistence/sqlite/settings.rs`, `scalar.rs`). The body is the profile
name, optionally followed by one space and the expiry epoch:

```
night
night 1758420600
```

ONE ROW RATHER THAN TWO, for `mute_expiry`'s own reason: a name and an expiry in two rows are two
values that can disagree about whether an override is standing. Anything else in the body is refused
the way a malformed `quiet-until` is: the override reads as absent, the rules decide, and the complaint
names the body it could not read.

## Location

A location is a named network fingerprint. On this machine the Wi-Fi name is NOT readable:
`ipconfig getsummary en0` prints `SSID : <redacted>` (measured 2026-09-17), because macOS gates the
name behind Location Services. So the fingerprint is the default gateway's hardware address:

1. `route -n get default` names the gateway's IPv4 address.
2. `arp -n <gateway>` names that address's hardware address.

The fingerprint is the hardware address, lower-cased, with each octet written in two digits, so that
`0:11:22:aa:bb:cc` and `00:11:22:AA:BB:CC` are one fingerprint rather than two. Nothing leaves the
machine: no GPS, no phone, no lookup.

```toml
# A named network, fingerprinted by the default gateway's hardware address
# rather than the Wi-Fi name, which macOS will not hand over without Location
# Services. `pns profile learn <name>` prints the row to paste here.
[profiles.locations]
home = "00:11:22:aa:bb:cc"
```

### Refusals

Both refusal cases answer "this machine is on no KNOWN network", which matches no `location` rule.
Neither is ever a partial fingerprint.

| Case | What is read | What `pns profile learn` says | Exit |
| --- | --- | --- | --- |
| No default route | `route -n get default` names no gateway, or the command could not run | `pns profile: this machine has no default route, so there is no network to learn` | 1 |
| Unreadable ARP entry | `arp -n <gateway>` prints `no entry`, prints `(incomplete)`, or names no hardware address | `pns profile: the gateway <address> has no ARP entry yet; try again once something has talked to it` | 1 |

On the resolver's side both are the same answer, `None`, and a rule naming `location` does not match.

GAP DECISION (`learn` prints, it does not write). pns has no writer for an existing config file, and
`~/.config/pns/config.toml` is a chezmoi target that the next apply would overwrite. So `learn` prints
the row and the operator pastes it into `dot_config/pns/config-values.toml` and runs
`just pns-config-render`. Printing is also what makes the command safe to run from a phone over SSH.

GAP DECISION (the two commands stay commands). The standing preference is a native OS call over a
shelled third-party tool (operator ruling 2026-09-19), and the routing table is reachable natively
through `PF_ROUTE`. The ruling for this task names `route -n get default` and `arp -n` explicitly, so
they stay, behind one adapter with one port, which is what makes a later native port a swap of that
adapter and nothing else. Both are stock macOS tools, not third-party ones.

## When it is evaluated

The resolver runs:

- on every daemon tick,
- on a Focus change, which the tick observes by reading the Focus store it already reads,
- within one location poll of a network change,
- immediately on every `pns profile` command that sets or clears an override.

GAP DECISION (the network poll). The ruling says "immediately on a network change". macOS can report
one through `SCNetworkReachability`, which is a callback, a run loop and a thread inside a daemon that
has none of those. The fingerprint is therefore read at most once every `[profiles] location_poll`
(default `"30s"`, bounded `"5s"` to `"5m"`), and every tick in between reuses the last reading. A
network change is observed within that window, which the acceptance run below is what proves.

## The resolver

A pure function. Inputs in, profile out, no clock, no file, no probe, no environment inside it, so
every rule combination has a deterministic unit test with no daemon.

```rust
pub struct Inputs {
    pub weekday: Option<u32>,          // 0 is Sunday, as libc gives it
    pub minutes_of_day: Option<u16>,   // minutes since local midnight
    pub location: Option<String>,      // the NAME the fingerprint resolved to
    pub focus: Option<String>,         // the asserted Focus mode's display name
    pub calendar_busy: Option<bool>,
}

pub enum Chose {
    Manual { until: Option<u64> },
    Rule { index: usize },             // one-based, as the refusals count
    Fallback,                          // no rule matched
}

pub struct Resolved {
    pub profile: String,
    pub chose: Chose,
    pub matched: Vec<&'static str>,    // the names of the inputs the winning rule matched on
}

pub fn resolve(
    rules: &[Rule],
    inputs: &Inputs,
    active_override: Option<&Override>,
    now_secs: Option<u64>,
) -> Resolved;
```

An override whose expiry has passed is not applied, and the expiry second itself is already over, which
is `mute::is_muted`'s own half-open rule. An override with no readable clock beside it stands: the
clock could not say it had expired, and the operator asked for it by hand.

The `hours` window reuses `pns_domain::lamps::window::{parse_window, quiet_now}`, which already reads
`HH:MM-HH:MM`, already joins the two ends of a midnight-crossing window, and already has its tests. It
is not copied and not promoted.

## The transition record

Every resolve compares against the profile that was last active. On a change the daemon writes ONE row
to a `profile_transitions` table in the same SQLite store and prints the same line on stdout, which
launchd captures in the gateway's own log:

```
pns gateway: profile default -> night (rule 1: hours 22:00-06:00)
pns gateway: profile night -> work (manual, until 17:30)
pns gateway: profile work -> default (override cleared)
```

The reason is the `Chose` the resolver returned, rendered by one pure function with its own test. A
resolve that changes nothing writes nothing and prints nothing, which is what keeps a one-second clock
from writing 86,400 rows a day.

GAP DECISION (a table of its own). "One line in the ledger" does not name a table. The activity store's
own rule is that the harness hooks are its only writers
(`pns/crates/pns/src/activity.rs`), so a transition goes in a table beside it rather than into it, and
`pns recap` can read it later without a second rule about which activity rows are not agent activity.

## What happens to what was suppressed

Nothing is dropped. An event a profile held back, meaning the profile subtracted every decorative
surface the presence decision had chosen, is recorded as held, with the profile that held it and the
same six fields the missed journal keeps (`pns_domain::missed::Entry`). The durable half is unaffected
where `discord` admits it: a held event is one the OPERATOR did not see, not one the log missed.

On a transition to a profile that admits a held surface, the engine delivers ONE roll-up through the
same replay path the missed journal uses (`pns/crates/pns/src/return_replay.rs` and
`pns-application/src/replay_missed.rs`), never the backlog one by one:

```
held while `work`: 1 blocked, 3 done, 1 failed
```

Wording rules, each pinned by a test:

- The profile named is the one that HELD the events, not the one being entered.
- Counts are by state, `blocked` FIRST, then the remaining states in descending count and alphabetical
  order within a tie. The blocked one is first because it is the one that was waiting on a person.
- A state with no events is not written. One event of a state is written `1 blocked`, not `1 blockeds`.
- The roll-up is one card and one durable line, whatever the count, exactly as `missed::summary` is.

Under `night` that roll-up is what `pns recap` shows for the overnight window (task 125 and slice 54).

GAP DECISION (held is its own record). The missed journal answers "the operator was away". A hold
answers "the operator was here and their profile said not now", carries the profile's name and is
flushed by a transition rather than by a return. Conflating the two would make `missed::summary`'s
wording answer two questions. The journal's own privacy rule (nothing prints an entry; only the
replayer reads one back) applies unchanged to the held record. Its depth rule does not carry over for
free, because the held record is a SQLite table rather than the missed journal's own file: `hold_event`
enforces the same number, `missed::KEPT` (25), by trimming the oldest row past it in the same write, so
a long or busy `night` window cannot grow the table without bound.

## `pns profile` output

Bare, with an override standing:

```
pns: profile `night` (manual, until 17:30)
     quiet on; banner none, Discord priority, phone priority, lights none
     held while `night`: 2 done
```

Bare, chosen by a rule:

```
pns: profile `work` (rule 2: days, hours)
     quiet on; banner all, Discord none, phone priority, lights none
     holding nothing
```

Bare, with no rule matching:

```
pns: profile `default` (no rule matched)
     quiet off; banner all, Discord all, phone all, lights all
     holding nothing
```

After a set or a clear, the report is READ BACK from the store rather than rendered from what the run
intended, which is `pns mute`'s own rule: the line cannot claim a profile that never landed.

## Errors and exit codes

| Situation | Where | Message | Exit |
| --- | --- | --- | --- |
| A rule names an undefined profile | config load | ``rule 2 names profile `meeting`, which no `[profiles.meeting]` table defines`` | load error |
| A profile's `discord` is `"none"` | config load | ``profile `night`'s `discord` is "none"; a priority page has to reach the durable log wherever you are, so `discord` must be "all" or "priority"`` | load error |
| A surface word is not one of the three | config load | ``unknown `profiles.night` value for `phone`; a surface is "all", "priority" or "none"`` | load error |
| `hours` is not `HH:MM-HH:MM` | config load | ``rule 1 has hours "22:00", which is not a HH:MM-HH:MM window`` | load error |
| `days` names no weekday | config load | ``rule 1 has day "Funday"; a day is Mon, Tue, Wed, Thu, Fri, Sat or Sun`` | load error |
| `pns profile <undefined>` | command | ``pns profile: no profile named `meeting`; this config defines default, night, work`` | 2 |
| A bad `--for` or `--until` | command | the duration parser's own refusal, then the usage | 2 |
| Any extra or unknown word | command | the usage | 2 |
| The override could not be written | command | ``pns: state error (the profile override could not be written: <error>); the profile was not changed`` | 1 |

A config load error aborts the load, which is what every other malformed table already does: a typo that
turns delivery off must not pass quietly.

## Testing

Unit tests, all pure, all in the domain crate:

- One per rule input: `days` matches and does not, `hours` inside and outside and across midnight,
  `location` known and unknown, `focus` asserted and not, `calendar_busy` both ways.
- One per unreadable input: each of the five reading `None` fails to match a rule that names it.
- Precedence: the first matching rule wins over a later matching one; a rule naming several inputs does
  not match when one of them does not; no rule matching resolves `default`; a rule naming no input
  matches always.
- The override: it beats every rule; an expired one does not; an unbounded one stands; an unreadable
  clock leaves an override standing; a malformed stored body reads as no override.
- The priority floor: for EVERY shipped profile, and for a generated profile of every surface
  combination the config admits, a priority page reaches Discord. A profile with `discord = "none"` is
  refused at load whatever the other three surfaces are, which is the other half and has its own test.
  Composed with `pns_domain::surface::plan` at the desk with the origin pane already on screen, the one
  presence state that zeroes both the banner and the phone card before a profile is even applied, the
  same page still reaches Discord through the composed `channel_plan`, which is what proves the floor
  holds against presence and not only against the profile read in isolation.
- The fingerprint: the two refusal cases, and the octet normalization.
- The roll-up wording: blocked first, singular and plural, a state with no events omitted, the profile
  named is the one that held.
- The transition line: one per `Chose` variant, and a resolve that changes nothing writes nothing.
- The config: every load error above, by message.
- The render: `dot_config/pns/private_config.toml.tmpl` regenerates byte-for-byte from
  `dot_config/pns/config-values.toml`, which is the existing test in `just test-rust` and covers the new
  tables for free once they are in the layout.

The operator's acceptance, on dresden, and the one thing no unit test can stand in for: move the
machine to another network and watch the transition line appear in the gateway's log and the row in the
table, with the old profile, the new one and the reason.

## Relations to other work

| Task | What profiles change about it |
| --- | --- |
| 126, the calendar input | It becomes the `calendar_busy` rule input rather than a switch of its own. The existing `[quiet.calendar]` poll is what supplies it. |
| B18, status lighting during quiet and Focus | The active profile's `lights` word decides whether status lighting runs. `default` has `lights = "all"`, which keeps B18's decided behaviour exactly. |
| `pns mute` | Unchanged, and documented as a temporary hush over the active profile's `quiet`. It is the operator's own hand, so it outranks the profile in both directions: a mute silences a profile that is loud, and it is the one hush a priority page still obeys. |

NOTE ON SPELLING. The ledger calls it `pns quiet`. That word was retired: the verb is `pns mute`, and
`pns quiet` is refused by name (`pns/crates/pns/src/invocation.rs`). This document uses the shipped
spelling.

## Files this design touches

Named here so the plan can be read against it.

- `pns/crates/pns-domain/src/profiles/` (new): the model, the resolver, the override codec, the
  fingerprint normalizer, the roll-up and the report, all pure.
- `pns/crates/pns-adapters/src/config/profiles.rs` (new) and `config/{mod,model,load,schema}.rs`: the
  parse and its refusals.
- `pns/crates/pns-adapters/src/config/render/{profiles.rs,layout/profiles.rs}` (new) and `render.rs`:
  the shipped text, including the array-of-tables branch the renderer does not have today.
- `pns/crates/pns-adapters/src/persistence/sqlite/profiles.rs` (new), `scalar.rs`, the schema: the
  override row, the transitions table, the held record.
- `pns/crates/pns-adapters/src/macos/network.rs` (new) and `macos/focus.rs`: the gateway fingerprint,
  and the asserted Focus mode NAME, which the store reader has but does not currently return.
- `pns/crates/pns/src/command_profile.rs` (new), `invocation.rs`, `subcommand_usage.rs`: the command.
- `pns/crates/pns/src/profile_runtime.rs` (new), `event_flow/execution.rs`, `daemon_runtime.rs`: the
  composition root's read and the tick.
- `pns/crates/pns-domain/src/{decision/overrides.rs,decision/arbitration.rs,routing.rs}`: the surface
  mask applied where the legs are planned.
- `dot_config/pns/config-values.toml` and the regenerated `dot_config/pns/private_config.toml.tmpl`.

Slices 54 and 55 (the recap engine and the summarizer) and task 170 (the dead-letter line) are landing
in parallel. Nothing above touches `pns/crates/pns-domain/src/recap/`,
`pns/crates/pns/src/command_recap/` or the failure page beyond reading what they already publish.
