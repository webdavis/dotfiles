# The PNS Tap Apple Shortcut

The shipped text of the "PNS Tap" iOS Shortcut, recorded by the operator on 2026-09-09. This is the
half of the tap feature that lives on the phone; the Mac half is `pns tap`, tasks 71 to 77 in
`docs/remaining-work.md`.

The Shortcut is hosted publicly for other people to install, so everything below reaches readers who
have never seen this repository. Nothing here may assume it exists.

## Actions, in order

1. **Get all global variables**, which supplies the three values the SSH action reads.
1. **Comment**, carrying the text below. It ships with the Shortcut and is what an installer reads
   first.
1. **Run script over SSH**, whose script is a single colon. The Mac's `authorized_keys` line forces
   its own command, so the script text never runs.
1. **Show notification**, carrying the text below.

The `Run script over SSH` fields, as configured:

| Field          | Value                             |
| -------------- | --------------------------------- |
| Host           | the `Hostname` global variable    |
| Port           | the `SSH Port` global variable    |
| User           | the `Username` global variable    |
| Authentication | SSH Key                           |
| SSH Key        | ed25519 Key                       |
| Input          | Choose Variable (Shortcuts' own default) |

Four of those six rows are what an installer fills: Host, Port and User from the three global
variables, and SSH Key, reached by setting Authentication to SSH Key. Script stays a single colon and
Input stays at its default. That is the split the Comment below gets wrong, and the correction is
drafted further down.

## The Comment, verbatim

Reproduced exactly as it ships, including its typography. Do not reflow, restyle or correct it here:
this file records what the Shortcut actually says, and a copy that has been tidied is no longer a
record of anything.

```text
PNS Tap tells your Mac that your phone has your attention, so pns sends notifications here instead of to the Mac's desktop. It opens an SSH connection to your Mac and touches one file. Nothing is uploaded anywhere.

Set the three global variables below to your Mac’s:

  ⋅ Hostname
  ⋅ SSH Port
  ⋅ Account Username
  ⋅ SSH Key and type ed25519

Leave the script as a single colon; your Mac replaces it.

Then run the following command from a terminal on your Mac:

  ⋅ pns tap install

That command will print the line you need to paste into the following SSH authorization file:

  ⋅ ~/.ssh/authorized_keys

Then PNS will walk you through the rest of the setup.
```

**One defect still shipping, and one that fixed itself.** The line says "the three global variables"
and four bullets follow, and the fourth (`SSH Key and type ed25519`) is a field on the SSH action
rather than a global variable. That one is live. The text also points at `pns tap install`, which
did not exist when the text was written and does now, so that half needs no correction. The live one is
recorded rather than corrected in place, because this file's job is to say what ships. The replacement
is drafted below and becomes the record once the operator has edited the Shortcut.

## The notification, verbatim

The `Show notification` action's body:

```text
Received! Notifications will come to this phone.
```

Title is left empty. **Play Sound is on**, which is the point of the action: the tap's whole purpose
is to move notifications to the phone, and a silent confirmation would be indistinguishable from a
Shortcut that never ran. Attachment is left at `Choose Variable`.

## The corrected instructions, awaiting the operator's phone-side edit

**NOT YET SHIPPED, and therefore not yet a record of anything.** Everything above says what the
Shortcut currently contains. This section says what it should contain, in the order the edits are made
on the phone. When the operator has made them and a tap has been verified, these blocks replace the
verbatim blocks above and this section goes away.

### Edit one, the Comment

Three global variables, then four fields on the SSH action. Same typography as the shipped text, so
the only difference is the correction:

```text
PNS Tap tells your Mac that your phone has your attention, so pns sends notifications here instead of to the Mac's desktop. It opens an SSH connection to your Mac and touches one file. Nothing is uploaded anywhere.

Set these three global variables to your Mac’s:

  ⋅ Hostname
  ⋅ SSH Port
  ⋅ Account Username

Then open the Run Script Over SSH action and fill four fields:

  ⋅ Host: the Hostname variable
  ⋅ Port: the SSH Port variable
  ⋅ User: the Account Username variable
  ⋅ SSH Key: set Authentication to SSH Key, then choose your ed25519 key

Leave the script as a single colon; your Mac replaces it.

Then run this command in a terminal on your Mac:

  ⋅ pns tap install

It prints the line to paste into your Mac’s SSH authorization file:

  ⋅ ~/.ssh/authorized_keys

and it walks you through the rest of the setup, including how to test it.
```

### Edit two, the notification

The body stops being fixed text and becomes the SSH action's own result. In the `Show notification`
action, clear the body and insert the `Run script over SSH` action's output, which Shortcuts offers as
the most recent result when the body field is selected. Set the title to `PNS Tap`, because a bare
surface line needs to say who is talking. Play Sound stays on, for the reason already recorded above.

That one field is the whole success wiring. `pns tap` prints exactly one line on stdout when it
records a tap, so the notification then reads `Tap recorded. Current surface: Mobile.` when the tap
won, `... Away.` when it won and the operator is out, and `... Desk.` when desk input is newer and
notifications are staying on the Mac. Mobile and Away both mean the phone gets the cards; Desk means
it does not. The fixed text could not tell those apart, which is the defect being fixed.

**There is no failure branch, deliberately.** A failing `pns tap` prints its one line on stderr and
exits non-zero, so no failure text arrives in the action's result: the failure signal is the SSH action
itself failing, which stops the Shortcut at that action and leaves the phone showing the SSH error and
no confirmation. The Mac's own troubleshooting output already says exactly this.

Two consequences worth writing down rather than discovering later:

- Whether the Shortcuts SSH action fails on a non-zero exit status alone, or only when the connection
  fails, is NOT ESTABLISHED: Apple documents neither. If the drill in `pns-tap-device-acceptance.md`
  shows a write failure arriving as a successful action with empty output, then one `If` action is the
  fix, showing the result when it contains `Tap recorded` and a failure line when it does not. That is
  a measurement away, so it is not built now.
- Branching on the text to produce friendlier wording was rejected. It would copy pns's own phrasing
  into the phone, where nothing keeps the two in step, and a silently stale copy of a confirmation is
  worse than a plain one. A pass-through cannot drift.

## The `--json` result, and the undo

`pns tap --json` prints ONE JSON object on stdout and nothing else. The exit code follows `ok`: zero
when it is true, non-zero when it is false, so a caller can branch on either. A failure is reported
in the same object rather than on stderr, which is what makes the JSON form safe to parse
unconditionally.

The fields, pinned by the `pns.tap/1` schema identifier the object carries:

| Field                     | Value                                                         |
| ------------------------- | ------------------------------------------------------------- |
| `schema`                  | `pns.tap/1`                                                   |
| `operation`               | `tap`, `info` or `install`                                    |
| `ok`                      | whether the operation succeeded                               |
| `write_status`            | `recorded`, `failed` or `not_requested`                       |
| `marker`                  | the object below, or null when no marker was resolved         |
| `marker.path`             | the absolute marker path                                      |
| `marker.source`           | `environment`, `config` or `default`                          |
| `marker.config_file`      | where the config would be read from, whether or not it exists |
| `marker.exists`           | whether the marker is there, or null when it cannot be read   |
| `marker.mtime_epoch_secs` | the tap instant in epoch seconds, or null                     |
| `marker.touched_at`       | the same instant, RFC 3339 in UTC, or null                    |
| `marker.age_secs`         | seconds since the tap, or null                                |
| `marker.fresh`            | whether the tap is inside the desk window, or null            |
| `surface`                 | `desk`, `mobile` or `away`, or null when none was read         |
| `message`                 | one line for a person                                         |
| `install`                 | the `--install` guide, otherwise null                         |
| `error`                   | null, or `{"code", "message"}` naming what failed             |

`write_status` is `not_requested` under `--info` and `--install`. Only `--info` reads the marker;
`--install` reports its guide and nothing about this machine's state, so both `marker` and `surface`
are null there. They are also null whenever the run fails before reaching them: `marker` on a
`path_error` or `config_error`, `surface` on any failure before the surface is read. A caller that
reads either must handle null. `install` is populated only by `--install`. Adding a field keeps this
schema identifier; removing or renaming one does not.

**The undo is one file.** A tap creates two things on a fresh machine: the marker file, and any
missing directories on its path (mode 0700, so `~/.local/state/pns` and its parents may be new).
Only the marker carries state, so deleting it is the whole undo and nothing else on the Mac changes;
the empty directories stay behind and mean nothing. The surface then reads as untapped again, which
can move notifications back to the desk or away.

## What `pns tap` prints, and what the phone can show

Read off the command on 2026-09-14, so the wiring above is pinned to the binary rather than to an
intention:

- **A tap that records.** Exit 0, and one line on stdout: `Tap recorded. Current surface: Desk.`,
  `... Mobile.` or `... Away.`
- **A tap that cannot write.** Exit 1, and one line on stderr: `pns tap: ` followed by the marker path
  and the operating system error, errno included.
- **An argument it does not know.** Exit 2, and one line on stderr:
  `pns tap: usage: pns tap [info | install] [--json]`.

`--json` changes only the shape: one object on stdout either way, exit code following `ok`, failures
inside the object rather than on stderr. The phone's forced command on this Mac decides which form it
gets, and the plain form is the one the notification above is wired for.

The cost the fixed notification used to hide is now visible instead: a tap fired seconds after typing
on the Mac loses to the desk under newest-signal-wins, and the phone says `Current surface: Desk.`
rather than claiming success. `pns tap info` on the Mac remains the fuller answer, with the marker
path, its age and where the path came from.
