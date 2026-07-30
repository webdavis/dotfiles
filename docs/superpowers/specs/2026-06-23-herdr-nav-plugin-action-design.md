# herdr smart-nav as a plugin_action design

Date: 2026-06-23

## Context

`herdr-smart-nav` (the compiled Ctrl-h/j/k/l nav binary, built 2026-06-21) is bound via four
`type = "shell"` keybindings. herdr runs a `shell`/`pane` keybinding command through `/bin/sh -lc`, which
measured at **5.24 ms** of constant per-keypress overhead, now the single largest avoidable component of
the keypress (the binary itself is ~10.2 ms; the old script was ~15.2 ms).

herdr's third keybinding command type, **`plugin_action`**, invokes an installed plugin's action whose
manifest command is an **argv array**. The plugin docs (herdr.dev/docs/plugins) state: *"Herdr does not
run [action commands] through a shell … commands execute directly as argv."* So binding the nav keys as
`plugin_action` eliminates the `/bin/sh -lc` wrapper. The existing `herdr-last-workspace` plugin already
rides this path (`prefix+ctrl+\`).

This converts `herdr-smart-nav` from a standalone `~/.local/bin` binary into a herdr **plugin**, bound via
`plugin_action`. The motivation is primarily **architectural** (idiomatic herdr plugin, no shell wrapper,
no stray standalone binary, consistent with `last-workspace`); the ~5 ms is a bonus and is
**imperceptible** (keypress ~16 ms → ~11 ms, still sub-frame). Recorded honestly so the gain is not
overstated.

## Pressure-test results (herdr plugin docs, done first)

- **Shell-skip: CONFIRMED.** Action commands execute directly as argv, no shell. The 5.24 ms wrapper is
  genuinely removed.
- **Pane env differs: a CORRECTNESS change, not just perf.** A plugin action receives **no
  `HERDR_ACTIVE_PANE_ID`** (the variable the binary currently reads). herdr injects, for an action:
  `HERDR_SOCKET_PATH`, `HERDR_BIN_PATH`, `HERDR_ENV=1`, `HERDR_PLUGIN_ID`, `HERDR_PLUGIN_ROOT`,
  `HERDR_PLUGIN_CONFIG_DIR`, `HERDR_PLUGIN_STATE_DIR`, `HERDR_PLUGIN_CONTEXT_JSON`,
  `HERDR_PLUGIN_ACTION_ID`, and **`HERDR_PANE_ID`** "when available for that invocation". So the binary
  must resolve the pane from `HERDR_PANE_ID`. Missing this silently degrades nav to `--current` (the
  "less certain" path the original deliberately avoided).
- **Testable.** `herdr plugin action invoke <plugin-id>.<action-id>` invokes an action directly, used for
  the e2e + perf gate (no keypress injection needed).
- **No runtime args / templating.** Action commands are static argv arrays → one action per direction
  (4 actions), each baking its direction into the argv.

## Goals

- Bind Ctrl-h/j/k/l via `plugin_action` so the keypress runs the nav tool with no `/bin/sh -lc` wrapper.
- Preserve identical nav behavior and the unit-tested pure core.
- Mirror the `last-workspace` plugin pattern (manifest + build/link chezmoiscript).

## Non-goals

- Any change to the nav decision logic (`direction_to_chord` / `is_nvim_foreground` / `decide`).
- Reducing the two `herdr` CLI spawns (still out of scope, preview socket protocol).
- Parsing `HERDR_PLUGIN_CONTEXT_JSON` (YAGNI, `HERDR_PANE_ID` is expected to suffice for a
  keypress-triggered action; revisit only if verification shows it absent).

## Design

**Relocate** the Cargo project `dot_local/share/herdr/herdr-smart-nav/` →
`dot_local/share/herdr/plugins/herdr-smart-nav/` (same level as `herdr-last-workspace`). The `**/target`
git/chezmoi ignores added on 2026-06-21 already cover the new path.

**Manifest** `plugins/herdr-smart-nav/herdr-plugin.toml` (mirrors `last-workspace`'s):

```toml
id = "herdr-smart-nav"
name = "Smart Nav"
version = "0.1.0"
min_herdr_version = "0.7.0"
description = "Ctrl-h/j/k/l seamless navigation across Neovim splits and herdr panes."
platforms = ["macos", "linux"]

[[build]]
command = ["cargo", "build", "--release", "--locked"]

[[actions]]
id = "nav_left"
title = "Nav left (Neovim split or herdr pane)"
command = ["./target/release/herdr-smart-nav", "left"]
# … nav_down/up/right identical with down/up/right
```

**Binary, one boundary change (pure core untouched):** add a pure, unit-tested
`resolve_pane(pane_id: Option<String>, active: Option<String>) -> Option<String>` returning the first
non-empty of `[HERDR_PANE_ID, HERDR_ACTIVE_PANE_ID]`; `main` reads both env vars and passes them in. The
`HERDR_ACTIVE_PANE_ID` fallback keeps the binary working if invoked outside a plugin action (manual/debug).
`direction_to_chord` / `is_nvim_foreground` / `decide` and their 8 tests are unchanged; one test is added
for `resolve_pane` precedence.

**Keybindings** `dot_config/herdr/config.toml`, the four ctrl+h/j/k/l blocks change from
`type = "shell"`, `command = "$HOME/.local/bin/herdr-smart-nav <dir>"` to `type = "plugin_action"`,
`command = "herdr-smart-nav.nav_<dir>"`.

**Build/link** `.chezmoiscripts/run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl`, mirror
`run_onchange_after_55` (darwin-gated, hashed on source, `command -v cargo` guard,
`cargo build --release --locked`, then `herdr plugin link` if not already linked, with the
"server not running → link later" hint). No `~/.local/bin` install; no state seed (nav is stateless).

**Retire** the standalone path: delete `.chezmoiscripts/run_onchange_after_56-build-herdr-smart-nav.sh.tmpl`,
update its entry in `scripts/lint.sh` `find_shell_templates` to the new `after_57` name, and remove the
deployed `~/.local/bin/herdr-smart-nav` during activation.

## Testing / acceptance gate

- **Unit:** `cargo test`, the 8 existing pure-core tests + the new `resolve_pane` precedence test.
- **e2e (wiring + pane):** after build/link, `herdr plugin action invoke herdr-smart-nav.nav_left` runs
  without error; a recording fake-`herdr` (via `HERDR_BIN_PATH`) confirms the three branches still issue
  the right commands (as in the 2026-06-21 scratch test). The definitive pane-targeting check is a real
  Ctrl-h keypress by the operator (only a keypress reliably sets `HERDR_PANE_ID`).
- **Perf:** measure `herdr plugin action invoke …` latency as an upper bound on dispatch; compare the
  keypress feel. **Accept** as long as it is **not measurably slower** than the current shell path: the
  architecture win stands at perf-parity. **Revert** only on a regression.

## Risks / caveats

- **Imperceptible gain**, documented; the value is architecture, not felt speed.
- **`HERDR_PANE_ID` on keypress**, expected present for a key-triggered action; if verification shows it
  absent, fall back to parsing `HERDR_PLUGIN_CONTEXT_JSON` (deferred, see non-goals).
- **herdr is preview**, same CLI surface dependency as today; risk unchanged.
- **Plugin link needs the herdr server**, the build script links best-effort and hints to link later if
  the server is down (same as `last-workspace`).

## Files touched

- `dot_local/share/herdr/herdr-smart-nav/{Cargo.toml,Cargo.lock,src/main.rs}` → moved to
  `dot_local/share/herdr/plugins/herdr-smart-nav/…`
- `dot_local/share/herdr/plugins/herdr-smart-nav/herdr-plugin.toml` (new)
- `dot_local/share/herdr/plugins/herdr-smart-nav/src/main.rs` (edit, `resolve_pane` + test)
- `.chezmoiscripts/run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl` (new)
- `.chezmoiscripts/run_onchange_after_56-build-herdr-smart-nav.sh.tmpl` (delete)
- `scripts/lint.sh` (edit, after_56 → after_57 in `find_shell_templates`)
- `dot_config/herdr/config.toml` (edit, 4 bindings → `plugin_action`)
- `CLAUDE.md` (edit, nav note: now a plugin via `plugin_action`, no shell wrapper)
- `~/.local/bin/herdr-smart-nav` (removed at activation)
