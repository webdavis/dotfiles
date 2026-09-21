# pns profiles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended)
> or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`)
> syntax for tracking.

**Goal:** One named bundle of delivery settings is active at a time, chosen by an ordered rule list or
by hand, and it subtracts surfaces from what presence already decided without ever silencing a priority
page.

**Architecture:** The resolver is a pure function in `pns-domain` with every input passed in, so the
whole rule matrix is tested with no daemon and no clock. The config layer parses `[profiles.<name>]`,
`[profiles.locations]` and `[[profiles.rules]]` and refuses each malformed shape by name. The
composition root reads the probes, resolves, and hands the resulting surface mask to the delivery
decision as one more `Overrides` field, so the mask is applied exactly where the legs are already
planned. The daemon resolves on its own tick, records a transition row when the answer changes, and
flushes the held roll-up through the replay path that already exists.

**Tech Stack:** Rust 2024, four independent cargo workspaces, `toml` for config, `rusqlite` for state,
`libc` for the local clock, stock macOS `route` and `arp` behind one port. Gates: `just test-rust` and
`just lint-check`.

**Spec:** `docs/superpowers/specs/2026-09-20-pns-profiles-design.md`

## Global Constraints

- The three surface words are `"all"`, `"priority"` and `"none"`, and nothing else parses.
- The five profile keys are `quiet`, `banner`, `discord`, `phone`, `lights`. No sixth.
- The three shipped profiles are `default` (`quiet = false`, every surface `"all"`), `work`
  (`quiet = true`, `banner = "all"`, `discord = "none"`, `phone = "priority"`, `lights = "none"`) and
  `night` (`quiet = true`, `banner = "none"`, `discord = "none"`, `phone = "priority"`,
  `lights = "none"`).
- The five rule inputs are `days`, `hours`, `location`, `focus` and `calendar_busy`. First match wins;
  no match is `default`; a rule naming no input matches always.
- An input that could not be read matches nothing.
- `days` is `Mon`, `Tue`, `Wed`, `Thu`, `Fri`, `Sat`, `Sun`, matched case-insensitively.
- `hours` is `HH:MM-HH:MM`, midnight-crossing written start-first, parsed by
  `pns_domain::lamps::window::parse_window` and evaluated by `pns_domain::lamps::window::quiet_now`.
  No second parser is written.
- A priority page is an event whose resolved route equals `[routes] urgent`, which on this machine is
  `priority`.
- A profile's `quiet` never applies to a priority page. A surface at `"priority"` admits one. A profile
  whose four surfaces are all `"none"` is a load error.
- `pns mute` is unchanged and is not bound by the priority floor.
- `calendar_busy` PARSES AND RESOLVES here and is read as `None` by the composition root: the live
  value is ledger task 126's to supply, and until it does, a rule naming `calendar_busy` matches
  nothing. That is the same direction every other unreadable input takes.
- The profile only ever SUBTRACTS from the delivery plan presence already chose.
- `--for` takes `pns_domain::mute::MUTE_RANGE`, one second to twenty-four hours. `--until` takes `HH:MM`
  and resolves to the next occurrence of that local minute.
- The override is ONE store row: the profile name, optionally one space and the expiry epoch.
- The location fingerprint is the default gateway's hardware address, lower-cased, two digits per octet.
- `[profiles] location_poll` defaults to `"30s"`, bounded `"5s"` to `"5m"`.
- `pns profile learn` PRINTS the row; it never writes a config file.
- The roll-up reads ``held while `<profile>`: 1 blocked, 3 done, 1 failed``, blocked first, then
  descending count, then alphabetical within a tie, and a state with no events is omitted.
- Exit codes: 0 for a report or a change that landed, 1 for a state error, 2 for argv the operator typed
  wrong or a profile name the config does not define.
- Every `.rs` file stays at 300 lines ideal and 500 hard cap, unit tests included.
- `dot_config/pns/private_config.toml.tmpl` is GENERATED. Never hand-edit it: run
  `just pns-config-render`, which `just test-rust` byte-compares.
- No em-dashes in any comment, message or document this plan writes.
- Every slice ends with `just test-rust` and `just lint-check` green.

---

## File Structure

New, in `pns/crates/pns-domain/src/`:

| File | Responsibility |
| --- | --- |
| `profiles.rs` | The module doc, `Profile`, `Admits`, `Surface`, `Rule`, `Override`, and the re-exports. |
| `profiles/admission.rs` | What one profile admits for one event: the surface mask and the priority floor. |
| `profiles/resolve.rs` | `resolve`: rules plus inputs plus override, in; `Resolved`, out. Pure. |
| `profiles/codec.rs` | The override row's parse and format. |
| `profiles/fingerprint.rs` | Normalizing a hardware address, and what the two refusals answer. |
| `profiles/rollup.rs` | The held roll-up's wording. |
| `profiles/report.rs` | The `pns profile` lines and the transition line. |

New, in `pns/crates/pns-adapters/src/`:

| File | Responsibility |
| --- | --- |
| `config/profiles.rs` | Parsing the three tables and every refusal. |
| `config/render/layout/profiles.rs` | The shipped prose and key list. |
| `config/render/profiles.rs` | The hardcoded branch that writes named profiles, locations and rules. |
| `persistence/sqlite/profiles.rs` | The override row, the transitions table, the held record. |
| `macos/network.rs` | `route -n get default` and `arp -n`, behind the existing `CommandRunner` port. |

New, in `pns/crates/pns/src/`:

| File | Responsibility |
| --- | --- |
| `command_profile.rs` | The `pns profile` verbs and their exit codes. |
| `profile_runtime.rs` | The composition root's read: probes in, one resolved profile out, for the event path and the tick. |

Modified: `pns-domain/src/lib.rs`, `decision/overrides.rs`, `decision/arbitration.rs`, `routing.rs`;
`pns-adapters/src/config/{mod,model,load,schema}.rs`, `config/render.rs`,
`config/render/layout.rs`, `persistence/sqlite/{mod,scalar,schema}.rs`, `macos/{mod,focus}.rs`,
`lib.rs`; `pns/src/{lib,invocation,subcommand_usage,daemon_runtime}.rs`,
`pns/src/event_flow/execution.rs`; `dot_config/pns/config-values.toml` and the regenerated
`dot_config/pns/private_config.toml.tmpl`.

---

## The slices

Five pull requests. Each is independently mergeable, each ends both gates green, and each leaves the
binary working.

SLICE 1: the profile model and its config tables
plan-items: the profile model, the three shipped profiles, the rules table, the locations table, every
load refusal, the rendered template.
why-this-order: first. Every later slice reads a parsed `Profile` or a parsed `Rule`, and the renderer
has no array-of-tables branch today, so the shape has to exist before anything resolves over it.
files: `pns/crates/pns-domain/src/profiles.rs`, `profiles/admission.rs` (and its `tests.rs`),
`pns/crates/pns-adapters/src/config/profiles.rs` (and its `tests.rs`),
`pns/crates/pns-adapters/src/config/{mod,model,load,schema}.rs`,
`pns/crates/pns-adapters/src/config/render/{layout.rs,layout/profiles.rs,profiles.rs,render.rs}`,
`dot_config/pns/config-values.toml`, `dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none. Nothing reads the new fields yet.
behaviour-to-pin: the three profiles and the two rules parse; a rule naming an undefined profile, a
profile with four `"none"` surfaces, an unknown surface word, a malformed `hours` and an unknown
weekday are each refused by their own message; the template regenerates byte-for-byte.
risk: low. The config gains tables and the binary's behaviour does not change. The one sharp edge is
the renderer: `[[profiles.rules]]` is the first array of tables in the layout, so it needs its own
branch beside `render_delivery_classes` rather than a `Table` row.
size: medium

SLICE 2: the resolver, the override row and `pns profile`
plan-items: the pure resolver, the override's storage, the report, the set, the clear and the bounds.
why-this-order: after slice 1, which is what it resolves over. Before slices 4 and 5, which need an
active profile to read.
files: `pns/crates/pns-domain/src/profiles/{resolve.rs,codec.rs,report.rs}` (each with its `tests.rs`),
`pns/crates/pns-adapters/src/persistence/sqlite/{profiles.rs,scalar.rs,mod.rs}`,
`pns/crates/pns/src/command_profile.rs` (and its `tests.rs`),
`pns/crates/pns/src/profile_runtime.rs`,
`pns/crates/pns/src/{invocation.rs,subcommand_usage.rs,lib.rs}`
callers-to-update-in-the-same-slice: none. `pns profile` is a new word; no existing caller types it.
behaviour-to-pin: each of the five inputs matches and fails to match; an unreadable input matches
nothing; the first matching rule wins; no match is `default`; an override beats every rule and expires;
`pns profile` prints the active profile and what chose it; an undefined name exits 2.
risk: low. Nothing reads the resolved profile yet, so a wrong answer costs a printed line.
size: medium

SLICE 3: the location fingerprint and the Focus mode name
plan-items: `route -n get default`, `arp -n`, the two refusals, `pns profile learn`, and the asserted
Focus mode's NAME, which the store reader has and does not return.
why-this-order: after slice 2, whose resolver already takes `location` and `focus` as plain `Option`
values, so this slice fills two inputs that are currently always `None` and changes no rule logic.
files: `pns/crates/pns-domain/src/profiles/fingerprint.rs` (and its `tests.rs`),
`pns/crates/pns-adapters/src/macos/{network.rs,mod.rs}`,
`pns/crates/pns-adapters/src/macos/focus.rs`, `pns/crates/pns-adapters/src/lib.rs`,
`pns/crates/pns/src/command_profile.rs`
callers-to-update-in-the-same-slice: none. `FocusReading` gains a field; its existing readers keep
reading `silenced`.
behaviour-to-pin: a gateway with an ARP entry fingerprints; no default route and an unreadable ARP
entry each answer no location and each exit 1 from `learn` with their own sentence; octets normalize;
the Focus probe returns the asserted mode's display name beside the silenced verdict.
risk: low, and bounded by the port: both commands run through `CommandRunner`, which the tests script.
size: small

SLICE 4: the delivery gate and the priority floor
plan-items: the surface mask applied where the legs are planned, the priority floor at delivery, the
composition root's read.
why-this-order: after slices 2 and 3, so the profile it applies is a real one. Before slice 5, which
records what this slice held.
files: `pns/crates/pns-domain/src/decision/{overrides.rs,arbitration.rs}`,
`pns/crates/pns-domain/src/routing.rs`, `pns/crates/pns/src/profile_runtime.rs` (extended),
`pns/crates/pns/src/event_flow/execution.rs`, `pns/crates/pns/src/lib.rs`
callers-to-update-in-the-same-slice: every constructor of `pns_domain::routing::channel_plan` and of
`Overrides`, which is `decision/arbitration.rs` plus the test support under
`pns/crates/pns/src/runtime_test_support/`.
behaviour-to-pin: a surface at `"none"` drops that leg; a surface at `"priority"` drops it for an
ordinary event and keeps it for a priority page; a profile's `quiet` silences an ordinary event and
never a priority page; `default` changes nothing about today's plan.
risk: medium, and the highest in the ladder. This is the slice that can silence a notification. The
guard is the floor test over every surface combination the config admits, plus the `default` test that
pins today's behaviour exactly.
size: medium

SLICE 5: the transition record, the held record and the roll-up
plan-items: evaluation on the daemon tick, the transition row and line, the held record, the one
roll-up on transition.
why-this-order: last. It records what slice 4 held and flushes it, so both halves exist before anything
is written.
files: `pns/crates/pns-domain/src/profiles/rollup.rs` (and its `tests.rs`),
`pns/crates/pns-domain/src/profiles/report.rs`,
`pns/crates/pns-adapters/src/persistence/sqlite/profiles.rs`,
`pns/crates/pns/src/{daemon_runtime.rs,profile_runtime.rs,return_replay.rs}`
callers-to-update-in-the-same-slice: none outside pns.
behaviour-to-pin: a resolve that changes nothing writes nothing; a change writes one row and prints one
line naming the old profile, the new one and the reason; a held event is recorded with the profile that
held it; a transition to a profile that admits the surface delivers ONE roll-up with blocked first.
risk: medium. The daemon writes on a one-second clock, so the no-change path has to be silent; the test
that a repeated resolve writes nothing is what holds it.
size: medium

---
## Slice 1 tasks

### Task 1: the profile model and the priority floor

**Files:**
- Create: `pns/crates/pns-domain/src/profiles.rs`
- Create: `pns/crates/pns-domain/src/profiles/admission.rs`
- Create: `pns/crates/pns-domain/src/profiles/admission/tests.rs`
- Modify: `pns/crates/pns-domain/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pns_domain::profiles::{Admits, Profile, Rule, Override, DEFAULT_PROFILE}`.
  `Admits::parse(&str) -> Option<Admits>`, `Admits::word(self) -> &'static str`,
  `Admits::admits(self, priority: bool) -> bool`, `Profile::admits_a_page(&self) -> bool`,
  `Profile::hushes(&self, priority: bool) -> bool`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-domain/src/profiles/admission/tests.rs`:

```rust
use super::*;

#[test]
fn a_surface_admits_what_its_word_says() {
    assert!(Admits::All.admits(false), "all admits an ordinary event");
    assert!(Admits::All.admits(true));
    assert!(!Admits::Priority.admits(false), "priority admits no ordinary event");
    assert!(Admits::Priority.admits(true));
    assert!(!Admits::None.admits(false));
    assert!(!Admits::None.admits(true), "none admits nothing, which the floor refuses at load");
}

#[test]
fn the_three_words_are_the_whole_vocabulary() {
    assert_eq!(Admits::parse("all"), Some(Admits::All));
    assert_eq!(Admits::parse("priority"), Some(Admits::Priority));
    assert_eq!(Admits::parse("none"), Some(Admits::None));
    assert_eq!(Admits::parse("All"), None, "the words are lower case");
    assert_eq!(Admits::parse("off"), None);
}

#[test]
fn a_profile_with_every_surface_off_admits_no_page() {
    let silent = Profile {
        quiet: true,
        banner: Admits::None,
        discord: Admits::None,
        phone: Admits::None,
        lights: Admits::None,
    };
    assert!(!silent.admits_a_page());
    assert!(Profile::default().admits_a_page());
    assert!(
        Profile { phone: Admits::Priority, ..silent.clone() }.admits_a_page(),
        "one surface at priority is the whole floor"
    );
}

#[test]
fn a_profiles_hush_never_reaches_a_priority_page() {
    let work = Profile { quiet: true, ..Profile::default() };
    assert!(work.hushes(false));
    assert!(!work.hushes(true));
    assert!(!Profile::default().hushes(false), "the default profile hushes nothing");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::`
Expected: FAIL, `file not found for module` or `unresolved import`.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-domain/src/profiles.rs`:

```rust
//! A profile: the named bundle of settings that decides what reaches the
//! operator. It changes DELIVERY only. The hooks still record every event and
//! the durable store still fills.
//!
//! IT ONLY EVER SUBTRACTS. The engine's own decision runs first and says where
//! the operator is and which surfaces an event would reach; a profile takes
//! surfaces away from that plan and can never add one, so no profile can card
//! a phone the operator is not near.

mod admission;
pub use admission::{Admits, Profile};

/// The profile a machine with no rules matching is on, and the one a config
/// with no `[profiles]` table has.
pub const DEFAULT_PROFILE: &str = "default";

/// One `[[profiles.rules]]` row. Every input is optional; a rule naming none
/// matches always, and the FIRST rule whose named inputs all match wins.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rule {
    pub profile: String,
    /// Weekdays, 0 for Sunday, as `libc` numbers them. Empty is not named.
    pub days: Vec<u32>,
    pub hours: Option<crate::lamps::window::QuietWindow>,
    pub location: Option<String>,
    pub focus: Option<String>,
    pub calendar_busy: Option<bool>,
}

/// The operator's own selection, which beats every rule while it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    pub profile: String,
    /// The epoch second it ends at, or None for one that stands until cleared.
    pub until: Option<u64>,
}
```

Create `pns/crates/pns-domain/src/profiles/admission.rs`:

```rust
/// What one surface admits: every event, a priority page alone, or nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admits {
    All,
    Priority,
    None,
}

impl Admits {
    /// The word the config spells it with, which is also what the report
    /// prints: one spelling, so a refusal and a report cannot disagree.
    pub fn word(self) -> &'static str {
        match self {
            Admits::All => "all",
            Admits::Priority => "priority",
            Admits::None => "none",
        }
    }

    /// The word as the config may write it, or None for anything else.
    pub fn parse(word: &str) -> Option<Admits> {
        match word {
            "all" => Some(Admits::All),
            "priority" => Some(Admits::Priority),
            "none" => Some(Admits::None),
            _ => None,
        }
    }

    /// Whether this surface takes an event, given whether it is a page.
    pub fn admits(self, priority: bool) -> bool {
        match self {
            Admits::All => true,
            Admits::Priority => priority,
            Admits::None => false,
        }
    }
}

/// One profile's five settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub quiet: bool,
    pub banner: Admits,
    pub discord: Admits,
    pub phone: Admits,
    pub lights: Admits,
}

impl Default for Profile {
    /// TODAY'S BEHAVIOUR, which is what a machine with no `[profiles]` table
    /// has: every surface loud and no hush of its own.
    fn default() -> Self {
        Profile {
            quiet: false,
            banner: Admits::All,
            discord: Admits::All,
            phone: Admits::All,
            lights: Admits::All,
        }
    }
}

impl Profile {
    /// Whether a priority page reaches anything at all under this profile.
    ///
    /// THE FLOOR, and it is checked at LOAD rather than at delivery: a config
    /// that cannot express a silenced page is one no rule and no override can
    /// select into silence.
    pub fn admits_a_page(&self) -> bool {
        [self.banner, self.discord, self.phone, self.lights]
            .iter()
            .any(|surface| *surface != Admits::None)
    }

    /// Whether this profile's own hush applies to an event.
    ///
    /// NEVER TO A PAGE. A page crosses a profile's quiet the way a
    /// `bypass_mute = true` class crosses the operator's timed mute.
    pub fn hushes(&self, priority: bool) -> bool {
        self.quiet && !priority
    }
}

#[cfg(test)]
mod tests;
```

Add to `pns/crates/pns-domain/src/lib.rs`, in module order:

```rust
pub mod profiles;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::`
Expected: PASS, four tests.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): the profile model and its priority floor"
```

---

### Task 2: parse `[profiles.<name>]`

**Files:**
- Create: `pns/crates/pns-adapters/src/config/profiles.rs`
- Create: `pns/crates/pns-adapters/src/config/profiles/tests.rs`
- Modify: `pns/crates/pns-adapters/src/config/mod.rs`

**Interfaces:**
- Consumes: `pns_domain::profiles::{Admits, Profile}` from Task 1.
- Produces: `pub(super) fn parse_profile(name: &str, value: &toml::Value)
  -> Result<Profile, ConfigError>`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-adapters/src/config/profiles/tests.rs`:

```rust
use super::*;

fn table(text: &str) -> toml::Value {
    toml::Value::Table(text.parse::<toml::Table>().expect("a table"))
}

#[test]
fn the_five_keys_parse() {
    let night = parse_profile(
        "night",
        &table(
            "quiet = true\nbanner = \"none\"\ndiscord = \"none\"\n\
             phone = \"priority\"\nlights = \"none\"\n",
        ),
    )
    .expect("night parses");
    assert_eq!(
        night,
        Profile {
            quiet: true,
            banner: Admits::None,
            discord: Admits::None,
            phone: Admits::Priority,
            lights: Admits::None,
        }
    );
}

#[test]
fn an_unwritten_key_keeps_todays_behaviour() {
    let sparse = parse_profile("sparse", &table("quiet = true\n")).expect("it parses");
    assert_eq!(sparse, Profile { quiet: true, ..Profile::default() });
}

#[test]
fn an_unknown_surface_word_is_refused_with_the_three_words() {
    let refusal = parse_profile("night", &table("phone = \"off\"\n")).expect_err("refused");
    assert_eq!(
        refusal.to_string(),
        "unknown `profiles.night` value for `phone`; a surface is \"all\", \"priority\" or \"none\""
    );
}

#[test]
fn a_profile_that_silences_every_surface_is_refused_by_name() {
    let refusal = parse_profile(
        "night",
        &table("banner = \"none\"\ndiscord = \"none\"\nphone = \"none\"\nlights = \"none\"\n"),
    )
    .expect_err("refused");
    assert_eq!(
        refusal.to_string(),
        "profile `night` admits no priority page; at least one of banner, discord, phone, lights \
         must be \"all\" or \"priority\""
    );
}

#[test]
fn an_unknown_key_is_refused_by_name() {
    let refusal = parse_profile("night", &table("lamps = \"none\"\n")).expect_err("refused");
    assert!(
        refusal.to_string().contains("unknown `profiles.<name>` key `lamps`"),
        "it said: {refusal}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters config::profiles`
Expected: FAIL, the module does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-adapters/src/config/profiles.rs`:

```rust
use super::*;
use pns_domain::profiles::{Admits, Profile};

/// The roster row the refusals print, shared by every `[profiles.<name>]`
/// table: the names are the operator's, so the roster holds the prefix and the
/// refusal names the path they wrote.
pub(super) const PROFILE_KEYS: &str = "profiles.<name>";

/// One `[profiles.<name>]` table.
///
/// AN UNWRITTEN KEY IS TODAY'S BEHAVIOUR, never off: a profile that named only
/// its hush still talks on every surface, which is the direction a mistyped
/// table has to fail in.
pub(super) fn parse_profile(name: &str, value: &toml::Value) -> Result<Profile, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(format!(
            "`profiles.{name}` is not a table"
        )));
    };
    let shown = format!("profiles.{name}");
    let mut profile = Profile::default();
    for (key, setting) in table {
        admits(PROFILE_KEYS, &shown, key)?;
        match key.as_str() {
            "quiet" => {
                profile.quiet = setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "`{shown}` key `quiet` has type `{}`, not boolean",
                        setting.type_str()
                    ))
                })?;
            }
            "banner" => profile.banner = surface(&shown, key, setting)?,
            "discord" => profile.discord = surface(&shown, key, setting)?,
            "phone" => profile.phone = surface(&shown, key, setting)?,
            "lights" => profile.lights = surface(&shown, key, setting)?,
            _ => return Err(unknown_key(PROFILE_KEYS, PROFILE_KEYS, key)),
        }
    }
    // THE FLOOR, refused here rather than at delivery: see `admits_a_page`.
    if !profile.admits_a_page() {
        return Err(ConfigError::Invalid(format!(
            "profile `{name}` admits no priority page; at least one of banner, discord, phone, \
             lights must be \"all\" or \"priority\""
        )));
    }
    Ok(profile)
}

fn surface(shown: &str, key: &str, setting: &toml::Value) -> Result<Admits, ConfigError> {
    setting
        .as_str()
        .and_then(Admits::parse)
        .ok_or_else(|| {
            ConfigError::Invalid(format!(
                "unknown `{shown}` value for `{key}`; a surface is \"all\", \"priority\" or \"none\""
            ))
        })
}

#[cfg(test)]
mod tests;
```

Add to `pns/crates/pns-adapters/src/config/mod.rs`, beside the other table modules:

```rust
mod profiles;
use profiles::{PROFILE_KEYS, parse_profile};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters config::profiles`
Expected: PASS, five tests.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src/config
SKIP_AI_COMMIT=1 git commit -m "feat(pns): parse one named profile table"
```

---

### Task 3: parse the rules list and the locations table

**Files:**
- Modify: `pns/crates/pns-adapters/src/config/profiles.rs`
- Modify: `pns/crates/pns-adapters/src/config/profiles/tests.rs`

**Interfaces:**
- Consumes: `parse_profile` from Task 2, `pns_domain::profiles::Rule` from Task 1,
  `pns_domain::lamps::window::parse_window`.
- Produces: `pub(super) struct Profiles { pub profiles: BTreeMap<String, Profile>,
  pub locations: BTreeMap<String, String>, pub rules: Vec<Rule>, pub location_poll_secs: u64 }` and
  `pub(super) fn parse_profiles(value: toml::Value) -> Result<Profiles, ConfigError>`.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns-adapters/src/config/profiles/tests.rs`:

```rust
fn parsed(text: &str) -> Result<Profiles, ConfigError> {
    parse_profiles(table(text))
}

#[test]
fn the_shipped_shape_parses() {
    let read = parsed(
        "location_poll = \"30s\"\n\
         [default]\nquiet = false\n\
         [night]\nquiet = true\nbanner = \"none\"\ndiscord = \"none\"\n\
         phone = \"priority\"\nlights = \"none\"\n\
         [locations]\nhome = \"00:11:22:aa:bb:cc\"\n\
         [[rules]]\nprofile = \"night\"\nhours = \"22:00-06:00\"\n\
         [[rules]]\nprofile = \"default\"\ndays = [\"Sat\", \"Sun\"]\n",
    )
    .expect("it parses");
    assert_eq!(read.location_poll_secs, 30);
    assert_eq!(read.locations.get("home").map(String::as_str), Some("00:11:22:aa:bb:cc"));
    assert_eq!(read.rules.len(), 2);
    assert_eq!(read.rules[0].profile, "night");
    assert!(read.rules[0].hours.is_some());
    assert_eq!(read.rules[1].days, vec![6, 0], "Saturday is 6 and Sunday is 0");
}

#[test]
fn a_rule_naming_an_undefined_profile_is_refused_with_its_index() {
    let refusal = parsed("[default]\nquiet = false\n[[rules]]\nprofile = \"meeting\"\n")
        .expect_err("refused");
    assert_eq!(
        refusal.to_string(),
        "rule 1 names profile `meeting`, which no `[profiles.meeting]` table defines"
    );
}

#[test]
fn a_malformed_window_and_an_unknown_weekday_each_say_what_they_are() {
    let hours = parsed("[default]\nquiet = false\n[[rules]]\nprofile = \"default\"\nhours = \"22:00\"\n")
        .expect_err("refused");
    assert_eq!(
        hours.to_string(),
        "rule 1 has hours \"22:00\", which is not a HH:MM-HH:MM window"
    );
    let day = parsed("[default]\nquiet = false\n[[rules]]\nprofile = \"default\"\ndays = [\"Funday\"]\n")
        .expect_err("refused");
    assert_eq!(
        day.to_string(),
        "rule 1 has day \"Funday\"; a day is Mon, Tue, Wed, Thu, Fri, Sat or Sun"
    );
}

#[test]
fn a_day_is_matched_case_insensitively_and_a_rule_may_name_nothing() {
    let read = parsed("[default]\nquiet = false\n[[rules]]\nprofile = \"default\"\ndays = [\"mon\"]\n")
        .expect("it parses");
    assert_eq!(read.rules[0].days, vec![1]);
    let bare = parsed("[default]\nquiet = false\n[[rules]]\nprofile = \"default\"\n")
        .expect("a rule may name no input");
    assert!(bare.rules[0].days.is_empty() && bare.rules[0].hours.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters config::profiles`
Expected: FAIL, `cannot find function parse_profiles`.

- [ ] **Step 3: Write minimal implementation**

Append to `pns/crates/pns-adapters/src/config/profiles.rs`:

```rust
use pns_domain::profiles::Rule;

/// `[profiles]` as a whole: the named profiles, the named networks, the
/// ordered rules and the one knob.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Profiles {
    pub profiles: BTreeMap<String, Profile>,
    pub locations: BTreeMap<String, String>,
    pub rules: Vec<Rule>,
    pub location_poll_secs: u64,
}

/// THIRTY SECONDS. A network change is observed within one poll, and a
/// `route` plus an `arp` twice a minute is two spawns nobody notices.
pub(super) const DEFAULT_LOCATION_POLL_SECS: u64 = 30;
const MIN_LOCATION_POLL_SECS: u64 = 5;
/// FIVE MINUTES at the top: past it a laptop can be on a new network for
/// longer than a meeting before the profile follows it.
const MAX_LOCATION_POLL_SECS: u64 = 300;

/// The seven weekday words, in `libc`'s own order, so the index IS `tm_wday`.
const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

pub(super) fn parse_profiles(value: toml::Value) -> Result<Profiles, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`profiles` is not a table".to_string()));
    };
    let mut read = Profiles {
        location_poll_secs: DEFAULT_LOCATION_POLL_SECS,
        ..Profiles::default()
    };
    let mut rules = None;
    for (key, setting) in table {
        match key.as_str() {
            "location_poll" => {
                read.location_poll_secs = nonzero_duration_key(
                    "profiles",
                    "location_poll",
                    &setting,
                    Duration::from_secs(MIN_LOCATION_POLL_SECS)
                        ..=Duration::from_secs(MAX_LOCATION_POLL_SECS),
                )?;
            }
            "locations" => read.locations = parse_locations(&setting)?,
            "rules" => rules = Some(setting),
            // EVERY OTHER KEY IS A PROFILE NAME, which is what makes the
            // table open: the names are the operator's own.
            name => {
                read.profiles
                    .insert(name.to_string(), parse_profile(name, &setting)?);
            }
        }
    }
    if let Some(rules) = rules {
        read.rules = parse_rules(&rules, &read.profiles)?;
    }
    Ok(read)
}

fn parse_locations(value: &toml::Value) -> Result<BTreeMap<String, String>, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(
            "`profiles.locations` is not a table".to_string(),
        ));
    };
    let mut locations = BTreeMap::new();
    for (name, fingerprint) in table {
        let text = fingerprint.as_str().ok_or_else(|| {
            ConfigError::Invalid(format!(
                "`profiles.locations` key `{name}` has type `{}`, not a string",
                fingerprint.type_str()
            ))
        })?;
        locations.insert(name.clone(), text.to_string());
    }
    Ok(locations)
}

fn parse_rules(
    value: &toml::Value,
    profiles: &BTreeMap<String, Profile>,
) -> Result<Vec<Rule>, ConfigError> {
    let toml::Value::Array(rows) = value else {
        return Err(ConfigError::Invalid(
            "`profiles.rules` is not a list of rules; write `[[profiles.rules]]`".to_string(),
        ));
    };
    let mut rules = Vec::with_capacity(rows.len());
    for (offset, row) in rows.iter().enumerate() {
        rules.push(parse_rule(offset + 1, row, profiles)?);
    }
    Ok(rules)
}

/// One rule, whose refusals count from ONE: the operator reading a refusal is
/// counting rows in their own file, not indexing an array.
fn parse_rule(
    index: usize,
    value: &toml::Value,
    profiles: &BTreeMap<String, Profile>,
) -> Result<Rule, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(format!("rule {index} is not a table")));
    };
    let mut rule = Rule::default();
    for (key, setting) in table {
        match key.as_str() {
            "profile" => rule.profile = rule_text(index, "profile", setting)?,
            "location" => rule.location = Some(rule_text(index, "location", setting)?),
            "focus" => rule.focus = Some(rule_text(index, "focus", setting)?),
            "hours" => {
                let stated = rule_text(index, "hours", setting)?;
                rule.hours = Some(pns_domain::lamps::window::parse_window(&stated).ok_or_else(
                    || {
                        ConfigError::Invalid(format!(
                            "rule {index} has hours {stated:?}, which is not a HH:MM-HH:MM window"
                        ))
                    },
                )?);
            }
            "days" => rule.days = parse_days(index, setting)?,
            "calendar_busy" => {
                rule.calendar_busy = Some(setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "rule {index} has a non-boolean `calendar_busy`"
                    ))
                })?);
            }
            other => {
                return Err(ConfigError::Invalid(format!(
                    "unknown rule key `{other}` in rule {index}; a rule serves calendar_busy, \
                     days, focus, hours, location, profile"
                )));
            }
        }
    }
    if rule.profile.is_empty() {
        return Err(ConfigError::Invalid(format!("rule {index} names no profile")));
    }
    if !profiles.contains_key(&rule.profile) {
        return Err(ConfigError::Invalid(format!(
            "rule {index} names profile `{}`, which no `[profiles.{}]` table defines",
            rule.profile, rule.profile
        )));
    }
    Ok(rule)
}

fn rule_text(index: usize, key: &str, setting: &toml::Value) -> Result<String, ConfigError> {
    setting
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| {
            ConfigError::Invalid(format!(
                "rule {index} has a `{key}` of type `{}`, not a string",
                setting.type_str()
            ))
        })
}

fn parse_days(index: usize, setting: &toml::Value) -> Result<Vec<u32>, ConfigError> {
    let toml::Value::Array(written) = setting else {
        return Err(ConfigError::Invalid(format!(
            "rule {index} has a `days` that is not a list of weekday names"
        )));
    };
    let mut days = Vec::with_capacity(written.len());
    for day in written {
        let stated = rule_text(index, "days", day)?;
        let found = DAYS
            .iter()
            .position(|known| known.eq_ignore_ascii_case(&stated))
            .ok_or_else(|| {
                ConfigError::Invalid(format!(
                    "rule {index} has day {stated:?}; a day is Mon, Tue, Wed, Thu, Fri, Sat or Sun"
                ))
            })?;
        days.push(u32::try_from(found).unwrap_or_default());
    }
    Ok(days)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters config::profiles`
Expected: PASS, nine tests.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src/config
SKIP_AI_COMMIT=1 git commit -m "feat(pns): parse the profile rules list and the named locations"
```

---

### Task 4: wire `[profiles]` into the schema, the load and the config model

**Files:**
- Modify: `pns/crates/pns-adapters/src/config/schema.rs`
- Modify: `pns/crates/pns-adapters/src/config/load.rs`
- Modify: `pns/crates/pns-adapters/src/config/model.rs`
- Modify: `pns/crates/pns-adapters/src/config/tests/loading.rs`

**Interfaces:**
- Consumes: `parse_profiles` and `Profiles` from Task 3.
- Produces: `Config::profiles: BTreeMap<String, Profile>`, `Config::profile_rules: Vec<Rule>`,
  `Config::profile_locations: BTreeMap<String, String>`, `Config::location_poll_secs: u64`.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns-adapters/src/config/tests/loading.rs`:

```rust
#[test]
fn the_profiles_table_reaches_the_config() {
    let config = parse_config(
        "[profiles.default]\nquiet = false\n\
         [profiles.night]\nquiet = true\nbanner = \"none\"\ndiscord = \"none\"\n\
         phone = \"priority\"\nlights = \"none\"\n\
         [[profiles.rules]]\nprofile = \"night\"\nhours = \"22:00-06:00\"\n",
    )
    .expect("it loads");
    assert_eq!(config.profiles.len(), 2);
    assert_eq!(config.profile_rules.len(), 1);
    assert_eq!(config.location_poll_secs, 30);
}

#[test]
fn a_config_with_no_profiles_table_has_the_default_profile_and_no_rules() {
    let config = parse_config("").expect("it loads");
    assert!(config.profiles.is_empty(), "no table names no profile");
    assert!(config.profile_rules.is_empty());
    assert_eq!(config.location_poll_secs, 30);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters config::tests::loading`
Expected: FAIL, `no field profiles on type Config`.

- [ ] **Step 3: Write minimal implementation**

In `pns/crates/pns-adapters/src/config/schema.rs`, add `"profiles"` to the `TOP_LEVEL` row's list,
keeping it alphabetical between `producer` and `quiet`, and add the roster row beside `quiet`'s:

```rust
    (
        crate::config::profiles::PROFILE_KEYS,
        &["banner", "discord", "lights", "phone", "quiet"],
    ),
    ("profiles", &["location_poll", "locations", "rules"]),
```

Add `"profiles.locations"` to `OPEN_TABLES` beside the other open rows, because its keys are the
operator's own network names.

In `pns/crates/pns-adapters/src/config/model.rs`, add to `Config`:

```rust
    /// `[profiles.<name>]`: the named delivery bundles, keyed by name.
    ///
    /// EMPTY IS EVERY MACHINE THAT NEVER WROTE THE TABLE, which resolves
    /// `default` and runs `Profile::default()`: today's behaviour exactly.
    pub profiles: BTreeMap<String, pns_domain::profiles::Profile>,
    /// `[profiles.locations]`: the named network fingerprints.
    pub profile_locations: BTreeMap<String, String>,
    /// `[[profiles.rules]]`: which profile is active, first match wins.
    pub profile_rules: Vec<pns_domain::profiles::Rule>,
    /// `[profiles] location_poll`: how often the gateway fingerprint is read.
    pub location_poll_secs: u64,
```

and set `location_poll_secs: DEFAULT_LOCATION_POLL_SECS` in `Config`'s hand-written `Default`, leaving
the other three empty.

In `pns/crates/pns-adapters/src/config/load.rs`, add the arm beside `"quiet"`:

```rust
            "profiles" => {
                let profiles = parse_profiles(value)?;
                config.profiles = profiles.profiles;
                config.profile_locations = profiles.locations;
                config.profile_rules = profiles.rules;
                config.location_poll_secs = profiles.location_poll_secs;
            }
```

In `pns/crates/pns-adapters/src/config/mod.rs`, export what the arm needs:

```rust
use profiles::{DEFAULT_LOCATION_POLL_SECS, PROFILE_KEYS, parse_profile, parse_profiles};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters config::`
Expected: PASS, including the existing schema walk over `TABLE_KEYS`.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src/config
SKIP_AI_COMMIT=1 git commit -m "feat(pns): load the profiles table into the config"
```

---

### Task 5: render the profiles tables and ship them

**Files:**
- Create: `pns/crates/pns-adapters/src/config/render/layout/profiles.rs`
- Create: `pns/crates/pns-adapters/src/config/render/profiles.rs`
- Modify: `pns/crates/pns-adapters/src/config/render/layout.rs`
- Modify: `pns/crates/pns-adapters/src/config/render.rs`
- Modify: `dot_config/pns/config-values.toml`
- Modify: `dot_config/pns/private_config.toml.tmpl` (regenerated, never by hand)

**Interfaces:**
- Consumes: the layout types `Table`, `Key`, `Sample` and the helpers `take_table`, `render_value`,
  `write_note` already in `render/write.rs`.
- Produces: `pub(super) fn render_profiles(out: &mut String, remaining: &mut toml::Table)
  -> Result<(), String>`.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns-adapters/src/config/render/tests/layout.rs`:

```rust
#[test]
fn the_three_profiles_and_the_rules_render_in_file_order() {
    let values: toml::Table = "[profiles.default]\nquiet = false\nbanner = \"all\"\n\
         [profiles.night]\nquiet = true\nbanner = \"none\"\n\
         [profiles.locations]\nhome = \"00:11:22:aa:bb:cc\"\n\
         [[profiles.rules]]\nprofile = \"night\"\nhours = \"22:00-06:00\"\n"
        .parse()
        .expect("values");
    let text = render(&values).expect("it renders");
    let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("no {needle}"));
    assert!(at("[profiles.default]") < at("[profiles.night]"), "named profiles come first");
    assert!(at("[profiles.night]") < at("[profiles.locations]"));
    assert!(at("[profiles.locations]") < at("[[profiles.rules]]"), "the rules come last");
    assert!(text.contains("home = \"00:11:22:aa:bb:cc\""));
    assert!(text.contains("hours = \"22:00-06:00\""));
    assert!(
        text.parse::<toml::Table>().is_ok(),
        "what it wrote has to load back"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters render::tests::layout`
Expected: FAIL, `unknown top-level key profiles`.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-adapters/src/config/render/layout/profiles.rs`:

```rust
use super::*;

/// The heading every named profile writes under, and the five keys each one
/// serves. A HARDCODED BRANCH writes the declarations, like the delivery
/// classes beside it, because the headings carry the operator's own names.
pub(super) const PROFILES: Table = Table {
    name: "profiles",
    prose: "# What reaches you, bundled by name. One profile is active at a time: the\n\
            # first matching rule below chooses it, or `pns profile <name>` does by\n\
            # hand. Each surface takes \"all\" (everything that surface would have\n\
            # shown), \"priority\" (pages on the [routes] urgent route and nothing\n\
            # else) or \"none\". A profile only ever SUBTRACTS from what presence\n\
            # already decided, so it can never card a phone you are not near, and a\n\
            # profile whose four surfaces are all \"none\" is refused: no profile can\n\
            # silence a page.\n",
    opt_in: false,
    children: &[],
    keys: &[Key {
        name: "location_poll",
        prose: "# How often the default gateway is fingerprinted, bounded \"5s\" to \"5m\".\n\
                # A network change moves the profile within one poll.\n",
        sample: Sample::Default("\"30s\""),
    }],
};

/// One `[profiles.<name>]` declaration's keys, in file order.
pub(super) const PROFILE: Table = Table {
    name: crate::config::profiles::PROFILE_KEYS,
    prose: "",
    opt_in: false,
    children: &[],
    keys: &[
        Key { name: "quiet", prose: "", sample: Sample::Default("false") },
        Key { name: "banner", prose: "", sample: Sample::Default("\"all\"") },
        Key { name: "discord", prose: "", sample: Sample::Default("\"all\"") },
        Key { name: "phone", prose: "", sample: Sample::Default("\"all\"") },
        Key { name: "lights", prose: "", sample: Sample::Default("\"all\"") },
    ],
};

/// The prose above the two tables the branch writes itself.
pub(super) const LOCATIONS_PROSE: &str =
    "# A named network, fingerprinted by the default gateway's hardware address\n\
     # rather than the Wi-Fi name, which macOS will not hand over without\n\
     # Location Services. `pns profile learn <name>` prints the row to paste here.\n";
pub(super) const RULES_PROSE: &str =
    "# Which profile is active, first match wins. An input this machine cannot\n\
     # read matches nothing, so an unreadable probe falls through to `default`\n\
     # rather than silencing anything. A rule naming no input matches always.\n";
```

Register it in `pns/crates/pns-adapters/src/config/render/layout.rs`: `mod profiles;`,
`use profiles::{LOCATIONS_PROSE, PROFILE, PROFILES, RULES_PROSE};`, and add `PROFILES` to `LAYOUT`
after the `[quiet.calendar]` row so the profiles ship below the mute settings they extend.

Create `pns/crates/pns-adapters/src/config/render/profiles.rs`:

```rust
use super::*;

/// Every `[profiles.*]` heading, in file order: the named profiles in sorted
/// order, then the locations, then the rules as an ARRAY OF TABLES.
///
/// A HARDCODED BRANCH, like `render_delivery_classes`: the profile names are
/// the operator's own, and `[[profiles.rules]]` is the layout's first array of
/// tables, which a `Table` row cannot describe.
pub(super) fn render_profiles(out: &mut String, remaining: &mut toml::Table) -> Result<(), String> {
    let mut table = take_table(remaining, "profiles")?;
    let locations = take_table(&mut table, "locations")?;
    let rules = match table.remove("rules") {
        None => Vec::new(),
        Some(toml::Value::Array(rows)) => rows,
        Some(other) => {
            return Err(format!(
                "`profiles.rules` has type `{}`, not a list of rules",
                other.type_str()
            ));
        }
    };
    let poll = table.remove("location_poll");
    render_block(out, &PROFILES, &mut named_only(poll), true)?;
    for (name, entry) in table {
        let toml::Value::Table(mut settings) = entry else {
            return Err(format!("`profiles.{name}` is not a table"));
        };
        write_note(out, take_note(&mut settings)?);
        out.push_str(&format!("[profiles.{name}]\n"));
        for key in PROFILE.keys {
            let literal = match settings.remove(key.name) {
                Some(value) => render_value(&value)
                    .map_err(|error| format!("`profiles.{name}` key `{}`: {error}", key.name))?,
                None => match key.sample {
                    Sample::Default(literal) | Sample::Example(literal) => literal.to_string(),
                },
            };
            out.push_str(&format!("{} = {literal}\n", key.name));
        }
        out.push('\n');
        if let Some(key) = settings.keys().next() {
            return Err(format!("unknown `profiles.{name}` key `{key}`"));
        }
    }
    if !locations.is_empty() {
        out.push_str(LOCATIONS_PROSE);
        out.push_str("[profiles.locations]\n");
        for (name, fingerprint) in locations {
            let literal = render_value(&fingerprint)
                .map_err(|error| format!("`profiles.locations` key `{name}`: {error}"))?;
            out.push_str(&format!("{name} = {literal}\n"));
        }
        out.push('\n');
    }
    if !rules.is_empty() {
        out.push_str(RULES_PROSE);
    }
    for row in rules {
        let toml::Value::Table(settings) = row else {
            return Err("`profiles.rules` holds something that is not a rule".to_string());
        };
        out.push_str("[[profiles.rules]]\n");
        // `profile` FIRST and the inputs after it, in the map's own sorted
        // order, which is what makes a regenerated template byte-stable.
        if let Some(named) = settings.get("profile") {
            out.push_str(&format!("profile = {}\n", render_value(named)?));
        }
        for (key, value) in &settings {
            if key == "profile" {
                continue;
            }
            out.push_str(&format!("{key} = {}\n", render_value(value)?));
        }
        out.push('\n');
    }
    Ok(())
}

/// The `[profiles]` heading's own settings: the one knob, and nothing the
/// named tables below it carry.
fn named_only(poll: Option<toml::Value>) -> toml::Table {
    let mut settings = toml::Table::new();
    if let Some(poll) = poll {
        settings.insert("location_poll".to_string(), poll);
    }
    settings
}
```

In `pns/crates/pns-adapters/src/config/render.rs`, declare the module and add the branch beside the
delivery-class one:

```rust
mod profiles;
use profiles::render_profiles;
```

```rust
        } else if table.name == "profiles" {
            render_profiles(&mut out, &mut remaining)?;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters render::`
Expected: PASS.

- [ ] **Step 5: Ship the three profiles and regenerate**

Append to `dot_config/pns/config-values.toml`, after the `[quiet.calendar]` block:

```toml
# Today's behaviour, which is also what a machine with no profiles at all has.
[profiles.default]
quiet = false
banner = "all"
discord = "all"
phone = "all"
lights = "all"

# At the desk on somebody else's clock: the screen in front of you still
# talks, the phone only for a page, and nothing pulses or posts.
[profiles.work]
quiet = true
banner = "all"
discord = "none"
phone = "priority"
lights = "none"

# Asleep. Only a page gets through, and everything else is held and delivered
# as one roll-up when the profile next admits it.
[profiles.night]
quiet = true
banner = "none"
discord = "none"
phone = "priority"
lights = "none"

[[profiles.rules]]
profile = "night"
hours = "22:00-06:00"

[[profiles.rules]]
profile = "work"
days = ["Mon", "Tue", "Wed", "Thu", "Fri"]
hours = "09:00-17:00"
```

Run: `just pns-config-render`
Then: `just test-rust` and `just lint-check`
Expected: both pass, and `git diff --stat` shows `dot_config/pns/private_config.toml.tmpl` changed.

- [ ] **Step 6: Commit**

```bash
git add pns/crates/pns-adapters/src/config dot_config/pns
SKIP_AI_COMMIT=1 git commit -m "feat(pns): ship the three profiles and their rules"
```

---
## Slice 2 tasks

### Task 6: the override row's codec

**Files:**
- Create: `pns/crates/pns-domain/src/profiles/codec.rs`
- Create: `pns/crates/pns-domain/src/profiles/codec/tests.rs`
- Modify: `pns/crates/pns-domain/src/profiles.rs`

**Interfaces:**
- Consumes: `Override` from Task 1.
- Produces: `pns_domain::profiles::{parse_override, format_override}`, with
  `parse_override(body: &str) -> Option<Override>` and `format_override(&Override) -> String`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-domain/src/profiles/codec/tests.rs`:

```rust
use super::*;

#[test]
fn a_bare_name_is_an_override_that_stands_until_cleared() {
    assert_eq!(
        parse_override("night"),
        Some(Override { profile: "night".to_string(), until: None })
    );
    assert_eq!(parse_override("night\n"), parse_override("night"), "one trailing newline");
}

#[test]
fn a_name_and_an_epoch_is_a_bounded_override() {
    assert_eq!(
        parse_override("night 1758420600"),
        Some(Override { profile: "night".to_string(), until: Some(1_758_420_600) })
    );
}

#[test]
fn anything_else_reads_as_no_override_at_all() {
    assert_eq!(parse_override(""), None);
    assert_eq!(parse_override("   "), None);
    assert_eq!(parse_override("night tomorrow"), None, "an expiry is an epoch second");
    assert_eq!(parse_override("night 1758420600 extra"), None);
    assert_eq!(parse_override("night\nwork"), None, "one line, not a list");
}

#[test]
fn what_it_writes_it_reads_back() {
    for standing in [
        Override { profile: "work".to_string(), until: None },
        Override { profile: "work".to_string(), until: Some(42) },
    ] {
        assert_eq!(parse_override(&format_override(&standing)), Some(standing));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::codec`
Expected: FAIL, the module does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-domain/src/profiles/codec.rs`:

```rust
use super::Override;

/// The stored override, out of the row's body.
///
/// ONE ROW RATHER THAN TWO, for `mute::expiry_from_state`'s own reason: a name
/// and an expiry in two rows are two values that can disagree about whether an
/// override is standing.
///
/// THE ONLY LENIENCY IS THE ONE TRAILING NEWLINE the writer itself may leave.
/// Anything else was written by another hand, and the fail-open rule is that
/// an unreadable body reads as NO override, which puts the rules back in
/// charge rather than pinning the machine to a profile nobody can see.
pub fn parse_override(body: &str) -> Option<Override> {
    let held = body.strip_suffix('\n').unwrap_or(body);
    if held.is_empty() || held.contains('\n') {
        return None;
    }
    let mut words = held.split(' ');
    let profile = words.next().filter(|name| !name.is_empty())?.to_string();
    let until = match words.next() {
        None => None,
        Some(epoch) => Some(crate::count::parse_count(epoch)?),
    };
    words.next().is_none().then_some(Override { profile, until })
}

/// The row's body for an override, which `parse_override` reads back exactly.
pub fn format_override(standing: &Override) -> String {
    match standing.until {
        None => standing.profile.clone(),
        Some(until) => format!("{} {until}", standing.profile),
    }
}

#[cfg(test)]
mod tests;
```

Add to `pns/crates/pns-domain/src/profiles.rs`:

```rust
mod codec;
pub use codec::{format_override, parse_override};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::codec`
Expected: PASS, four tests.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): read and write the profile override row"
```

---

### Task 7: the resolver

**Files:**
- Create: `pns/crates/pns-domain/src/profiles/resolve.rs`
- Create: `pns/crates/pns-domain/src/profiles/resolve/tests.rs`
- Modify: `pns/crates/pns-domain/src/profiles.rs`

**Interfaces:**
- Consumes: `Rule`, `Override`, `DEFAULT_PROFILE` from Task 1.
- Produces: `pns_domain::profiles::{Inputs, Chose, Resolved, resolve}` with
  `resolve(rules: &[Rule], inputs: &Inputs, standing: Option<&Override>, now_secs: Option<u64>)
  -> Resolved`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-domain/src/profiles/resolve/tests.rs`:

```rust
use super::*;
use crate::lamps::window::parse_window;

fn rule(profile: &str) -> Rule {
    Rule { profile: profile.to_string(), ..Rule::default() }
}

fn inputs() -> Inputs {
    Inputs {
        weekday: Some(3),
        minutes_of_day: Some(10 * 60),
        location: Some("home".to_string()),
        focus: Some("Work".to_string()),
        calendar_busy: Some(false),
    }
}

#[test]
fn each_input_matches_and_fails_to_match() {
    let cases: Vec<(Rule, bool)> = vec![
        (Rule { days: vec![3], ..rule("work") }, true),
        (Rule { days: vec![0, 6], ..rule("work") }, false),
        (Rule { hours: parse_window("09:00-17:00"), ..rule("work") }, true),
        (Rule { hours: parse_window("22:00-06:00"), ..rule("work") }, false),
        (Rule { location: Some("home".to_string()), ..rule("work") }, true),
        (Rule { location: Some("office".to_string()), ..rule("work") }, false),
        (Rule { focus: Some("Work".to_string()), ..rule("work") }, true),
        (Rule { focus: Some("Sleep".to_string()), ..rule("work") }, false),
        (Rule { calendar_busy: Some(false), ..rule("work") }, true),
        (Rule { calendar_busy: Some(true), ..rule("work") }, false),
    ];
    for (rule, matches) in cases {
        let resolved = resolve(std::slice::from_ref(&rule), &inputs(), None, Some(0));
        assert_eq!(
            resolved.profile == "work",
            matches,
            "{rule:?} should {}match",
            if matches { "" } else { "not " }
        );
    }
}

#[test]
fn an_unreadable_input_matches_nothing_and_falls_through_to_default() {
    let unread = Inputs {
        weekday: None,
        minutes_of_day: None,
        location: None,
        focus: None,
        calendar_busy: None,
    };
    for rule in [
        Rule { days: vec![3], ..rule("work") },
        Rule { hours: parse_window("09:00-17:00"), ..rule("work") },
        Rule { location: Some("home".to_string()), ..rule("work") },
        Rule { focus: Some("Work".to_string()), ..rule("work") },
        Rule { calendar_busy: Some(false), ..rule("work") },
    ] {
        let resolved = resolve(&[rule], &unread, None, Some(0));
        assert_eq!(resolved.profile, DEFAULT_PROFILE);
        assert_eq!(resolved.chose, Chose::Fallback);
    }
}

#[test]
fn the_first_matching_rule_wins_and_the_names_of_its_inputs_come_out() {
    let rules = [
        Rule { days: vec![0], ..rule("night") },
        Rule { days: vec![3], hours: parse_window("09:00-17:00"), ..rule("work") },
        rule("default"),
    ];
    let resolved = resolve(&rules, &inputs(), None, Some(0));
    assert_eq!(resolved.profile, "work");
    assert_eq!(resolved.chose, Chose::Rule { index: 2 }, "the index counts from one");
    assert_eq!(resolved.matched, vec!["days", "hours"]);
}

#[test]
fn a_rule_naming_no_input_matches_always_and_no_rule_matching_is_default() {
    let always = resolve(&[rule("work")], &inputs(), None, Some(0));
    assert_eq!(always.profile, "work");
    assert!(always.matched.is_empty(), "it matched on nothing, which is why it matched");
    let none = resolve(&[], &inputs(), None, Some(0));
    assert_eq!(none.profile, DEFAULT_PROFILE);
    assert_eq!(none.chose, Chose::Fallback);
}

#[test]
fn a_rule_naming_two_inputs_does_not_match_when_one_of_them_does_not() {
    let rules = [Rule { days: vec![3], focus: Some("Sleep".to_string()), ..rule("work") }];
    assert_eq!(resolve(&rules, &inputs(), None, Some(0)).profile, DEFAULT_PROFILE);
}

#[test]
fn an_override_beats_every_rule_until_it_expires() {
    let rules = [rule("work")];
    let standing = Override { profile: "night".to_string(), until: Some(100) };
    let inside = resolve(&rules, &inputs(), Some(&standing), Some(99));
    assert_eq!(inside.profile, "night");
    assert_eq!(inside.chose, Chose::Manual { until: Some(100) });
    let at_the_second = resolve(&rules, &inputs(), Some(&standing), Some(100));
    assert_eq!(at_the_second.profile, "work", "the expiry second is already over");
}

#[test]
fn an_unbounded_override_stands_and_an_unreadable_clock_does_not_end_one() {
    let rules = [rule("work")];
    let forever = Override { profile: "night".to_string(), until: None };
    assert_eq!(resolve(&rules, &inputs(), Some(&forever), Some(9_999)).profile, "night");
    let bounded = Override { profile: "night".to_string(), until: Some(1) };
    assert_eq!(
        resolve(&rules, &inputs(), Some(&bounded), None).profile,
        "night",
        "a clock nobody could read cannot say it expired"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::resolve`
Expected: FAIL, the module does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-domain/src/profiles/resolve.rs`:

```rust
use super::{DEFAULT_PROFILE, Override, Rule};

/// Everything the choice rests on, read ONCE by the composition root and
/// passed in. `None` is a reading nobody could take, which is never the same
/// as a reading that did not match.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inputs {
    /// The local weekday, 0 for Sunday, as `libc` numbers it.
    pub weekday: Option<u32>,
    /// Minutes since local midnight.
    pub minutes_of_day: Option<u16>,
    /// The NAME `[profiles.locations]` gives this network, never a fingerprint.
    pub location: Option<String>,
    /// The asserted macOS Focus mode's display name.
    pub focus: Option<String>,
    pub calendar_busy: Option<bool>,
}

/// What chose the active profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chose {
    /// The operator typed it. `until` is the epoch it ends at, if it has one.
    Manual { until: Option<u64> },
    /// A rule, counted from ONE, the way the load refusals count.
    Rule { index: usize },
    /// No rule matched, which is `DEFAULT_PROFILE`.
    Fallback,
}

/// The answer, with enough beside it to say why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub profile: String,
    pub chose: Chose,
    /// The names of the inputs the winning rule matched on, in rule order.
    pub matched: Vec<&'static str>,
}

/// The active profile. A PURE FUNCTION: no clock, no file, no probe and no
/// environment inside it, which is what puts the whole rule matrix under unit
/// test with no daemon.
pub fn resolve(
    rules: &[Rule],
    inputs: &Inputs,
    standing: Option<&Override>,
    now_secs: Option<u64>,
) -> Resolved {
    if let Some(standing) = standing.filter(|standing| still_standing(standing, now_secs)) {
        return Resolved {
            profile: standing.profile.clone(),
            chose: Chose::Manual { until: standing.until },
            matched: Vec::new(),
        };
    }
    for (offset, rule) in rules.iter().enumerate() {
        if let Some(matched) = matches(rule, inputs) {
            return Resolved {
                profile: rule.profile.clone(),
                chose: Chose::Rule { index: offset + 1 },
                matched,
            };
        }
    }
    Resolved {
        profile: DEFAULT_PROFILE.to_string(),
        chose: Chose::Fallback,
        matched: Vec::new(),
    }
}

/// HALF OPEN, `mute::is_muted`'s own rule: an override ends when it says it
/// does. A clock nobody could read leaves it standing, because not knowing
/// cannot be the thing that ends something the operator typed by hand.
fn still_standing(standing: &Override, now_secs: Option<u64>) -> bool {
    match (standing.until, now_secs) {
        (Some(until), Some(now)) => now < until,
        _ => true,
    }
}

/// The names of the inputs this rule matched on, or None for a rule that did
/// not match. A rule naming NO input matches on an empty list, which is why
/// the two cases cannot be one `Vec`.
fn matches(rule: &Rule, inputs: &Inputs) -> Option<Vec<&'static str>> {
    let mut matched = Vec::new();
    if !rule.days.is_empty() {
        let weekday = inputs.weekday?;
        if !rule.days.contains(&weekday) {
            return None;
        }
        matched.push("days");
    }
    if let Some(window) = rule.hours.as_ref() {
        let minutes = inputs.minutes_of_day?;
        // THE LAMPS' OWN WINDOW, which already joins the two ends of a
        // midnight-crossing window and already has its tests.
        if !crate::lamps::window::quiet_now(Some(window), Some(minutes)) {
            return None;
        }
        matched.push("hours");
    }
    if let Some(named) = rule.location.as_deref() {
        if inputs.location.as_deref()? != named {
            return None;
        }
        matched.push("location");
    }
    if let Some(named) = rule.focus.as_deref() {
        if inputs.focus.as_deref()? != named {
            return None;
        }
        matched.push("focus");
    }
    if let Some(wanted) = rule.calendar_busy {
        if inputs.calendar_busy? != wanted {
            return None;
        }
        matched.push("calendar_busy");
    }
    Some(matched)
}

#[cfg(test)]
mod tests;
```

Add to `pns/crates/pns-domain/src/profiles.rs`:

```rust
mod resolve;
pub use resolve::{Chose, Inputs, Resolved, resolve};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::resolve`
Expected: PASS, seven tests.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): the profile resolver as a pure function"
```

---

### Task 8: the report lines

**Files:**
- Create: `pns/crates/pns-domain/src/profiles/report.rs`
- Create: `pns/crates/pns-domain/src/profiles/report/tests.rs`
- Modify: `pns/crates/pns-domain/src/profiles.rs`

**Interfaces:**
- Consumes: `Chose`, `Profile`, `Admits` from Tasks 1 and 7.
- Produces: `pns_domain::profiles::{because, surfaces_line, transition_line}` with
  `because(chose: &Chose, matched: &[&str], until: Option<&str>) -> String`,
  `surfaces_line(&Profile) -> String`,
  `transition_line(from: &str, to: &str, reason: &str) -> String`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-domain/src/profiles/report/tests.rs`:

```rust
use super::*;

#[test]
fn every_way_a_profile_can_be_chosen_reads_back() {
    assert_eq!(because(&Chose::Fallback, &[], None), "no rule matched");
    assert_eq!(because(&Chose::Rule { index: 2 }, &["days", "hours"], None), "rule 2: days, hours");
    assert_eq!(because(&Chose::Rule { index: 1 }, &[], None), "rule 1: every time");
    assert_eq!(because(&Chose::Manual { until: None }, &[], None), "manual, until cleared");
    assert_eq!(
        because(&Chose::Manual { until: Some(1) }, &[], Some("17:30")),
        "manual, until 17:30"
    );
    assert_eq!(
        because(&Chose::Manual { until: Some(1) }, &[], None),
        "manual, until an hour this machine cannot read",
        "an unreadable clock never prints a time it made up"
    );
}

#[test]
fn the_surfaces_read_in_one_line() {
    assert_eq!(
        surfaces_line(&Profile::default()),
        "quiet off; banner all, Discord all, phone all, lights all"
    );
    assert_eq!(
        surfaces_line(&Profile {
            quiet: true,
            banner: Admits::None,
            discord: Admits::None,
            phone: Admits::Priority,
            lights: Admits::None,
        }),
        "quiet on; banner none, Discord none, phone priority, lights none"
    );
}

#[test]
fn a_transition_names_both_profiles_and_the_reason() {
    assert_eq!(
        transition_line("default", "night", "rule 1: hours"),
        "profile default -> night (rule 1: hours)"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::report`
Expected: FAIL, the module does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-domain/src/profiles/report.rs`:

```rust
use super::{Admits, Chose, Profile};

/// Why this profile is the active one, in one clause.
///
/// `until` IS HANDED IN ALREADY RENDERED, because a local time is a system
/// fact this crate has no business reading. A bounded override with no
/// rendered time says so rather than printing an epoch second at an operator.
pub fn because(chose: &Chose, matched: &[&str], until: Option<&str>) -> String {
    match chose {
        Chose::Fallback => "no rule matched".to_string(),
        Chose::Rule { index } if matched.is_empty() => format!("rule {index}: every time"),
        Chose::Rule { index } => format!("rule {index}: {}", matched.join(", ")),
        Chose::Manual { until: None } => "manual, until cleared".to_string(),
        Chose::Manual { .. } => match until {
            Some(clock) => format!("manual, until {clock}"),
            None => "manual, until an hour this machine cannot read".to_string(),
        },
    }
}

/// The hush and the four surfaces, in the order the config writes them.
pub fn surfaces_line(profile: &Profile) -> String {
    let hush = if profile.quiet { "on" } else { "off" };
    format!(
        "quiet {hush}; banner {}, Discord {}, phone {}, lights {}",
        profile.banner.word(),
        profile.discord.word(),
        profile.phone.word(),
        profile.lights.word()
    )
}

/// One transition, as the ledger row and the gateway's own line both carry it.
///
/// ONE SPELLING FOR BOTH, so the row an operator reads back and the line they
/// saw scroll past cannot describe the same moment differently.
pub fn transition_line(from: &str, to: &str, reason: &str) -> String {
    format!("profile {from} -> {to} ({reason})")
}

#[cfg(test)]
mod tests;
```

Add to `pns/crates/pns-domain/src/profiles.rs`:

```rust
mod report;
pub use report::{because, surfaces_line, transition_line};
```

Add `use super::Admits;` to the report tests' scope by re-exporting `Admits` from `profiles.rs`, which
Task 1 already does.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::report`
Expected: PASS, three tests.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): the profile report and transition lines"
```

---

### Task 9: store the override

**Files:**
- Create: `pns/crates/pns-adapters/src/persistence/sqlite/profiles.rs`
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/{mod.rs,scalar.rs,migrations.rs}`
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/tests.rs`

**Interfaces:**
- Consumes: `parse_override` and `format_override` from Task 6.
- Produces: `SqliteStore::profile_override(&self) -> Result<Option<Override>, StoreError>` and
  `SqliteStore::set_profile_override(&self, standing: Option<&Override>) -> Result<(), StoreError>`.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns-adapters/src/persistence/sqlite/tests.rs`:

```rust
#[test]
fn the_profile_override_round_trips_and_clears() {
    let store = SqliteStore::new(state());
    assert_eq!(store.profile_override().expect("a read"), None);
    let standing = pns_domain::profiles::Override {
        profile: "night".to_string(),
        until: Some(1_758_420_600),
    };
    store.set_profile_override(Some(&standing)).expect("a write");
    assert_eq!(store.profile_override().expect("a read"), Some(standing));
    store.set_profile_override(None).expect("a clear");
    assert_eq!(store.profile_override().expect("a read"), None);
}

#[test]
fn a_body_nothing_can_read_is_no_override_rather_than_an_error() {
    let store = SqliteStore::new(state());
    store.write_profile_override_body("night tomorrow").expect("a write");
    assert_eq!(
        store.profile_override().expect("a read"),
        None,
        "an unreadable body puts the rules back in charge"
    );
}
```

`state()` is the file's own path fixture, already there at the head of `tests.rs`, and
`write_profile_override_body` is the `#[cfg(test)]`-gated raw writer added in Step 3.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters the_profile_override`
Expected: FAIL, `no method named profile_override`.

- [ ] **Step 3: Write minimal implementation**

In `scalar.rs`, add the variant and its two names:

```rust
    Profile,
```
```rust
            Self::Profile => "profile-override",
```
```rust
            Self::Profile => "profile_override",
```

In `migrations.rs`, raise `VERSION` to `12` and add the arm at the end of the ladder:

```rust
    if version < 12 {
        super::profiles::create(&transaction)?;
    }
```

Create `pns/crates/pns-adapters/src/persistence/sqlite/profiles.rs`:

```rust
//! The profile's own durable state: the operator's override.
//!
//! THE OVERRIDE IS ONE ROW ON THE SCALAR MODEL the quiet expiry already uses,
//! for that row's own reason: a name and an expiry in two rows are two values
//! that can disagree about whether an override is standing.

use super::{SqliteStore, StoreError, scalar::Scalar};
use pns_domain::profiles::{Override, format_override, parse_override};
use rusqlite::Transaction;

pub(in crate::persistence::sqlite) fn create(
    transaction: &Transaction<'_>,
) -> Result<(), StoreError> {
    transaction.execute_batch(
        "CREATE TABLE profile_override (id INTEGER PRIMARY KEY CHECK(id = 1), body TEXT NOT NULL);",
    )?;
    Ok(())
}

impl SqliteStore {
    /// The standing override, or None where there is none and where the body
    /// could not be read.
    ///
    /// AN UNREADABLE BODY IS NO OVERRIDE. The rules then decide, which is the
    /// fail-open direction every other reading here takes: not knowing costs
    /// the narrowing, never the notification.
    pub fn profile_override(&self) -> Result<Option<Override>, StoreError> {
        Ok(Scalar::Profile
            .stored(&self.connect()?)?
            .as_deref()
            .and_then(parse_override))
    }

    /// Set the override, or clear it with `None`.
    pub fn set_profile_override(&self, standing: Option<&Override>) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            Scalar::Profile.write(transaction, standing.map(format_override).as_deref())
        })
    }

    #[cfg(test)]
    pub(super) fn write_profile_override_body(&self, body: &str) -> Result<(), StoreError> {
        self.transaction(|transaction| Scalar::Profile.write(transaction, Some(body)))
    }
}
```

In `mod.rs`, add `mod profiles;` in alphabetical order.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters persistence::sqlite`
Expected: PASS, including the existing migration tests at the new version.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src/persistence/sqlite
SKIP_AI_COMMIT=1 git commit -m "feat(pns): store the profile override"
```

---

### Task 10: `pns profile`

**Files:**
- Create: `pns/crates/pns/src/command_profile.rs`
- Create: `pns/crates/pns/src/command_profile/tests.rs`
- Create: `pns/crates/pns/src/profile_runtime.rs`
- Modify: `pns/crates/pns/src/{lib.rs,invocation.rs,subcommand_usage.rs}`

**Interfaces:**
- Consumes: everything from Tasks 1, 6, 7, 8 and 9.
- Produces: `pub(crate) fn profile_mode() -> i32`,
  `pub(crate) fn parse_bound(words: &[String], now_secs: Option<u64>, minutes_now: Option<u16>)
  -> Result<Option<u64>, String>`, and in `profile_runtime`:
  `pub(crate) struct Reading { pub resolved: Resolved, pub profile: Profile,
  pub until_clock: Option<String> }`, `pub(crate) fn active(records: &SqliteStore) -> Reading`,
  `pub(crate) fn defined_profiles() -> Vec<String>`,
  `pub(crate) fn minutes_now() -> Option<u16>`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns/src/command_profile/tests.rs`:

```rust
use super::*;

fn words(argv: &[&str]) -> Vec<String> {
    argv.iter().map(|word| word.to_string()).collect()
}

#[test]
fn no_bound_is_an_override_that_stands_until_cleared() {
    assert_eq!(parse_bound(&words(&[]), Some(1_000), Some(600)), Ok(None));
}

#[test]
fn a_duration_bound_is_added_to_the_clock() {
    assert_eq!(
        parse_bound(&words(&["--for", "2h"]), Some(1_000), Some(600)),
        Ok(Some(1_000 + 2 * 3_600))
    );
}

#[test]
fn a_clock_time_that_has_passed_today_means_tomorrow() {
    // 10:00 local, asked to run until 09:00: that is nine tomorrow morning.
    let until = parse_bound(&words(&["--until", "09:00"]), Some(10_000), Some(10 * 60))
        .expect("a bound")
        .expect("a time");
    assert_eq!(until, 10_000 + 23 * 3_600, "twenty three hours from now");
    let later = parse_bound(&words(&["--until", "17:30"]), Some(10_000), Some(10 * 60))
        .expect("a bound")
        .expect("a time");
    assert_eq!(later, 10_000 + 7 * 3_600 + 30 * 60);
}

#[test]
fn a_bad_bound_says_what_is_wrong() {
    assert!(parse_bound(&words(&["--for", "900h"]), Some(0), Some(0)).is_err());
    assert!(parse_bound(&words(&["--until", "25:00"]), Some(0), Some(0)).is_err());
    assert!(parse_bound(&words(&["--for"]), Some(0), Some(0)).is_err());
    assert!(parse_bound(&words(&["--soon", "2h"]), Some(0), Some(0)).is_err());
}

#[test]
fn a_bound_needs_a_clock() {
    assert!(parse_bound(&words(&["--for", "2h"]), None, None).is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns command_profile`
Expected: FAIL, the module does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns/src/command_profile.rs`:

```rust
use crate::*;
use pns_adapters::SqliteStore;
use pns_domain::profiles::{Override, because, resolve, surfaces_line};

pub(crate) const PROFILE_USAGE: &str =
    "usage: pns profile [<name> [--for <duration> | --until HH:MM] | clear]";

/// The `profile` mode: which bundle of delivery settings is active.
///
/// TYPED BY HAND AND NEVER A HOOK, so a typo is a refusal rather than a
/// silent fallthrough, exactly as `pns mute` argues for itself.
///
/// THE REPORT IS READ BACK from the store after whatever was asked for, so the
/// line cannot claim a profile that never landed.
pub(crate) fn profile_mode() -> i32 {
    let argv: Vec<String> = crate::arguments_after_subcommand();
    let records = SqliteStore::for_records(state_dir());
    match argv.split_first() {
        None => report(&records),
        Some((word, rest)) if word == "clear" && rest.is_empty() => {
            if let Err(error) = records.set_profile_override(None) {
                eprintln!(
                    "pns: state error (the profile override could not be written: {error}); \
                     the profile was not changed"
                );
                return 1;
            }
            report(&records)
        }
        Some((name, rest)) => select(&records, name, rest),
    }
}

fn select(records: &SqliteStore, name: &str, rest: &[String]) -> i32 {
    let defined = crate::profile_runtime::defined_profiles();
    if !defined.contains(&name.to_string()) {
        eprintln!(
            "pns profile: no profile named `{name}`; this config defines {}",
            defined.join(", ")
        );
        return 2;
    }
    let until = match parse_bound(rest, now_secs(), crate::profile_runtime::minutes_now()) {
        Ok(until) => until,
        Err(refusal) => {
            eprintln!("{refusal}");
            eprintln!("{PROFILE_USAGE}");
            return 2;
        }
    };
    let standing = Override { profile: name.to_string(), until };
    if let Err(error) = records.set_profile_override(Some(&standing)) {
        eprintln!(
            "pns: state error (the profile override could not be written: {error}); \
             the profile was not changed"
        );
        return 1;
    }
    report(records)
}

/// The three lines `pns profile` prints.
fn report(records: &SqliteStore) -> i32 {
    let reading = crate::profile_runtime::active(records);
    println!(
        "pns: profile `{}` ({})",
        reading.resolved.profile,
        because(
            &reading.resolved.chose,
            &reading.resolved.matched,
            reading.until_clock.as_deref()
        )
    );
    println!("     {}", surfaces_line(&reading.profile));
    0
}

/// `--for <duration>` or `--until HH:MM`, or no bound at all.
///
/// `--until` PAST TODAY'S CLOCK MEANS TOMORROW. Asked at 17:00 to run until
/// 09:00, the operator means nine in the morning; refusing it would make them
/// do date arithmetic to say something unambiguous.
pub(crate) fn parse_bound(
    words: &[String],
    now_secs: Option<u64>,
    minutes_now: Option<u16>,
) -> Result<Option<u64>, String> {
    let [flag, value] = words else {
        if words.is_empty() {
            return Ok(None);
        }
        return Err(format!("pns profile: {PROFILE_USAGE}"));
    };
    let (now, minutes) = match (now_secs, minutes_now) {
        (Some(now), Some(minutes)) => (now, minutes),
        _ => return Err("pns: state error (the clock cannot be read); no bound was set".to_string()),
    };
    match flag.as_str() {
        "--for" => {
            let held = pns_domain::duration::parse_duration(
                "profile duration",
                value,
                pns_domain::mute::MUTE_RANGE,
            )?;
            Ok(Some(now.saturating_add(held.as_secs())))
        }
        "--until" => {
            let wanted = pns_domain::profiles::minute_of_clock(value)
                .ok_or_else(|| format!("pns profile: {value:?} is not an HH:MM time of day"))?;
            let ahead = if wanted > minutes {
                u64::from(wanted - minutes)
            } else {
                u64::from(1440 + u32::from(wanted) - u32::from(minutes))
            };
            Ok(Some(now.saturating_add(ahead * 60)))
        }
        _ => Err(format!("pns profile: {PROFILE_USAGE}")),
    }
}

#[cfg(test)]
mod tests;
```

Create `pns/crates/pns/src/profile_runtime.rs`, the composition root's own read:

```rust
//! The composition root's read of the active profile: config, store and
//! probes in, one resolved answer out.
//!
//! EVERY PROBE IS READ ONCE HERE and handed to the resolver as a plain value,
//! which is what keeps the resolver pure and makes the whole rule matrix
//! testable with no daemon.

use crate::*;
use pns_adapters::SqliteStore;
use pns_domain::profiles::{Inputs, Profile, Resolved, resolve};

/// What one read answered: the resolution, the profile it names, and the
/// local clock time a bounded override ends at.
pub(crate) struct Reading {
    pub resolved: Resolved,
    pub profile: Profile,
    pub until_clock: Option<String>,
}

pub(crate) fn active(records: &SqliteStore) -> Reading {
    let config = loaded_config();
    let now = now_secs();
    let standing = records.profile_override().ok().flatten();
    let resolved = resolve(
        &config.profile_rules,
        &inputs(now),
        standing.as_ref(),
        now,
    );
    // A NAME NO TABLE DEFINES RUNS THE DEFAULT PROFILE rather than nothing: a
    // stored override can outlive the table it named, and the fail-open rule
    // is that not knowing costs the narrowing, never the notification.
    let profile = config
        .profiles
        .get(&resolved.profile)
        .cloned()
        .unwrap_or_default();
    let until_clock = match &resolved.chose {
        pns_domain::profiles::Chose::Manual { until: Some(until) } => {
            pns_adapters::local_minutes_since_midnight(*until).map(clock)
        }
        _ => None,
    };
    Reading { resolved, profile, until_clock }
}

/// Every profile the config defines, in sorted order, which is what a refusal
/// lists back at an operator who typed a name it does not serve.
pub(crate) fn defined_profiles() -> Vec<String> {
    loaded_config().profiles.keys().cloned().collect()
}

pub(crate) fn minutes_now() -> Option<u16> {
    now_secs().and_then(pns_adapters::local_minutes_since_midnight)
}

/// The five inputs, each read once. An unreadable one stays `None`, which
/// matches no rule that names it.
///
/// `location` AND `focus` ARE FILLED IN THE NEXT SLICE and are `None` here,
/// so a rule naming either does not match yet. `calendar_busy` is the mute
/// the calendar poll already sets, read off the same expiry.
fn inputs(now: Option<u64>) -> Inputs {
    let minutes_of_day = now.and_then(pns_adapters::local_minutes_since_midnight);
    let weekday = now.and_then(pns_adapters::local_civil).map(|(_, weekday)| weekday);
    Inputs {
        weekday,
        minutes_of_day,
        location: None,
        focus: None,
        calendar_busy: None,
    }
}

/// A config that will not load names no profile and no rule, which resolves
/// `default` and runs `Profile::default()`: today's behaviour.
fn loaded_config() -> pns_adapters::Config {
    let home = std::env::var("HOME").unwrap_or_default();
    match pns_adapters::load_config(&pns_adapters::config_path(&home)) {
        Ok(pns_adapters::LoadOutcome::Loaded(config)) => config,
        _ => pns_adapters::Config::default(),
    }
}

/// Minutes since local midnight as `HH:MM`.
fn clock(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}
```

`minute_of_clock` is one line in `pns/crates/pns-domain/src/profiles.rs`, exported so the command does
not reach into the lamps module for a private helper:

```rust
/// `HH:MM` as minutes since local midnight, the lamps' own reader.
pub fn minute_of_clock(clock: &str) -> Option<u16> {
    crate::lamps::window::parse_window(&format!("{clock}-{clock}")).map(|window| window.ends_at())
}
```

In `invocation.rs`, add the branch beside `mute`:

```rust
    // Which bundle of delivery settings is active. A MODE beside the mute's
    // for the same reason: it writes the state the event path reads and
    // delivers nothing itself.
    if first == "profile" {
        std::process::exit(crate::command_profile::profile_mode());
    }
```

In `subcommand_usage.rs`, add `("profile", PROFILE_USAGE)` to the table it already keeps, and declare
`pub(crate) mod command_profile;` and `pub(crate) mod profile_runtime;` in `lib.rs`. `Config` and
`local_civil` are already exported from `pns_adapters`; add `Config::default()` derivation only if the
crate does not already provide it, which it does through the hand-written `Default` Task 4 edited.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns command_profile`
Expected: PASS, five tests.

- [ ] **Step 5: Run both gates and commit**

Run: `just test-rust` and `just lint-check`
Expected: both pass.

```bash
git add pns/crates/pns/src pns/crates/pns-domain/src/profiles.rs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): pns profile reports, selects and clears"
```

---
## Slice 3 tasks

### Task 11: the fingerprint

**Files:**
- Create: `pns/crates/pns-domain/src/profiles/fingerprint.rs`
- Create: `pns/crates/pns-domain/src/profiles/fingerprint/tests.rs`
- Modify: `pns/crates/pns-domain/src/profiles.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pns_domain::profiles::{normalized_hardware_address, gateway_of, hardware_address_of,
  named_location}`, with `normalized_hardware_address(&str) -> Option<String>`,
  `gateway_of(route_output: &str) -> Option<String>`,
  `hardware_address_of(arp_output: &str) -> Option<String>`, and
  `named_location(fingerprint: Option<&str>, locations: &BTreeMap<String, String>) -> Option<String>`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-domain/src/profiles/fingerprint/tests.rs`:

```rust
use super::*;
use std::collections::BTreeMap;

const ROUTE: &str = "   route to: default\ndestination: default\n       gateway: 192.168.1.1\n \
                     interface: en0\n";

#[test]
fn an_address_normalizes_to_two_digits_an_octet_in_lower_case() {
    assert_eq!(
        normalized_hardware_address("0:11:22:AA:bb:cc"),
        Some("00:11:22:aa:bb:cc".to_string())
    );
    assert_eq!(
        normalized_hardware_address("00:11:22:aa:bb:cc"),
        normalized_hardware_address("0:11:22:AA:BB:CC"),
        "one network is one fingerprint, however the tool spelled it"
    );
    assert_eq!(normalized_hardware_address("00:11:22:aa:bb"), None, "six octets, not five");
    assert_eq!(normalized_hardware_address("incomplete"), None);
    assert_eq!(normalized_hardware_address("00:11:22:aa:bb:zz"), None);
}

#[test]
fn the_gateway_comes_off_the_route_output() {
    assert_eq!(gateway_of(ROUTE), Some("192.168.1.1".to_string()));
    assert_eq!(gateway_of("   route to: default\n  interface: en0\n"), None);
    assert_eq!(gateway_of(""), None, "no default route is no gateway");
}

#[test]
fn the_hardware_address_comes_off_the_arp_output() {
    assert_eq!(
        hardware_address_of("? (192.168.1.1) at 0:11:22:aa:bb:cc on en0 ifscope [ethernet]\n"),
        Some("00:11:22:aa:bb:cc".to_string())
    );
    assert_eq!(
        hardware_address_of("? (192.168.1.1) at (incomplete) on en0 ifscope [ethernet]\n"),
        None,
        "an incomplete entry is no fingerprint"
    );
    assert_eq!(hardware_address_of("192.168.1.1 -- no entry\n"), None);
    assert_eq!(hardware_address_of(""), None);
}

#[test]
fn an_unknown_network_is_named_by_nothing() {
    let mut locations = BTreeMap::new();
    locations.insert("home".to_string(), "00:11:22:AA:BB:CC".to_string());
    assert_eq!(
        named_location(Some("00:11:22:aa:bb:cc"), &locations),
        Some("home".to_string()),
        "the config's own spelling is normalized too"
    );
    assert_eq!(named_location(Some("de:ad:be:ef:00:01"), &locations), None);
    assert_eq!(named_location(None, &locations), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::fingerprint`
Expected: FAIL, the module does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-domain/src/profiles/fingerprint.rs`:

```rust
//! The network a machine is on, as a name.
//!
//! THE WI-FI NAME IS NOT READABLE on this machine: `ipconfig getsummary en0`
//! prints `SSID : <redacted>` (measured 2026-09-17), because macOS gates it
//! behind Location Services. The fingerprint is therefore the default
//! gateway's hardware address, which is a fact about the network nobody has
//! to grant anything to read.
//!
//! POLICY ONLY. Every function here is a total function of text, so the two
//! commands that produce that text live in the adapter and the parsing is
//! under test with no network at all.

use std::collections::BTreeMap;

/// One hardware address, normalized: lower case, two digits an octet, six
/// octets, or None for anything that is not one.
///
/// NORMALIZED BECAUSE BOTH SIDES SPELL IT DIFFERENTLY. `arp` writes
/// `0:11:22:aa:bb:cc` and an operator pasting from a router writes
/// `00:11:22:AA:BB:CC`; one network has to be one fingerprint.
pub fn normalized_hardware_address(stated: &str) -> Option<String> {
    let octets: Vec<&str> = stated.trim().split(':').collect();
    if octets.len() != 6 {
        return None;
    }
    let mut normalized = Vec::with_capacity(6);
    for octet in octets {
        if octet.is_empty() || octet.len() > 2 || !octet.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return None;
        }
        normalized.push(format!("{:0>2}", octet.to_ascii_lowercase()));
    }
    Some(normalized.join(":"))
}

/// The default gateway's address, off `route -n get default`.
pub fn gateway_of(route_output: &str) -> Option<String> {
    route_output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("gateway:"))
        .map(|address| address.trim().to_string())
        .find(|address| !address.is_empty())
}

/// The gateway's hardware address, off `arp -n <gateway>`.
///
/// `no entry` AND `(incomplete)` BOTH ANSWER NOTHING, which is the same
/// answer an unparsable line gives: this machine is on no KNOWN network, and
/// a rule naming a location does not match.
pub fn hardware_address_of(arp_output: &str) -> Option<String> {
    arp_output
        .split_whitespace()
        .skip_while(|word| *word != "at")
        .nth(1)
        .and_then(normalized_hardware_address)
}

/// The name `[profiles.locations]` gives this fingerprint, or None for an
/// unknown network and for no reading at all.
pub fn named_location(
    fingerprint: Option<&str>,
    locations: &BTreeMap<String, String>,
) -> Option<String> {
    let fingerprint = normalized_hardware_address(fingerprint?)?;
    locations
        .iter()
        .find(|(_, stated)| {
            normalized_hardware_address(stated).is_some_and(|known| known == fingerprint)
        })
        .map(|(name, _)| name.clone())
}

#[cfg(test)]
mod tests;
```

Add to `pns/crates/pns-domain/src/profiles.rs`:

```rust
mod fingerprint;
pub use fingerprint::{
    gateway_of, hardware_address_of, named_location, normalized_hardware_address,
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::fingerprint`
Expected: PASS, four tests.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): fingerprint a network by its default gateway"
```

---

### Task 12: read the fingerprint off the machine

**Files:**
- Create: `pns/crates/pns-adapters/src/macos/network.rs`
- Create: `pns/crates/pns-adapters/src/macos/network/tests.rs`
- Modify: `pns/crates/pns-adapters/src/macos/mod.rs`
- Modify: `pns/crates/pns-adapters/src/lib.rs`

**Interfaces:**
- Consumes: `gateway_of` and `hardware_address_of` from Task 11,
  `pns_application::CommandRunner` (`fn run(&self, program: &str, args: &[&str]) -> Option<String>`).
- Produces: `pns_adapters::{GatewayFingerprint, gateway_fingerprint}` with
  `gateway_fingerprint(runner: &impl CommandRunner) -> Result<String, GatewayFingerprint>` and
  `enum GatewayFingerprint { NoDefaultRoute, NoArpEntry { gateway: String } }`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-adapters/src/macos/network/tests.rs`:

```rust
use super::*;

struct Scripted(Vec<(String, Option<String>)>);

impl CommandRunner for Scripted {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        let typed = format!("{program} {}", args.join(" "));
        self.0
            .iter()
            .find(|(line, _)| *line == typed)
            .and_then(|(_, answer)| answer.clone())
    }
}

fn scripted(lines: &[(&str, Option<&str>)]) -> Scripted {
    Scripted(
        lines
            .iter()
            .map(|(line, answer)| (line.to_string(), answer.map(str::to_string)))
            .collect(),
    )
}

#[test]
fn a_gateway_with_an_arp_entry_fingerprints() {
    let runner = scripted(&[
        ("route -n get default", Some("   gateway: 192.168.1.1\n")),
        ("arp -n 192.168.1.1", Some("? (192.168.1.1) at 0:11:22:aa:bb:cc on en0\n")),
    ]);
    assert_eq!(gateway_fingerprint(&runner), Ok("00:11:22:aa:bb:cc".to_string()));
}

#[test]
fn no_default_route_says_so_and_asks_no_arp() {
    for route in [None, Some("   route to: default\n")] {
        let runner = scripted(&[("route -n get default", route)]);
        assert_eq!(gateway_fingerprint(&runner), Err(GatewayFingerprint::NoDefaultRoute));
    }
}

#[test]
fn an_unreadable_arp_entry_names_the_gateway_it_asked_about() {
    for arp in [None, Some("? (192.168.1.1) at (incomplete) on en0\n"), Some("no entry\n")] {
        let runner = scripted(&[
            ("route -n get default", Some("   gateway: 192.168.1.1\n")),
            ("arp -n 192.168.1.1", arp),
        ]);
        assert_eq!(
            gateway_fingerprint(&runner),
            Err(GatewayFingerprint::NoArpEntry { gateway: "192.168.1.1".to_string() })
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters macos::network`
Expected: FAIL, the module does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-adapters/src/macos/network.rs`:

```rust
//! Which network this machine is on, as the default gateway's hardware
//! address.
//!
//! TWO STOCK COMMANDS BEHIND ONE PORT. The routing table is reachable
//! natively through `PF_ROUTE`, and the ruling for this feature names
//! `route -n get default` and `arp -n`, so they stay: one adapter, one port,
//! and a later native read is a swap of this file and nothing else.
//!
//! EVERY READING IS BOUNDED, through `CommandRunner`, whose production
//! implementation runs each spawn under `PROBE_DEADLINE`: a wedged command
//! must never hold a notification open.

use pns_application::CommandRunner;
use pns_domain::profiles::{gateway_of, hardware_address_of};

/// Why there is no fingerprint. BOTH ARE "this machine is on no KNOWN
/// network" to the resolver; they differ only in what `pns profile learn`
/// tells the operator to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayFingerprint {
    NoDefaultRoute,
    NoArpEntry { gateway: String },
}

/// The current network's fingerprint, or why there is none.
pub fn gateway_fingerprint(runner: &impl CommandRunner) -> Result<String, GatewayFingerprint> {
    let gateway = runner
        .run("route", &["-n", "get", "default"])
        .as_deref()
        .and_then(gateway_of)
        .ok_or(GatewayFingerprint::NoDefaultRoute)?;
    runner
        .run("arp", &["-n", &gateway])
        .as_deref()
        .and_then(hardware_address_of)
        .ok_or(GatewayFingerprint::NoArpEntry { gateway })
}

#[cfg(test)]
mod tests;
```

In `macos/mod.rs` add `mod network;` and `pub use network::{GatewayFingerprint, gateway_fingerprint};`,
and re-export both from `pns/crates/pns-adapters/src/lib.rs` beside `focus_now`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters macos::network`
Expected: PASS, three tests.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src/macos pns/crates/pns-adapters/src/lib.rs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): read the default gateway's hardware address"
```

---

### Task 13: the Focus mode name, `pns profile learn`, and the two live inputs

**Files:**
- Modify: `pns/crates/pns-adapters/src/macos/focus.rs`
- Modify: `pns/crates/pns-application/src/doctor/focus.rs` (the `FocusReading` type)
- Modify: `pns/crates/pns-adapters/src/macos/focus/tests.rs`
- Modify: `pns/crates/pns/src/{command_profile.rs,profile_runtime.rs}`

**Interfaces:**
- Consumes: `gateway_fingerprint` from Task 12, `named_location` from Task 11,
  `pns_adapters::focus_now`.
- Produces: `FocusReading::asserted: Vec<String>` (the display names of the modes asserted now), and
  `pub(crate) fn learn(words: &[String]) -> i32` in `profile_runtime`.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns-adapters/src/macos/focus/tests.rs`, reusing whatever store fixture the file
already writes:

```rust
#[test]
fn the_asserted_modes_come_out_by_their_display_names() {
    let home = focus_store_with(
        "{\"data\":[{\"storeAssertionRecords\":[{\"assertionDetails\":\
         {\"assertionDetailsModeIdentifier\":\"com.apple.focus.work\"}}]}]}",
        "{\"data\":[{\"modeConfigurations\":{\"a\":{\"mode\":\
         {\"modeIdentifier\":\"com.apple.focus.work\",\"name\":\"Work\"}}}}]}",
    );
    let reading = focus_now(home.path_as_str(), &["Work".to_string()]).expect("a reading");
    assert!(reading.silenced, "the named mode still silences");
    assert_eq!(reading.asserted, vec!["Work".to_string()]);
}

#[test]
fn a_mode_with_no_catalog_entry_comes_out_under_its_identifier() {
    let home = focus_store_with(
        "{\"data\":[{\"storeAssertionRecords\":[{\"assertionDetails\":\
         {\"assertionDetailsModeIdentifier\":\"com.apple.focus.sleep\"}}]}]}",
        "{}",
    );
    let reading = focus_now(home.path_as_str(), &["Work".to_string()]).expect("a reading");
    assert!(!reading.silenced);
    assert_eq!(reading.asserted, vec!["com.apple.focus.sleep".to_string()]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters macos::focus`
Expected: FAIL, `no field asserted on type FocusReading`.

- [ ] **Step 3: Write minimal implementation**

Add the field to `FocusReading` in `pns/crates/pns-application/src/doctor/focus.rs`:

```rust
    /// The Focus modes asserted right now, by the display name Control Center
    /// shows, falling back to the raw identifier where the catalog resolved
    /// none. THE ROSTER IS NOT CONSULTED for this: `silenced` beside it
    /// answers "is one of the named ones on", and a profile rule asks about a
    /// mode the roster may never have named.
    pub asserted: Vec<String>,
```

In `pns/crates/pns-adapters/src/macos/focus.rs`, read the store even when the roster is empty, because
a rule can name a mode `[focus] modes` does not:

```rust
pub fn focus_now(home: &str, silence: &[String]) -> std::io::Result<FocusReading> {
    let store = Path::new(home).join(FOCUS_DB);
    let assertions = crate::readable_state_file(&store.join("Assertions.json"), RING_READ_MAX)?;
    let catalog = crate::readable_state_file(&store.join("ModeConfigurations.json"), RING_READ_MAX);
    let active = active_modes(&assertions);
    let names = mode_names(catalog.as_deref().unwrap_or_default());
    Ok(FocusReading {
        silenced: pns_domain::focus_silenced(&active, &names, silence),
        asserted: active
            .iter()
            .map(|identifier| names.get(identifier).cloned().unwrap_or_else(|| identifier.clone()))
            .collect(),
        catalog: catalog.as_ref().err().map(std::io::Error::kind),
    })
}
```

The early return for an empty roster goes, and its comment goes with it: the store is now read for the
rules as well, and a machine with neither a roster nor a rule pays two small reads on the daemon's own
tick rather than on every event.

In `pns/crates/pns/src/profile_runtime.rs`, fill the two inputs and add `learn`:

```rust
fn inputs(now: Option<u64>) -> Inputs {
    let config = loaded_config();
    let minutes_of_day = now.and_then(pns_adapters::local_minutes_since_midnight);
    let weekday = now.and_then(pns_adapters::local_civil).map(|(_, weekday)| weekday);
    let fingerprint = pns_adapters::gateway_fingerprint(&pns_adapters::SystemCommandRunner).ok();
    let home = std::env::var("HOME").unwrap_or_default();
    Inputs {
        weekday,
        minutes_of_day,
        location: pns_domain::profiles::named_location(
            fingerprint.as_deref(),
            &config.profile_locations,
        ),
        focus: pns_adapters::focus_now(&home, &config.focus_modes)
            .ok()
            .and_then(|reading| reading.asserted.first().cloned()),
        calendar_busy: None,
    }
}

/// `pns profile learn <name>`: print the row for the network this machine is
/// on now.
///
/// IT PRINTS RATHER THAN WRITES. pns has no writer for an existing config
/// file, and the deployed config is a chezmoi target the next apply would
/// overwrite; printing is also what makes this safe to run from a phone.
pub(crate) fn learn(words: &[String]) -> i32 {
    let [name] = words else {
        eprintln!("{}", crate::command_profile::PROFILE_USAGE);
        return 2;
    };
    match pns_adapters::gateway_fingerprint(&pns_adapters::SystemCommandRunner) {
        Ok(fingerprint) => {
            println!("[profiles.locations]");
            println!("{name} = \"{fingerprint}\"");
            0
        }
        Err(pns_adapters::GatewayFingerprint::NoDefaultRoute) => {
            eprintln!(
                "pns profile: this machine has no default route, so there is no network to learn"
            );
            1
        }
        Err(pns_adapters::GatewayFingerprint::NoArpEntry { gateway }) => {
            eprintln!(
                "pns profile: the gateway {gateway} has no ARP entry yet; try again once \
                 something has talked to it"
            );
            1
        }
    }
}
```

In `pns/crates/pns/src/command_profile.rs`, put the verb back in the usage and in the match:

```rust
pub(crate) const PROFILE_USAGE: &str =
    "usage: pns profile [<name> [--for <duration> | --until HH:MM] | clear | learn <name>]";
```
```rust
        Some((word, rest)) if word == "learn" => crate::profile_runtime::learn(rest),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-adapters macos::focus`
Then: `just test-rust` and `just lint-check`
Expected: all pass. The existing `focus_now` callers read `silenced` and are unchanged.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src pns/crates/pns-application/src pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): the location and Focus rule inputs, and pns profile learn"
```

---
## Slice 4 tasks

### Task 14: the profile reaches the delivery decision

**Files:**
- Modify: `pns/crates/pns-domain/src/decision/overrides.rs`
- Modify: `pns/crates/pns-domain/src/decision/arbitration.rs`
- Modify: `pns/crates/pns-domain/src/decision/tests.rs`

**Interfaces:**
- Consumes: `Profile` from Task 1.
- Produces: `Overrides::profile: Profile`, `Overrides::priority: bool`,
  `Overrides::silenced(&self) -> bool` unchanged in name and widened in meaning.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns-domain/src/decision/tests.rs`:

```rust
#[test]
fn a_profiles_hush_silences_an_ordinary_event_and_never_a_page() {
    let work = Overrides {
        profile: pns_profile_work(),
        priority: false,
        ..Overrides::default()
    };
    assert!(work.silenced(), "the profile's quiet is a hush like any other");
    let page = Overrides { priority: true, ..work };
    assert!(!page.silenced(), "a priority page crosses a profile's quiet");
}

#[test]
fn the_default_profile_silences_nothing() {
    assert!(!Overrides::default().silenced(), "today's behaviour, unchanged");
}

/// The shipped `work` profile, so the assertions above read against the real
/// thing rather than a shape invented here.
fn pns_profile_work() -> crate::profiles::Profile {
    crate::profiles::Profile {
        quiet: true,
        banner: crate::profiles::Admits::All,
        discord: crate::profiles::Admits::None,
        phone: crate::profiles::Admits::Priority,
        lights: crate::profiles::Admits::None,
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain decision::`
Expected: FAIL, `no field profile on type Overrides`.

- [ ] **Step 3: Write minimal implementation**

In `pns/crates/pns-domain/src/decision/overrides.rs`, add the two fields and widen the predicate:

```rust
    /// The ACTIVE PROFILE, and the THIRD field here that never comes from the
    /// environment, for the two above it: a variable able to set it would let
    /// any producer silence the operator. The composition root resolves it and
    /// states it.
    ///
    /// THE DEFAULT IS TODAY'S BEHAVIOUR, so every caller that does not know
    /// about profiles keeps the plan it already had.
    pub profile: crate::profiles::Profile,
    /// Whether this event is a priority page, which is the one thing no
    /// profile may silence. Stated by the composition root off the event's
    /// RESOLVED ROUTE, never re-derived from the class here.
    pub priority: bool,
```

Add `profile: crate::profiles::Profile::default(), priority: false,` to the `from_env` constructor
beside `muted: false, focus_active: false`, and derive `Default` as before: `Profile`'s own `Default`
is today's behaviour, so `Overrides::default()` is unchanged in meaning.

Widen `silenced`:

```rust
    /// The operator told everything to be quiet: their own typed mute, a
    /// macOS Focus they named, or the active profile's own hush.
    ///
    /// THE PROFILE'S HALF NEVER REACHES A PAGE. `Profile::hushes` is where
    /// that is stated, once, and this is its only caller: the priority floor
    /// has to be one rule, not one per reader.
    pub fn silenced(&self) -> bool {
        self.muted || self.focus_active || self.profile.hushes(self.priority)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain decision::`
Expected: PASS, including every existing arbitration test, because `Overrides::default()` carries the
default profile.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src/decision
SKIP_AI_COMMIT=1 git commit -m "feat(pns): the active profile is an input to the delivery decision"
```

---

### Task 15: the surface mask drops legs

**Files:**
- Modify: `pns/crates/pns-domain/src/routing.rs`
- Modify: `pns/crates/pns-domain/src/decision/arbitration.rs`
- Create: `pns/crates/pns-domain/src/routing/profile_tests.rs`

**Interfaces:**
- Consumes: `Profile`, `Admits` from Task 1; `Overrides` from Task 14.
- Produces: `channel_plan(enabled: &Selection, scope: DeliveryScope, delivery: DeliveryPlan,
  admits: &SurfaceMask) -> Vec<Leg>` and
  `pub struct SurfaceMask { pub banner: bool, pub discord: bool, pub phone: bool }`, with
  `SurfaceMask::of(profile: &Profile, priority: bool) -> SurfaceMask`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-domain/src/routing/profile_tests.rs`:

```rust
use super::*;
use crate::profiles::{Admits, Profile};

fn night() -> Profile {
    Profile {
        quiet: true,
        banner: Admits::None,
        discord: Admits::None,
        phone: Admits::Priority,
        lights: Admits::None,
    }
}

#[test]
fn a_surface_at_none_drops_its_leg_and_all_keeps_it() {
    assert_eq!(
        SurfaceMask::of(&night(), false),
        SurfaceMask { banner: false, discord: false, phone: false }
    );
    assert_eq!(
        SurfaceMask::of(&Profile::default(), false),
        SurfaceMask { banner: true, discord: true, phone: true },
        "the default profile masks nothing, which is today's behaviour"
    );
}

#[test]
fn a_surface_at_priority_opens_only_for_a_page() {
    assert_eq!(
        SurfaceMask::of(&night(), true),
        SurfaceMask { banner: false, discord: false, phone: true }
    );
}

#[test]
fn every_profile_the_config_admits_lets_a_page_through_somewhere() {
    let words = [Admits::All, Admits::Priority, Admits::None];
    for banner in words {
        for discord in words {
            for phone in words {
                for lights in words {
                    let profile = Profile { quiet: true, banner, discord, phone, lights };
                    if !profile.admits_a_page() {
                        continue; // refused at load, which has its own test
                    }
                    let mask = SurfaceMask::of(&profile, true);
                    let lit = profile.lights.admits(true);
                    assert!(
                        mask.banner || mask.discord || mask.phone || lit,
                        "{profile:?} silenced a priority page"
                    );
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain routing::`
Expected: FAIL, `cannot find type SurfaceMask`.

- [ ] **Step 3: Write minimal implementation**

In `pns/crates/pns-domain/src/routing.rs`, add the mask and take it in `channel_plan`:

```rust
/// What the active profile leaves of a plan, per destination KIND rather than
/// per plugin name, which is the same axis `channel_plan` already filters on.
///
/// THE LIGHTS ARE NOT HERE. The pulse is not a leg: it rides
/// `DeliveryPlan::pulse`, and the profile's `lights` word is applied where the
/// pulse is decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceMask {
    pub banner: bool,
    pub discord: bool,
    pub phone: bool,
}

impl Default for SurfaceMask {
    /// Nothing masked, which is what every caller with no profile gets.
    fn default() -> Self {
        SurfaceMask { banner: true, discord: true, phone: true }
    }
}

impl SurfaceMask {
    /// What this profile admits for an event of this priority.
    pub fn of(profile: &crate::profiles::Profile, priority: bool) -> Self {
        SurfaceMask {
            banner: profile.banner.admits(priority),
            discord: profile.discord.admits(priority),
            phone: profile.phone.admits(priority),
        }
    }
}
```

Give `channel_plan` the fourth argument and apply the mask inside the `filter_map` that already reads
the declarations, so the profile is applied exactly once and on the same axis:

```rust
pub fn channel_plan(
    enabled: &Selection,
    scope: crate::DeliveryScope,
    delivery: crate::surface::DeliveryPlan,
    admits: &SurfaceMask,
) -> Vec<Leg> {
```
```rust
        .filter_map(|(name, routing)| {
            let decorative = routing.presence_gated || routing.local;
            let wanted = if routing.presence_gated {
                delivery.phone_card && admits.phone
            } else if routing.local {
                delivery.banner && admits.banner
            } else {
                admits.discord
            };
            wanted.then_some(Leg { name, mode, decorative })
        })
```

In `arbitration.rs`, pass the mask the overrides already carry and apply the lights word to the pulse:

```rust
    let mask = crate::routing::SurfaceMask::of(&overrides.profile, overrides.priority);
    let delivery = crate::surface::DeliveryPlan {
        pulse: delivery.pulse && overrides.profile.lights.admits(overrides.priority),
        ..delivery
    };
    Decision {
        legs: crate::routing::channel_plan(selection, scope, delivery, &mask),
        plan: delivery,
        pane_dropped: !pane.is_empty() && !crate::safety::pane_is_safe(pane),
        inputs: world,
    }
```

Place both lines after the silence arbitration that already computes `delivery`, so the mask is the
LAST thing applied and nothing downstream can add a surface back.

Declare the new test module at the foot of `routing.rs`:

```rust
#[cfg(test)]
#[path = "routing/profile_tests.rs"]
mod profile_tests;
```

Update every other caller of `channel_plan` in the same change. `git grep -n 'channel_plan('` finds
them; the production caller is `arbitration.rs` above, and the rest are tests that pass
`&SurfaceMask::default()`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain`
Expected: PASS, three new tests and every existing routing test.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): the active profile subtracts surfaces from the plan"
```

---

### Task 16: the composition root states the profile

**Files:**
- Modify: `pns/crates/pns/src/event_flow/execution.rs`
- Modify: `pns/crates/pns/src/profile_runtime.rs`
- Modify: `pns/crates/pns/src/event_flow/tests.rs`

**Interfaces:**
- Consumes: `profile_runtime::active` from Task 10, `Overrides` from Task 14.
- Produces: `pub(crate) fn for_event(records: &SqliteStore, route: &str, urgent: &str)
  -> (pns_domain::profiles::Profile, bool)` in `profile_runtime`.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns/src/event_flow/tests.rs`:

```rust
#[test]
fn a_page_is_a_page_because_of_the_route_it_resolved_to() {
    assert_eq!(
        crate::profile_runtime::priority_of("priority", "priority"),
        true,
        "the event's route is the urgent one"
    );
    assert_eq!(crate::profile_runtime::priority_of("pns-events", "priority"), false);
    assert_eq!(
        crate::profile_runtime::priority_of("", "priority"),
        false,
        "an event that named no route took the default one"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns event_flow::tests::a_page_is_a_page`
Expected: FAIL, `cannot find function priority_of`.

- [ ] **Step 3: Write minimal implementation**

Add to `pns/crates/pns/src/profile_runtime.rs`:

```rust
/// Whether an event is a priority page: its RESOLVED route, which the event
/// carries in `channel`, is the one `[routes] urgent` names.
///
/// THE ROUTE AND NOT THE CLASS. The route is settled once, where the event is
/// routed against its class, so reading it here is reading the same answer the
/// ledger row and every destination read already carry. An empty route is the
/// default route, which is never the urgent one.
pub(crate) fn priority_of(route: &str, urgent: &str) -> bool {
    !route.is_empty() && route == urgent
}

/// The profile and the priority verdict one event is decided under.
pub(crate) fn for_event(
    records: &SqliteStore,
    route: &str,
    urgent: &str,
) -> (Profile, bool) {
    (active(records).profile, priority_of(route, urgent))
}
```

In `pns/crates/pns/src/event_flow/execution.rs`, state both beside the mute and the Focus, where the
routed event and the `[routes]` table are both already in hand:

```rust
    let (profile, priority) =
        crate::profile_runtime::for_event(&store, &event.channel, routes.urgent_route());
    let overrides = Overrides {
        muted: muted_now(now_secs),
        focus_active: focus_now(&home, &focus_silence).is_ok_and(|reading| reading.silenced),
        profile,
        priority,
        ..overrides_from_env()
    };
```

The `store` binding is created a few lines below today; move its construction above the `overrides`
binding so both read the same handle rather than opening two.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns`
Then: `just test-rust` and `just lint-check`
Expected: all pass. The hook and producer paths are unchanged on a machine with no `[profiles]` table,
which the existing event-flow tests are the guard for.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): decide each event under the active profile"
```

---

## Slice 5 tasks

### Task 17: the held roll-up's wording

**Files:**
- Create: `pns/crates/pns-domain/src/profiles/rollup.rs`
- Create: `pns/crates/pns-domain/src/profiles/rollup/tests.rs`
- Modify: `pns/crates/pns-domain/src/profiles.rs`

**Interfaces:**
- Consumes: `pns_domain::missed::Entry`.
- Produces: `pns_domain::profiles::held_rollup(profile: &str, held: &[Entry]) -> Option<String>`.

- [ ] **Step 1: Write the failing test**

Create `pns/crates/pns-domain/src/profiles/rollup/tests.rs`:

```rust
use super::*;

fn entry(state: &str) -> crate::missed::Entry {
    crate::missed::Entry { state: state.to_string(), ..crate::missed::Entry::default() }
}

#[test]
fn the_blocked_one_comes_first_whatever_the_counts() {
    let held = [entry("done"), entry("done"), entry("blocked"), entry("done"), entry("failed")];
    assert_eq!(
        held_rollup("work", &held),
        Some("held while `work`: 1 blocked, 3 done, 1 failed".to_string())
    );
}

#[test]
fn the_rest_go_by_count_then_alphabetically() {
    let held = [entry("failed"), entry("done"), entry("failed"), entry("asked")];
    assert_eq!(
        held_rollup("night", &held),
        Some("held while `night`: 2 failed, 1 asked, 1 done".to_string())
    );
}

#[test]
fn one_event_is_singular_and_a_state_with_none_is_not_written() {
    assert_eq!(
        held_rollup("night", &[entry("blocked")]),
        Some("held while `night`: 1 blocked".to_string())
    );
}

#[test]
fn nothing_held_is_no_roll_up_at_all() {
    assert_eq!(held_rollup("night", &[]), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::rollup`
Expected: FAIL, the module does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `pns/crates/pns-domain/src/profiles/rollup.rs`:

```rust
use crate::missed::Entry;
use std::collections::BTreeMap;

/// The ONE line a transition delivers for everything a profile held.
///
/// ONE LINE AND NEVER THE BACKLOG. The events themselves already reached the
/// durable log where the profile admitted it; what the operator needs on
/// coming back is the shape of what they missed, not a replay of it.
///
/// BLOCKED FIRST, because it is the one that was waiting on a person. The
/// rest go by count and then alphabetically, so two runs of the same events
/// read the same way.
///
/// THE PROFILE NAMED IS THE ONE THAT HELD THEM, never the one being entered:
/// the operator is being told what the last few hours cost, not what the next
/// few will.
pub fn held_rollup(profile: &str, held: &[Entry]) -> Option<String> {
    if held.is_empty() {
        return None;
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in held {
        *counts.entry(entry.state.as_str()).or_default() += 1;
    }
    let mut ordered: Vec<(&str, usize)> = counts.into_iter().collect();
    ordered.sort_by(|(left, left_count), (right, right_count)| {
        let blocked = |state: &str| state != "blocked";
        blocked(left)
            .cmp(&blocked(right))
            .then(right_count.cmp(left_count))
            .then(left.cmp(right))
    });
    let body: Vec<String> = ordered
        .iter()
        .map(|(state, count)| format!("{count} {state}"))
        .collect();
    Some(format!("held while `{profile}`: {}", body.join(", ")))
}

#[cfg(test)]
mod tests;
```

Add to `pns/crates/pns-domain/src/profiles.rs`:

```rust
mod rollup;
pub use rollup::held_rollup;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns-domain profiles::rollup`
Expected: PASS, four tests.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-domain/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): the held roll-up's wording"
```

---

### Task 18: the transition row and the daemon's own evaluation

**Files:**
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/{profiles.rs,migrations.rs}`
- Modify: `pns/crates/pns/src/{daemon_runtime.rs,profile_runtime.rs}`
- Create: `pns/crates/pns/src/profile_runtime/tests.rs`
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/tests.rs`

**Interfaces:**
- Consumes: `transition_line` and `because` from Task 8, `active` from Task 10.
- Produces: `SqliteStore::{active_profile, record_profile_transition, profile_transitions}` with
  `active_profile(&self) -> Result<Option<String>, StoreError>`,
  `record_profile_transition(&self, at: u64, from: &str, to: &str, reason: &str)
  -> Result<(), StoreError>`,
  `profile_transitions(&self, limit: usize) -> Result<Vec<(u64, String)>, StoreError>`; and
  `profile_runtime::tick(records: &SqliteStore, now: u64) -> Option<String>`.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns-adapters/src/persistence/sqlite/tests.rs`:

```rust
#[test]
fn a_transition_is_one_row_and_the_active_profile_follows_it() {
    let (store, _guard) = store_in_a_temporary_directory();
    assert_eq!(store.active_profile().expect("a read"), None);
    store
        .record_profile_transition(100, "default", "night", "rule 1: hours")
        .expect("a write");
    assert_eq!(store.active_profile().expect("a read"), Some("night".to_string()));
    store
        .record_profile_transition(200, "night", "default", "no rule matched")
        .expect("a write");
    let rows = store.profile_transitions(10).expect("a read");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], (200, "profile night -> default (no rule matched)".to_string()));
    assert_eq!(rows[1], (100, "profile default -> night (rule 1: hours)".to_string()));
}
```

And append to `pns/crates/pns/src/profile_runtime/tests.rs` (created here):

```rust
use super::*;

#[test]
fn a_resolve_that_changes_nothing_writes_nothing() {
    let store = pns_adapters::SqliteStore::new(crate::runtime_test_support::scratch("profile-tick"));
    store
        .record_profile_transition(100, "default", "night", "rule 1: hours")
        .expect("a write");
    assert_eq!(
        transition(&store, 200, "night", "rule 1: hours"),
        None,
        "the same profile twice is not a transition"
    );
    assert_eq!(
        transition(&store, 300, "default", "no rule matched"),
        Some("profile night -> default (no rule matched)".to_string())
    );
    assert_eq!(store.profile_transitions(10).expect("a read").len(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml profile_transition`
Expected: FAIL, `no method named record_profile_transition`.

- [ ] **Step 3: Write minimal implementation**

Extend `create` in `pns/crates/pns-adapters/src/persistence/sqlite/profiles.rs`:

```rust
        "CREATE TABLE profile_override (id INTEGER PRIMARY KEY CHECK(id = 1), body TEXT NOT NULL);
         CREATE TABLE profile_transitions (
           seq INTEGER PRIMARY KEY AUTOINCREMENT,
           at INTEGER NOT NULL,
           profile TEXT NOT NULL,
           line TEXT NOT NULL);",
```

and add the three methods:

```rust
    /// The profile the last transition moved to, or None on a machine that
    /// has never transitioned.
    pub fn active_profile(&self) -> Result<Option<String>, StoreError> {
        Ok(self
            .connect()?
            .query_row(
                "SELECT profile FROM profile_transitions ORDER BY seq DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// One transition: the old profile, the new one and the reason, rendered
    /// ONCE by the domain so the row and the printed line cannot disagree.
    pub fn record_profile_transition(
        &self,
        at: u64,
        from: &str,
        to: &str,
        reason: &str,
    ) -> Result<(), StoreError> {
        let line = pns_domain::profiles::transition_line(from, to, reason);
        self.transaction(|transaction| {
            transaction.execute(
                "INSERT INTO profile_transitions(at, profile, line) VALUES (?1, ?2, ?3)",
                rusqlite::params![at, to, line],
            )?;
            Ok(())
        })
    }

    /// The newest transitions first, for `pns profile` and for the recap.
    pub fn profile_transitions(&self, limit: usize) -> Result<Vec<(u64, String)>, StoreError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT at, line FROM profile_transitions ORDER BY seq DESC LIMIT ?1",
        )?;
        let rows = statement
            .query_map([limit], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
```

`use rusqlite::OptionalExtension;` goes at the head of the file beside the existing imports.

Add to `pns/crates/pns/src/profile_runtime.rs`:

```rust
/// Resolve, and record the change if there is one. Returns the line to print,
/// or None when nothing moved.
///
/// SILENT WHEN NOTHING CHANGED, which is what keeps a one-second clock from
/// writing 86,400 rows a day.
pub(crate) fn tick(records: &SqliteStore, now: u64) -> Option<String> {
    let reading = active(records);
    transition(
        records,
        now,
        &reading.resolved.profile,
        &pns_domain::profiles::because(
            &reading.resolved.chose,
            &reading.resolved.matched,
            reading.until_clock.as_deref(),
        ),
    )
}

/// The write half, separated so the decision is testable against a store with
/// no probes behind it.
pub(crate) fn transition(
    records: &SqliteStore,
    now: u64,
    to: &str,
    reason: &str,
) -> Option<String> {
    let from = records
        .active_profile()
        .ok()
        .flatten()
        .unwrap_or_else(|| pns_domain::profiles::DEFAULT_PROFILE.to_string());
    if from == to {
        return None;
    }
    if let Err(error) = records.record_profile_transition(now, &from, to, reason) {
        eprintln!("pns gateway: the profile transition could not be recorded: {error}");
        return None;
    }
    Some(pns_domain::profiles::transition_line(&from, to, reason))
}

#[cfg(test)]
mod tests;
```

In `pns/crates/pns/src/daemon_runtime.rs`, call it from the tick body beside the other per-tick work:

```rust
        |now, children| {
            resolve_profile(now);
            prune_activity(now, &mut next_sweep);
            start_retry(now, children)?;
            start_page(now, children)
        },
```

```rust
/// The active profile, resolved on the clock, with a line on stdout for every
/// change. launchd captures that line in the gateway's own log, which is where
/// a live network-change transition is observed.
fn resolve_profile(now: u64) {
    let records = pns_adapters::SqliteStore::for_records(state_dir());
    if let Some(line) = crate::profile_runtime::tick(&records, now) {
        println!("pns gateway: {line}");
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml profile_transition`
Then: `cargo test --manifest-path pns/Cargo.toml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src/persistence pns/crates/pns/src
SKIP_AI_COMMIT=1 git commit -m "feat(pns): record every profile transition on the gateway's clock"
```

---

### Task 19: hold what a profile suppressed, and flush it once

**Files:**
- Modify: `pns/crates/pns-adapters/src/persistence/sqlite/profiles.rs`
- Modify: `pns/crates/pns/src/{event_flow/execution.rs,profile_runtime.rs,command_profile.rs}`
- Modify: `pns/crates/pns/src/profile_runtime/tests.rs`

**Interfaces:**
- Consumes: `held_rollup` from Task 17, `transition` from Task 18, `missed::Entry`.
- Produces: `SqliteStore::{hold_event, held_events, clear_held}` with
  `hold_event(&self, profile: &str, entry: &pns_domain::missed::Entry) -> Result<(), StoreError>`,
  `held_events(&self) -> Result<Vec<(String, pns_domain::missed::Entry)>, StoreError>`,
  `clear_held(&self) -> Result<(), StoreError>`; and
  `profile_runtime::flush_held(records: &SqliteStore, entering: &Profile) -> Option<String>`.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns/src/profile_runtime/tests.rs`:

```rust
fn held(state: &str) -> pns_domain::missed::Entry {
    pns_domain::missed::Entry { state: state.to_string(), ..Default::default() }
}

#[test]
fn a_hold_survives_until_a_profile_admits_it_and_is_then_flushed_once() {
    let store = pns_adapters::SqliteStore::new(crate::runtime_test_support::scratch("profile-hold"));
    store.hold_event("work", &held("done")).expect("a hold");
    store.hold_event("work", &held("blocked")).expect("a hold");
    let silent = pns_domain::profiles::Profile {
        quiet: true,
        banner: pns_domain::profiles::Admits::None,
        discord: pns_domain::profiles::Admits::None,
        phone: pns_domain::profiles::Admits::Priority,
        lights: pns_domain::profiles::Admits::None,
    };
    assert_eq!(
        flush_held(&store, &silent),
        None,
        "a profile that shows nothing does not flush"
    );
    assert_eq!(
        flush_held(&store, &pns_domain::profiles::Profile::default()),
        Some("held while `work`: 1 blocked, 1 done".to_string())
    );
    assert_eq!(
        flush_held(&store, &pns_domain::profiles::Profile::default()),
        None,
        "the flush empties the hold, so a second transition says nothing"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path pns/Cargo.toml -p pns profile_runtime`
Expected: FAIL, `no method named hold_event`.

- [ ] **Step 3: Write minimal implementation**

Extend `create` in `pns/crates/pns-adapters/src/persistence/sqlite/profiles.rs` with the table, in the
same batch, so a fresh install and a migrated one get the same schema:

```rust
         CREATE TABLE profile_held (
           seq INTEGER PRIMARY KEY AUTOINCREMENT,
           profile TEXT NOT NULL,
           at INTEGER,
           agent TEXT NOT NULL,
           state TEXT NOT NULL,
           project TEXT NOT NULL,
           branch TEXT NOT NULL,
           detail TEXT NOT NULL);
```

and add the three methods:

```rust
    /// Record one event a profile held back.
    ///
    /// THE SAME SIX FIELDS THE MISSED JOURNAL KEEPS, and the same privacy
    /// rule: nothing prints an entry, and the only reader is the flush, which
    /// counts them.
    pub fn hold_event(
        &self,
        profile: &str,
        entry: &pns_domain::missed::Entry,
    ) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            transaction.execute(
                "INSERT INTO profile_held(profile, at, agent, state, project, branch, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    profile,
                    entry.at,
                    entry.agent,
                    entry.state,
                    entry.project,
                    entry.branch,
                    entry.detail
                ],
            )?;
            Ok(())
        })
    }

    /// Everything held, oldest first, with the profile that held each one.
    pub fn held_events(&self) -> Result<Vec<(String, pns_domain::missed::Entry)>, StoreError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT profile, at, agent, state, project, branch, detail
             FROM profile_held ORDER BY seq ASC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    pns_domain::missed::Entry {
                        at: row.get(1)?,
                        agent: row.get(2)?,
                        state: row.get(3)?,
                        project: row.get(4)?,
                        branch: row.get(5)?,
                        detail: row.get(6)?,
                    },
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn clear_held(&self) -> Result<(), StoreError> {
        self.transaction(|transaction| {
            transaction.execute("DELETE FROM profile_held", [])?;
            Ok(())
        })
    }
```

Add to `pns/crates/pns/src/profile_runtime.rs`:

```rust
/// The one roll-up a transition delivers, or None when there is nothing held
/// or the profile being entered would hide it too.
///
/// THE FLUSH EMPTIES THE HOLD, so a second transition says nothing: one
/// roll-up per absence, however many transitions end it.
pub(crate) fn flush_held(records: &SqliteStore, entering: &Profile) -> Option<String> {
    // A profile that shows the operator nothing has nowhere to put the
    // roll-up, and delivering it into a held state would lose it.
    if !entering.banner.admits(false) && !entering.phone.admits(false) {
        return None;
    }
    let held = records.held_events().ok()?;
    let (profile, _) = held.first()?;
    let entries: Vec<pns_domain::missed::Entry> =
        held.iter().map(|(_, entry)| entry.clone()).collect();
    let line = pns_domain::profiles::held_rollup(profile, &entries)?;
    if let Err(error) = records.clear_held() {
        eprintln!("pns gateway: the held events could not be cleared: {error}");
        return None;
    }
    Some(line)
}
```

Call it from `tick`, right after a transition was recorded, and deliver the line through the same
replay path `return_replay` already uses:

```rust
pub(crate) fn tick(records: &SqliteStore, now: u64) -> Option<String> {
    let reading = active(records);
    let moved = transition(
        records,
        now,
        &reading.resolved.profile,
        &pns_domain::profiles::because(
            &reading.resolved.chose,
            &reading.resolved.matched,
            reading.until_clock.as_deref(),
        ),
    )?;
    if let Some(rollup) = flush_held(records, &reading.profile) {
        crate::return_replay::deliver_rollup(&rollup);
    }
    Some(moved)
}
```

In `pns/crates/pns/src/return_replay.rs`, add the one entry point, beside `replay_missed`, which
submits the roll-up as an ordinary event on the durable route so the existing ledger, retry and
destination path carries it:

```rust
/// The profile roll-up, delivered as ONE event through the path every other
/// notification takes.
///
/// IT IS AN EVENT AND NOT A SECOND DELIVERY MECHANISM: the ledger row, the
/// retry and the destination reads are the ones that already exist, and the
/// roll-up is simply the detail they carry.
pub(crate) fn deliver_rollup(rollup: &str) {
    let event = pns_domain::EventArgs {
        agent: "pns".to_string(),
        state: "observation".to_string(),
        detail: rollup.to_string(),
        ..pns_domain::EventArgs::default()
    };
    let _ = crate::run_event(
        &event,
        &crate::system_probes(),
        &crate::HookPayload::default(),
        crate::Attempt::of_state(&event.state),
    );
}
```

Finally, hold what was suppressed. In `pns/crates/pns/src/event_flow/execution.rs`, after the decision
and beside the existing `record_missed` call, record the hold when the profile is what emptied the
plan:

```rust
    // HELD RATHER THAN DROPPED, and only where the PROFILE is what emptied the
    // plan: an operator who was simply away is the missed journal's business,
    // and the two answer different questions.
    if !priority && !decision.legs.iter().any(|leg| leg.decorative) && profile_held(&overrides) {
        let _ = store.hold_event(&active_profile_name, &entry_of(event, now_secs));
    }
```

with the two helpers beside it:

```rust
/// Whether the active profile is what took the decorations away, rather than
/// presence or the operator's own mute.
fn profile_held(overrides: &Overrides) -> bool {
    overrides.profile != pns_domain::profiles::Profile::default()
        && !overrides.muted
        && !overrides.focus_active
}

/// The event as the hold keeps it: the six fields the missed journal keeps.
fn entry_of(event: &pns_domain::EventArgs, now_secs: Option<u64>) -> pns_domain::missed::Entry {
    pns_domain::missed::Entry {
        at: now_secs,
        agent: event.agent.clone(),
        state: event.state.clone(),
        project: event.project.clone(),
        branch: event.branch.clone(),
        detail: event.detail.clone(),
    }
}
```

`active_profile_name` is `overrides.profile`'s name, which `for_event` returns alongside it: widen that
function to `(Profile, String, bool)`, the profile, its name and the priority verdict, and update its
one caller in the same edit.

Add the third report line to `pns/crates/pns/src/command_profile.rs`'s `report`, now that there is
something to count:

```rust
    let held = records.held_events().unwrap_or_default();
    match held.first() {
        None => println!("     holding nothing"),
        Some((profile, _)) => {
            let entries: Vec<pns_domain::missed::Entry> =
                held.iter().map(|(_, entry)| entry.clone()).collect();
            println!(
                "     {}",
                pns_domain::profiles::held_rollup(profile, &entries)
                    .unwrap_or_else(|| "holding nothing".to_string())
            );
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path pns/Cargo.toml`
Then: `just test-rust` and `just lint-check`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add pns/crates
SKIP_AI_COMMIT=1 git commit -m "feat(pns): hold what a profile suppressed and flush it as one roll-up"
```

---

## The operator's acceptance

After the slice-5 apply, on dresden, with the gateway running:

1. `pns profile` prints the active profile, what chose it, and what it is holding.
2. `pns profile learn home` prints the row; paste it into `dot_config/pns/config-values.toml`, run
   `just pns-config-render`, and apply.
3. Add a rule naming that location, apply, and move the machine to another network.
4. Watch the gateway's log for the transition line within one `location_poll`, and read the row back
   with the store query in Task 18's test.
5. Send a priority page under `night` (`pns send --producer drill --state failed
   --delivery-class health --detail "floor drill"`) and confirm it lands on the phone.

That fifth step is the one no unit test stands in for: the floor is what the whole feature is judged
on.
