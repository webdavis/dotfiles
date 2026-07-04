# Headless tailscaled system daemon design

Date: 2026-06-23

## Context

This Mac is an always-on daemon host (Paseo/agent host). Remote access runs over Tailscale. The user's
recurring pain: an app needs a GUI click / re-approval after an OS or app update, which can't be done
remotely — locking them out. Tailscale's GUI variants (App Store; Standalone/macsys) are the cause: both
require a logged-in user AND run on a Network/System Extension that macOS can demand re-approval for after
updates.

**Evaluation (done, user approved):** switch from the `tailscale-app` GUI cask to the open-source
`tailscale` **formula** run as a launchd **system daemon**. The CLI daemon uses the `utun` interface (no
Network/System Extension → no re-approval click) and runs at boot before any login. Verified against
tailscale.com/docs/concepts/macos-variants, kb/1088/run-unattended, the Tailscaled-on-macOS wiki, and the
key-expiry/auth-keys/OAuth docs.

**Machine note (interim vs future):** dresden is currently the home daemon host, but it is also a
carried/roaming laptop. A new home MacBook (~3–6 months out) will take over the stationary daemon-host
role; at that point dresden cuts back to the GUI app (better roaming DNS, user present to click) and the
new Mac runs this tailscaled setup, made machine-conditional in chezmoi (by hostname/role). So this is the
**interim** config — and because dresden roams during this window, the DNS design below is deliberately
roaming-safe.

## Pressure-test results (done first)

- **State persists across reboots, no re-auth.** The system daemon stores node state at `/Library/Tailscale`;
  the launchd daemon loads it at boot. Auth survives reboots/power outages.
- **The re-auth-on-reboot bug (#17645) is GUI-only** — a regression in the macOS standalone .pkg/GUI build
  (v1.90.2, since-closed). It does not affect the open-source daemon; it's another reason to drop the GUI.
- **Daemon mechanism — DECIDED with evidence:** `brew info tailscale` caveats recommend
  `brew services start tailscale` (the formula ships a service). Use **`sudo brew services start tailscale`**
  (sudo → a `/Library/LaunchDaemons` system daemon, boot/pre-login) over `sudo tailscaled
  install-system-daemon` — the latter *copies* the binary to `/usr/local/bin`, so `brew upgrade` would
  leave the daemon on a stale copy (the atuin-daemon drift class). brew-services points at the brew binary.
- **The `tailscale` formula is already installed** but absent from the YAML, so the next
  `brew bundle --cleanup` would uninstall it. Adding it to the YAML is load-bearing, not just declarative.
- **Auth — Option A (user-chosen):** one-time interactive `sudo tailscale up` + flip "Disable Key Expiry"
  on the node in the admin console. Auth keys (90-day cap) and OAuth-client rotation were considered and
  rejected as fleet-provisioning machinery this single host doesn't need; disabling node-key expiry
  (default 180 days) makes the node never re-auth, so there is nothing to rotate.

## Goals

- Tailscale connectivity that survives reboots and app/OS updates with zero clicks and zero logins.
- Drop the GUI app; run the open-source daemon, codified in chezmoi.
- No ongoing key/auth maintenance.

## Non-goals

- OAuth-client auth-key automation (YAGNI for one node — see above).
- The other deferred remote-access features (Ethernet, Pi beachhead, fallback VPN) — separate specs.

## Design

**1. Packages** (`.chezmoidata/system_packages_autoinstall.yaml`): remove the `tailscale-app` cask; add
the `tailscale` formula (alphabetical, between `tart` and `tealdeer`).

**2. Daemon:** `sudo brew services start tailscale` → a system LaunchDaemon (root, boot, pre-login) running
the brew `tailscaled`, KeepAlive, pointed at state in `/Library/Tailscale`.

**3. Auth (Option A, one-time, interactive):** `sudo tailscale up` (click the login link), then in the
admin console flip **Disable Key Expiry** on this node. After that it never re-authenticates.

**4. Cutover** (one-time, user present at the machine — a brief Tailscale gap is locally recoverable):

1. Quit the GUI app; `brew uninstall --cask tailscale-app`.
2. `tailscale` formula is already installed (the YAML change keeps it).
3. `sudo brew services start tailscale`.
4. `sudo tailscale up --accept-dns` → authenticate.
5. Admin console → Disable Key Expiry on the node.
6. Remove the now-offline old GUI node from the admin console (cosmetic).

**5. chezmoi codification:**
- The YAML change above (the durable "keep the formula" guarantee).
- A darwin-gated `run_onchange_after_*` **status/remind** script (sudo-free): checks `tailscale status`;
  if the daemon isn't running it prints `sudo brew services start tailscale`; if `NeedsLogin` it prints
  the `sudo tailscale up` + Disable-Key-Expiry steps. It does NOT run sudo or auto-authenticate (Option A
  is deliberately manual). Mirrors the repo's "loader/remind" daemon scripts (atuin/happy).
- Add the new script to `find_shell_templates` in `scripts/lint.sh`.
- Document under a new "Tailscale (headless daemon)" subsection in `CLAUDE.md`.

**6. DNS — `--accept-dns=true`, NOT a static resolver (roaming-safe):** `tailscale up --accept-dns=true`
lets the daemon *dynamically* manage the resolver — tailnet names → `100.100.100.100`, everything else →
the current network's DNS, re-applied as dresden roams. Do **NOT** hardcode `100.100.100.100` as a static
DNS server — it breaks resolution off-tailnet (fatal for a roaming laptop). Caveat: the OSS `tailscaled`'s
macOS DNS handling is its known weak spot (version-specific regressions, e.g. issues #13461 / #14746), so
resolving *other tailnet* hostnames *from* dresden may be occasionally flaky while roaming — normal
internet DNS is unaffected, and connecting *to* dresden remotely is unaffected (that uses the client's
DNS). Plan-stage: verify MagicDNS resolves on dresden both at home and on a foreign network; if flaky, pin
the few needed tailnet hosts in `/etc/hosts` (static, works offline too) as the documented mitigation.

**7. Updates — specified, verify-in-plan:** the weekly brew-upgrade LaunchAgent already updates the
`tailscale` formula. The running daemon loads the new binary on the next **reboot**; for an immediate
refresh it needs `sudo brew services restart tailscale`. The weekly helper runs unattended (no sudo), so
the plan will decide between: (a) accept reboot-time refresh (verify tailscale tolerates a minor-version
client/daemon skew, unlike atuin's gRPC drift), or (b) a single scoped `NOPASSWD` sudoers entry for
`brew services restart tailscale` so the helper can bounce it. Default to (a) unless the skew proves
problematic.

## Testing / acceptance

- Post-cutover: `tailscale status` shows Connected; `tailscale ip` returns the node IP.
- MagicDNS: resolve a tailnet hostname.
- **Reboot survival** (the core promise): after a reboot, the daemon comes up authenticated with no
  interaction (test when a reboot is convenient — disruptive, so operator-scheduled).
- GUI app gone; no Network/System Extension prompt occurs on the next update.
- `just lint-check` + `just test` green for the repo changes.

## Risks / caveats

- **tailscaled on macOS is "less tested"** than the GUI (Tailscale's own wording) — acceptable for a
  daemon host; the CLI surface is stable.
- **Cutover needs sudo + interaction** (one-time, user present) — fine.
- **Brief Tailscale gap during cutover** — user is home; locally recoverable.
- **MagicDNS on the OSS macOS daemon** is the known weak spot (issues #13461/#14746): `--accept-dns` keeps
  normal DNS working while roaming, but resolving tailnet names *from* dresden may be flaky — mitigated by
  `/etc/hosts` pinning (verify-in-plan).
- **Post-upgrade bounce** needs sudo (verify-in-plan; reboot-time refresh is the default).

## Future work

When the new home MacBook (~3–6 months out) takes over the stationary daemon-host role: cut dresden back
to the GUI `tailscale-app` cask (better roaming DNS; user logged in + present to click), and run this
tailscaled setup on the new Mac instead — making the chezmoi config machine-conditional (by hostname/role)
so each machine gets the variant suited to it. Track this so it is not forgotten.

## Files touched

- `.chezmoidata/system_packages_autoinstall.yaml` (−`tailscale-app` cask, +`tailscale` formula)
- `.chezmoiscripts/run_onchange_after_66-tailscaled-status.sh.tmpl` (new — status/remind, sudo-free)
- `scripts/lint.sh` (add the new script to `find_shell_templates`)
- `CLAUDE.md` (new "Tailscale (headless daemon)" subsection)
- (possibly) `dot_local/bin/homebrew-weekly-upgrade.sh` + a sudoers entry — only if plan picks update-bounce option (b)
