# PNS Tap device acceptance

The tap is finished on the Mac and unfinished on the devices. `pns tap` records the phone's attention
signal and `pns tap --install` prints the setup, but nothing has been run yet against a sleeping Mac, a
Mac that is off, or a phone on somebody else's network. This file is that missing half: the drills the
operator runs, written down so they are not re-derived at the moment of running them.

Nothing below is a claim about how these devices behave. The expectations are what the code and Apple's
documentation imply, and the drill exists because an implication is not evidence. Record what actually
happened, including where it contradicts this file, and correct the file afterwards.

Both drills change device state on purpose: they put the Mac to sleep and shut it down. Neither changes
anything the feature owns beyond the marker file, and the undo for a stray tap is in
`pns-tap-apple-shortcut.md`.

## Before either drill

On the Mac:

- Remote Login is on, in System Settings, General, Sharing, Remote Login. Without it nothing else here
  means anything.
- The forced command from `pns tap --install` is in `~/.ssh/authorized_keys` for the key the phone holds.
- `pns tap --info` runs and reports a marker path. It reads; it never writes.

On the phone:

- The PNS Tap shortcut is configured against this Mac, per `pns-tap-apple-shortcut.md`.
- Settings, Shortcuts, Advanced, Allow Running Scripts is on. Apple documents this toggle for scripts in
  general rather than for the SSH (Secure Shell) action specifically, and people report the action
  failing without it, so check it before blaming the network.

Three readings taken on the Mac before the first drill, because the sleep drill is meaningless without
them. The values in parentheses are what this machine reported on 2026-09-14, kept as an example of what
the output looks like rather than as a target:

- `pmset -g custom` prints the power settings once per power source. `womp` is the wake-on-network
  setting, and it is per source (`womp 1` on the power adapter, `womp 0` on battery here).
- `system_profiler SPAirPortDataType | grep "Wake On Wireless"` says whether the wireless hardware can
  wake at all (`Supported` here).
- `pmset -g` names anything currently holding sleep off (`sleep 0 (sleep prevented by caffeinate ...)`
  here). A Mac that never sleeps cannot be drilled, which is why every sleeping case below forces sleep
  with `pmset sleepnow` rather than waiting for it.

## Why network wake is conditional

The tap never promises that a request wakes the Mac, and this is the reason. Four separate things have to
hold, and the operator controls only some of them:

- The setting is per power source. On a laptop it is System Settings, Battery, Options, Wake for network
  access, and `pmset -g custom` shows it as `womp` under Battery Power and again under AC Power. The same
  tap therefore behaves differently depending only on whether a cable is plugged in.
- The hardware has to support it. `Wake On Wireless: Supported` is the wireless half of that answer.
- Off the Mac's own subnet it needs a Bonjour sleep proxy answering on the phone's behalf. Apple's Remote
  Desktop guide states that without a sleep proxy running on the other subnets, a computer on a different
  subnet cannot be woken. A phone on cellular data has no wake path at all, however correct the Mac is.
- Waking takes time. Even in the good case the Mac has to wake, bring its network up and let sshd answer,
  and the phone's SSH action can give up first.

The last one produces the outcome worth naming: a tap can wake the Mac, be recorded on the Mac, and
still be reported to the phone as a failure. That is why state B below checks the marker after the phone
has already said no.

## Drill 1: sleep and wake

Four states, in this order, because each one leaves the Mac where the next one starts.

### State A: Mac awake

1. On the Mac, run `pns tap --info` and note the `Last tap` line.
1. On the phone, run the shortcut with the trigger you actually use.
1. Expect the shortcut to finish and show its confirmation.
1. On the Mac, run `pns tap --info` again. `Last tap` is now seconds ago and `Fresh` reads yes.

Record the confirmation text verbatim and the two `Last tap` readings.

### State B: asleep, network wake possible

1. Plug in the power adapter and confirm `pmset -g custom` shows `womp 1` under AC Power. Keep the phone
   on the same network as the Mac.
1. Run `pmset sleepnow` on the Mac and wait about thirty seconds so the sleep settles.
1. Run the shortcut on the phone and start timing.
1. Expect one of two outcomes. Both are legitimate results of this drill: the Mac wakes and the shortcut
   confirms, or the SSH action gives up before the Mac finishes waking and the phone shows that error.
1. Wake the Mac by hand and run `pns tap --info`.

Record which outcome, how long it took, and whether the marker moved. A marker seconds old after the
phone reported a failure is the split case from the section above, and it is the single most useful
observation in this drill.

### State C: asleep, no network wake

1. Unplug the power adapter, so the `womp 0` setting applies, or turn Wake for network access off.
1. Run `pmset sleepnow` and wait about thirty seconds.
1. Run the shortcut on the phone.
1. Expect the SSH action to fail. The shortcut stops at that action, so the phone shows the SSH error and
   never the confirmation.
1. Wake the Mac by hand and run `pns tap --info`. `Last tap` is unchanged from state B.

Record the error text verbatim and how long the phone took to show it.

### State D: Mac off

1. Shut the Mac down from the Apple menu.
1. Run the shortcut on the phone.
1. Expect the same shape of failure as state C, and possibly not the same text or the same delay: a
   machine that is off refuses or drops the connection where a sleeping one simply does not answer.
   Which of those two happens is part of what this state measures.
1. Start the Mac and run `pns tap --info`. `Last tap` is unchanged from state B.

Record the error text verbatim and how long the phone took to show it.

### The table

| State               | Power source | Phone network | What the phone showed | Seconds | Marker moved |
| ------------------- | ------------ | ------------- | --------------------- | ------- | ------------ |
| A, awake            |              |               |                       |         |              |
| B, asleep, wake on  |              |               |                       |         |              |
| C, asleep, wake off |              |               |                       |         |              |
| D, off              |              |               |                       |         |              |

Put the full text underneath the table when it does not fit in a cell. The exact words matter: the Mac's
own troubleshooting output claims the phone shows the SSH error, and this drill is what makes that claim
true or a defect.
