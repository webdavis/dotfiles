# Glossary

Every term below was read out of `src/` in this crate on 2026-09-02, not from a design document. Where a
word in circulation does not appear in the code, this file says so rather than legitimising it. The rule
for the whole refactor is that the code wins: if a later document and this glossary disagree, re-derive
the entry from `src/` before changing either.

## How to re-derive this file

```
grep -rn '^pub \(enum\|struct\|type\|trait\) ' src/*.rs src/channels/*.rs
```

That is the type inventory. The prose terms below were then confirmed by grepping for each word across
`src/` and reading the definition site.

## Words in circulation that the code does NOT use

| Word                         | Status in `src/`                                 | What the code says instead                                                                                                           |
| ---------------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| decision trace               | zero occurrences                                 | `decision ring` (the file `decisions`), and `journal` for the missed-notification file                                               |
| quiet place                  | one occurrence, in prose only, naming no concept | `quiet window`, `quiet hours`, `dim window`                                                                                          |
| home presence                | zero occurrences as a phrase                     | `home probe` and `router` in prose. But note the TYPE is named `HomePresence` (`src/home.rs`), so the word survives as an identifier |
| held light                   | zero occurrences                                 | `held` (`src/lights.rs:Held`, `HeldEntry`), and the `unread` lamp for the state itself                                               |
| plugin (as a universal role) | present, but as three distinct kinds             | `src/registry.rs:PluginKind` separates the kinds; a sensor is not a destination                                                      |

`signal` deserves its own line. In `src/` today it names two unrelated things and no pns concept: the Hue
bridge's own JSON field (`"signal": "on_off_color"` in `src/channels/hue.rs`) and the POSIX signal mask
built in `src/main.rs`. It is free for the refactor to take as the name of the normalized producer
concept, and taking it collides with nothing.

## `unread`, and where `glow` still lives

The lamp state is `unread`. An operator ruling on 2026-08-31 renamed it, and the rename landed in the
types: `src/config.rs:Behaviour::Unread`, `src/lights.rs:Unread` (the two flavours `Success` and
`Failure`), `src/lights.rs:Held::UnreadSuccess` and `Held::UnreadFailure`, and the colour constants
`UNREAD_SUCCESS_COLOR` and `FAILURE_COLOR` in `src/pulse.rs`.

`glow` was NOT eliminated. It survives in three places, and a refactor that assumes a clean rename will
be wrong about all three:

1. `lights-glow`, the legacy state entry. Two things commonly said about it are wrong, and both were
   checked here: it is a FILE, not a directory, and the migration DELETES it rather than reading it.
   `src/main.rs:sweep_legacy_state` calls `std::fs::remove_file` on `lights-glow` and on
   `lights-working-since`, and `std::fs::remove_dir_all` on the `lights-needs` directory, with no read of
   any of them. The test `src/main.rs:tests::the_first_tick_sweeps_the_state_the_old_names_held` pins
   exactly that, and its comment says why: the old held record "names lamps only the binary that is gone
   knew how to put out". The record that replaced it is `lights-held` (`src/main.rs:LIGHTS_HELD`).
1. Comment prose in `src/main.rs`, describing the steady write the lamp is holding.
1. Test names, for example
   `tests/dispatch.rs:the_operators_return_puts_out_a_glow_without_any_daemon_running` and
   `an_event_holding_no_glow_reaches_the_bridge_for_nothing`.

Prefer `unread` in new code and new prose. Renaming the surviving comments and test names is a separate
change. The three legacy names in `sweep_legacy_state` must not be renamed at all while that sweep is
still deployed, because the string in the source is the only thing that names the file to delete.

## Domain terms, by area

### Submission and the producer surface

| Term      | Defined at                  | What it is                                                             |
| --------- | --------------------------- | ---------------------------------------------------------------------- |
| producer  | `src/args.rs:EventArgs`     | Anything that states an event to pns in argv or through a hook         |
| event     | `src/channels/mod.rs:Event` | One rendered notification, as a destination receives it                |
| attempt   | `src/main.rs:Attempt`       | Which try this is: `First`, `Nudge` or `Observation`                   |
| decision  | `src/engine.rs:Decision`    | The verdict `decide` returns over a request and the machine's readings |
| overrides | `src/engine.rs:Overrides`   | Environment-supplied forcings that bypass a reading                    |

### Surface, presence and visibility

| Term                     | Defined at                                                 | What it is                                                                                  |
| ------------------------ | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| surface                  | `src/surface.rs:Surface`                                   | Where the operator is: `Desk`, `Mobile` or `Away`                                           |
| visibility               | `src/surface.rs:Visibility`                                | Whether the pane that produced the event is on screen                                       |
| session view             | `src/surface.rs:SessionView`                               | The herdr reading a visibility decision is taken from                                       |
| delivery plan            | `src/surface.rs:DeliveryPlan`                              | Which destinations the surface and visibility together allow                                |
| home probe               | `src/home.rs`, sensor `router` in `src/registry.rs:ROSTER` | The router reading that answers whether the operator's devices are home                     |
| home presence (the type) | `src/home.rs:HomePresence`                                 | `Home`, `NotHome` or `Unknown`. `Unknown` is preserved separately from `NotHome` on purpose |
| device key               | `src/home.rs:DeviceKey`                                    | Which identifier a configured device is matched by                                          |
| staleness                | `src/home.rs:Staleness`                                    | How out of date a router listing is allowed to be before the reading is refused             |

### Routing and delivery

| Term                | Defined at                           | What it is                                                                                       |
| ------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------ |
| leg                 | `src/routing.rs:Leg`                 | One destination this decision will be delivered to                                               |
| report mode         | `src/routing.rs:ReportMode`          | Whether a leg's outcome is printed. The wire words are `async` and `sync`, not the variant names |
| decorative          | `src/routing.rs:Leg::decorative`     | A leg whose failure is not worth journalling                                                     |
| routing declaration | `src/registry.rs:Routing`            | What a registration says about how its destination may be reached                                |
| plugin kind         | `src/registry.rs:PluginKind`         | `Channel` or `Sensor`. A sensor can never become a leg                                           |
| roster              | `src/registry.rs:ROSTER`             | The compiled-in registration table                                                               |
| core                | `src/registry.rs:CORE`               | The destinations that run when configuration cannot be read                                      |
| route               | `src/channels/hermes.rs:channel_url` | The named path a durable post is addressed to, selected by `--channel`                           |
| delivery            | `src/channels/mod.rs:Delivery`       | The outcome of one leg: `Silent`, `Delivered`, `Failed` or `Unlaunched`                          |
| dispatch precedence | `src/channels/mod.rs:native_first`   | Whether a compiled-in destination or an executable of the same name wins                         |

### Lighting

| Term                     | Defined at                                                | What it is                                                                           |
| ------------------------ | --------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| pulse                    | `src/pulse.rs`                                            | A timed blink on the lamps, fired on an exit code or an event                        |
| unread                   | `src/lights.rs:Unread`, `src/config.rs:Behaviour::Unread` | The steady lamp state saying there is news the operator has not seen                 |
| held                     | `src/lights.rs:Held`, `HeldEntry`                         | Which lamp is currently carrying which state, recorded so a later run can put it out |
| phase                    | `src/lights.rs:Phase`                                     | One step of a breath                                                                 |
| streak                   | `src/lights.rs:Streak`                                    | How long the working state has run without a break                                   |
| house                    | `src/lights.rs:House`                                     | The whole lamp picture one tick reconciles                                           |
| quiet window             | `src/channels/hue.rs:QuietWindow`                         | The configured hours in which the lamps stay dark                                    |
| dim window               | `src/channels/hue.rs:DimWindow`                           | The configured hours in which the lamps are allowed on, but dimmer                   |
| muting                   | `src/channels/hue.rs:Muting`, `src/lights.rs:Muted`       | Lamps the operator silenced by hand through `pns lights quiet`                       |
| fixture, lamp, inventory | `src/channels/hue.rs:Fixture`, `Lamp`, `Inventory`        | What the bridge reports it has                                                       |
| bridge                   | `src/channels/hue.rs:Bridge`                              | The trait the Hue transport is behind, so tests never reach a real bridge            |

### Persistence and coordination

| Term          | Defined at                                                                      | What it is                                                                  |
| ------------- | ------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| decision ring | `src/main.rs:DECISIONS`, format `src/decision_log.rs:Record`                    | A bounded ring of recent decisions, with free text reduced to `unprintable` |
| journal       | `src/main.rs:MISSED_NOTIFICATIONS`, entries `src/missed_notifications.rs:Entry` | The missed-notification file replayed when the operator returns             |
| activity ring | `src/main.rs:ACTIVITY`                                                          | The bounded ring the recap is composed from                                 |
| claim         | `src/main.rs:claim_by_rename`, `take_claim`, `Claimed`                          | Taking ownership of a file by renaming it, never by removing it             |
| lease         | `src/main.rs:ORDINARY_LEASE_SECS`, `JOURNALLED_LEASE_SECS`                      | A time-bounded hold on a job or a lamp                                      |
| marker        | `src/main.rs:marker_path`, `write_marker`                                       | A one-epoch file naming a state a later sweep will age out                  |
| ring lock     | `src/main.rs:claim_ring_lock`, `HeldLock`                                       | The exclusive-creation lock that serialises appends to a ring               |

### Jobs and diagnostics

| Term                        | Defined at                                      | What it is                                                        |
| --------------------------- | ----------------------------------------------- | ----------------------------------------------------------------- |
| job                         | `src/daemon.rs:Job`                             | A unit of work the clock runs between ticks                       |
| spool                       | `src/daemon.rs` (see `src/main.rs:drain_spool`) | Where scheduled jobs wait for the clock                           |
| tick                        | `src/main.rs:daemon_tick`                       | One pass of the clock                                             |
| nag                         | `src/nag.rs:Record`, `src/main.rs:nag_mode`     | The repeat card about an approval nobody answered                 |
| recap                       | `src/recap.rs`                                  | The composed account of what happened while the operator was away |
| timeline, section, evidence | `src/recap.rs:Timeline`, `Section`, `Sourced`   | The recap's structure and where each line came from               |
| doctor                      | `src/doctor.rs:Check`, `CheckKind`, `Outcome`   | The diagnostic census                                             |

## Naming rules this glossary is enforcing

1. A module or type is named for a pns concept, a capability, a policy, a protocol or an adapter, never
   for a vague role. `manager`, `processor`, `handler`, `service`, `helpers`, `utils`, `common` and
   `misc` are only acceptable where they accurately name a recognised role and no pns term exists.
1. A producer's own event name stays source metadata. It never becomes the value pns branches its
   routing, state or lighting policy on.
1. `Unknown` is never collapsed into a confidently negative reading. `src/home.rs:HomePresence` is the
   pattern: `NotHome` and `Unknown` are separate variants because the fail directions differ.
