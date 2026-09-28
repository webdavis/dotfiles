# macOS defaults

`.chezmoiscripts/run_onchange_after_40-sudo-macos-defaults.sh.tmpl` applies the declarative settings in
`.chezmoidata/macos_defaults.yaml` at `chezmoi apply` time on darwin, and no-ops on Linux. Mostly
per-user `defaults write` records; records carrying `scope: system` render instead as
`system_defaults_write <plist> ...` against `/Library/Preferences/<domain>`. The file also holds a
`killall` list (Dock, Finder, SystemUIServer, cfprefsd, in that order). Killing cfprefsd is what makes
plist changes take effect immediately.

A second data file, `.chezmoidata/posture_controls.yaml`, is verify-tier only and is read by the osquery
posture poller at runtime rather than by the runner.

Settings that need admin rights are plain scripts that carry `sudo` in their name and run back to back,
numbered 40 to 45, so one password covers them: `run_onchange_after_40-sudo-macos-defaults.sh.tmpl` (the
`scope: system` records), `run_after_41-sudo-osquery-known-good-manifests.sh`,
`run_after_42-sudo-install-nix.sh.tmpl`, `run_after_43-sudo-install-nix-repair-hook.sh.tmpl`,
`run_onchange_after_44-sudo-macos-firewall.sh.tmpl` and
`run_onchange_after_45-sudo-ssh-hardening.sh.tmpl`.

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

## Record schema

Each record under `macos.defaults` is one declared control. The runner refuses to render a record whose
fields do not fit its tier.

| Field        | Meaning                                                                                                      |
| ------------ | ------------------------------------------------------------------------------------------------------------ |
| `domain`     | The preference domain, such as `com.apple.dock` or `NSGlobalDomain`.                                         |
| `key`        | The preference key inside that domain.                                                                       |
| `type`       | The `defaults` value type, such as `bool`, `int`, `float` or `string`, on enforce and verify.                |
| `value`      | The declared value, which must match `type`, on enforce and verify.                                          |
| `tier`       | Required, and one of `enforce`, `verify` or `manual`.                                                        |
| `host`       | Optional: `current` writes through `defaults -currentHost`, and it cannot combine with system.               |
| `scope`      | Optional: `user` by default, or `system` for a root-owned plist written under sudo.                          |
| `plist_path` | Optional, system scope only: the absolute path of a plist kept outside `/Library/Preferences`, as LuLu's is. |
| `runbook`    | The section that fixes the control by hand: required on manual, optional on verify, forbidden on enforce.    |

- **`enforce`** is settable from the command line, and the runner renders one `defaults write` for it.
- **`verify`** is readable but not settable here, so nothing writes it and `just D` compares it.
- **`manual`** needs an interactive step or a profile, so the runner prints only its runbook pointer.

## Posture controls

Each record under `posture.controls` renders into `~/.local/libexec/posture/controls.json`, which
`posture poll` reads on every tick. Adding a control is a data change and an apply, never a poller edit.

| Field         | Meaning                                                                                              |
| ------------- | ---------------------------------------------------------------------------------------------------- |
| `id`          | Unique lowercase `[a-z0-9_]`, and the control's baseline field name, so never a built-in poller one. |
| `description` | The noun phrase a page prints to name the control.                                                   |
| `tier`        | Always `verify`, because the poller only reads state and never fixes it.                             |
| `reader`      | How the poller reads the control, one of the readers below.                                          |
| `expect`      | The value the control must hold, inside its reader's domain, and any deviation pages.                |
| `target`      | The absolute binary path whose LuLu rule must exist, required by the rule readers only.              |
| `remedy`      | Optional fix-it line for the page, kept free of apostrophes because the alert render is bash-quoted. |

| Reader                       | Values                | Note                                                                                 |
| ---------------------------- | --------------------- | ------------------------------------------------------------------------------------ |
| `fdesetup_status`            | `on`, `off`           | Runs `fdesetup status`, because osquery's disk query reads FileVault off on APFS.    |
| `csrutil_status`             | `enabled`, `disabled` | Runs `csrutil status`, and SIP is deliberately disabled on this machine.             |
| `defaults_autologin`         | `on`, `off`           | Reads the declared `autoLoginUser`, not the effective state FileVault can mask.      |
| `sysadminctl_guest`          | `enabled`, `disabled` | Runs `sysadminctl -guestAccount status`, which reports on stderr and exits 0 always. |
| `pgrep_oversight`            | `running`, `stopped`  | Probes the live process, because the app bundle stays on disk after a quit.          |
| `pgrep_lulu_extension`       | `running`, `stopped`  | Matches only root's processes, because the network extension runs as root.           |
| `lulu_rule_present`          | `present`, `absent`   | Searches the rules archive for a rule naming `target`.                               |
| `lulu_rule_resolved_present` | `present`, `absent`   | Resolves `target` first, following the Hermes venv symlink to the real interpreter.  |

Both rule readers prove only that a rule naming the binary exists, never that it allows. Both consult the
base rules archive, and the poller's active-profile guard pages when a LuLu profile makes that archive
stale.

## Aerospace required defaults

`com.apple.dock mru-spaces=false` is the single most common Aerospace breakage. Five
`com.apple.WindowManager` keys are tracked off as well: `GloballyEnabled` (Stage Manager),
`EnableStandardClickToShowDesktop`, `EnableTilingByEdgeDrag`, `EnableTilingOptionAccelerator` and
`EnableTopTilingByEdgeDrag`. The design spec at
`docs/superpowers/specs/2026-05-05-macos-defaults-management-design.md` carries the full list.

## Implementation gotchas that must not be "cleaned up"

- **`macos-defaults-drift.sh` reads its records from a here-string, never from a pipe**
  (`scripts/macos-defaults/macos-defaults-drift.sh`, `check_every_record`). Bash runs the right-hand side
  of a pipeline in a subshell, so the `drift_count` increments inside a `... | while` loop would be
  discarded after the loop, and `just D` would exit 0 even when drift exists, a silent false negative.
  The same applies to `unreadable_count`, which drives a separate fail-closed `exit 3`.
- **The Tier 1 runner template uses `{{ if index . "host" }}`, not `{{ if .host }}`.** Go's
  `text/template` errors with `map has no entry for key "host"` when the YAML record has no `host` field,
  which is the common case. The `index` form returns the empty value for absent keys (treated as falsy by
  `if`); the `.field` form throws. Don't simplify.
