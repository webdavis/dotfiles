#!/usr/bin/env bash
# herdr-build-scripts-resilience.sh — the herdr plugin build chezmoiscript
# (run_onchange_after_55) must survive a fresh/headless machine without aborting
# the whole `chezmoi apply`. It renders the REAL script with the host chezmoi and
# runs the rendered body against stub `herdr`/`cargo` on PATH, asserting:
#
#   F1  herdr server down (rc=3)      -> script exits 0 (link step tolerates it)
#   F1  herdr emits error JSON (rc=0) -> script exits 0 (link step tolerates it)
#   F3  seed shape (built binary)     -> when the build produced a binary, the
#                                        script runs `<binary> seed` (the MRU seed
#                                        now lives in the plugin, not in bash — its
#                                        state-path/idempotence correctness is a
#                                        Rust cfg(test) concern in src/main.rs);
#                                        when the build is skipped, seed is not run
#   F2  cargo only on PATH            -> NOT used: the build resolves cargo at the
#                                        deterministic ~/.cargo/bin/cargo only, so
#                                        a PATH-only cargo is ignored (skip+hint)
#   F2  cargo at ~/.cargo/bin + PATH  -> the absolute ~/.cargo/bin/cargo is the
#                                        one invoked, never the PATH cargo
#   F2  cargo absent everywhere       -> script exits 0 with a hint (never aborts)
#
# This exercises the rendered code path itself (not a copy). Stubs shadow the
# live `herdr`/`cargo` on PATH and stand in for the built plugin binary, so the
# test never touches the running server or needs a real cargo build.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_onchange_after_55-build-herdr-last-workspace-plugin.sh.tmpl"
PLUGIN_ID="herdr-last-workspace"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Host-tool guards: plain test/*.sh scripts run outside the Nix shell.
for tool in chezmoi jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'SKIP: %s not on PATH; cannot render/run the plugin build script\n' "$tool"
    exit 0
  fi
done
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Render the darwin-only script once (scratch HOME, CI=1 — same mechanics as the
# treefmt rendered-template lint). Empty render == non-darwin host: skip.
rendered="$work/rendered.sh"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$SCRIPT" >"$rendered" || fail "chezmoi failed to render $SCRIPT"
rm -rf "$render_home"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# PATH with every dir that carries an executable `cargo` removed, so the "no
# cargo" cases genuinely find none while coreutils/jq stay reachable.
path_without_cargo() {
  local -a dirs=() keep=()
  IFS=: read -ra dirs <<<"$PATH"
  local d
  for d in "${dirs[@]}"; do
    [[ -n $d ]] || continue
    [[ -x $d/cargo ]] && continue
    keep+=("$d")
  done
  local IFS=:
  printf '%s' "${keep[*]}"
}
PATH_NO_CARGO="$(path_without_cargo)"

# Write a stub `herdr` for the given mode into $1.
make_herdr_stub() {
  local dir="$1" mode="$2"
  case "$mode" in
    down) # server unreachable — every subcommand fails
      cat >"$dir/herdr" <<'STUB'
#!/bin/bash
exit 3
STUB
      ;;
    errorjson) # reachable but returns an error envelope with exit 0
      cat >"$dir/herdr" <<'STUB'
#!/bin/bash
if [[ "$1 $2" == "workspace list" ]]; then
  printf '{"id":"x","error":{"code":-1,"message":"server error"}}\n'
fi
exit 0
STUB
      ;;
    happy) # reachable, one focused workspace (id wW)
      cat >"$dir/herdr" <<'STUB'
#!/bin/bash
if [[ "$1 $2" == "workspace list" ]]; then
  printf '{"result":{"workspaces":[{"focused":true,"workspace_id":"wW","label":"dotfiles"},{"focused":false,"workspace_id":"wY","label":"todoist"}]}}\n'
fi
exit 0
STUB
      ;;
    *) fail "unknown herdr stub mode: $mode" ;;
  esac
  chmod +x "$dir/herdr"
}

# Write a stub `cargo` that records its argv into $2, at path $1.
make_cargo_stub() {
  local path="$1" record="$2"
  mkdir -p "$(dirname "$path")"
  cat >"$path" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"$record"
exit 0
STUB
  chmod +x "$path"
}

# run_case <name> <herdr-mode> <cargo-mode>
#   cargo-mode: home     -> cargo only at the absolute ~/.cargo/bin/cargo
#               pathonly -> cargo only on PATH (must be ignored by the build)
#               both     -> cargo at BOTH ~/.cargo/bin and on PATH
#               none     -> no cargo anywhere
# Populates: RC, ERR (stderr text), CASE_HOME, CARGO_RECORD (absolute-cargo
# argv), PATH_CARGO_RECORD (PATH-cargo argv), SEED_RECORD (built-binary argv)
run_case() {
  local name="$1" herdr_mode="$2" cargo_mode="$3"
  CASE_HOME="$work/$name/home"
  local bin="$work/$name/bin"
  CARGO_RECORD="$work/$name/cargo-abs-argv"
  PATH_CARGO_RECORD="$work/$name/cargo-path-argv"
  SEED_RECORD="$work/$name/seed-argv"
  local plugin_dir="$CASE_HOME/.local/share/herdr/plugins/$PLUGIN_ID"
  mkdir -p "$bin" "$plugin_dir/target/release"

  make_herdr_stub "$bin" "$herdr_mode"

  # Stand in for the compiled plugin binary the real `cargo build` would emit.
  # It records its argv so we can assert the script runs it as `<binary> seed`.
  # (The stub `cargo` never builds anything, so the script would otherwise have
  # no binary to seed with.)
  cat >"$plugin_dir/target/release/last-workspace" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"$SEED_RECORD"
exit 0
STUB
  chmod +x "$plugin_dir/target/release/last-workspace"

  local case_path
  case "$cargo_mode" in
    home)
      make_cargo_stub "$CASE_HOME/.cargo/bin/cargo" "$CARGO_RECORD"
      case_path="$bin:$PATH_NO_CARGO"
      ;;
    pathonly)
      make_cargo_stub "$bin/cargo" "$PATH_CARGO_RECORD"
      case_path="$bin:$PATH_NO_CARGO"
      ;;
    both)
      make_cargo_stub "$CASE_HOME/.cargo/bin/cargo" "$CARGO_RECORD"
      make_cargo_stub "$bin/cargo" "$PATH_CARGO_RECORD"
      case_path="$bin:$PATH_NO_CARGO"
      ;;
    none)
      case_path="$bin:$PATH_NO_CARGO"
      ;;
    *) fail "unknown cargo mode: $cargo_mode" ;;
  esac

  local errf="$work/$name/err"
  RC=0
  HOME="$CASE_HOME" PATH="$case_path" \
    bash "$rendered" >/dev/null 2>"$errf" || RC=$?
  ERR="$(cat "$errf")"
}

# --- F1: herdr server down — the link step must not abort the apply --------
run_case f1-down down home
[[ $RC -eq 0 ]] || fail "F1 down: expected exit 0, got $RC (stderr: $ERR)"

# --- F1: herdr returns an error envelope -----------------------------------
run_case f1-errorjson errorjson home
[[ $RC -eq 0 ]] || fail "F1 errorjson: expected exit 0, got $RC (stderr: $ERR)"

# --- F3: happy path — the script runs the built binary's `seed` subcommand --
run_case f3-happy happy home
[[ $RC -eq 0 ]] || fail "F3 happy: expected exit 0, got $RC (stderr: $ERR)"
[[ -s $SEED_RECORD ]] || fail "F3 happy: the plugin binary was not run to seed the MRU"
grep -qx 'seed' "$SEED_RECORD" ||
  fail "F3 happy: binary not invoked as \`<binary> seed\` (recorded: $(cat "$SEED_RECORD"))"

# --- F2: a PATH-only cargo must NOT be used (deterministic absolute path) ---
run_case f2-pathonly happy pathonly
[[ $RC -eq 0 ]] || fail "F2 pathonly: expected exit 0, got $RC (stderr: $ERR)"
grep -qi 'cargo not found' <<<"$ERR" ||
  fail "F2 pathonly: a PATH-only cargo must be ignored (skip-with-hint expected) ($ERR)"
[[ ! -e $PATH_CARGO_RECORD ]] ||
  fail "F2 pathonly: a PATH cargo was invoked; only ~/.cargo/bin/cargo is authoritative ($(cat "$PATH_CARGO_RECORD"))"
[[ ! -e $SEED_RECORD ]] ||
  fail "F2 pathonly: seed ran despite the build being skipped"

# --- F2: absolute cargo wins even when a PATH cargo also exists -------------
run_case f2-both happy both
[[ $RC -eq 0 ]] || fail "F2 both: expected exit 0, got $RC (stderr: $ERR)"
[[ -s $CARGO_RECORD ]] || fail "F2 both: ~/.cargo/bin/cargo was not invoked"
grep -q 'build' "$CARGO_RECORD" || fail "F2 both: absolute cargo not invoked to build ($(cat "$CARGO_RECORD"))"
[[ ! -e $PATH_CARGO_RECORD ]] ||
  fail "F2 both: the PATH cargo was invoked instead of the absolute path ($(cat "$PATH_CARGO_RECORD"))"
[[ -s $SEED_RECORD ]] || fail "F2 both: seed did not run after a successful build"

# --- F2: cargo absent everywhere -------------------------------------------
run_case f2-none down none
[[ $RC -eq 0 ]] || fail "F2 no-cargo: expected exit 0 (must never abort apply), got $RC (stderr: $ERR)"
grep -qi 'cargo not found' <<<"$ERR" || fail "F2 no-cargo: missing skip-with-hint message ($ERR)"
[[ ! -e $CARGO_RECORD ]] || fail "F2 no-cargo: cargo ran despite being absent"
[[ ! -e $SEED_RECORD ]] || fail "F2 no-cargo: seed ran despite the build being skipped"

printf 'PASS: after_55 tolerates a down/erroring herdr server and a missing cargo toolchain, resolves cargo at the deterministic absolute path, and seeds via the built plugin binary\n'
