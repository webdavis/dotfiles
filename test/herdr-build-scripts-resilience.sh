#!/usr/bin/env bash
# herdr-build-scripts-resilience.sh — the herdr plugin build chezmoiscript
# (run_onchange_after_55) must survive a fresh/headless machine without aborting
# the whole `chezmoi apply`. It renders the REAL script with the host chezmoi and
# runs the rendered body against stub `herdr`/`cargo` on PATH, asserting:
#
#   F1  herdr server down (rc=3)      -> script exits 0, no jq error on stderr
#   F1  herdr emits error JSON (rc=0) -> script exits 0, no jq error on stderr
#   F4  happy path w/ XDG_STATE_HOME  -> seed writes to $HOME/.local/state
#                                        (the plugin's hardcoded read path), NOT
#                                        the XDG_STATE_HOME location
#   F2  cargo only in ~/.cargo/bin    -> the absolute-path fallback is used (a
#                                        fresh machine has rustup installed there
#                                        but not yet on the apply shell's PATH)
#   F2  cargo absent everywhere       -> script exits 0 with a hint (never aborts)
#
# This exercises the rendered code path itself (not a copy). Stubs shadow the
# live `herdr`/`cargo` on PATH, so the test never touches the running server.
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

# run_case <name> <herdr-mode> <cargo-mode> [xdg_state_home]
#   cargo-mode: path | home | none
# Populates: RC, ERR (stderr text), CASE_HOME, CARGO_RECORD
run_case() {
  local name="$1" herdr_mode="$2" cargo_mode="$3" xdg="${4:-}"
  CASE_HOME="$work/$name/home"
  local bin="$work/$name/bin"
  CARGO_RECORD="$work/$name/cargo-argv"
  mkdir -p "$bin" "$CASE_HOME/.local/share/herdr/plugins/$PLUGIN_ID"

  make_herdr_stub "$bin" "$herdr_mode"

  local case_path
  case "$cargo_mode" in
    path)
      make_cargo_stub "$bin/cargo" "$CARGO_RECORD"
      case_path="$bin:$PATH"
      ;;
    home)
      make_cargo_stub "$CASE_HOME/.cargo/bin/cargo" "$CARGO_RECORD"
      case_path="$bin:$PATH_NO_CARGO"
      ;;
    none)
      case_path="$bin:$PATH_NO_CARGO"
      ;;
    *) fail "unknown cargo mode: $cargo_mode" ;;
  esac

  local errf="$work/$name/err"
  RC=0
  if [[ -n $xdg ]]; then
    HOME="$CASE_HOME" PATH="$case_path" XDG_STATE_HOME="$xdg" \
      bash "$rendered" >/dev/null 2>"$errf" || RC=$?
  else
    HOME="$CASE_HOME" PATH="$case_path" \
      bash "$rendered" >/dev/null 2>"$errf" || RC=$?
  fi
  ERR="$(cat "$errf")"
}

no_jq_error() {
  ! grep -qiE 'jq: error|cannot iterate' <<<"$ERR"
}

# --- F1: herdr server down -------------------------------------------------
run_case f1-down down path
[[ $RC -eq 0 ]] || fail "F1 down: expected exit 0, got $RC (stderr: $ERR)"
no_jq_error || fail "F1 down: jq error leaked to stderr: $ERR"

# --- F1: herdr returns an error envelope -----------------------------------
run_case f1-errorjson errorjson path
[[ $RC -eq 0 ]] || fail "F1 errorjson: expected exit 0, got $RC (stderr: $ERR)"
no_jq_error || fail "F1 errorjson: jq error leaked to stderr: $ERR"

# --- F4: seed path must match the plugin's hardcoded read path -------------
xdg_dir="$work/f4-happy/xdg-state"
run_case f4-happy happy path "$xdg_dir"
[[ $RC -eq 0 ]] || fail "F4 happy: expected exit 0, got $RC (stderr: $ERR)"
seeded="$CASE_HOME/.local/state/herdr/plugins/$PLUGIN_ID/mru"
[[ -s $seeded ]] || fail "F4 happy: seed not written to the plugin read path ($seeded)"
head -n1 "$seeded" | grep -qx "wW" || fail "F4 happy: seed content wrong ($(cat "$seeded"))"
[[ ! -e "$xdg_dir/herdr/plugins/$PLUGIN_ID/mru" ]] ||
  fail "F4 happy: seed honored XDG_STATE_HOME; plugin never reads there"

# --- F2: cargo only in ~/.cargo/bin (fresh-machine fallback) ---------------
run_case f2-home down home
[[ $RC -eq 0 ]] || fail "F2 home-fallback: expected exit 0, got $RC (stderr: $ERR)"
[[ -s $CARGO_RECORD ]] || fail "F2 home-fallback: ~/.cargo/bin/cargo fallback was not used"
grep -q 'build' "$CARGO_RECORD" || fail "F2 home-fallback: cargo not invoked to build ($(cat "$CARGO_RECORD"))"

# --- F2: cargo absent everywhere -------------------------------------------
run_case f2-none down none
[[ $RC -eq 0 ]] || fail "F2 no-cargo: expected exit 0 (must never abort apply), got $RC (stderr: $ERR)"
grep -qi 'cargo not found' <<<"$ERR" || fail "F2 no-cargo: missing skip-with-hint message ($ERR)"
[[ ! -e $CARGO_RECORD ]] || fail "F2 no-cargo: cargo ran despite being absent"

printf 'PASS: after_55 tolerates a down/erroring herdr server and a missing cargo toolchain, and seeds the plugin read path\n'
