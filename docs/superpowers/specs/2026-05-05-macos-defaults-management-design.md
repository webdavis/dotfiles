# macOS Defaults Management Design Spec

**Date:** 2026-05-05
**Scope:** Declarative tracking of per-user macOS defaults and sudo-required system settings, with
chezmoi as the source of truth and a runbook for everything that can't be automated.
**Out of scope:** TCC privacy grants, Bluetooth/Wi-Fi pairing, Apple ID / iCloud sign-in, Touch ID
enrollment, Login Items, Hot Corners (deferred), App Store interactive purchase confirmations,
Karabiner-Elements rules (already managed by Karabiner JSON), SIP-protected toggles
(`nvram` / `csrutil`), per-app keyboard shortcuts (deferred to schema v2).

## Background

Goal: a fresh Mac should reach a known-good state with `chezmoi apply` plus one manual sudo invocation,
not a tribal-knowledge wandering tour. Today there is no such tracking — every defaults change is a
mental note that gets lost across machine rebuilds.

### Approaches considered

| # | Approach | Verdict | Reason |
|---|----------|---------|--------|
| 1 | **chezmoi-native** (this design) | **Chosen** | First-class fit with existing `.chezmoidata` + `.chezmoiscripts` patterns; zero new dependencies; KeePassXC interaction already proven by other templates. |
| 2 | `dsully/macos-defaults` | Rejected | Active `--dry-run` bug, bus factor 1 (single maintainer, sporadic commits). |
| 3 | `nix-darwin` | Rejected | Sudo on every apply, four open Tahoe-incompatibility issues, and a steep bootstrap cost the workstation doesn't otherwise need. |
| 4 | mathiasbynens-style inline bash | Rejected | Single shell file becomes a dump zone; harder to lint, harder to diff per-setting. The data/code split here is worth the slight overhead. |

## §1 — Architecture & file layout

Eight deliverables across three categories, plus three glue changes. All live in this chezmoi repo.

### Per-user defaults workflow (chezmoi-managed)

| Path | Purpose |
|------|---------|
| `.chezmoidata/macos_defaults.yaml` | Declarative data file. Schema: `{domain, key, type, value, host?}`. Plus a sibling `killall` list. |
| `.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl` | Tier 1 runner. `{{ if eq .chezmoi.os "darwin" }}` guarded; loops over YAML and runs `defaults [-currentHost] write` for each entry, then `killall` on each process. |
| `dot_local/bin/executable_macos-defaults-drift.sh` | Drift checker (`just D`). Read-only by construction; exits non-zero on drift. Linux-gated via `.chezmoiignore`. |
| `dot_local/bin/executable_macos-defaults-apply.sh` | Forced reapplier (`just defaults-apply`). Same logic as Tier 1 but invocable directly without bumping the YAML hash. Linux-gated. |
| `dot_local/bin/executable_macos-defaults-capture.sh` | Capture helper (`just defaults-capture <domain> <key> [--host current]`). Reads the live value+type via `defaults [-currentHost] read-type` + `defaults read`, normalizes to the schema's type tag, appends to `macos_defaults.yaml` if not already present, no-ops if the entry already matches. Used both for initial seeding and for ongoing "I just toggled this in System Settings, track it" workflow. Linux-gated. |

### Sudo-required system settings (chezmoi-managed; one prompt at apply time)

| Path | Purpose |
|------|---------|
| `.chezmoidata/macos_system_setup.yaml` | Declarative data: `{description, command, sudo}` triplets. |
| `.chezmoiscripts/run_onchange_after_40-macos-system-setup.sh.tmpl` | Tier 2 runner. `sudo -v` pre-flight, then loops over the YAML executing each command (with `sudo` prefix when `sudo: true`). |

### Manual setup reference (no execution)

| Path | Purpose |
|------|---------|
| `docs/runbooks/macos-fresh-machine-quickstart.md` | Checklist of TCC-gated and System-Settings-only steps. Read on fresh-machine bootstrap. |

### Glue changes

- `justfile` — add `D` (drift), `defaults-apply`, and `defaults-capture` recipes.
- `CLAUDE.md` — add a "macOS Defaults" section mirroring "Claude Code Settings" / "Homebrew install
  workflow".
- `.chezmoiignore` — gate the three helper scripts off Linux.

### Aerospace compatibility (required and recommended defaults)

Aerospace is the workstation's tiling window manager. Several macOS preferences govern how the OS
itself manages windows and Spaces; the wrong values break Aerospace even though
`~/.aerospace.toml` (chezmoi-tracked separately as `dot_aerospace.toml`) is untouched. These defaults
belong in `macos_defaults.yaml` from day one — they are first-class entries in this design, not edge
cases.

**Required (Aerospace breaks without these):**

| domain | key | type | value | Effect |
|--------|-----|------|-------|--------|
| `com.apple.dock` | `mru-spaces` | bool | `false` | Stops macOS from auto-reordering Spaces by recency, which scrambles Aerospace's workspace mapping. The single most common Aerospace breakage. |

**Recommended (Aerospace works without these but UX is cleaner):**

| domain | key | type | value | Effect |
|--------|-----|------|-------|--------|
| `com.apple.dock` | `expose-group-apps` | bool | `false` | Mission Control groups by app — collapses tiled windows. False keeps each tile addressable. |
| `com.apple.WindowManager` | `GloballyEnabled` | bool | `false` | Disables Stage Manager (actively fights tiling WMs). |
| `com.apple.WindowManager` | `EnableStandardClickToShowDesktop` | bool | `false` | Stops "click wallpaper to reveal desktop"; Sequoia ships this enabled and it hides Aerospace tiles on stray clicks. |
| `com.apple.WindowManager` | `EnableTilingByEdgeDrag` | bool | `false` | Disables Sequoia's drag-to-edge tiling (collides with Aerospace's tiling). |
| `com.apple.WindowManager` | `EnableTilingOptionAccelerator` | bool | `false` | Disables hold-Option-to-tile (collides with Aerospace's keybindings). |
| `com.apple.WindowManager` | `EnableTopTilingByEdgeDrag` | bool | `false` | Disables drag-to-top-edge-to-maximize. |

**Manual System Settings steps** (no reliable `defaults` equivalent — go in the runbook):

- **System Settings → Desktop & Dock → Mission Control → Displays have separate Spaces.** Per-machine
  preference (uriel: ON for the tri-monitor layout; single-monitor machines: OFF). No defaults
  equivalent that survives major-version changes.
- **System Settings → Desktop & Dock → Click wallpaper to reveal desktop.** Set to "Only in Stage
  Manager" — the `defaults` key changes name across Sequoia point releases, so the runbook approach is
  more durable than YAML.

### File-relationship diagram

```
.chezmoidata/macos_defaults.yaml ─┐
                                  ├─→ run_onchange_after_30 (chezmoi apply driven)
                                  ├─→ macos-defaults-drift.sh    ←─ just D
                                  ├─→ macos-defaults-apply.sh    ←─ just defaults-apply
                                  └─← macos-defaults-capture.sh  ←─ just defaults-capture <dom> <key>

.chezmoidata/macos_system_setup.yaml ─→ run_onchange_after_40 (chezmoi apply driven; sudo)

docs/runbooks/macos-fresh-machine-quickstart.md ←─ user reads on fresh-Mac bootstrap
```

## §2 — Data model

Two declarative YAML files under `.chezmoidata/`. Both are consumed by their corresponding runner
template at chezmoi apply time. The chezmoi hash gate makes "did anything change?" a free check.

### `.chezmoidata/macos_defaults.yaml`

```yaml
macos:
  defaults:
    - { domain: <string>, key: <string>, type: <string>, value: <scalar>, host: <string> }
  killall:
    - <process-name>
```

| Field | Required | Values | Notes |
|-------|----------|--------|-------|
| `domain` | yes | `com.apple.dock`, `NSGlobalDomain`, `com.apple.finder`, etc. | The preference domain. |
| `key` | yes | `tilesize`, `autohide`, etc. | The key within the domain. |
| `type` | yes | `bool` \| `int` \| `float` \| `string` | Explicit type tag — avoids YAML's inference footguns (e.g. the string `"yes"` silently becoming a boolean) and maps directly to `defaults write -<type>`. |
| `value` | yes | scalar matching `type` | Arrays/dicts not supported in v1; rare keys that need them go to manual setup doc and are deferred to schema v2. |
| `host` | no | `current` | Omitted = global storage in `~/Library/Preferences/<domain>.plist`. `current` = ByHost storage in `~/Library/Preferences/ByHost/<domain>.<HardwareUUID>.plist`; runner uses `defaults -currentHost write`. |

**Per-machine variance** via inline template branches inside scalar values:

```yaml
- domain: "com.apple.dock"
  key: "tilesize"
  type: int
  value: {{ if eq .chezmoi.hostname "dresden" }}48{{ else }}64{{ end }}
```

**ByHost example:**

```yaml
- domain: "com.apple.AppleMultitouchTrackpad"
  key: "Clicking"
  type: bool
  value: true
  host: current
```

**`killall` list** — processes restarted after the defaults loop completes. Defaults: `Dock`, `Finder`,
`SystemUIServer`, `cfprefsd`. The `cfprefsd` entry is non-obvious but critical: macOS caches preferences
in `cfprefsd`'s memory; without killing it, many `defaults write` calls don't take effect until the next
reboot or app relaunch.

### `.chezmoidata/macos_system_setup.yaml`

```yaml
macos:
  system_setup:
    - { description: <string>, command: <string>, sudo: <bool> }
```

| Field | Required | Notes |
|-------|----------|-------|
| `description` | yes | Human-readable label; runner echoes it before executing each command. Useful trace output during `just a` so the user sees what's happening. |
| `command` | yes | Bash command, **no `sudo` prefix** (the runner adds it when `sudo: true`). |
| `sudo` | yes | When true, runner prefixes with `sudo`. The Tier 2 runner does one `sudo -v` upfront so the user doesn't get spammed with prompts. |

**Idempotency contract** — Tier 2 commands must be idempotent. Most sudo system commands inherently are
(`pmset -c sleep 0` is a no-op when sleep is already 0). The user is responsible for adding only
idempotent commands; this is documented in the YAML's leading comment.

## §3 — Runtime semantics

### Tier 1 — `.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl`

- **Trigger:** chezmoi hash gate on the rendered template body. Re-runs only when `macos_defaults.yaml`
  content changes (template-substituted into the script).
- **OS guard:** `{{ if eq .chezmoi.os "darwin" }}...{{ end }}` outer wrap. Renders to empty body on
  Linux → chezmoi treats as no-op.
- **Pre-flight:** `osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true` —
  closes Settings if open. Critical: open Settings caches plist values and writes them back over yours
  when closed (long-standing macOS footgun).
- **Main loop:** for each record, emit
  `defaults ${host:+-currentHost} write "$domain" "$key" -$type "$value"`. The `${host:+...}` syntax
  includes the `-currentHost` flag only when the `host` field is set, blank otherwise.
- **Post-loop:** for each `killall` entry, `killall "$proc" 2>/dev/null || true`. The `cfprefsd` kill
  is non-negotiable for changes to take effect immediately.
- **Idempotency:** `defaults write` is overwrite-by-default and microsecond-cheap. No read-then-skip
  logic.

### Tier 2 — `.chezmoiscripts/run_onchange_after_40-macos-system-setup.sh.tmpl`

- **Trigger / OS guard:** same as Tier 1.
- **Pre-flight:** `sudo -v` upfront (refresh sudo timestamp). One password prompt at start, none during
  the loop.
- **Main loop:** for each record, `echo "→ $description"` then run the command. If `sudo: true` →
  `sudo $command`. Otherwise → bare `$command`.
- **No killall:** system commands (`pmset`, `systemsetup`, etc.) handle their own cache invalidation;
  no Dock/Finder restart needed.
- **Idempotency:** assumed per command (see §2 contract).

### Tier 1 helpers

#### `dot_local/bin/executable_macos-defaults-drift.sh` — `just D`

- Read-only by construction (no `defaults write` calls).
- Loop: for each YAML record, `defaults ${host:+-currentHost} read "$domain" "$key" 2>/dev/null` (or
  `<unset>` on failure). Compare against declared `value`. Bool normalization: `true`/`yes` → `1`,
  `false`/`no` → `0` (defaults stores bools as `0`/`1`).
- Output: tab-aligned table — `DOMAIN | KEY | EXPECTED | ACTUAL` — only drifted rows.
- Exit codes: `0` clean, `1` drift detected, `2` data file missing.

#### `dot_local/bin/executable_macos-defaults-apply.sh` — `just defaults-apply`

- Same `defaults write` + `killall` loop as Tier 1, but invocable on demand without bumping the
  chezmoi hash gate (so the user can replay the loop after fiddling in System Settings).
- Linux-gated via `.chezmoiignore`.

#### `dot_local/bin/executable_macos-defaults-capture.sh` — `just defaults-capture <domain> <key> [--host current]`

- **Inputs:** `<domain>` (e.g. `com.apple.dock`), `<key>` (e.g. `tilesize`), optional `--host current`
  flag (sets `host: current` on the emitted record, runner uses `defaults -currentHost`).
- **Read sequence:**
  1. `defaults [-currentHost] read-type "$domain" "$key"` → returns `Type is boolean|integer|float|string`
     (or non-zero if unset). Map to schema's `bool` / `int` / `float` / `string`.
  1. `defaults [-currentHost] read "$domain" "$key"` → returns the value. Bool normalization:
     `1` → `true`, `0` → `false` (so the YAML reads as the user expects, not as macOS stores it).
- **Write:** append `- { domain: <d>, key: <k>, type: <t>, value: <v>[, host: current] }` to the
  `macos.defaults:` list in `.chezmoidata/macos_defaults.yaml`.
- **Idempotency:** if a record with the same `(domain, key, host?)` already exists, the script no-ops
  with exit 0 if the captured value matches, and exits with `2 — drift` if the existing YAML value
  differs from disk (forcing the user to either `just defaults-apply` to revert, or hand-edit the YAML
  to capture intent).
- **Exit codes:** `0` appended or already in sync, `1` key not currently set on this Mac, `2` YAML
  drifts from disk (resolve before re-running), `3` malformed args.
- **Linting:** the appended record passes `yq` parse on subsequent runs.
- **Linux-gated** via `.chezmoiignore`.

## §4 — Bootstrap, modification ergonomics, out-of-scope

### §4.1 — Bootstrap flow

On a fresh Mac, ordered execution under `chezmoi apply`:

1. `run_once_before_00-install-homebrew.sh.tmpl` — Homebrew install.
1. `.install-password-manager.sh` — KeePassXC if missing.
1. `run_onchange_before_10-system-packages.sh.tmpl` — `brew bundle` from generated Brewfile.
1. File materialization (dotfiles, configs, scripts under `dot_local/bin/`).
1. **`run_onchange_after_30-macos-defaults.sh.tmpl`** — defaults loop + killall + cfprefsd kill.
1. **`run_onchange_after_40-macos-system-setup.sh.tmpl`** — sudo system commands.

**Why this order:**

- Defaults run *after* package install so apps targeted by `defaults write` (Dock tile sizes, Finder
  behavior, Karabiner) actually exist on disk. Writing to a not-yet-installed app's domain succeeds at
  the `defaults` level but the app may overwrite the plist on first launch.
- System-setup runs *after* defaults so the user doesn't stare at two sudo prompts before the visible
  portion of the apply (file install, defaults) is even done.
- `cfprefsd` kill is the closing act of Tier 1; everything that runs after it (Tier 2) doesn't read
  user defaults, so the cache flush is harmless.

**User-side manual steps gating first apply** (also in the runbook):

- Install Xcode CLT (`xcode-select --install`) before `chezmoi init`.
- `chezmoi init <repo>` plus KeePassXC db retrieved from offline backup or iCloud Drive.
- Sign into Apple ID (so iCloud Drive populates and `mas` can install App Store apps).

**User-side manual steps after first apply:** TCC grants, Bluetooth pairing, browser sign-ins, app
licenses (see §4.3).

### §4.2 — Modification ergonomics

| Operation | Workflow |
|-----------|----------|
| **Add a new default** | Toggle the setting in System Settings → `just defaults-capture <domain> <key> [--host current]` (the helper reads the live value+type and appends a normalized record to `macos_defaults.yaml`) → `chezmoi apply` (hash gate fires, runner replays the full loop, killalls fire). |
| **Change an existing value** | Edit the value in YAML → `chezmoi apply`. |
| **Remove a default** | Two-step: delete from YAML → run `defaults delete <domain> <key>` manually. The runner is intentionally write-only; auto-delete is out of scope to keep the apply loop side-effect predictable. Documented in the runbook. |
| **Per-machine variance** | Edit the inline template branch: `value: {{ if eq .chezmoi.hostname "dresden" }}48{{ else }}64{{ end }}`. |

**Drift workflow** (when `just D` shows differences):

```
just D
→ DOMAIN              KEY        EXPECTED  ACTUAL
→ com.apple.dock      tilesize   64        48
→ Drift detected (exit 1)
```

User decides per row:

- **Capture intent** (the GUI change is what I want now): copy ACTUAL into YAML; `chezmoi apply` no-ops
  because there's nothing left to drift.
- **Revert** (someone or something else changed it; my YAML is canonical): `just defaults-apply`
  replays the loop without bumping the hash gate.

The two recipes are the entire user-facing surface for "something is out of sync." `chezmoi apply`
itself stays uninvolved, matching the "observation tools never mutate" principle from earlier in the
design discussion.

### §4.3 — Out-of-scope items

These live in `docs/runbooks/macos-fresh-machine-quickstart.md` as a checklist, not in YAML:

| Category | Why it's out | Lives in |
|----------|--------------|----------|
| **TCC privacy grants** (Full Disk Access for Ghostty/Karabiner/Hammerspoon, Screen Recording, Accessibility) | No CLI surface; macOS requires GUI drag-and-drop into the Privacy sheet for the user-consent integrity guarantee. | Runbook |
| **iCloud / Apple ID sign-in** | Interactive 2FA flow. | Runbook |
| **Bluetooth pairing** (AirPods, mice, keyboards) | Per-device pairing button. | Runbook |
| **Wi-Fi profiles / 802.1X** | Captive portals + interactive auth. | Runbook |
| **Touch ID enrollment** | Requires physical sensor interaction. | Runbook |
| **Login Items** | `osascript` integration with System Settings → General is flaky on Sonoma+. | Runbook (manual step) |
| **Hot Corners / Mission Control assignments** | Possible via `defaults` but undocumented keys vary by macOS major version; high maintenance, low value for v1. | Deferred to v2 |
| **App Store apps requiring purchase confirmation** | `mas` can't bypass the "click to confirm purchase" sheet. | Runbook (after `mas install <id>`, click confirmation) |
| **Karabiner-Elements rules** | Already managed by Karabiner's own JSON in `dot_config/private_karabiner/`. | Out of scope here, lives there |
| **SIP-protected toggles** (`nvram`, `csrutil`) | Recovery-mode-only commands; running them outside of recovery silently no-ops. | Runbook (only relevant on a brand-new Mac) |
| **Per-app keyboard shortcuts** (System Settings → Keyboard → Keyboard Shortcuts → App Shortcuts) | Stored in `NSGlobalDomain` `NSUserKeyEquivalents` per-bundle-ID dict; arrays-of-dicts not supported by v1 schema. | Deferred to v2 with schema extension |

## Implementation order

1. **Data files first** — `.chezmoidata/macos_defaults.yaml` and `.chezmoidata/macos_system_setup.yaml`
   with empty `defaults:`, `killall:`, and `system_setup:` arrays plus leading comments. Lints clean
   via `just y`.
1. **Tier 1 runner** — `.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl`. Test with the
   YAML still empty (loop is a no-op).
1. **Drift, apply, and capture helpers** — `dot_local/bin/executable_macos-defaults-drift.sh`,
   `executable_macos-defaults-apply.sh`, and `executable_macos-defaults-capture.sh`. `.chezmoiignore`
   Linux gate added for all three.
1. **Tier 2 runner** — `.chezmoiscripts/run_onchange_after_40-macos-system-setup.sh.tmpl`.
1. **Justfile recipes** — `D` (drift), `defaults-apply`, and `defaults-capture`.
1. **Aerospace baseline** — populate `macos_defaults.yaml` with the §1 Aerospace-required and
   Aerospace-recommended entries (the `mru-spaces`/Stage Manager/Sequoia-tiling block) using the
   capture helper. These are the only entries that must be present from day one for the workstation to
   function correctly.
1. **Initial seeding pass** — the user runs `just defaults-capture <domain> <key>` per setting they
   want tracked beyond the Aerospace baseline (Dock tile size, Finder hidden files, trackpad clicking,
   etc.). The helper appends each one and exits cleanly when already in sync, so this can be done
   incrementally over days.
1. **CLAUDE.md update** — new "macOS Defaults" section with usage and the killall semantics call-out.
1. **Runbook** — `docs/runbooks/macos-fresh-machine-quickstart.md` with the §4.3 checklist plus the
   §4.1 manual gating steps and the §1 Aerospace manual System Settings steps.

## Validation criteria

- `chezmoi apply` on darwin: Tier 1 + Tier 2 run cleanly, exit 0, killalls fire.
- `chezmoi apply` on linux: both runners render to empty bodies; helpers are absent (chezmoiignore).
- `just D` on a clean Mac (post-apply): exit 0, no drift output.
- `just D` after manually changing a tracked default in System Settings: exit 1, drift row printed.
- `just defaults-apply`: replays loop without the hash gate, killalls fire, drift cleared.
- `just defaults-capture com.apple.dock tilesize` on a fresh value: appends a record with the right
  type tag and exits 0; running again with no changes exits 0 and is a no-op; running after a manual
  System Settings tweak that diverges from the captured value exits 2 (drift) until resolved.
- `just defaults-capture` on a never-set key: exits 1 with a "key not currently set" message and does
  not modify YAML.
- Editing a YAML value and running `chezmoi apply`: hash gate detects change, runner re-runs.
- Removing a key from YAML: runner skips it on next apply (and the value persists on disk until the
  user runs `defaults delete` manually, per §4.2).
- After applying, `defaults read com.apple.dock mru-spaces` returns `0` (Aerospace required).
- `shellcheck` and `shfmt` pass on all new shell files.
- `yq` parses both YAML files.
