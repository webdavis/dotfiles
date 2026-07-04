# herdr smart-nav plugin_action Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the `herdr-smart-nav` standalone binary into a herdr plugin bound via `type = "plugin_action"`, removing the `/bin/sh -lc` keybinding wrapper.

**Architecture:** Move the existing Cargo project under `plugins/`, add a `herdr-plugin.toml` declaring 4 direction actions, adapt the binary's pane-resolution to the plugin env (`HERDR_PANE_ID`), rebind the keys, and build+link via a chezmoiscript mirroring the `last-workspace` plugin. The unit-tested pure core is untouched.

**Tech Stack:** Rust (edition 2021, `serde_json`), herdr plugins, chezmoi `run_onchange` scripts, `just`/`scripts/lint.sh`.

**Reference spec:** `docs/superpowers/specs/2026-06-23-herdr-nav-plugin-action-design.md`

## Global Constraints

- Pure core (`direction_to_chord` / `is_nvim_foreground` / `decide`) and its 8 tests **unchanged**.
- herdr plugin actions get `HERDR_PANE_ID`, **not** `HERDR_ACTIVE_PANE_ID` — the binary must read the former.
- Action commands are static argv arrays (no args/templating) → **4 actions**, one per direction.
- Cargo: `edition = "2021"`, dep `serde_json = "1"`, `Cargo.lock` vendored, build `--locked`.
- Connection-safe (cargo build + `herdr plugin link` + a nav-keybinding swap; no network/SSH/Tailscale). Every commit passes the pre-commit hook (`just lint-check` + `just test`); `cargo test` must pass.
- `dot_config/herdr/config.toml` edits stay `taplo`-clean.
- **Lint-green invariant:** `scripts/lint.sh` renders every template in `find_shell_templates`; a build script whose `{{ include }}` points at a moved/missing file fails to render. So the relocate, new build script, and old-script deletion are one atomic commit (Task 1).

---

### Task 1: Relocate as a plugin — `resolve_pane` (TDD), manifest, build/link plumbing

**Files:**
- Move: `dot_local/share/herdr/herdr-smart-nav/{Cargo.toml,Cargo.lock,src/main.rs}` → `dot_local/share/herdr/plugins/herdr-smart-nav/…`
- Create: `dot_local/share/herdr/plugins/herdr-smart-nav/herdr-plugin.toml`
- Modify: `dot_local/share/herdr/plugins/herdr-smart-nav/src/main.rs` (add `resolve_pane` + tests; change `main`)
- Create: `.chezmoiscripts/run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl`
- Delete: `.chezmoiscripts/run_onchange_after_56-build-herdr-smart-nav.sh.tmpl`
- Modify: `scripts/lint.sh` (`find_shell_templates`: after_56 → after_57)

**Interfaces:**
- Produces: plugin `herdr-smart-nav` with actions `nav_left/nav_down/nav_up/nav_right`; pure fn `resolve_pane(pane_id: Option<String>, active: Option<String>) -> Option<String>`.

- [ ] **Step 1: Relocate the Cargo project**

```bash
cd /Users/stephen/workspaces/Ivy/webdavis/dotfiles
mkdir -p dot_local/share/herdr/plugins/herdr-smart-nav/src
git mv dot_local/share/herdr/herdr-smart-nav/Cargo.toml   dot_local/share/herdr/plugins/herdr-smart-nav/Cargo.toml
git mv dot_local/share/herdr/herdr-smart-nav/Cargo.lock   dot_local/share/herdr/plugins/herdr-smart-nav/Cargo.lock
git mv dot_local/share/herdr/herdr-smart-nav/src/main.rs  dot_local/share/herdr/plugins/herdr-smart-nav/src/main.rs
rm -rf dot_local/share/herdr/herdr-smart-nav   # remove the orphaned (gitignored) target/
```

- [ ] **Step 2: Add the failing `resolve_pane` test + a stub** (in the moved `src/main.rs`)

Add a stub above the `#[cfg(test)]` module (compiles, returns `None`):

```rust
fn resolve_pane(_pane_id: Option<String>, _active: Option<String>) -> Option<String> {
    None // stub
}
```

Add these tests inside `mod tests`:

```rust
    #[test]
    fn resolve_pane_prefers_pane_id() {
        assert_eq!(resolve_pane(Some("p1".into()), Some("p2".into())), Some("p1".into()));
    }

    #[test]
    fn resolve_pane_falls_back_to_active_when_pane_id_missing_or_empty() {
        assert_eq!(resolve_pane(None, Some("p2".into())), Some("p2".into()));
        assert_eq!(resolve_pane(Some(String::new()), Some("p2".into())), Some("p2".into()));
    }

    #[test]
    fn resolve_pane_none_when_all_absent_or_empty() {
        assert_eq!(resolve_pane(None, None), None);
        assert_eq!(resolve_pane(Some(String::new()), Some(String::new())), None);
    }
```

- [ ] **Step 3: Run tests, verify the new ones fail**

Run: `cd dot_local/share/herdr/plugins/herdr-smart-nav && cargo test`
Expected: FAIL — `resolve_pane_prefers_pane_id` (got `None`), `resolve_pane_falls_back_…` (got `None`). The other 8 still pass.

- [ ] **Step 4: Implement `resolve_pane`** (replace the stub)

```rust
/// Resolve the pane to act on. A plugin action receives HERDR_PANE_ID; the old
/// shell keybinding (and manual use) set HERDR_ACTIVE_PANE_ID. Prefer the former,
/// fall back to the latter, ignore empty strings.
fn resolve_pane(pane_id: Option<String>, active: Option<String>) -> Option<String> {
    [pane_id, active].into_iter().flatten().find(|p| !p.is_empty())
}
```

- [ ] **Step 5: Wire `main` to use it** — replace the current pane line

Old:
```rust
    let pane = env::var("HERDR_ACTIVE_PANE_ID")
        .ok()
        .filter(|p| !p.is_empty());
```
New:
```rust
    let pane = resolve_pane(
        env::var("HERDR_PANE_ID").ok(),
        env::var("HERDR_ACTIVE_PANE_ID").ok(),
    );
```

- [ ] **Step 6: Run tests + build**

Run: `cargo test` → Expected: PASS (11 tests). Then `cargo build --release --locked` → Expected: builds clean (binary at `target/release/herdr-smart-nav`).

- [ ] **Step 7: Add the plugin manifest** — `herdr-plugin.toml`

```toml
id = "herdr-smart-nav"
name = "Smart Nav"
version = "0.1.0"
min_herdr_version = "0.7.0"
description = "Seamless Ctrl-h/j/k/l navigation across Neovim splits and herdr panes."
platforms = ["macos", "linux"]

# Compiled at install time. `herdr plugin link` (local dev) does NOT run this, so
# the chezmoiscript that links the plugin builds it first.
[[build]]
command = ["cargo", "build", "--release", "--locked"]

# Bound to ctrl+h/j/k/l via [[keys.command]] type = "plugin_action" in the herdr
# config. One action per direction (a keybinding cannot pass arguments, so the
# direction is baked into each action's argv).
[[actions]]
id = "nav_left"
title = "Nav left (Neovim split or herdr pane)"
command = ["./target/release/herdr-smart-nav", "left"]

[[actions]]
id = "nav_down"
title = "Nav down (Neovim split or herdr pane)"
command = ["./target/release/herdr-smart-nav", "down"]

[[actions]]
id = "nav_up"
title = "Nav up (Neovim split or herdr pane)"
command = ["./target/release/herdr-smart-nav", "up"]

[[actions]]
id = "nav_right"
title = "Nav right (Neovim split or herdr pane)"
command = ["./target/release/herdr-smart-nav", "right"]
```

- [ ] **Step 8: Create the build/link script** — `.chezmoiscripts/run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl`

```text
{{ if eq .chezmoi.os "darwin" -}}
#!/bin/bash

set -euo pipefail

# Builds and links the herdr-smart-nav plugin (the ctrl-h/j/k/l seamless-nav
# action). `herdr plugin link` does NOT run the manifest's [[build]] step, so the
# binary is compiled here. Re-runs whenever the plugin source changes.
#
# Plugin source (hashed so this script re-runs on change):
#   {{ include "dot_local/share/herdr/plugins/herdr-smart-nav/src/main.rs" | sha256sum }}
#   {{ include "dot_local/share/herdr/plugins/herdr-smart-nav/Cargo.lock" | sha256sum }}
#   {{ include "dot_local/share/herdr/plugins/herdr-smart-nav/herdr-plugin.toml" | sha256sum }}

plugin_dir="$HOME/.local/share/herdr/plugins/herdr-smart-nav"
plugin_id="herdr-smart-nav"

if ! command -v cargo &>/dev/null; then
  echo "cargo not found on PATH; run the rustup bootstrap first" >&2
  exit 1
fi

# --locked keeps the vendored Cargo.lock authoritative so it does not drift.
(cd "$plugin_dir" && cargo build --release --locked)

# Link with herdr if not already linked. Linking talks to the running herdr
# server; if it is down (headless apply), skip with a hint — link later.
if herdr plugin list 2>/dev/null | grep -q "$plugin_id"; then
  echo "herdr plugin $plugin_id already linked"
elif herdr plugin link "$plugin_dir" >/dev/null 2>&1; then
  echo "linked herdr plugin $plugin_id"
else
  echo "could not link $plugin_id (herdr server not running?); link later with: herdr plugin link $plugin_dir" >&2
fi

# Remove the retired standalone binary (superseded by the plugin action).
rm -f "$HOME/.local/bin/herdr-smart-nav"
{{ end -}}
```

- [ ] **Step 9: Delete the old standalone build script**

```bash
git rm .chezmoiscripts/run_onchange_after_56-build-herdr-smart-nav.sh.tmpl
```

- [ ] **Step 10: Update lint's template list** — `scripts/lint.sh`

Replace the line:
```bash
    -o -name "run_onchange_after_56-build-herdr-smart-nav.sh.tmpl" \
```
with:
```bash
    -o -name "run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl" \
```

- [ ] **Step 11: Verify + commit**

Run: `CI=1 chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl | shellcheck -` → clean.
Run: `just s` → ✅. Run: `(cd dot_local/share/herdr/plugins/herdr-smart-nav && cargo test)` → 11 pass.

```bash
git add dot_local/share/herdr/plugins/herdr-smart-nav/ \
        .chezmoiscripts/run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl \
        scripts/lint.sh
git commit -m "feat(herdr): make herdr-smart-nav a plugin (plugin_action); HERDR_PANE_ID-aware"
```
(The `git mv` and `git rm` from Steps 1/9 are already staged; confirm `target/` is not staged.)

---

### Task 2: Rebind keybindings + document

**Files:**
- Modify: `dot_config/herdr/config.toml` (4 nav blocks: `shell` → `plugin_action`)
- Modify: `CLAUDE.md` (herdr nav note)

- [ ] **Step 1: Repoint the 4 bindings** — `dot_config/herdr/config.toml`

`ctrl+h` block — replace:
```toml
type = "shell"
command = "$HOME/.local/bin/herdr-smart-nav left"
```
with:
```toml
type = "plugin_action"
command = "herdr-smart-nav.nav_left"
```

`ctrl+j` block — replace `type = "shell"` / `command = "$HOME/.local/bin/herdr-smart-nav down"` with `type = "plugin_action"` / `command = "herdr-smart-nav.nav_down"`.

`ctrl+k` block — replace `type = "shell"` / `command = "$HOME/.local/bin/herdr-smart-nav up"` with `type = "plugin_action"` / `command = "herdr-smart-nav.nav_up"`.

`ctrl+l` block — replace `type = "shell"` / `command = "$HOME/.local/bin/herdr-smart-nav right"` with `type = "plugin_action"` / `command = "herdr-smart-nav.nav_right"`.

(Keep each block's `key` and `description`.)

- [ ] **Step 2: Update the CLAUDE.md note** — under "### Herdr Workspace Management"

Replace the existing paragraph that begins `Ctrl-h/j/k/l "seamless nav across Neovim splits and herdr panes" is a compiled Rust binary` with:

```markdown
Ctrl-h/j/k/l "seamless nav across Neovim splits and herdr panes" is a herdr **plugin**
(`dot_local/share/herdr/plugins/herdr-smart-nav/`, a Rust binary), bound via four `type = "plugin_action"`
keybindings (`herdr-smart-nav.nav_<dir>`) — so herdr execs it directly as argv, with no `/bin/sh -lc`
wrapper. Built + linked by `run_onchange_after_57` (mirrors the `last-workspace` plugin). It shells the
`herdr` CLI (no Rust SDK); the gain over the old shell-keybinding binary is ~5 ms (the wrapper) and is
imperceptible — the value is the idiomatic plugin integration. Plugin actions get `HERDR_PANE_ID` (not
`HERDR_ACTIVE_PANE_ID`).
```

- [ ] **Step 3: Validate + commit**

Run: `just t` (taplo — config.toml clean) and `just m` (mdformat — CLAUDE.md) → ✅.

```bash
git add dot_config/herdr/config.toml CLAUDE.md
git commit -m "feat(herdr): bind ctrl-h/j/k/l via plugin_action; document the plugin"
```

---

### Task 3: Activate + verify (live, connection-safe)

**Files:** none (live actions on this machine).

- [ ] **Step 1: Deploy + build + link the plugin**

```bash
cd /Users/stephen/workspaces/Ivy/webdavis/dotfiles
chezmoi apply --force "$HOME/.local/share/herdr/plugins/herdr-smart-nav"
rm -rf "$HOME/.local/share/herdr/herdr-smart-nav"   # remove the old standalone source dir
CI=1 chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl | bash
herdr plugin list 2>/dev/null | grep herdr-smart-nav && echo "plugin linked ✓"
test ! -e "$HOME/.local/bin/herdr-smart-nav" && echo "old binary removed ✓"
```
Expected: plugin linked; old `~/.local/bin/herdr-smart-nav` gone.

- [ ] **Step 2: Apply the new keybindings + reload**

```bash
chezmoi apply --force "$HOME/.config/herdr/config.toml"
herdr server reload-config 2>&1 | tail -1
grep -c 'plugin_action' "$HOME/.config/herdr/config.toml"   # expect 4
grep -c 'herdr-smart-nav left\|/.local/bin/herdr-smart-nav' "$HOME/.config/herdr/config.toml"  # expect 0
```
Expected: reload `applied`, no diagnostics; 4 plugin_action bindings; 0 stale shell refs.

- [ ] **Step 3: e2e correctness — action invoke + recording fake-herdr**

```bash
herdr plugin action invoke herdr-smart-nav.nav_left 2>&1 | tail -2 && echo "invoke ok"
# recording fake herdr: prove the 3 branches still issue correct commands
fakedir=$(mktemp -d); log="$fakedir/calls.log"
cat > "$fakedir/herdr" <<EOF
#!/usr/bin/env bash
echo "\$*" >> "$log"
[[ "\$1 \$2" == "pane process-info" ]] && cat "$fakedir/pi.json"
exit 0
EOF
chmod +x "$fakedir/herdr"
bin="$HOME/.local/share/herdr/plugins/herdr-smart-nav/target/release/herdr-smart-nav"
echo '{"result":{"process_info":{"foreground_processes":[{"name":"bash"}]}}}' > "$fakedir/pi.json"
: >"$log"; HERDR_BIN_PATH="$fakedir/herdr" HERDR_PANE_ID="wW:p8" "$bin" left
echo "[non-nvim,left] expect process-info then 'pane focus --direction left --pane wW:p8':"; sed 's/^/  /' "$log"
echo '{"result":{"process_info":{"foreground_processes":[{"name":"nvim"}]}}}' > "$fakedir/pi.json"
: >"$log"; HERDR_BIN_PATH="$fakedir/herdr" HERDR_PANE_ID="wW:p8" "$bin" right
echo "[nvim,right] expect process-info then 'pane send-keys wW:p8 ctrl+l':"; sed 's/^/  /' "$log"
rm -rf "$fakedir"
```
Expected: invoke runs without error; fake-herdr log shows the correct commands (confirms `HERDR_PANE_ID` is honored).

- [ ] **Step 4: Perf gate + final suite**

```bash
bin="$HOME/.local/share/herdr/plugins/herdr-smart-nav/target/release/herdr-smart-nav"
s=$EPOCHREALTIME; for i in $(seq 1 30); do herdr plugin action invoke herdr-smart-nav.nav_left >/dev/null 2>&1; done; e=$EPOCHREALTIME
awk -v s=$s -v e=$e 'BEGIN{printf "plugin action invoke (CLI, dispatch upper bound): %.2f ms/call\n",(e-s)/30*1000}'
just l && just test
```
Then the operator presses **ctrl-h** in a multi-pane workspace to confirm real nav + judge the feel.
**Acceptance:** keep if not measurably slower than the prior shell path; revert only on a clear regression.

---

## Self-Review

**Spec coverage:** relocate → T1 S1; manifest (4 actions) → T1 S7; `resolve_pane` TDD + main → T1 S2-6; build/link script (+ rm old binary) → T1 S8; delete after_56 + lint → T1 S9-10; rebind config → T2 S1; CLAUDE.md → T2 S2; activate/verify + perf gate → T3; pure core unchanged (only `resolve_pane` added) ✓. No gaps.

**Placeholder scan:** none — full `resolve_pane` + tests, full manifest, full build script, exact config/CLAUDE.md edits, exact commands.

**Type/name consistency:** `resolve_pane(Option<String>, Option<String>) -> Option<String>` identical in stub (T1 S2), impl (T1 S4), and call site (T1 S5). Action ids `nav_left/nav_down/nav_up/nav_right` identical in the manifest (T1 S7) and the bindings (T2 S1). Plugin id `herdr-smart-nav` identical in manifest, build script, bindings, and T3. Binary path `…/plugins/herdr-smart-nav/target/release/herdr-smart-nav` consistent in T3.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-23-herdr-nav-plugin-action.md`.
