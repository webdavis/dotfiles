# Keeping iMessages alive on a locked Mac

**System sleep — not the lock screen — kills iMessage syncing.** When a MacBook's display turns off, the
system quickly follows into full sleep, powering down Wi-Fi and severing the persistent connection to
Apple's push notification servers. The fix is straightforward: decouple display sleep from system sleep
so your screen goes dark and locks, but the Mac itself stays fully awake. This works on all recent macOS
versions (Ventura, Sonoma, Sequoia) and carries no meaningful security trade-offs. The lock screen
remains active and protective while iMessages flow in real-time.

## The real culprit is the sleep chain reaction

The problem feels like screen lock breaks iMessage, but the actual mechanism is a four-step cascade.
First, your inactivity timer expires and the screen locks. Second, display sleep kicks in and the screen
goes dark. Third — and this is the critical step — **the system enters full sleep within seconds on a
MacBook by default**. Fourth, full sleep suspends all processes, powers down the Wi-Fi chipset, and
terminates the persistent TLS connection to Apple's push notification service (APNs on TCP port 5223).
With that connection gone, new iMessages queue on Apple's servers until the Mac wakes and reconnects.

The lock screen is purely a security overlay. It does not affect network connectivity, background
processes, or app execution. Display sleep alone is also harmless — the CPU keeps running, all apps
function normally, and network connections remain active. The entire problem traces to **full system
sleep following display sleep almost immediately on laptops**. On MacBooks, Apple's default behavior
aggressively transitions from display-off to system sleep unless you explicitly prevent it.

## One checkbox fixes the core problem

The single most important setting lives in **System Settings → Battery → Options → "Prevent automatic
sleeping on power adapter when the display is off."** Enable this, and the system stays fully awake even
after the screen goes dark. iMessage keeps its APNs connection alive, and messages arrive in real-time.
This checkbox is the GUI equivalent of running `sudo pmset -c sleep 0` in Terminal.

For this to work reliably, three additional settings matter:

**Disable Low Power Mode entirely** (System Settings → Battery → Low Power Mode → Never). Multiple users
on Apple Community forums traced persistent iMessage sync failures to Low Power Mode, which aggressively
overrides other power settings and forces earlier sleep. One widely upvoted solution specifically
identified Low Power Mode as the sole cause when the Mac was unplugged.

**Set "Wake for network access" to Always** (System Settings → Battery → Options). While this primarily
allows other devices to wake your Mac over the network, users report it contributes to more reliable
background connectivity.

**Keep the MacBook plugged in.** The "prevent automatic sleeping" option explicitly applies only when on
the power adapter. On battery, the Mac will sleep regardless of this setting.

Configure your lock screen timing separately in **System Settings → Lock Screen**, setting "Turn display
off on power adapter when inactive" to your preferred timeout (5–10 minutes works well) and "Require
password after screen saver begins or display is turned off" to **Immediately**. Lock manually anytime
with **⌃⌘Q** (Control+Command+Q), which locks the screen without triggering sleep.

## Terminal commands for precise control

The `pmset` utility gives granular control over every power management parameter, and its settings
**persist across reboots** — no daemons or startup items required. Here is the recommended configuration
for an always-on iMessage Mac:

```bash
# Prevent system sleep on AC power (the critical command)
sudo pmset -c sleep 0

# Allow display to sleep after 5 minutes
sudo pmset -c displaysleep 5

# Keep TCP connections alive during any brief sleep transitions
sudo pmset -c tcpkeepalive 1

# Enable Power Nap as an extra safety net
sudo pmset -c powernap 1

# Enable Wake on LAN
sudo pmset -c womp 1

# Prevent deep standby and auto power-off
sudo pmset -c standby 0
sudo pmset -c autopoweroff 0

# Disable Low Power Mode
sudo pmset -c lowpowermode 0

# Verify your settings
pmset -g
```

The `-c` flag applies settings only when on the charger, which is ideal for a laptop that stays plugged
in at home. Use `-a` instead to apply to all power sources. Key parameters explained: **`sleep 0`**
disables system sleep entirely; **`displaysleep 5`** lets the screen go dark after 5 minutes of
inactivity; **`tcpkeepalive 1`** instructs the system to maintain TCP connections; **`standby 0`**
prevents the deeper standby mode where RAM contents get written to disk and RAM powers down.

The `caffeinate` command offers a lighter-weight alternative that doesn't require `sudo`:

```bash
# Prevent idle and system sleep, allow display to sleep (run in background)
caffeinate -is &
```

The `-i` flag prevents idle sleep and `-s` prevents system sleep on AC power. The `&` backgrounds the
process. To stop it: `killall caffeinate`. The drawback is that `caffeinate` does not persist across
reboots — you would need a launchd daemon or login item to make it permanent. For a home Mac that stays
plugged in, `pmset -c sleep 0` is the simpler, more robust choice.

## Amphetamine adds automation and reliability

For users who prefer a GUI tool with advanced control, **Amphetamine** (free on the Mac App Store) is the
most widely recommended option. It creates power management assertions that prevent sleep, and
critically, it offers an **"Allow Display Sleep" option** — meaning the screen goes dark while the system
stays awake, exactly what this use case requires.

Amphetamine's trigger system is particularly useful for an always-on iMessage Mac. You can configure it
to keep the Mac awake automatically when connected to your home Wi-Fi network, when plugged into a power
adapter, or when Messages.app is running. Add Amphetamine to your Login Items (System Settings → General
→ Login Items) and it starts automatically on boot.

Other alternatives include **KeepingYouAwake** (free, open-source `caffeinate` wrapper with a menu bar
icon), **Lungo** ($2.99), and **Caffeinated** ($3.99). None match Amphetamine's trigger-based automation
or its ability to independently control display sleep versus system sleep. The original Caffeine app is
discontinued, and InsomniaX is no longer maintained.

## Display sleep and system sleep are entirely different states

Understanding this distinction is essential. During **display sleep**, only the monitor turns off. The
CPU remains active, all applications continue executing, Wi-Fi stays connected, and push notifications
arrive normally. iMessage works perfectly in this state. During **system sleep**, the CPU suspends, the
Wi-Fi chipset powers down, all processes freeze, and every network connection terminates. iMessage stops
completely.

On older Macs, Energy Saver had two visible sliders making this distinction obvious. Modern macOS hides
it behind the "Prevent automatic sleeping when the display is off" checkbox, which effectively decouples
the two timers. Power Nap and Wake for Network Access provide **periodic** wakes during system sleep —
the Mac briefly reconnects to sync iCloud data including Messages — but these are not real-time. Power
Nap might sync every 15–60 minutes, which means significant message delivery delays. For real-time
iMessage delivery, **preventing system sleep entirely is the only reliable approach**.

On Apple Silicon Macs (M1 and later), Power Nap functionality is handled natively by the efficiency
cores, and the dedicated Power Nap toggle may not appear in System Settings. The `powernap` parameter
still works via `pmset`. Regardless, Power Nap remains periodic and insufficient for real-time message
delivery.

## Security stays intact when sleep is disabled

A locked, awake Mac has the **same security posture as a locked Mac with the screen on** — which is to
say, it's the normal state your Mac is in whenever you step away during active use. The lock screen
requires authentication before granting desktop access. FileVault encryption keys remain in memory (as
they do during any awake or standard-sleep state), protected by the Secure Enclave on Apple Silicon and
T2-chip Macs. DMA attacks via Thunderbolt are mitigated by hardware-level protections on all modern Macs.

The only theoretical difference is that an always-awake Mac maintains active network connections,
presenting a marginally larger attack surface than a sleeping Mac. For a home network environment, this
is negligible. Power consumption on an idle Apple Silicon Mac is minimal thanks to the efficiency cores —
the M-series chips draw very little power when idle with the display off.

One important note about lid behavior: **closing a MacBook lid forces sleep regardless of other
settings** unless you have an external display connected or use Amphetamine's closed-lid mode (version
5.0+). Since the user's Mac stays at home, keeping the lid open with the screen set to turn off after a
few minutes is the simplest approach.

## Conclusion

The fix requires just three actions: enable "Prevent automatic sleeping on power adapter when the display
is off," disable Low Power Mode, and keep the Mac plugged in. This lets the screen lock and go dark
normally while the system stays awake and iMessages arrive in real-time. For users wanting extra
reliability, `sudo pmset -c sleep 0` in Terminal achieves the same result and persists across reboots.
Amphetamine adds sophisticated automation for free. The key insight is that **screen lock and display
sleep are cosmetic** — they don't affect iMessage. Only system sleep, which kills Wi-Fi and severs the
APNs connection, causes the sync interruption. Prevent system sleep, and the problem disappears entirely.
