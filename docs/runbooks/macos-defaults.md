# macOS defaults

`.chezmoiscripts/run_onchange_after_40-sudo-macos-defaults.sh.tmpl` applies the declarative settings in
`.chezmoidata/macos_defaults.yaml` at `chezmoi apply` time on darwin, and no-ops on Linux. Mostly
per-user `defaults write` records; records carrying `scope: system` render instead as
`system_defaults_write <plist> ...` against `/Library/Preferences/<domain>`. The file also holds a
`killall` list (Dock, Finder, SystemUIServer, cfprefsd, in that order). Killing cfprefsd is what makes
plist changes take effect immediately.

A second data file, `.chezmoidata/macos_posture_controls.yaml`, is verify-tier only and is read by the
osquery posture poller at runtime rather than by the runner.

Settings that need admin rights are plain scripts that carry `sudo` in their name and run back to back,
numbered 40 to 46, so one password covers them: `run_onchange_after_40-sudo-macos-defaults.sh.tmpl` (the
`scope: system` records), `run_after_41-sudo-osquery-known-good-manifests.sh`,
`run_after_42-sudo-install-nix.sh.tmpl`, `run_after_43-sudo-install-nix-repair-hook.sh.tmpl`,
`run_onchange_after_44-sudo-macos-firewall.sh.tmpl`, `run_onchange_after_45-sudo-ssh-hardening.sh.tmpl`,
and `run_onchange_after_46-sudo-tailscale-magicdns-fallback-hosts.sh.tmpl`, which hands one host per line
from `.chezmoidata/tailscale.yaml` to `~/.cargo/bin/tailnet-pin` (built by
`run_after_37-build-tailnet-pin.sh.tmpl`) and refuses to run unless the builder's own record says that
binary was built from the source this apply rendered.

## Daily workflow

| Operation                           | Command                                          |
| ----------------------------------- | ------------------------------------------------ |
| Discover available domains          | `just defaults-list`                             |
| Browse one domain's keys            | `just defaults-show <domain>`                    |
| Bulk inspection (paged)             | `just defaults-dump`                             |
| Capture a setting into YAML         | `just defaults-capture <domain> <key> [current]` |
| Check for drift                     | `just D`                                         |
| Force reapply (revert disk to YAML) | `just defaults-apply`                            |

The capture helper is the canonical way to add a tracked setting: toggle it in System Settings, run
`just defaults-capture`, then `chezmoi apply` to commit. The helper refuses to silently overwrite a
tracked entry whose live value diverges from YAML (exits 4). Resolve that by running
`just defaults-apply` to revert the disk, or by hand-editing YAML to capture the new intent.

## Aerospace required defaults

`com.apple.dock mru-spaces=false` is the single most common Aerospace breakage. Five
`com.apple.WindowManager` keys are tracked off as well: `GloballyEnabled` (Stage Manager),
`EnableStandardClickToShowDesktop`, `EnableTilingByEdgeDrag`, `EnableTilingOptionAccelerator` and
`EnableTopTilingByEdgeDrag`. The design spec at
`docs/superpowers/specs/2026-05-05-macos-defaults-management-design.md` carries the full list.

## Implementation gotchas that must not be "cleaned up"

- **`macos-defaults-drift.sh` reads its records from a here-string, never from a pipe**
  (`scripts/macos-defaults/macos-defaults-drift.sh`, `check_every_record`). Bash runs the right-hand
  side of a pipeline in a subshell, so the `drift_count` increments inside a `... | while` loop would be
  discarded after the loop, and `just D` would exit 0 even when drift exists, a silent false negative.
  The same applies to `unreadable_count`, which drives a separate fail-closed `exit 3`.
- **The Tier 1 runner template uses `{{ if index . "host" }}`, not `{{ if .host }}`.** Go's
  `text/template` errors with `map has no entry for key "host"` when the YAML record has no `host` field,
  which is the common case. The `index` form returns the empty value for absent keys (treated as falsy by
  `if`); the `.field` form throws. Don't simplify.
