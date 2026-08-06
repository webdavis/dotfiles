# Local daemons: atuin, happy, tailscaled

Three long-running services on dresden, each with its own failure mode and diagnostic ladder. The
LaunchAgent plists live under `Library/LaunchAgents/`; the loaders that bootstrap them are
`.chezmoiscripts/run_onchange_after_*` scripts keyed on the plist's own hash, so a loader re-runs when
its plist changes rather than on every apply.

## Shell history (atuin)

Atuin daemon mode is enabled (`[daemon] enabled = true; autostart = false`, in
`dot_config/atuin/private_config.toml.tmpl`). The daemon's lifecycle is managed by
`~/Library/LaunchAgents/com.webdavis.atuin-daemon.plist` (`KeepAlive=true`, `RunAtLoad=true`, running
`atuin daemon start --force` so a stale socket from a prior crash auto-cleans on restart). Command
recording is decoupled from `PROMPT_COMMAND` via the daemon.

History is stored in SQLite under `~/.local/share/atuin/` (atuin's default location beneath
`XDG_DATA_HOME`, which `~/.bashrc` exports as `$HOME/.local/share`; no repo file sets `db_path`). Sync v2
records are opt-in (`[sync] records = true`), which future-proofs the local schema even though
`auto_sync = false`. `filter_mode = "host"` restricts Ctrl-R to the current machine's history, and
`filter_mode_shell_up_key_binding = "session"` is moot in practice because bashrc runs
`atuin init bash --disable-up-arrow`. Bash's built-in history is fully removed (no `HISTFILE`,
`HISTSIZE`, `histappend`, `HISTCONTROL` or `HISTIGNORE` anywhere in the bashrc); atuin owns all
recording.

**Diagnostic ladder** when history stops recording:

```bash
atuin doctor                              # built-in: socket, db, env, shell hooks
launchctl list | grep atuin               # status: '0' = healthy, '-' = not running
ps aux | grep '[a]tuin daemon'            # daemon process
tail ~/.local/log/atuin-daemon.log        # crash messages
atuin daemon status; atuin --version      # 'Version' line should equal 'atuin <ver>'
```

`atuin status` is for *sync* status only and errors when not logged in. It is not a "is the daemon
working" check; use `atuin daemon status` (reports `Version`, `Protocol`, `Healthy`) for daemon health.

**Past failures**, each now self-healing:

- A stale daemon socket under `~/.local/share/atuin/` caused `EADDRINUSE` restart loops. Fixed by
  `--force` in the plist.
- `bash-preexec` went missing after atuin 18.x dropped its bundle. Fixed by sourcing
  `${HOMEBREW_PREFIX}/etc/profile.d/bash-preexec.sh` in the bashrc before `atuin init`.
- `brew` upgrading atuin in-place while the daemon kept running stale code silently broke recording via
  gRPC schema drift. Two independent guards now catch it:
  `.chezmoiscripts/run_after_45-bounce-atuin-daemon-on-upgrade.sh.tmpl` compares the version recorded in
  `~/.local/share/atuin/atuin-daemon.pid` against `atuin --version`, and `dot_bashrc.tmpl` compares the
  binary's mtime against that same pid file right after `atuin init`. Either one triggers
  `launchctl kickstart -k gui/$(id -u)/com.webdavis.atuin-daemon`.

## Happy daemon (remote agent control)

[happy](https://happy.engineering/) bridges Claude Code sessions to the Happy mobile and web apps for
remote control; the local daemon is that bridge. Its lifecycle is managed by
`~/Library/LaunchAgents/com.webdavis.happy-daemon.plist` (`KeepAlive=true`, `RunAtLoad=true`), loaded by
`.chezmoiscripts/run_onchange_after_62-load-happy-daemon-launchagent.sh.tmpl` (`bootout` plus `bootstrap`
with a 3-try retry loop, mirroring the atuin loader). `happy` itself is an npm global tracked under
`npm:` in `.chezmoidata/system_packages_autoinstall.yaml`, and logs go to
`~/.local/log/happy-daemon.log`.

**The one gotcha: use `start-sync`, not `start`.** The plist runs `happy daemon start-sync`, which keeps
the daemon in the foreground. The documented command, `happy daemon start`, detaches (forks, then
returns), which under `KeepAlive` looks like an instant exit and restart-loops, orphaning a daemon each
cycle. `start-sync` is the foreground entry point that `start` spawns internally, and happy ships no
documented `--foreground` flag, so the plist comment is where the reason is recorded. launchd then
supervises a two-process tree: the `start-sync` process it keeps alive, which in turn manages the real
daemon.

**Diagnostic ladder** when remote control stops connecting:

```bash
happy daemon status                        # 'Daemon is running' + PID, port, version
launchctl list | grep happy                # col 1 = live PID, col 2 = last exit status
ps aux | grep '[h]appy daemon'             # supervised start-sync process + the daemon it spawns
tail ~/.local/log/happy-daemon.log         # crash messages
happy doctor                               # full diagnostics ('happy doctor clean' kills runaways)
```

## Tailscale (headless daemon)

Tailscale runs as the open-source `tailscale` **formula** (not the `tailscale-app` GUI cask) as a launchd
**system daemon** via `sudo tailscaled install-system-daemon`, which places a root-owned copy in
`/usr/local/bin` while the brew formula stays user-owned so `brew upgrade` runs unattended. It boots
before login and uses the `utun` interface, so there is no Network or System Extension to re-approve
after updates (the GUI variants' weakness on a headless host). State persists at `/Library/Tailscale`
across reboots.

Auth is a one-time manual `sudo tailscale up --accept-dns=true` plus flipping **Disable Key Expiry** on
the node in the admin console, after which node-key expiry will not force reauthentication (no auth keys,
no rotation, no KeePassXC). `run_onchange_after_66-tailscaled-status.sh.tmpl` is a sudo-free reminder: it
reads `tailscale status --json`, branches on `.BackendState`, and prints those one-time steps when the
daemon is starting, unauthenticated, awaiting machine auth or stopped. It is silent when the daemon is
running, exits 0 on every path, and never runs sudo or authenticates.

### DNS

Always `--accept-dns=true`, never a static `100.100.100.100` global resolver (that breaks off-tailnet).
The weak spot on the open-source macOS build is the resolver registration layer
(`tailscale/tailscale#13461`, `#19139`): tailscaled's internal MagicDNS resolver stays healthy, but its
registration of the `<tailnet>.ts.net` suffix route with macOS can silently half-fail (search-domain
fragment written, no nameserver route), including at home, so tailnet names stop resolving through the
system resolver while all other DNS works.

Remedy:

```bash
sudo tailscale set --accept-dns=false && sudo tailscale set --accept-dns=true
sudo dscacheutil -flushcache; sudo killall -HUP mDNSResponder
dscacheutil -q host -a name <peer>.<tailnet>.ts.net   # not dig, which bypasses /etc/resolver
```

Durable fallback: needed peers are pinned in `/etc/hosts` declaratively, from structured `tailnet_pins`
data in `.chezmoidata/macos_system_setup.yaml`. The Tier 2 sudo runner hands one pin per line to
`~/.local/libexec/tailscale/reconcile-hosts-pin.sh`, which converges the file to exactly one line per
pin. Tailscaled never manages `/etc/hosts`, so the entries coexist, and tailnet IPs are stable per node.

### Updates

`brew upgrade` updates the user-owned formula (no extension re-approval needed), but the running daemon
is a separate root-owned copy a formula upgrade does not touch. After upgrading the `tailscale` formula,
re-run `sudo /opt/homebrew/opt/tailscale/bin/tailscaled install-system-daemon` to refresh the daemon
copy. On dresden `sudo` is passwordless (the operator's `!authenticate` sudoers config, not managed by
this repo), so the re-copy is a single command; on a fresh machine expect a password prompt.

### Daemon-host role

When an always-home Mac exists and takes over the daemon-host role, dresden (which is carried) cuts back
to the GUI `tailscale-app` cask for better roaming DNS, and the always-home Mac runs this daemon. Make
the chezmoi config machine-conditional then.
