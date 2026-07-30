# herdr smart-nav Rust binary design

Date: 2026-06-21

## Context

`dot_local/bin/executable_herdr-smart-nav.sh` backs the Ctrl-h/j/k/l "seamless nav across Neovim splits
and herdr panes" keybinding. herdr invokes it per keypress via `/bin/sh -lc "…/herdr-smart-nav.sh
left|down|up|right"`. The script detects whether the focused pane runs Neovim
(`herdr pane process-info` + `jq`) and either forwards the chord (`herdr pane send-keys`, for
smart-splits.nvim) or moves herdr focus (`herdr pane focus --direction`).

After this session's earlier shell-startup fixes (the 107 ms keypress is gone), the residual per-keypress
cost is ~15 ms. **Measured decomposition** (warm, this machine):

| Component | Cost | Removable by a Rust binary? |
|---|---|---|
| bash startup (script interpreter) | ~5.2 ms | yes (Rust ~1 ms) |
| `jq` (in the detect pipeline) | ~1 ms (overlaps herdr) | yes (native serde_json) |
| **`herdr` CLI spawn ×2** (process-info + focus/send-keys) | **~9 ms** (~4 ms each) | **no** |

`herdr --version` (spawn, no IPC) is 4.0 ms vs `herdr pane process-info` (spawn + IPC) 4.6 ms, so the
herdr server IPC is only ~0.6 ms; the ~4 ms/call is the **`herdr` CLI's own process startup**. Two calls
are inherent (detect, then act).

**Feasibility verdict (pressure-test, done first):** a Rust binary helps only at the bash+jq layer
(~15 ms → ~10 ms, ~5 ms saved). The dominant ~9 ms is two `herdr` CLI spawns; the *only* way to remove it
is to speak herdr's socket directly (skipping the CLI). herdr is `0.7.0-preview` with an undocumented,
unstable socket protocol. Reimplementing it is fragile and violates "supported options only", so it is
out of scope. There is no combined/batch command and no Rust SDK (the existing `last-workspace` plugin
also shells out to the CLI). **~5 ms on a keypress is below one 60 Hz frame and below the OS key-repeat
interval, i.e. imperceptible.**

This binary is therefore built **deliberately as a marginal optimization**, accepted by the user for: a
clean, unit-tested, well-structured implementation; elimination of bash+jq startup; and consistency with
the `last-workspace` Rust pattern. It is **not** a fix for a felt-slowness problem (that was the shell
startup, already fixed). Future maintainers: do **not** "improve" this by reimplementing herdr's socket
protocol.

## Goals

- Replace the nav shell script with a compiled binary, preserving identical behavior.
- Keep the branching logic (direction→chord, nvim-detection, action selection) **pure and unit-tested**,
  with no live herdr server required for tests.
- Build + install via chezmoi, mirroring the existing `last-workspace` plugin pattern.

## Non-goals

- Reducing the two `herdr` CLI spawns (requires the preview/undocumented socket, out of scope).
- Any change to smart-splits.nvim or herdr config beyond repointing the keybinding.

## Design

Split into a **pure core** (unit-tested) and a **thin impure boundary** (shells out to `herdr`, exactly
like `last-workspace`'s `Command::new(herdr_bin())`).

**Pure core (unit-tested):**

- `direction_to_chord(dir: &str) -> Option<&'static str>`: `left→ctrl+h`, `down→ctrl+j`, `up→ctrl+k`,
  `right→ctrl+l`; `None` for anything else (drives the usage-error exit).
- `is_nvim_foreground(process_info_json: &str) -> bool`: serde_json parse of the `pane process-info`
  payload; true iff any `result.process_info.foreground_processes[].name == "nvim"`. Mirrors the script's
  `jq` and the plugin's `parse_focused_id` parsing style; tolerant of malformed input (returns false).
- `decide(pane: Option<&str>, is_nvim: bool, dir: &str, chord: &str) -> Action` where
  `enum Action { SendKeys { pane, chord }, Focus { pane, dir }, FocusCurrent { dir } }`:
  - `Some(pane)` + nvim → `SendKeys`
  - `Some(pane)` + not nvim → `Focus`
  - `None` → `FocusCurrent` (best-effort, mirrors the script's `--current` fallback)

**Impure boundary (thin, not unit-tested, integration-only, like the plugin):**

- `herdr_bin() -> String`: `HERDR_BIN_PATH` or `"herdr"` (same as the plugin).
- `run_herdr(args) -> Output / status`.
- `main`: read arg → `direction_to_chord` (None → usage error, exit 2) → read `HERDR_ACTIVE_PANE_ID` →
  if pane set, `run_herdr(["pane","process-info","--pane",pane])` → `is_nvim_foreground` → `decide` →
  execute the `Action` via `run_herdr`. Preserve the script's `set -euo pipefail`-equivalent care: a
  failed `process-info` falls back to a focus action rather than aborting (so nav still works if detect
  fails), matching the script's `2>/dev/null` tolerance.

The pure/impure split is the same shape as `last-workspace` (pure `next_mru`/`parse_focused_id` tested;
`Command` calls untested), so it fits the repo's established Rust idiom.

## TDD approach

Tests live in `#[cfg(test)] mod tests` in `main.rs` (same as `last-workspace`), run by `cargo test`:

- `direction_to_chord`: each of the four directions maps correctly; an invalid direction is `None`.
- `is_nvim_foreground`: a sample `process-info` JSON with `nvim` → true; without → false; malformed/empty
  → false.
- `decide`: the three branches (SendKeys / Focus / FocusCurrent) for the (pane, is_nvim) combinations.

The two `herdr` shell-outs are the only untested surface (no live server in CI/tests), acceptable, and
identical to the plugin's testing boundary.

## Build / install / wiring

Mirror `last-workspace` + `run_onchange_after_55-build-herdr-last-workspace-plugin.sh.tmpl`:

- **Source:** Cargo project at `dot_local/share/herdr/herdr-smart-nav/` (chezmoi-deployed to
  `~/.local/share/herdr/herdr-smart-nav/`): `Cargo.toml` (package `herdr-smart-nav`, dep `serde_json`),
  `Cargo.lock`, `src/main.rs`.
- **Build script:** `.chezmoiscripts/run_onchange_after_56-build-herdr-smart-nav.sh.tmpl` (darwin-gated,
  hashed on the source like the plugin loader): `cargo build --release --locked`, then install the binary
  to `~/.local/bin/herdr-smart-nav` (`install -m 0755`), and `rm -f ~/.local/bin/herdr-smart-nav.sh`
  (remove the superseded script).
- **Keybinding:** in `dot_config/herdr/config.toml`, repoint the four ctrl+h/j/k/l bindings from
  `…/herdr-smart-nav.sh <dir>` to `…/herdr-smart-nav <dir>`.
- **Remove** the old `dot_local/bin/executable_herdr-smart-nav.sh` from the repo.
- **Lint:** the build `.sh.tmpl` joins `find_shell_templates` in `scripts/lint.sh`; `cargo` is gated
  behind a `command -v cargo` check (as the plugin build does).

## Performance expectation (honest)

~15 ms → ~10 ms per keypress (bash + jq eliminated; the ~9 ms two-spawn floor remains). Imperceptible to
the user; this is a code-quality/consistency change, not a felt-speed change. Recorded here so the gain is
not overstated.

## Risks / caveats

- **Marginal/imperceptible gain**, documented above; the value is the clean tested binary, not speed.
- **herdr is preview**, its CLI surface (`pane process-info`/`focus`/`send-keys`) could change; the
  binary depends on the same CLI the script already did, so risk is unchanged, not increased.
- **Keybinding swap**, a slip breaks pane nav (not the connection); caught by verifying the binding +
  a manual nav test when present. Build is connection-safe.
- **cargo required at apply**, same dependency the plugin already introduced.

## Files touched

- `dot_local/share/herdr/herdr-smart-nav/Cargo.toml` (new)
- `dot_local/share/herdr/herdr-smart-nav/Cargo.lock` (new)
- `dot_local/share/herdr/herdr-smart-nav/src/main.rs` (new, pure core + tests + thin herdr boundary)
- `.chezmoiscripts/run_onchange_after_56-build-herdr-smart-nav.sh.tmpl` (new, build + install + remove old .sh)
- `dot_config/herdr/config.toml` (edit, repoint the 4 ctrl+h/j/k/l bindings)
- `dot_local/bin/executable_herdr-smart-nav.sh` (remove)
- `scripts/lint.sh` (edit, add the build script to `find_shell_templates`)
- `CLAUDE.md` (edit, note the binary replaced the script under the herdr nav notes)
