# lights: a standalone keyboard controller for Hue lights (design)

Status: proposed. Every name in this document is proposed and needs confirmation before anything is
created.

`lights` replaces `dot_local/libexec/executable_control-hue-lights.sh`, the bash script the seven
aerospace function keys call today. The script shells out to `openhue`, `jq` and GNU `getopt`, so one
keypress costs three to five process spawns and two or three separate `openhue` invocations, each of
which opens its own connection to the bridge. `lights` is one binary that talks to the bridge directly.

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
config. Nothing above the trait knows that Hue exists, that the transport is HTTPS, or that a room is
addressed by a UUID.

| Role       | Name                       |
| ---------- | -------------------------- |
| Trait      | `LightController`          |
| Production | `HueLightController`       |
| Test       | `RecordingLightController` |

The name is proposed, confirm before creating. The alternative is `RoomLights`, implemented by
`HueRoomLights`: every method on the trait addresses a room rather than an individual bulb, so
`RoomLights` puts the scope in the name and drops a role word that decades of MVC have worn thin.
`LightController` is still the recommendation, because it survives the test implementer's name better.
`RecordingLightController` reads as a thing that controls lights and records what it was asked to do,
while `RecordingRoomLights` reads as a collection of lights.

```rust
pub trait LightController {
    fn room(&self, name: &RoomName) -> Result<RoomState, LightControlError>;
    fn scenes(&self, room: &RoomRef) -> Result<Vec<SceneState>, LightControlError>;
    fn set_power(&self, room: &RoomRef, on: bool) -> Result<(), LightControlError>;
    fn set_brightness(&self, room: &RoomRef, change: BrightnessChange) -> Result<(), LightControlError>;
    fn set_scene(&self, scene: &SceneRef) -> Result<(), LightControlError>;
}
```

`RoomRef` and `SceneRef` are opaque newtypes the controller mints and the controller consumes. The domain
passes them around and never looks inside one, which is what keeps a Hue resource identifier from leaking
upward into policy.

```rust
pub struct RoomState { pub room: RoomRef, pub on: bool, pub brightness: Option<Brightness> }
pub struct SceneState { pub scene: SceneRef, pub name: String, pub active: bool }

pub enum BrightnessChange { Absolute(Brightness), Step { direction: Direction, percent: u8 } }
pub struct Brightness(u8);  // 1 to 100, smart constructor, no public field

pub enum LightControlError {
    Unreachable { detail: String },
    Refused { status: u16 },
    UnknownRoom { name: String },
    UnknownScene { name: String, room: String },
    Malformed { detail: String },
}
```

`brightness` is `Option` because the bridge reports a grouped light's dimming as the average over the
lights that are currently on. A room with every light off carries no brightness at all, and the type says
so rather than inventing a zero.

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

Each row names the bash lines it preserves, by line number in the script as it stands on `main`.

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

**L005. `lights on` and `lights off` write the state directly with no read.** New behavior. The bash had
only a toggle, so a binding that wanted a known end state could not have one.

### Brightness

**L006. A brightness step moves 15 percentage points.** The step is configurable. Preserves bash:333.

**L007. Brightness is clamped to the range the bridge actually honors.** See the correction in L022.
Preserves bash:341-345 in shape.

**L008. A direction that is not `up` or `down` and is not a number in range is a usage error.** Preserves
bash:319.

**L009. `lights brightness <n>` sets an absolute level.** New behavior, and it is where the domain clamp
lives.

### Scenes

**L010. `lights scene <name>` activates a scene by name in the target room.** Preserves bash:481.

**L011. `next` and `previous` cycle a configured rotation.** Ships as `Dimmed`, `Read`, `Energize`,
`Concentrate`. Preserves bash:224. What the list holds is an open operator decision; see the end of this
document.

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
Preserves bash:397-418.

**L016. A room whose active scene the bridge does not report renders as `unknown`.** Preserves bash:411.

### Failures

Every failure prints exactly one line to stderr, prefixed `lights: `, and exits non-zero. There is no
path on which a failed action exits zero.

| Exit | Condition                                                              | Preserves    |
| ---- | ---------------------------------------------------------------------- | ------------ |
| 0    | The action landed                                                      |              |
| 1    | Usage error: unknown subcommand, unknown flag, bad direction or number | bash:468-471 |
| 2    | The target room is not on the bridge                                   | bash:297-307 |
| 3    | The named scene is not in the target room                              | bash:350-361 |
| 4    | The bridge did not answer, or refused the call                         | new          |
| 5    | The config is missing, unparseable, or names no bridge and key         | new          |

**L017. An unknown room exits 2 and names the room.** Preserves bash:297-307.

**L018. An unknown scene exits 3 and names both the scene and the room.** Preserves bash:350-361.

**L019. A bridge that does not answer within the timeout exits 4 and says so.** New behavior. The bash
had no timeout and no transport error handling at all: `openhue` printing nothing and exiting zero on a
network failure would have made the script report success.

**L020. A config that names no bridge address or no key exits 5 rather than guessing.** New behavior.

### Notification

**L021. `--notify` sends one event to pns after the write lands, and never before.** Replaces bash:60-66,
which called `osascript` directly.

The event is the producer argv the shell notifier already uses:

```
~/.local/libexec/pns/pns --agent lights --state done --project <room> --detail "<action line>" --local-only
```

It is spawned detached with both streams discarded and its exit status ignored. A notification that fails
must never fail the light change, and a machine with no pns installed is silence, not an error.
`--local-only` is deliberate: you are standing in the room with your hand on the key, so there is nothing
for the phone to tell you.

## Deliberate corrections

Two behaviors change on purpose. Both are named here so nobody reads them as regressions.

**L022. The brightness floor is 1, not 0.** The bash clamps down to 0 and reports `0%`. The bridge
documents brightness as a percentage where "value cannot be 0, writing 0 changes it to lowest possible
brightness", so what the operator saw reported and what the lights did have never matched. `lights`
clamps to 1 and reports 1. Brightness down at the floor leaves the room at its dimmest and does not turn
it off; `lights off` is what turns it off, and it is bound to its own key.

**L023. A held brightness key composes.** The bash reads the current level, adds 15 and writes the
result. The bridge takes about a tenth of a second to reflect a write, so holding the key down issues
several reads that all see the same pre-change value and several writes that all name the same target.
The net movement is one step no matter how many times the key fires. `lights` sends the bridge's own
relative dimming instead, which the bridge applies to whatever the level is when it arrives, so five
presses move five steps.

## The Hue implementer

The bridge speaks CLIP v2 at `https://<address>/clip/v2/resource`, authenticated with a
`hue-application-key` header. It serves a self-signed certificate for its own address, so certificate
verification is disabled for these calls; there is no authority that could vouch for a Hue bridge, and
this is what `openhue` does as well. Every endpoint and field below was read from openhue's own OpenAPI
specification, which is the source its client is generated from.

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
the bridge documents it as clipping at the maximum and minimum levels itself. That is what makes L023
work and what moves the up and down clamp off our side.

### The call budget

The adapter reads `GET /clip/v2/resource` at most once per process and memoizes it. Every trait read is
served from that one response, so resolving a room name, finding its grouped light, listing the room's
scenes and finding the active one all share a single round trip. Then the operation writes at most one
PUT.

That gives a uniform budget: **one GET and one PUT for any action, one GET and nothing else for status.**
The memo is private to the adapter and invisible above it, and it is trivially safe because the process
is one shot and exits.

Room and scene names are matched against that one snapshot, and a scene is matched on its name **and**
its `group.rid`. The room filter is not optional. Hue creates its stock scenes, `Read` and `Concentrate`
among them, in every room, so a name alone is ambiguous on this bridge and would let a keypress in the
studio activate a scene in the bedroom.

The process uses one HTTP agent for both calls, so the TLS handshake is paid once rather than per call.
The call timeout is short on purpose, because a human is holding a key down.

**One thing here is unmeasured.** `GET /clip/v2/resource` returns every resource the bridge knows, and
nobody has timed it against this bridge. If it measures slower than roughly 150 milliseconds, the
fallback is two targeted listings, `GET .../room` and `GET .../scene`, which costs a second round trip on
the scene paths and keeps the same trait. The first live smoke run measures it, and the answer goes in a
decision record inside the crate.

## The command line

```
lights                                toggle the default room
lights toggle                         toggle power
lights on                             turn the room on
lights off                            turn the room off
lights brightness up                  one step brighter
lights brightness down                one step dimmer
lights brightness <1-100>             set an absolute level
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
# scene this list does not name starts the cycle at `fallback`.
[scenes]
rotation = ["Dimmed", "Read", "Energize", "Concentrate"]
fallback = "Read"
```

The bare keys precede every table, because a bare key written after a table header belongs to that table
in TOML.

An unknown key is refused by name at load rather than ignored, which is what pns does and for the same
reason: a typo in a config key otherwise looks exactly like a setting that quietly does nothing.

## What the bash did that is dropped

1. **The debug log at `~/Library/Logs/smart-lights.log`.** Every invocation appended to it, nothing
   rotated it, and nothing ever read it. Failures now go to stderr, where the caller sees them.
1. **The `SUCCESS` and `FAILED` exit banner.** It printed into that same unread log, and it printed
   `SUCCESS` on every ordinary run. The exit code carries this.
1. **The ANSI color branch.** All seven bindings run under aerospace with no terminal attached, so the
   color path was dead in real use and only the plain path ever ran. Status prints plain text.
1. **The `osascript` notification.** pns is the notifier now, and it already knows about presence, quiet
   hours and every other delivery decision that a raw `display notification` cannot make.
1. **`openhue`, `jq` and GNU `getopt`.** Three spawns and three install-time dependencies replaced by one
   binary. GNU `getopt` stays installed, because `dot_bashrc.tmpl` and the osquery allowlist script both
   still use it.
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
1. **Brightness up and down send `dimming_delta`.** The bridge applies it to whatever the level is when
   it arrives, so a held key composes instead of losing presses to a read-modify-write race, and the
   clamp is the bridge's own documented clipping. Absolute `lights brightness <n>` keeps a domain clamp.
1. **One bulk `GET /clip/v2/resource`, memoized per process.** It gives a uniform budget of one read and
   one write for any action. The bound is 150 milliseconds: the first live smoke times this call as a
   named step, and if it comes in slower the fallback is two targeted listings, `GET .../room` and
   `GET .../scene`, which costs a second round trip on the scene paths and changes no trait.
1. **The brightness floor is 1, and it is reported as 1.** See L022. The bridge rewrites a written 0 to
   its lowest level, so the old `0%` report was never true.
1. **The bridge credential is the existing `OpenHue :: API Key (hue-bridge-pro)` entry.** It already
   holds the address in its username field and the key in its password field, and the pns config template
   already reads the same two. One bridge credential in two places would be a rotation hazard.
1. **The `openhue` formula stays declared** in `.chezmoidata/system_packages_autoinstall.yaml`. After the
   cutover nothing in the tree calls it, but removal in this repository is manual by standing rule, and
   the cutover is not the moment to also uninstall the fallback.

### OPERATOR DECISION PENDING (recommended: add both Halo scenes)

**What `next` and `previous` cycle through.** Today the rotation is `Dimmed`, `Read`, `Energize`,
`Concentrate`, while F4 and F7 set `CC Halo Daylight` and `CC Halo Amber`, which are not in it. So
pressing F4 and then F6 does not advance from Daylight; it hits L013 and jumps to `Read`.

- **Add both Halo scenes to the rotation.** Six entries, and every scene reachable by a key is also
  reachable by cycling. The cost is that a full cycle now takes six presses instead of four.
- **Leave the rotation at four.** The Halo scenes stay as direct keys only, and the jump to `Read` after
  pressing one of them stays the behavior.

Recommended: add both. The current arrangement means two of the seven keys put the room into a state the
other two cannot cycle out of, which reads as a bug every time it happens.

This is a product behavior change and it is the operator's call. **Nothing in the plan depends on the
answer:** the rotation is a list in `~/.config/lights/config.toml`, so settling it edits one line of the
config template and no code, in any pull request or after all four.
