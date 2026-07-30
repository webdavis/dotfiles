# herdr last-workspace plugin: design

Date: 2026-06-20

## Problem

herdr ships `last_pane` (a builtin most-recently-used toggle for panes) but has no `last_workspace`
equivalent. The proposing upstream PR (ogulcancelik/herdr#708) was closed unmerged, and the installed
binary's `KeysConfig` exposes only `last_pane`, confirming no builtin exists in herdr 0.7.0.

A first attempt implemented the toggle as a key-bound shell script (`herdr-bounce.sh`) that recorded the
"previous" workspace, with the jump chords recording state on each jump. It had a reproducible bug: after
switching workspaces by mouse or the builtin picker, the bounce jumped to a stale workspace instead of
the one actually focused just before.

Root cause: a key-bound script can only observe workspace switches that flow **through it** (the jump
chords, its own bounce). Mouse clicks and the picker (`prefix+g`/`prefix+w`) change focus without
invoking any script, so the script's recorded state desynced from reality. This is an architectural limit
of the script approach, not a fixable logic slip: the observer is blind to most switches.

## Goal and scope

A reliable workspace MRU toggle:

- Invoking it focuses the workspace that was focused immediately before the current one.
- Invoking it again returns to where you were, a symmetric **2-way toggle** between the two
  most-recently-focused workspaces (matching tmux `last-window` and the original intent).
- Correct regardless of how the switch happened: chord, mouse, or picker.

The plugin is **keybinding-agnostic**: it exposes a `last_workspace` action only and imposes no key. The
keybinding is the user's choice, owned in their own herdr config (`dot_config/herdr/config.toml`), the
default is `prefix+ctrl+\`, but it can be set to anything (or invoked manually via
`herdr plugin action invoke`). This mirrors the example plugins, which ship an action and leave the bind to
the user.

Out of scope: walking a deeper history (3+ workspaces). The state is 2-deep (current, previous) only.

## Approach

Implement it as a **herdr plugin** rather than a shell script. A plugin can hook herdr's
`workspace.focused` event, which herdr fires for **every** focus change regardless of trigger, closing
the observation gap that broke the script.

Language: Rust, a single compiled binary with two subcommands. (User's choice; matches the
rust-release-check plugin example. Zero non-crates dependencies beyond `serde_json`, so the build is
offline-capable via the sparse crates.io registry.)

The load-bearing assumption: herdr fires `workspace.focused` for mouse and picker switches identically to
API switches. The spike verified the event fires for API focus with payload
`{"event":"workspace_focused","data":{"type":"workspace_focused","workspace_id":"<id>"}}`; mouse/picker
are the same underlying focus operation. If herdr's event model ever changes, this is the claim to
re-verify.

## Components

One crate at `~/.local/share/herdr/plugins/herdr-last-workspace/` (chezmoi source under
`dot_local/share/herdr/plugins/herdr-last-workspace/`): `herdr-plugin.toml`, `Cargo.toml`, `Cargo.lock`,
`src/main.rs`.

The binary, `last-workspace`, takes one subcommand:

- `record`, declared as the `workspace.focused` event handler. Reads the newly-focused id from
  `HERDR_PLUGIN_EVENT_JSON` (`.data.workspace_id`). If it differs from the stored `current`, shifts
  `current → previous` and sets `current` to the new id.
- `bounce`, the binary subcommand backing the **`last_workspace`** action (the bindable one). Focuses
  the stored `previous` via `herdr workspace focus`. That focus re-fires `workspace.focused`, which
  re-enters `record` and flips the pair, so the next invocation returns. Symmetric toggle, no extra
  bookkeeping. The action id is `last_workspace` (mirroring herdr's `last_pane`), so the keybinding
  command is `herdr-last-workspace.last_workspace`.

## State model

Two lines (`current`, `previous`) in `$HERDR_PLUGIN_STATE_DIR/mru`. `record` shifts on each distinct
focus; `bounce` reads `previous`. herdr provides `HERDR_PLUGIN_STATE_DIR` (per-plugin) and
`HERDR_BIN_PATH` to the plugin process.

## Edge cases

- **Cold start.** The plugin only learns a workspace when it is focused, so the workspace focused
  *before* the plugin started observing is absent from the MRU until re-focused. The build/link
  chezmoiscript seeds `current` with the live focused workspace so the first bounce after install works.
- **Stale target.** `herdr workspace focus` returns exit 0 even on a removed id, so `bounce` checks the
  live `herdr workspace list` first and clears a vanished `previous` instead of focusing nowhere.
- **Refocus of the same workspace.** `record` no-ops when the new id equals `current` (nothing moved).

## Integration (chezmoi)

- Vendor the crate source under `dot_local/share/herdr/plugins/herdr-last-workspace/`.
- `run_onchange_after_55-build-herdr-last-workspace-plugin.sh.tmpl` (darwin-guarded, keyed on the plugin
  source hashes): `cargo build --release --locked`, then `herdr plugin link` if not already linked
  (best-effort, skips with a hint if the herdr server is not running), then seed the MRU state.
- Add the chezmoiscript to `scripts/lint.sh`'s shell-template shellcheck enumeration.
- Rebind `prefix+ctrl+\` in `dot_config/herdr/config.toml` from the shell command to
  `type = "plugin_action"`, `command = "herdr-last-workspace.last_workspace"`.
- Retire the old approach: delete `dot_local/bin/executable_herdr-bounce.sh` and revert
  `dot_local/bin/executable_herdr-jump.sh` to plain create-or-focus (the plugin records state now).
- `last_pane` on `prefix+\` is unchanged. Result: `prefix+\` bounces panes, `prefix+ctrl+\` bounces
  workspaces.

## Testing

- `cargo build --release --locked`, `cargo fmt --check`, and `cargo clippy` all clean.
- Live bug scenario: focus A → B → C (the third via a non-chord path), then bounce returns to B (the true
  previous), and a second bounce returns to C. The spike confirmed this passes where the shell script
  failed.
- Chezmoiscript idempotency: a second run reports "already linked" and skips re-seeding.
- `just l` green.

## Risks and assumptions

- Depends on herdr firing `workspace.focused` for all focus sources (see Approach).
- Depends on `cargo` being present (the rustup bootstrap provides it); the chezmoiscript fails loudly if
  not.
- The plugin's `target/` build directory lives only in `$HOME` (built in place), never in the repo.
