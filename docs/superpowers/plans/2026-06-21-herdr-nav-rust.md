# herdr smart-nav Rust binary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the ctrl-h/j/k/l nav shell script with a compiled, unit-tested Rust binary `herdr-smart-nav`.

**Architecture:** A pure core (direction→chord, nvim-detection, action selection) that is unit-tested via `cargo test`, plus a thin impure boundary that shells out to the `herdr` CLI (mirrors the `last-workspace` plugin). Built + installed via a chezmoi `run_onchange` script.

**Tech Stack:** Rust (edition 2021), `serde_json`, chezmoi Go-template scripts, launchd-free (plain binary), the repo's `just`/`scripts/lint.sh` tooling.

**Reference spec:** `docs/superpowers/specs/2026-06-21-herdr-nav-rust-design.md`

## Global Constraints

- **Behavior parity** with `executable_herdr-smart-nav.sh`: `left=ctrl+h, down=ctrl+j, up=ctrl+k, right=ctrl+l`; detect nvim via `herdr pane process-info`; nvim→`herdr pane send-keys`, else→`herdr pane focus --direction`; no `HERDR_ACTIVE_PANE_ID`→`focus --current`; bad arg→exit 2.
- **Pure core unit-tested; herdr shell-outs untested** (no live server in tests), same boundary as `last-workspace`.
- **No reimplementing herdr's socket protocol** (preview/undocumented; out of scope). Shell the `herdr` CLI.
- Cargo: `edition = "2021"`, dep `serde_json = "1"`, **`Cargo.lock` vendored**, build `--locked`.
- Connection-safe (cargo build + a nav-keybinding swap; no network/SSH/Tailscale). Every commit passes the pre-commit hook (`just lint-check` + `just test`); `cargo test` must pass.
- Edits to `dot_config/herdr/config.toml` must stay `taplo`-clean.

---

### Task 1: The `herdr-smart-nav` Rust binary (pure core TDD + thin boundary)

**Files:**
- Create: `dot_local/share/herdr/herdr-smart-nav/Cargo.toml`
- Create: `dot_local/share/herdr/herdr-smart-nav/src/main.rs`
- Create (generated): `dot_local/share/herdr/herdr-smart-nav/Cargo.lock`

**Interfaces:**
- Produces: binary `herdr-smart-nav` taking one arg (`left|down|up|right`). Pure fns: `direction_to_chord(&str)->Option<&'static str>`, `is_nvim_foreground(&str)->bool`, `decide(Option<&str>,bool,&str,&str)->Action`, `enum Action { SendKeys{pane,chord}, Focus{pane,direction}, FocusCurrent{direction} }`.

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "herdr-smart-nav"
version = "0.1.0"
edition = "2021"

[dependencies]
serde_json = "1"
```

- [ ] **Step 2: `src/main.rs`: tests + compiling stubs** (so `cargo test` compiles and *fails*)

```rust
use std::env;
use std::process::Command;

fn direction_to_chord(_direction: &str) -> Option<&'static str> {
    None // stub
}

fn is_nvim_foreground(_process_info_json: &str) -> bool {
    false // stub
}

#[derive(Debug, PartialEq)]
enum Action {
    SendKeys { pane: String, chord: String },
    Focus { pane: String, direction: String },
    FocusCurrent { direction: String },
}

fn decide(_pane: Option<&str>, _is_nvim: bool, direction: &str, _chord: &str) -> Action {
    Action::FocusCurrent { direction: direction.to_string() } // stub
}

fn main() {} // filled in Step 6

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_to_chord_maps_all_four() {
        assert_eq!(direction_to_chord("left"), Some("ctrl+h"));
        assert_eq!(direction_to_chord("down"), Some("ctrl+j"));
        assert_eq!(direction_to_chord("up"), Some("ctrl+k"));
        assert_eq!(direction_to_chord("right"), Some("ctrl+l"));
    }

    #[test]
    fn direction_to_chord_rejects_unknown() {
        assert_eq!(direction_to_chord("sideways"), None);
        assert_eq!(direction_to_chord(""), None);
    }

    #[test]
    fn is_nvim_foreground_true_when_present() {
        let j = r#"{"result":{"process_info":{"foreground_processes":[{"name":"bash"},{"name":"nvim"}]}}}"#;
        assert!(is_nvim_foreground(j));
    }

    #[test]
    fn is_nvim_foreground_false_when_absent() {
        let j = r#"{"result":{"process_info":{"foreground_processes":[{"name":"bash"}]}}}"#;
        assert!(!is_nvim_foreground(j));
    }

    #[test]
    fn is_nvim_foreground_false_on_garbage() {
        assert!(!is_nvim_foreground("not json"));
        assert!(!is_nvim_foreground("{}"));
    }

    #[test]
    fn decide_sendkeys_when_nvim() {
        assert_eq!(
            decide(Some("wW:p8"), true, "left", "ctrl+h"),
            Action::SendKeys { pane: "wW:p8".into(), chord: "ctrl+h".into() }
        );
    }

    #[test]
    fn decide_focus_when_not_nvim() {
        assert_eq!(
            decide(Some("wW:p8"), false, "left", "ctrl+h"),
            Action::Focus { pane: "wW:p8".into(), direction: "left".into() }
        );
    }

    #[test]
    fn decide_focus_current_when_no_pane() {
        assert_eq!(
            decide(None, false, "left", "ctrl+h"),
            Action::FocusCurrent { direction: "left".into() }
        );
    }
}
```

- [ ] **Step 3: Run tests, verify they fail**

Run: `cd dot_local/share/herdr/herdr-smart-nav && cargo test`
Expected: compiles; FAILS: `direction_to_chord_maps_all_four` (got `None`), `is_nvim_foreground_true_when_present` (got false), `decide_sendkeys_when_nvim`/`decide_focus_when_not_nvim` (got `FocusCurrent`).

- [ ] **Step 4: Implement the pure core** (replace the three stubs)

```rust
fn direction_to_chord(direction: &str) -> Option<&'static str> {
    match direction {
        "left" => Some("ctrl+h"),
        "down" => Some("ctrl+j"),
        "up" => Some("ctrl+k"),
        "right" => Some("ctrl+l"),
        _ => None,
    }
}

fn is_nvim_foreground(process_info_json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(process_info_json)
        .ok()
        .and_then(|v| {
            v["result"]["process_info"]["foreground_processes"]
                .as_array()
                .map(|procs| procs.iter().any(|p| p["name"].as_str() == Some("nvim")))
        })
        .unwrap_or(false)
}

fn decide(pane: Option<&str>, is_nvim: bool, direction: &str, chord: &str) -> Action {
    match pane {
        Some(p) if is_nvim => Action::SendKeys { pane: p.to_string(), chord: chord.to_string() },
        Some(p) => Action::Focus { pane: p.to_string(), direction: direction.to_string() },
        None => Action::FocusCurrent { direction: direction.to_string() },
    }
}
```

- [ ] **Step 5: Run tests, verify they pass**

Run: `cargo test`
Expected: PASS (8 tests).

- [ ] **Step 6: Implement the thin herdr boundary + `main`** (replace the `fn main() {}` stub and add the boundary fns)

```rust
fn herdr_bin() -> String {
    env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_string())
}

fn herdr_capture(args: &[&str]) -> String {
    Command::new(herdr_bin())
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

fn herdr_run(args: &[&str]) {
    let _ = Command::new(herdr_bin()).args(args).status();
}

fn execute(action: &Action) {
    match action {
        Action::SendKeys { pane, chord } => herdr_run(&["pane", "send-keys", pane, chord]),
        Action::Focus { pane, direction } => {
            herdr_run(&["pane", "focus", "--direction", direction, "--pane", pane])
        }
        Action::FocusCurrent { direction } => {
            herdr_run(&["pane", "focus", "--direction", direction, "--current"])
        }
    }
}

fn main() {
    let direction = env::args().nth(1).unwrap_or_default();
    let Some(chord) = direction_to_chord(&direction) else {
        eprintln!("herdr-smart-nav: usage: herdr-smart-nav left|down|up|right");
        std::process::exit(2);
    };
    let pane = env::var("HERDR_ACTIVE_PANE_ID").ok().filter(|p| !p.is_empty());
    let is_nvim = match pane.as_deref() {
        Some(p) => is_nvim_foreground(&herdr_capture(&["pane", "process-info", "--pane", p])),
        None => false,
    };
    execute(&decide(pane.as_deref(), is_nvim, &direction, chord));
}
```

- [ ] **Step 7: Build (generates Cargo.lock + binary), re-test, commit**

Run: `cargo build --release --locked 2>/dev/null || cargo build --release` (first build generates `Cargo.lock`), then `cargo test`, then `cargo clippy` if available.
Expected: builds clean; 8 tests pass. Confirm `Cargo.lock` exists.

```bash
cd /Users/stephen/workspaces/Ivy/webdavis/dotfiles
git add dot_local/share/herdr/herdr-smart-nav/Cargo.toml \
        dot_local/share/herdr/herdr-smart-nav/Cargo.lock \
        dot_local/share/herdr/herdr-smart-nav/src/main.rs
git commit -m "feat(herdr): herdr-smart-nav Rust binary (pure core + tests)"
```
(`target/` is git-ignored repo-wide via `.gitignore`/`.chezmoiignore`; confirm it is not staged.)

---

### Task 2: Build chezmoiscript + lint wiring

**Files:**
- Create: `.chezmoiscripts/run_onchange_after_56-build-herdr-smart-nav.sh.tmpl`
- Modify: `scripts/lint.sh` (add the script to `find_shell_templates`)

- [ ] **Step 1: Write the build script** (mirrors `run_onchange_after_55`, minus plugin link/seed)

```text
{{ if eq .chezmoi.os "darwin" -}}
#!/bin/bash

set -euo pipefail

# Builds the herdr-smart-nav binary (compiled replacement for the ctrl-h/j/k/l
# nav shell script) and installs it to ~/.local/bin. Re-runs whenever the source
# changes (run_onchange keyed on the hashes below). Mirrors the last-workspace
# plugin build.
#
# Source (hashed so this script re-runs on change):
#   {{ include "dot_local/share/herdr/herdr-smart-nav/src/main.rs" | sha256sum }}
#   {{ include "dot_local/share/herdr/herdr-smart-nav/Cargo.lock" | sha256sum }}
#   {{ include "dot_local/share/herdr/herdr-smart-nav/Cargo.toml" | sha256sum }}

src_dir="$HOME/.local/share/herdr/herdr-smart-nav"

if ! command -v cargo &>/dev/null; then
  echo "cargo not found on PATH; run the rustup bootstrap first" >&2
  exit 1
fi

# --locked keeps the vendored Cargo.lock authoritative so it does not drift.
(cd "$src_dir" && cargo build --release --locked)

install -m 0755 "$src_dir/target/release/herdr-smart-nav" "$HOME/.local/bin/herdr-smart-nav"

# Remove the superseded shell script.
rm -f "$HOME/.local/bin/herdr-smart-nav.sh"
{{ end -}}
```

- [ ] **Step 2: Add to `find_shell_templates`**: `scripts/lint.sh`, after the homebrew-weekly-upgrade loader line

```bash
    -o -name "run_onchange_after_65-load-homebrew-weekly-upgrade-launchagent.sh.tmpl" \
    -o -name "run_onchange_after_56-build-herdr-smart-nav.sh.tmpl" \
```

- [ ] **Step 3: Render + shellcheck the build script**

Run: `CI=1 chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_56-build-herdr-smart-nav.sh.tmpl | shellcheck -`
Expected: clean.
Run: `just s`
Expected: ✅ shellcheck.

- [ ] **Step 4: Commit**

```bash
git add .chezmoiscripts/run_onchange_after_56-build-herdr-smart-nav.sh.tmpl scripts/lint.sh
git commit -m "feat(herdr): build+install script for herdr-smart-nav + lint wiring"
```

---

### Task 3: Repoint keybindings, retire the script, document

**Files:**
- Modify: `dot_config/herdr/config.toml` (4 nav `command` lines + the comment)
- Remove: `dot_local/bin/executable_herdr-smart-nav.sh`
- Modify: `CLAUDE.md` (one-line note under "### Herdr Workspace Management")

- [ ] **Step 1: Repoint the bindings + comment**: `dot_config/herdr/config.toml`

Replace every occurrence of `herdr-smart-nav.sh` with `herdr-smart-nav` (4 `command = "$HOME/.local/bin/herdr-smart-nav.sh <dir>"` lines → drop `.sh`; the comment at line ~175 likewise). Use an exact replace-all of the string `herdr-smart-nav.sh` → `herdr-smart-nav`.

- [ ] **Step 2: Remove the old script**

```bash
git rm dot_local/bin/executable_herdr-smart-nav.sh
```

- [ ] **Step 3: CLAUDE.md note**: append to the "### Herdr Workspace Management" section

```markdown
Ctrl-h/j/k/l "seamless nav across Neovim splits and herdr panes" is a compiled Rust binary
(`~/.local/bin/herdr-smart-nav`, source `dot_local/share/herdr/herdr-smart-nav/`, built by
`run_onchange_after_56`), not a shell script. It shells the `herdr` CLI (no Rust SDK exists); the speedup
over the old `.sh` is small (~5ms, bash+jq removed), the value is a unit-tested binary, not felt speed.
```

- [ ] **Step 4: Validate + commit**

Run: `just t` (taplo, config.toml clean) and `just m` (mdformat, CLAUDE.md)
Expected: ✅.

```bash
git add dot_config/herdr/config.toml CLAUDE.md
git commit -m "feat(herdr): repoint ctrl-h/j/k/l to the herdr-smart-nav binary; retire the script"
```
(`git rm` from Step 2 is included in this commit.)

---

### Task 4: Activate + verify (live, connection-safe)

**Files:** none (live actions on this machine; no real nav side effects beyond an optional benign focus move).

- [ ] **Step 1: Deploy the source + build/install the binary**

```bash
chezmoi apply --force "$HOME/.local/share/herdr/herdr-smart-nav"
CI=1 chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_56-build-herdr-smart-nav.sh.tmpl | bash
test -x "$HOME/.local/bin/herdr-smart-nav" && echo "binary installed"
test ! -e "$HOME/.local/bin/herdr-smart-nav.sh" && echo "old script removed"
```
Expected: binary installed; old script gone.

- [ ] **Step 2: Apply the new keybinding + reload herdr**

```bash
chezmoi apply --force "$HOME/.config/herdr/config.toml"
herdr server reload-config 2>&1 | tail -1
herdr config get 2>/dev/null | grep -c 'herdr-smart-nav.sh' # expect 0 (no stale .sh refs)
```
Expected: reload ok; 0 stale `.sh` references.

- [ ] **Step 3: Non-disruptive smoke + final suite**

```bash
"$HOME/.local/bin/herdr-smart-nav" bogus; echo "usage-exit: $?"   # expect 2 (binary runs + arg-parses; no nav side effect)
just l && just test
```
Expected: usage-exit 2; all ✅. The real nav (focus move / nvim forward) is exercised on the next ctrl-h/j/k/l keypress.

---

## Self-Review

**Spec coverage:** pure core + tests → Task 1; thin herdr boundary + main → Task 1 Step 6; build/install mirroring the plugin → Task 2; keybinding repoint + script removal → Task 3; lint wiring → Task 2; CLAUDE.md → Task 3; activate/verify → Task 4; "no socket reimpl" honored (shells CLI). No gaps.

**Placeholder scan:** none: full Cargo.toml, full `main.rs` (stubs + real), full build script, exact edits and commands.

**Type/name consistency:** `Action` variants (`SendKeys{pane,chord}`, `Focus{pane,direction}`, `FocusCurrent{direction}`) are identical in the enum decl (Step 2), `decide` (Step 4), `execute` (Step 6), and the tests. `direction_to_chord`/`is_nvim_foreground`/`decide` signatures match between stubs (Step 2), real impls (Step 4), and call sites (Step 6). Binary name `herdr-smart-nav` is identical across Cargo.toml, the build script's `install`, the keybinding, and Task 4.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-21-herdr-nav-rust.md`.
