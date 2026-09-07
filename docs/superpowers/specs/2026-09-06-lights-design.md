# lights: a standalone keyboard controller for Hue lights (design)

Status: proposed specification. `lights`, `LightController` and `HueLightController` are settled names.

`lights` replaces `dot_local/libexec/executable_control-hue-lights.sh`, the bash script the seven
aerospace function keys call today. The script shells out to `openhue`, `jq` and an external option
parser, with repeated client invocations per action. `lights` is one binary that talks to the bridge
directly.

pns is not involved in any of this. pns is a notification system, and it supports lamps only as a way to
notify. Controlling the lights is a different job, so `lights` is a separate tool with its own config,
its own bridge client and no shared crate. When an action wants to announce itself, `lights` calls pns as
a producer, the same way the shell's long-command notifier does.

## The boundaries

Boundary names follow one rule. A trait is named for the role it fills, and every implementer is that
trait's own name with its technology in front, the way `FeedStore` is implemented by `CoreDataFeedStore`.
There are two boundaries here, `LightController` and `Notifier`, and each has a production implementer
and a test implementer built that way.

### LightController

`LightController` is a trait `lights` owns. Hue is one implementer, selected by `type = "hue"` in the
config. Nothing above the trait knows that Hue exists, that the transport is encrypted web traffic, or
that a room is addressed by a vendor resource identifier.

| Role       | Name                       |
| ---------- | -------------------------- |
| Trait      | `LightController`          |
| Production | `HueLightController`       |
| Test       | `RecordingLightController` |

The standalone tool owns this boundary and its Hue implementer. Neither belongs to pns, and the two
tools share no crate.

```rust
pub trait LightController {
    fn room(&self, name: &RoomName) -> Result<RoomState, LightControlError>;
    fn scenes(&self, room: &RoomRef) -> Result<Vec<SceneState>, LightControlError>;
    fn set_power(&self, room: &RoomRef, on: bool) -> Result<(), LightControlError>;
    fn set_brightness(&self, room: &RoomRef, change: BrightnessChange) -> Result<(), LightControlError>;
    fn set_scene(&self, scene: &SceneRef) -> Result<(), LightControlError>;
}
```

`RoomRef` and `SceneRef` wrap private `usize` indices into the controller's per-process snapshot. The
application exports `from_index(usize)` and `index() -> usize` for implementers in other crates,
including recording fakes. Only implementers construct or inspect them; use cases pass them unchanged.
They are opaque to policy, not secret or unforgeable capabilities. Each controller checks bounds and
resource kind before use, returning `InvalidReference` without a write for an invalid index.
References belong to the controller instance that produced them and cannot be persisted or passed
between instances.
Hue identifiers remain in the adapter's private tables, never in these public values.

```rust
pub struct RoomState {
    pub room: RoomRef, pub on: bool, pub brightness: Option<ReportedBrightness>,
}
pub struct SceneState { pub scene: SceneRef, pub name: String, pub active: bool }

pub enum BrightnessChange { Absolute(Brightness), Step { direction: Direction, percent: u8 } }
pub struct Brightness(u8);  // 1 to 100, smart constructor, no public field

pub enum LightControlError {
    Unreachable { detail: String },
    Refused { detail: String },
    UnknownRoom { name: String },
    UnknownScene { name: String, room: String },
    Malformed { detail: String },
    InvalidReference,
}
```

The schema defines grouped brightness as the average over lights that are on, without guaranteeing
whether dimming is present when all lights are off. A missing dimming value is `None`, never an invented
zero. Observed brightness uses a separate validated `ReportedBrightness` percentage that preserves the
schema's fractional 0 through 100 range; the request-only `Brightness` clamp must not alter readings.

### Notifier

`Notifier` is the second boundary, and it exists so a use case can announce what it did without knowing
that pns is what hears it, and so a test can assert the announcement without spawning a process.

```rust
pub trait Notifier {
    fn announce(&self, action: &Action);
}
```

| Role       | Name                |
| ---------- | ------------------- |
| Trait      | `Notifier`          |
| Production | `PnsNotifier`       |
| Test       | `RecordingNotifier` |

`announce` returns nothing. A notification that fails must never fail the light change, so there is no
outcome for a caller to mishandle.

### Reading the config is not a boundary

There is no trait over the config, and no `ConfigLoader` type. Settings are read once in `main.rs`,
before any use case exists, and are handed down as plain values; nothing above ever asks where they came
from. A trait there would have exactly one implementer and no seam to sit in, so parsing is a free
function, `settings::parse`, with a thin `settings::load` reading the file. If a second source ever
appears the boundary can be introduced then, and the rule above names it `SettingsSource` with
`TomlSettingsSource` beneath it.

## Behaviors

Line references name `dot_local/libexec/executable_control-hue-lights.sh` at `1cd246f3`. Rows distinguish
preserved Bash behavior from intentional Rust changes; schema facts do not establish hardware timing.

### Resolution and defaults

**L001. A command with no `--room` targets the configured default room.** Ships as `3F - Studio`.
Preserves bash:432.

**L002. A room alias expands to a full room name; anything else is passed through as typed.** The three
shipped aliases are `studio`, `bedroom` and `kitchen`. Preserves bash:154-162, with the mapping moved
from a `case` statement into config.

**L003. `lights` with no arguments toggles the default room's power.** Preserves bash:493-497.

### Power

**L004. `lights toggle` reads the room's aggregated on state and writes the opposite.** The bridge
reports a grouped light as on when any light in the group is on, so a half-lit room toggles off.
Preserves bash:190-204.

**L005. `lights on` and `lights off` write the requested state after resolving the room.** New behavior.
`room(name)` may read to obtain the `RoomRef`; its returned power never influences the requested value.
There is no extra read to decide whether to write, and no write is skipped because the room already
appears on or off. Bash had only a toggle.

### Brightness

**L006. A brightness step requests a 15 percentage point adjustment.** The step is configurable.
Preserves bash:333.

**L007. Absolute brightness requests clamp to 1 through 100; relative steps use bridge clipping.**
This changes Bash's 0 through 100 clamp at bash:341-345. See L022 for the reporting and hardware limits.

**L008. A brightness argument must be `up`, `down` or a non-negative integer.** Invalid syntax exits 1.
Bash accepts only the two directions (bash:312-320). Numeric input is new and clamps per L009/L022;
it is parsed before narrowing to `u8`, so 101 clamps to 100 and an overflowing integer is a usage error.

**L009. `lights brightness <n>` sets an absolute level.** New behavior, and it is where the domain clamp
lives.

### Scenes

**L010. `lights scene <name>` activates a scene by name in the target room.** Preserves bash:481.

**L011. `next` and `previous` cycle a configured rotation.** Ships as `Dimmed`, `Read`, `Energize`,
`Concentrate`. Preserves bash:224. The two scenes F4 and F7 set are deliberately not in the list, so
cycling away from either of them hits the L013 fallback.

**L012. Both directions wrap.** `next` from the last entry lands on the first; `previous` from the first
lands on the last. Preserves bash:240-247.

The bash gets the backward wrap right by accident. `$(((index - 1) % 4))` evaluates to `-1` at index
zero, and bash reads a negative array subscript from the end, so `${scenes[-1]}` is the last entry. The
same arithmetic in Rust underflows a `usize` before the modulo ever runs. The rotation must be written as
`(index + len - 1) % len`, and a test pins the wrap from index zero specifically.

**L013. A room sitting on a scene the rotation does not name starts the cycle at a configured fallback.**
Ships as `Read`. Preserves bash:234-238.

**L014. The active scene is the one the bridge reports with `status.active` set to `static`.** Preserves
bash:206-218. `static` is one of three values the bridge uses, alongside `inactive` and
`dynamic_palette`.

### Status

**L015. `lights status` prints the room's power, brightness and active scene, and writes nothing.**
Preserves the report fields at bash:397-418. Fractional brightness output is a deliberate change:
Bash truncates the reading to an integer at bash:277; Rust preserves a valid reported fraction.

**L016. A successful scene lookup with no static scene renders as `unknown`.** Deliberate change.
Bash prints a blank scene when its lookup succeeds without a static match; `unknown` is only its fallback
when that lookup fails (bash:206-218,411). Rust distinguishes absence from lookup failure: absence is
`unknown`, while a failed scene read exits 4 without printing a successful status report.

### Failures

Every failure prints one diagnostic line to stderr, prefixed `lights: `, and exits non-zero. Usage errors
may also print usage there. Moving room/scene validator diagnostics from stdout to stderr and assigning
distinct exit codes are deliberate changes. A rejected response never produces success output or a
notification; a lost write response cannot establish whether the physical write landed.

| Exit | Condition                                              | Relation to Bash                    |
| ---- | ------------------------------------------------------ | ----------------------------------- |
| 0    | Request accepted, status reported, or help printed      | Does not prove a physical end state |
| 1    | Usage error                                            | Retains catch-all exit 1            |
| 2    | Target room absent                                     | Changed from exit 1 on stdout       |
| 3    | Named scene absent in target room                      | Changed from exit 1 on stdout       |
| 4    | Transport, response, or controller-reference failure    | Uniform exit 4 contract is new      |
| 5    | Missing or invalid config                              | New                                 |

**L017. An unknown room exits 2 and names the room on stderr.** Deliberate change from bash:297-307,
which exits 1 and writes its diagnostic to stdout.

**L018. An unknown scene exits 3 and names both the scene and the room on stderr.** Deliberate change
from bash:350-361, which exits 1 and writes its diagnostic to stdout.

**L019. A bridge that does not answer within the timeout exits 4 and says so.** New behavior. The bash
has no explicit per-call timeout or uniform transport-error contract. `get_static_scene` catches failed
command pipelines, emits a diagnostic and returns 1 (bash:206-218). `show_status` substitutes `unknown`
for that failed lookup (bash:411), while scene rotation propagates the failure (bash:379). The Rust
timeout and uniform exit 4 contract are deliberate changes.

**L020. A config that names no bridge address or no key exits 5 rather than guessing.** New behavior.

### Notification

**L021. `--notify` sends one event to pns after a write response confirms acceptance, and never before.**
The response must pass status and envelope validation. Status and failed writes never notify. Replaces
bash:60-66, which called `osascript` directly.

The event is the producer argv the shell notifier already uses:

```
~/.local/libexec/pns/pns --agent lights --state done --project <room> \
  --detail "<action line>" --local-only
```

It is spawned detached with both streams discarded and its exit status ignored. A notification that fails
must never fail the light change, and a machine with no pns installed is silence, not an error.
`--local-only` is deliberate: you are standing in the room with your hand on the key, so there is nothing
for the phone to tell you.

## Deliberate brightness changes and hardware acceptance

These changes join the new commands, error contract, missing-scene rendering and notifier described
above. They are not claims of Bash parity.

**L022. Absolute requests have a software floor of 1, not 0.** Bash clamps down to 0, sends that value
and reports `0%` (bash:280-294,334-345). OpenHue's `Brightness.yaml` says a written zero selects the
device's lowest possible brightness; it does not identify that minimum as 1%. Choosing 1 through 100
is the Rust input policy, not a measured device range. `BrightnessSet` reports the requested level as
"requested 1%" at the floor. `BrightnessStepped` reports direction only, never a resulting percentage.
Neither performs a readback. Status reports a separate snapshot and cannot certify a previous write.
Brightness commands do not send power-off writes. `lights off` is available explicitly; the seven-key
map keeps F9 as toggle and has no dedicated off key. The operator verifies floor and power behavior.

**L023. Relative steps replace the Bash read-modify-write sequence.** OpenHue's `DimmingDelta.yaml`
specifies adjustment relative to the current brightness and clipping at the device minimum and maximum.
It does not promise accumulation for rapid requests or describe transition overlap. The old sequence
can reuse a stale reading, but neither a fixed bridge delay nor lost presses was measured here.
Sending one relative request per invocation removes our absolute read-modify-write race. Held-key
accumulation remains an operator acceptance requirement: temporarily configure a 5-point step and
compare five isolated presses with five rapid presses from 50%, in both directions, away from clipping.
Restore the default 15-point step afterward. If movement differs or
presses disappear, stop cutover and revise the design from that evidence. Do not claim five steps from
five requests based on the schema alone.

## The Hue implementer

The proposed adapter uses `https://<address>/clip/v2/resource` with a `hue-application-key` header.
The current design disables certificate verification for the configured bridge. The retained schemas
cannot verify the bridge's certificate or the supported rustls mechanism; confirm the transport setup
from authoritative source before implementing it. Do not infer that secure verification is impossible.
The retained OpenHue schemas identified in the plan support the resource fields and brightness limits.
Scene and write endpoint declarations still need authoritative source verification before their adapter
slices; they are not claimed as locally verified here. Hardware behavior and timing remain unmeasured.

### Endpoints

Paths below are relative to `/clip/v2/resource`.

```
GET  .                     read everything the bridge knows, in one response
PUT  grouped_light/<id>    {"on": {"on": true}}
PUT  grouped_light/<id>    {"dimming": {"brightness": 42}}
PUT  grouped_light/<id>    {"dimming_delta": {"action": "up", "brightness_delta": 15}}
PUT  scene/<id>            {"recall": {"action": "active"}}
```

A room object carries `metadata.name` and a `services` array; the entry whose `rtype` is `grouped_light`
holds the `rid` every write above is addressed to. A grouped light carries `on.on` and
`dimming.brightness`. A scene carries `metadata.name`, a `group` reference whose `rid` is its room, and
`status.active`.

`dimming_delta` takes `action` of `up`, `down` or `stop` and a `brightness_delta` between 0 and 100, and
the schema describes clipping at the device maximum and minimum. This moves clipping out of the
relative-step use case; it does not establish rapid-request accumulation (L023).

### The call budget

The adapter reads `GET /clip/v2/resource` at most once per process and memoizes it. Every trait read is
served from that one response, so resolving a room name, finding its grouped light, listing the room's
scenes and finding the active one all share a single round trip. Then the operation writes at most one
PUT.

The successful bulk path costs **one GET and one PUT per state-changing command; status costs one GET.**
Help, usage/config errors and unresolved targets do not require that full budget.
The memo is private to the adapter and invisible above it, and it is trivially safe because the process
is one shot and exits.

Room and scene names are matched against that one snapshot, and a scene is matched on its name **and**
its `group.rid`. The room filter is not optional. A scene name can occur in multiple rooms; matching
only the name could let a studio keypress activate a bedroom scene. Synthetic fixtures must exercise
that case.

The process reuses one HTTP (Hypertext Transfer Protocol) agent, allowing connection reuse and avoiding
repeated TLS (Transport Layer Security) handshakes when the connection stays open. The call timeout is
short because a human is holding a key down.

**The read strategy has an unmeasured acceptance bound.** The operator times the bulk read before
cutover. At or under 150 milliseconds it remains the chosen strategy. Over that bound, measure targeted
reads before adopting them in a separate implementation slice. This is a design selection, not an
automatic retry after a slow bulk request. A failed read is not retried inside the same invocation.

With the current `room(name) -> RoomState` port, targeted reads require both `GET .../room` for service
references and `GET .../grouped_light/<id>` for power and dimming. Even explicit on/off therefore cost
those two reads, although existing power cannot affect their write. A scene path also needs
`GET .../scene`, filtered to the resolved room. Each response is memoized for that invocation.

| Path                        | Targeted reads                           | Writes |
| --------------------------- | ---------------------------------------- | ------ |
| Toggle, on/off, brightness  | Room listing, selected grouped light     | 1      |
| Named or rotated scene     | Room listing, grouped light, scenes      | 1      |
| Status                     | Room listing, grouped light, scenes      | 0      |

The maximum targeted budget is three reads plus one write, or three reads for status. Room and scene
listings alone cannot implement this port. Record the measured strategy and budgets with the operator's
acceptance evidence; do not promise that more round trips will be faster.

### Response validation

Every response must have a successful HTTP status and a valid JSON (JavaScript Object Notation)
envelope with `errors` and `data` arrays. A nonempty `errors` array is `Refused`, including an HTTP 200
response with both data and errors. A missing or mistyped array, malformed body, or missing required
resource fields is `Malformed`. Both map to exit 4. Validate before caching reads or reporting write
success; none of these failures can notify. Diagnostics exclude credentials and raw response bodies.

## The command line

```
lights                                toggle the default room
lights toggle                         toggle power
lights on                             turn the room on
lights off                            turn the room off
lights brightness up                  one step brighter
lights brightness down                one step dimmer
lights brightness <n>                 request an absolute level, clamped to 1-100
lights scene <name>                   activate a scene by name
lights scene next                     rotate forward
lights scene previous                 rotate back
lights status                         print power, brightness and scene
lights --help                         print this

  --room <alias|name>   target a room (default: the configured default room)
  --notify              raise a notification for this command
```

An unknown subcommand or flag prints usage to stderr and exits 1. It never falls through to help with a
zero exit.

## The config

`~/.config/lights/config.toml`, deployed from a chezmoi template that reads the bridge address and key
from KeePassXC at apply time. Every key that has a default ships uncommented at that default, so the file
states what is true and nothing is discoverable only by reading source.

```toml
# The room a command targets when no --room is given.
default_room = "3F - Studio"

# Whether every action also raises a notification. --notify turns it on for a
# single command; this turns it on for all of them.
notify = false

# What controls the lights. `type` selects the compiled-in implementer, and it
# is required: a table naming none, or naming one nothing answers, is refused
# by name rather than read as this one.
[controller]
type = "hue"
address = "<from KeePassXC at apply time>"
key = "<from KeePassXC at apply time>"
# How long one bridge call may take, in seconds. Short on purpose: a key is
# held down while this runs.
timeout_secs = 2

[brightness]
# How far one step moves, in percentage points.
step = 15

# Short names --room accepts. A name this table does not list is sent to the
# bridge exactly as typed.
[rooms]
studio = "3F - Studio"
bedroom = "3F - Master Bedroom"
kitchen = "2F - Kitchen"

# The scenes `next` and `previous` cycle through, in order. A room sitting on a
# scene this list does not name starts the cycle at `fallback`. The one-shot
# mood scenes are deliberately absent: this is the working-light cycle.
[scenes]
rotation = ["Dimmed", "Read", "Energize", "Concentrate"]
fallback = "Read"
```

The bare keys precede every table, because a bare key written after a table header belongs to that table
in TOML (Tom's Obvious Minimal Language).

An unknown key is refused by name at load rather than ignored, which is what pns does and for the same
reason: a typo in a config key otherwise looks exactly like a setting that quietly does nothing.

## What the bash did that is dropped

1. **The debug log at `~/Library/Logs/smart-lights.log`.** Every invocation appended to it, nothing
   rotated it, and nothing ever read it. Failures now go to stderr, where the caller sees them.
1. **The `SUCCESS` and `FAILED` exit banner.** It printed into that same unread log, and it printed
   `SUCCESS` on every ordinary run. The exit code carries this.
1. **The terminal color branch.** All seven bindings run under aerospace with no terminal attached,
   so the color path was dead in real use and only the plain path ever ran. Status prints plain text.
1. **The `osascript` notification.** pns is the notifier now, and it already knows about presence, quiet
   hours and every other delivery decision that a raw `display notification` cannot make.
1. **Shelling out to `openhue`, `jq` and the external option parser.** The binary replaces these
   subprocess calls. The option parser stays installed because other scripts still use it; `openhue`
   remains an operator fallback, as recorded below.
1. **The `--next` and `--last` options.** They are declared in the `getopt` spec at bash:421-422 and have
   no matching `case` arm, so passing either one reaches the catch-all and errors out. They have never
   worked. `scene next` and `scene previous` are the spellings that do.
1. **The single-letter flags.** `-p`, `-b`, `-s`, `-S`, `-r` and `-N` become subcommands, so what a
   binding does is readable in the keybinding file without consulting the script.
1. **The two direct `openhue set scene` bindings.** F4 and F7 call `openhue` rather than the script
   today, which is why they bypass its room resolution entirely. All seven keys go through one tool.

## Decisions

Settled. Each is written into the design above; this is the record of what was chosen and why.

1. **Install to `~/.local/libexec/lights`.** The repository rule puts everything a keybinding, launchd, a
   hook or a `just` recipe invokes under `libexec`, and pns already lives there even though the operator
   types `pns doctor` by hand. The aerospace keys are the dominant caller.

1. **Brightness up and down send `dimming_delta`.** This removes the client's absolute
   read-modify-write race and delegates clipping to the bridge. Rapid accumulation is unverified until
   the operator runs L023. Absolute requests retain the software clamp and report requested levels.

1. **Start with one bulk read, memoized per process.** The operator measures the 150 millisecond bound.
   A slower result requires measuring the targeted strategy and its two- or three-read budget before
   adoption. There is no automatic fallback or readback in the current call budget.

1. **The absolute request floor is 1.** This is an input policy, not a measured device minimum. Relative
   steps report direction, and absolute writes report the requested level. See L022.

1. **Reuse the bridge application programming interface credential** named
   `OpenHue :: API Key (hue-bridge-pro)`. The existing template contract says this entry
   holds the address in its username field and the key in its password field, and the pns config template
   already reads the same two. One bridge credential in two places would be a rotation hazard.

1. **The `openhue` formula stays declared** in `.chezmoidata/system_packages_autoinstall.yaml`. After the
   cutover nothing in the tree calls it, but removal in this repository is manual by standing rule, and
   the cutover is not the moment to also uninstall the fallback.

1. **The rotation stays at four scenes, and the two Halo scenes stay out of it.** F4 and F7 remain
   one-shot keys, so `CC Halo Daylight` and `CC Halo Amber` are reachable only by pressing their own key,
   and cycling from either of them lands on the L013 fallback rather than advancing.

Adding both Halo scenes was considered and rejected. The argument for it was that F4 and F7 currently put
the room into a state F5 and F6 cannot cycle out of, which can read as a bug. The operator's answer is
that this is the intended shape: the four-scene rotation is the working-light cycle, and the two Halo
scenes are a separate one-shot mood the cycle is not meant to walk through. A six-entry rotation would
also make a full cycle six presses instead of four. Recorded here so a future reader knows the
alternative was weighed rather than missed.
