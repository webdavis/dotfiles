# Lamp drills

Three hardware drills still gate the key cutover: `seven_commands_preserve_key_intent`,
`brightness_floor_and_power` and `held_steps_match_isolated_steps`. They cannot be satisfied by fixtures
or request-count assertions, because what is under test is a real lamp: whether a request the binary
reports as accepted moves the room the way the old key did, whether the brightness floor stops short of a
power-off, and whether rapid relative steps accumulate the way isolated ones do. The fourth drill,
`bulk_read_latency`, is already answered: seven samples measured a 210 millisecond median against a 150
millisecond design bound, the targeted alternatives measured worse (267 and 455 milliseconds), and the
recorded verdict is to keep the bulk read.

This file is the runbook and the record. Fill the tables in place and keep them; the acceptance evidence
lives here, not in a chat log.

## The drill room

**The drills run in the Kitchen, or in the Master Bedroom. Not in the Studio.** F4 through F10 set the
Studio lights and the configured `default_room` is `3F - Studio`, so a bare `lights brightness up` is
what a key will run; but the Studio is the room an agent is watched from, and pns animates it while that
agent works. A level read mid-animation is not the level the command set. Every drill command below
therefore carries `--room kitchen`. Only the after-apply key-press check runs in the Studio, because by
then the keys themselves are what is under test.

The other rooms are quieter, not quiet. All three carry pns routes:

| Target                                                              | States it shows             | What that looks like                                      |
| ------------------------------------------------------------------- | --------------------------- | --------------------------------------------------------- |
| Rooms `3F - Studio`, `2F - Kitchen`, `3F - MBedroom`                | `done`, `failed`            | a transient flash as a turn ends, and on a failure        |
| `3F - Studio - HCL3`, `2F - Kitchen - HCD6`, `3F - MBedroom - HCL3` | `loop`, `blocked`, `unread` | a held state the lamp breathes until its condition clears |

The three held states arm on: `loop`, an agent streak past the configured 360 seconds, a shell command
running that long, or a live `pns loop` lease; `blocked`, an agent waiting on the operator (blocked,
asked, asking); `unread`, a failure the operator has not come back to. Between 22:00 and 07:00 those
three render dimmed rather than full, which changes the level again.

So a room is quiet for a reading when no agent session is working, no long shell command is in flight, no
loop lease is out, no approval is outstanding and no failure is unread. What the lamps are holding right
now is readable:

```bash
cat ~/.local/state/pns/lights-held     # each entry ends in the state word; no output means nothing held
```

Mute the drill room's two routed names for the length of the sitting and clear them afterwards. A
duration is `<count>s`, `<count>m` or `<count>h`, from 1 second to 24 hours:

```bash
pns lights mute "2F - Kitchen" 45m
pns lights mute "2F - Kitchen - HCD6" 45m
# afterwards
pns lights mute "2F - Kitchen" off
pns lights mute "2F - Kitchen - HCD6" off
```

In the Master Bedroom the pair is `3F - MBedroom` and `3F - MBedroom - HCL3`.

### Spelling the alternate room

`lights --room <alias|name>` takes a configured alias or a literal bridge room name, and a name that
matches no alias falls through as the literal name. `kitchen` resolves to `2F - Kitchen`, which is what
the bridge calls that room.

The shipped alias `bedroom` resolves to `3F - MBedroom`, which is what the bridge's own inventory calls
that room, so `--room bedroom` and `--room "3F - MBedroom"` reach the same room. Until 2026-09-14 both
the configuration template and the compiled default carried `3F - Master Bedroom`, a name the inventory
does not list, and `--room bedroom` exited 2 with `unknown room`.

### Exit codes

A row that "did nothing" is usually one of these rather than a lamp fault: 0 accepted, 1 usage, 2 unknown
room, 3 unknown scene in that room, 4 bridge unreachable, refused or malformed, 5 unreadable config.

## Drill 1: `seven_commands_preserve_key_intent`

**Purpose.** Prove that each command the cutover will bind to F4 through F10 has the same effect on the
real room as the binding it replaces, before any key is repointed.

Run these in order. `lights --room kitchen status` prints
`2F - Kitchen: ON | brightness: 42% | scene: Read`, and that is the reading to record in every row that
asks for one. In the Master Bedroom, pass `--room bedroom` in every row.

| #   | Key it stands for    | Command                                       | What to see on the lamp                               |
| --- | -------------------- | --------------------------------------------- | ----------------------------------------------------- |
| 1   | baseline             | `lights --room kitchen status`                | nothing changes; note power, brightness and scene     |
| 2   | F9                   | `lights --room kitchen toggle`                | the room goes dark (or lights, if it started off)     |
| 3   | F9                   | `lights --room kitchen toggle`                | the room returns to the state row 1 recorded          |
| 4   | F10                  | `lights --room kitchen brightness up`         | one visible step brighter; prints `brightness up`     |
| 5   | F8                   | `lights --room kitchen brightness down`       | one visible step dimmer; back to the row 1 level      |
| 6   | F6                   | `lights --room kitchen scene next`            | the rotation scene after the one row 1 recorded       |
| 7   | setup for the wraps  | `lights --room kitchen scene Energize`        | the last scene of the rotation activates              |
| 8   | F6 at the wrap       | `lights --room kitchen scene next`            | wraps forward to `Nightlight`, the first of the seven |
| 9   | F5 at the wrap       | `lights --room kitchen scene previous`        | wraps backward to `Energize`                          |
| 10  | F5                   | `lights --room kitchen scene previous`        | steps backward to `Read`                              |
| 11  | one-shot scene       | `lights --room kitchen scene "<other scene>"` | that scene activates, as a preset key would           |
| 12  | F6 off the rotation  | `lights --room kitchen scene next`            | the room lands on `Read`, the fallback                |
| 13  | one-shot scene again | `lights --room kitchen scene "<other scene>"` | the same scene, to set up row 14                      |
| 14  | F5 off the rotation  | `lights --room kitchen scene previous`        | the room lands on `Read` again                        |

Rows 11 through 14 stand in for a key that activates one named scene outside the rotation. Pick a scene
the drill room has that is outside the rotation (`Nightlight`, `Soho`, `Rest`, `Dimmed`, `Relax`, `Read`,
`Energize`), use it as `<other scene>`, and record the name. List them with:

```bash
openhue get scene --room "2F - Kitchen"
```

The behavior those rows pin is the behavior a named-scene key needs: a named scene activates, and cycling
from a scene the rotation does not contain lands on the configured fallback, `Read`.

**Values to record.** For each row: the printed line, the exit code, and the scene or brightness the lamp
actually shows (read it off the lamp, then confirm with `status`).

**Pass condition.** Every row exits 0, the printed line names the room and the action taken, the lamp
matches the intent column, rows 8 and 9 wrap where the table says, and rows 12 and 14 both land on
`Read`.

| Row | Printed line | Exit | Lamp shows | Matches intent (yes/no) |
| --- | ------------ | ---- | ---------- | ----------------------- |
| 1   |              |      |            |                         |
| 2   |              |      |            |                         |
| 3   |              |      |            |                         |
| 4   |              |      |            |                         |
| 5   |              |      |            |                         |
| 6   |              |      |            |                         |
| 7   |              |      |            |                         |
| 8   |              |      |            |                         |
| 9   |              |      |            |                         |
| 10  |              |      |            |                         |
| 11  |              |      |            |                         |
| 12  |              |      |            |                         |
| 13  |              |      |            |                         |
| 14  |              |      |            |                         |

Room used: \_\_\_\_\_\_\_\_\_\_\_\_\_\_ Substitute scene, if any: \_\_\_\_\_\_\_\_\_\_\_\_\_\_ Date:
\_\_\_\_\_\_\_\_\_\_

## Drill 2: `brightness_floor_and_power`

**Purpose.** Prove the deliberate floor change is safe on hardware: an absolute request is clamped to 1
rather than sent as 0, the reported level is the level requested and not a readback, and neither the
floor nor a step taken at the floor turns the room off.

The old script clamped down to 0 and reported `0%`. The new binary clamps to 1 through 100 and prints
`brightness requested 1%` at the floor, so the two lines differ by design. A relative step reports the
direction only and never a resulting percentage.

| #   | Command                                 | What to see on the lamp                                         |
| --- | --------------------------------------- | --------------------------------------------------------------- |
| 1   | `lights --room kitchen on`              | the room is lit; the drill starts from a known power state      |
| 2   | `lights --room kitchen brightness 50`   | a clear mid level; prints `brightness requested 50%`            |
| 3   | `lights --room kitchen brightness 1`    | the dimmest the lamp goes while still lit                       |
| 4   | `lights --room kitchen status`          | still `ON`; note the brightness the bridge reports at the floor |
| 5   | `lights --room kitchen brightness 0`    | no visible change; prints `brightness requested 1%`, never `0%` |
| 6   | `lights --room kitchen status`          | still `ON`, the same reported brightness as row 4               |
| 7   | `lights --room kitchen brightness down` | the lamp stays lit at the floor; prints `brightness down`       |
| 8   | `lights --room kitchen status`          | still `ON`; the room never powered itself off                   |

**Values to record.** The printed line for rows 2, 3, 5 and 7; the reported brightness and the power word
from every `status` row.

**Pass condition.** Rows 4, 6 and 8 all report `ON`; row 5 prints `requested 1%`; row 7 leaves the lamp
lit; nothing in the sequence sends a power-off write or leaves the room dark.

| Row | Printed line | Exit | Reported brightness | Power word | Lamp still lit (yes/no) |
| --- | ------------ | ---- | ------------------- | ---------- | ----------------------- |
| 2   |              |      |                     |            |                         |
| 3   |              |      |                     |            |                         |
| 4   |              |      |                     |            |                         |
| 5   |              |      |                     |            |                         |
| 6   |              |      |                     |            |                         |
| 7   |              |      |                     |            |                         |
| 8   |              |      |                     |            |                         |

Room used: \_\_\_\_\_\_\_\_\_\_\_\_\_\_ Date: \_\_\_\_\_\_\_\_\_\_

## Drill 3: `held_steps_match_isolated_steps`

**Purpose.** Prove that five rapid relative steps move the room as far as five isolated ones. Each
invocation sends a single relative request and the bridge is what accumulates; nothing in the schemas
promises that rapid requests all land, so hardware is the only evidence.

Use a 5 point step and start at 50 percent, which keeps both directions away from the clipping points at
1 and 100. Do not hand-edit the deployed configuration for this: it is a chezmoi-managed file and the
next apply would silently take the drill step back. Point the binary at a scratch copy instead. The copy
holds the bridge key, so keep it in a private directory and trash it afterwards.

```bash
drill="$(mktemp -d)"                                 # mktemp gives a 0700 directory
mkdir -p "$drill/lights"
cp ~/.config/lights/config.toml "$drill/lights/config.toml"
sed -i '' 's/^step = 15$/step = 5/' "$drill/lights/config.toml"
grep -n '^step' "$drill/lights/config.toml"          # confirm it reads step = 5
```

Every drill command then carries the scratch directory, and nothing else on the machine changes:

```bash
XDG_CONFIG_HOME="$drill" lights --room kitchen brightness down
```

Run the four passes below. "Isolated" means one command, wait two seconds, next command. "Rapid" means
five back to back with no wait, which is what a held key produces.

| Pass | Setup                                                          | The five invocations            |
| ---- | -------------------------------------------------------------- | ------------------------------- |
| A    | `XDG_CONFIG_HOME="$drill" lights --room kitchen brightness 50` | five isolated `brightness down` |
| B    | `XDG_CONFIG_HOME="$drill" lights --room kitchen brightness 50` | five rapid `brightness down`    |
| C    | `XDG_CONFIG_HOME="$drill" lights --room kitchen brightness 50` | five isolated `brightness up`   |
| D    | `XDG_CONFIG_HOME="$drill" lights --room kitchen brightness 50` | five rapid `brightness up`      |

The rapid passes are one line each:

```bash
for i in 1 2 3 4 5; do XDG_CONFIG_HOME="$drill" lights --room kitchen brightness down; done
```

After each pass read the result with `XDG_CONFIG_HOME="$drill" lights --room kitchen status`. During the
isolated passes also read the status after each single step, so a step that disappears shows up where it
happened rather than only in the total.

**Values to record.** The reported brightness after each pass, and for the isolated passes the reading
after each of the five steps.

**Pass condition.** Pass B lands within 1 point of pass A, and pass D within 1 point of pass C. The
isolated passes themselves land near 25 (down) and near 75 (up); a pass that ends at 45 has lost four of
its five steps. Any difference in movement, or a step that leaves no trace, stops the cutover and the
design gets revised from that evidence rather than from the schema.

| Pass | Step 1 | Step 2 | Step 3 | Step 4 | Step 5 | Final reported brightness |
| ---- | ------ | ------ | ------ | ------ | ------ | ------------------------- |
| A    |        |        |        |        |        |                           |
| B    | n/a    | n/a    | n/a    | n/a    | n/a    |                           |
| C    |        |        |        |        |        |                           |
| D    | n/a    | n/a    | n/a    | n/a    | n/a    |                           |

B within 1 point of A (yes/no): \_\_\_\_\_\_ D within 1 point of C (yes/no): \_\_\_\_\_\_ Room used:
\_\_\_\_\_\_\_\_\_\_\_\_ Date: \_\_\_\_\_\_\_\_\_\_

Afterwards, trash the scratch directory (it holds the bridge key) and confirm the deployed configuration
still reads `step = 15`:

```bash
trash "$drill"
grep -n '^step' ~/.config/lights/config.toml
```

Nothing needs restoring in `~/.config`: the drill never wrote there.

## After the apply: the key-press check

This is `seven_keys_preserve_actions`, the part of task 62 that only real keys can answer. It runs after
the operator's own apply has repointed the seven aerospace bindings, and it is the one part that does run
in the Studio, because that is the room the keys are for. Keep the Studio quiet the way the drill room
was kept quiet: no agent session working, `lights-held` empty, and the two routed names muted for the
sitting.

```bash
pns lights mute "3F - Studio" 20m
pns lights mute "3F - Studio - HCL3" 20m
# afterwards
pns lights mute "3F - Studio" off
pns lights mute "3F - Studio - HCL3" off
```

Press each key once, in this order, and watch the lamps rather than the terminal. The bindings are fire
and forget, so a key that does nothing prints nothing anywhere.

| Key                     | What it does                   | What to observe                                                                                            |
| ----------------------- | ------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| F9                      | toggle power                   | the room goes dark, and a second press brings it back                                                      |
| F10                     | one brightness step up         | one visible step brighter, the size the old key gave                                                       |
| F8                      | one brightness step down       | one visible step dimmer                                                                                    |
| F6                      | next scene in the rotation     | the next of `Nightlight`, `Soho`, `Rest`, `Dimmed`, `Relax`, `Read`, `Energize`                            |
| F5                      | previous scene in the rotation | the rotation steps backward, and wraps at `Nightlight`                                                     |
| F4                      | whole-house preset `dusk`      | all three rooms activate `Rest`                                                                            |
| F7                      | whole-house preset `night`     | all three rooms activate `Nightlight`                                                                      |
| F6 or F5 after F4 or F7 | cycle on from a preset scene   | the cycle continues from the preset's own scene, so F6 takes `Rest` to `Dimmed` and `Nightlight` to `Soho` |

**The pause.** The bridge stops reporting any scene as active once a room has sat untouched, which is why
a press after a long pause used to restart the cycle at `Read`. With `remember_position = true` under
`[scenes]` the press continues from the last rotation scene `lights` set in that room; with the setting
off it lands on `Read`. Leave ten minutes or so between the F6 rows to observe it, and read
`~/.local/state/lights/position.toml` to see what was remembered.

**Held keys.** The deployed step is back to 15 points, so from 50 percent there are only about three
steps before the lamp clips. Set a known level with `lights brightness 50`, hold F8 for about two
seconds, let go, and read `lights status`. Repeat with F10. Record how many discrete steps were visible,
where the lamp ended, and whether the room ever switched off. macOS key repeat is what decides whether a
held key fires the binding more than once, so "no repeat at all" is a legitimate observation rather than
a failure.

**Pass condition.** Every key performs the action its row names; both wraps behave; a preset key is a
one-shot whose only effect on the rotation is that the next cycle continues from the scene it set; and a
held key either does nothing extra or moves the room in whole steps, clipping at the floor without ever
powering it off.

| Key | Action seen | Matches the old binding (yes/no) | Notes |
| --- | ----------- | -------------------------------- | ----- |
| F4  |             |                                  |       |
| F5  |             |                                  |       |
| F6  |             |                                  |       |
| F7  |             |                                  |       |
| F8  |             |                                  |       |
| F9  |             |                                  |       |
| F10 |             |                                  |       |

| Held key | Start level | Steps seen | End level | Room ever off (yes/no) |
| -------- | ----------- | ---------- | --------- | ---------------------- |
| F8       |             |            |           |                        |
| F10      |             |            |           |                        |

Date: \_\_\_\_\_\_\_\_\_\_

## Checking `preset now`

`lights preset now` picks the preset whose `[[preset_windows]]` entry covers the current minute, so one
key can stand in for the five time-of-day presets. Nothing ships configured: with no `[[preset_windows]]`
entry the command exits 1 and names what to add, which is itself worth pressing once. To check the clock
path without touching the deployed configuration, copy it to a scratch directory the way drill 3 does,
append two windows covering the hour on either side of now, and run
`XDG_CONFIG_HOME="$drill" lights preset now` twice with a window boundary between the runs.

| What to run                                    | What to see                                                 |
| ---------------------------------------------- | ----------------------------------------------------------- |
| `lights preset now` with no windows configured | exit 1, one line naming `[[preset_windows]]`                |
| `XDG_CONFIG_HOME="$drill" lights preset now`   | the rooms the covering window's preset names, one line each |
| the same after the boundary passes             | the next window's preset instead                            |

A window whose end is earlier than its start wraps past midnight, two windows that overlap resolve to the
one written first, and a minute no window covers exits 1 naming that minute.

## Checking `--all`

`--all` sends a scene or a brightness to every room the alias table names, which on dresden is the
Studio, the Master Bedroom and the Kitchen. It reads the bridge once and then writes each room on its
own, so `lights --all scene next` leaves each room one step past the scene that room was actually on
rather than all three on one scene. A room that refuses does not stop the rest, and the exit code is the
first refusal's. `--all` and `--room` together are refused by name, and `--all` on anything but `scene`
or `brightness` is refused too, so the drill rooms above stay reachable one at a time. Nothing binds it
to a key yet: quiet all three rooms the way drill 1 quiets one before running it, or the pns routes will
move a lamp mid-reading.

| What to run                      | What to see                                                      |
| -------------------------------- | ---------------------------------------------------------------- |
| `lights --all brightness up`     | one visible step in all three rooms; one printed line per room   |
| `lights --all scene next`        | each room lands on the scene after its own, not on a shared one  |
| `lights --all --room kitchen on` | exit 1, one line naming both `--all` and `--room`, no lamp moves |

## Checking `--over`

`--over <duration>` hands the bridge its own transition duration rather than stepping the change from
here, so one write per room still goes out and the fade finishes even if the command is interrupted. A
brightness write carries it as `dynamics.duration` and a scene recall as `recall.duration`, both in
milliseconds. The accepted spellings are a whole number with one unit, `750ms`, `2s` or `5m`; anything
else, including a bare number, is refused by name before the bridge is read, as is a duration longer than
one hour and the flag on any command but `scene` or `brightness`. Omitting it changes nothing: no
transition field is sent at all. It composes with `--all`, which fades every room.

| What to run                            | What to see                                          |
| -------------------------------------- | ---------------------------------------------------- |
| `lights --over 5s brightness 40`       | the room ramps to 40% over about five seconds        |
| `lights --over 5s scene Read`          | the room crossfades into `Read` rather than snapping |
| `lights --all --over 5s brightness 40` | all three rooms ramp together, one printed line each |
| `lights --over 2 brightness 40`        | exit 1, one line naming `--over`, no lamp moves      |
| `lights --over 2s off`                 | exit 1, one line naming `--over`, no lamp moves      |

## When all four are filled

The cutover is complete only with drills 1 through 3 and the key-press check all recorded as passing. A
failure in any of them keeps the keys on the old script and gets reported as evidence; it is not grounds
for adding a retry or an accumulation mechanism that nothing measured.
