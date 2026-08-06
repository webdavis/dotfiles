# macOS defaults and system setup

Two `.chezmoiscripts/` runners apply declarative macOS settings at `chezmoi apply` time on darwin, and
no-op on Linux. A third data file, `.chezmoidata/macos_posture_controls.yaml`, is verify-tier only and is
read by the osquery posture poller at runtime rather than by either runner.

- `.chezmoidata/macos_defaults.yaml` plus `.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl`
  (Tier 1). Mostly per-user `defaults write` records; records carrying `scope: system` render instead as
  `system_defaults_write <plist> ...` against `/Library/Preferences/<domain>`. The file also holds a
  `killall` list (Dock, Finder, SystemUIServer, cfprefsd, in that order). Killing cfprefsd is what makes
  plist changes take effect immediately.
- `.chezmoidata/macos_system_setup.yaml` plus
  `.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl` (Tier 2). Sudo system commands (one
  `sudo -v` upfront, then a loop), plus structured `tailnet_pins` data. The runner early-returns when
  both lists are empty, and it emits the single `sudo -v` only when there is at least one enforce record
  or pin, so a file of purely verify or manual records prompts for nothing.

The `/etc/hosts` pin work is not inline in the Tier 2 template. The template hands one pin per line to
`~/.local/libexec/tailscale/reconcile-hosts-pin.sh` (source:
`dot_local/libexec/tailscale/executable_reconcile-hosts-pin.sh`), which converges the record to exactly
one line per pin rather than guarding and appending.

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

- **`macos-defaults-drift.sh` requires `shopt -s lastpipe`**
  (`dot_local/libexec/macos-defaults/executable_macos-defaults-drift.sh`, line 22). Bash's default
  behavior runs the right-hand side of a pipeline in a subshell, so the `drift_count` increments inside
  the `yq | while ...` loop would be discarded after the loop. Without `lastpipe`, `just D` would always
  exit 0 even when drift exists, a silent false negative. The same applies to `indeterminate_count`,
  which drives a separate fail-closed `exit 3`. The setting is a correctness requirement, not cosmetic.
- **The Tier 1 runner template uses `{{ if index . "host" }}`, not `{{ if .host }}`.** Go's
  `text/template` errors with `map has no entry for key "host"` when the YAML record has no `host` field,
  which is the common case. The `index` form returns the empty value for absent keys (treated as falsy by
  `if`); the `.field` form throws. Don't simplify.
