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

  ⋅ pns tap --install

That command will print the line you need to paste into the following SSH authorization file:

  ⋅ ~/.ssh/authorized_keys

Then PNS will walk you through the rest of the setup.
```

**One known discrepancy, left in place deliberately.** The line says "the three global variables" and
four bullets follow, and the fourth (`SSH Key and type ed25519`) is a field on the SSH action rather
than a global variable. Recorded rather than corrected, because this file's job is to say what ships.
Fix it in the Shortcut first, then update this file to match.

## The notification, verbatim

The `Show notification` action's body:

```text
Received! Notifications will come to this phone.
```

Title is left empty. **Play Sound is on**, which is the point of the action: the tap's whole purpose
is to move notifications to the phone, and a silent confirmation would be indistinguishable from a
Shortcut that never ran. Attachment is left at `Choose Variable`.

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

**The undo is one file.** Delete the marker file and nothing else on the Mac changes: `pns tap`
writes that file and no other state. The surface then reads as untapped again, which can move
notifications back to the desk or away.

## What `pns tap` must print, for this to stay true

The notification above is fixed text on the phone, so it says the same thing whatever happened on the
Mac. That is a deliberate simplification with one real cost: a tap fired seconds after typing on the
Mac loses to the desk under newest-signal-wins, and the phone still says notifications will come here.

`pns tap`'s own stdout travels back over SSH, so a later revision can feed the notification from it
and tell the two cases apart. If that is done, the two strings are:

- the tap wins: `Notifications will come to this phone.`
- the desk is still newer: `Still going to <hostname>. You typed there more recently.`

Until then, `pns tap --info` on the Mac is the place that reports the real answer.
