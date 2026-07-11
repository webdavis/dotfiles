#!/usr/bin/env bash
# herdr-build-scripts-resilience.sh: the herdr plugin build chezmoiscript
# (run_onchange_after_55) separates BUILD from REGISTRATION, each with its own
# failure envelope, and treats a missing toolchain or an unverified registration
# as a RETRYABLE non-success, NOT a satisfied trigger.
#
# This REVERSES the previous contract (which asserted "cargo absent -> exit 0,
# never aborts", implying the trigger was consumed/satisfied). chezmoi records a
# run_onchange script as done on ANY exit 0, so a bare exit 0 on a skipped build
# would make the next apply believe the work is finished and never retry. The
# retry state is carried by a marker file the template embeds into the rendered
# trigger (bumping it re-fires the run_onchange; the re-fire itself is proven in
# test/herdr-build-input-hashing.sh's sibling render-diff assertions), so the
# CONTRACT this script checks is expressed as the marker, not the exit code:
#
#   retryable non-success  -> exit 0, retry marker PRESENT (trigger un-consumed)
#   full success           -> exit 0, retry marker ABSENT  (trigger settles)
#   hard build failure     -> exit NON-ZERO (loud; chezmoi does not record it, so
#                             it retries on its own after the source is fixed)
#
# Cases:
#   R1  missing cargo (absent everywhere) -> retryable: marker present, exit 0,
#                                            no build, no seed
#   R1  cargo only on PATH                -> same as missing (only ~/.cargo/bin
#                                            is authoritative): retryable
#   R2  build ok + herdr server down      -> registration unverified: retryable
#                                            (marker present), exit 0
#   R2  build ok + herdr error envelope   -> registration unverified: retryable
#   R3  build ok + link registers plugin  -> success: marker ABSENT (cleared even
#                                            if it pre-existed), exit 0, seed ran
#   R3  build ok + already registered     -> success, link NOT attempted
#   R4  cargo build fails (compile error) -> loud: exit NON-ZERO
#   F2  cargo at ~/.cargo/bin + PATH      -> the absolute cargo is the one used
#
# The rendered code path itself is exercised (not a copy). Stubs shadow the live
# herdr/cargo on PATH and stand in for the built plugin binary.
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

# Render the darwin-only script once (scratch HOME, CI=1; same mechanics as the
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

# Write a stub `herdr` for the given mode into $1. STATE/$LINK_REC are per-case
# files the stub uses to model registration across its (stateless) invocations.
#   registered   -> `plugin list --plugin` reports the plugin from the start
#   linkok       -> not registered until `plugin link` runs, then reported
#   down         -> every subcommand fails (server unreachable, exit 3)
#   errorjson    -> `plugin list`/`plugin link` return a herdr error envelope
make_herdr_stub() {
  local dir="$1" mode="$2" state="$3" link_rec="$4" pid="$5"
  case "$mode" in
    registered) : >"$state" ;; # pre-registered
    linkok | down | errorjson) rm -f "$state" ;;
    *) fail "unknown herdr stub mode: $mode" ;;
  esac
  cat >"$dir/herdr" <<STUB
#!/bin/bash
mode="$mode"
state="$state"
link_rec="$link_rec"
pid="$pid"
sub="\$1 \$2"
if [[ \$mode == down ]]; then
  exit 3
fi
case "\$sub" in
  "plugin list")
    if [[ \$mode == errorjson ]]; then
      printf '{"id":"cli:plugin","error":{"code":-1,"message":"server error"}}\n'
      exit 0
    fi
    if [[ -f \$state ]]; then
      printf '{"id":"cli:plugin","result":{"plugins":[{"plugin_id":"%s"}],"type":"plugin_list"}}\n' "\$pid"
    else
      printf '{"id":"cli:plugin","result":{"plugins":[],"type":"plugin_list"}}\n'
    fi
    exit 0
    ;;
  "plugin link")
    printf '%s\n' "\$*" >>"\$link_rec"
    if [[ \$mode == errorjson ]]; then
      printf '{"id":"cli:plugin","error":{"code":-1,"message":"link refused"}}\n'
      exit 0
    fi
    : >"\$state"
    exit 0
    ;;
esac
exit 0
STUB
  chmod +x "$dir/herdr"
}

# Write a stub `cargo` at path $1 recording argv to $2. If $3 == fail, `build`
# exits non-zero (a real compile failure).
make_cargo_stub() {
  local path="$1" record="$2" build_mode="${3:-ok}"
  mkdir -p "$(dirname "$path")"
  cat >"$path" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"$record"
if [[ "\$1" == build && "$build_mode" == fail ]]; then
  echo "error: could not compile" >&2
  exit 101
fi
exit 0
STUB
  chmod +x "$path"
}

# run_case <name> <herdr-mode> <cargo-mode> [build-mode] [pre-mark]
#   cargo-mode: home | pathonly | both | none
#   build-mode: ok | fail   (only meaningful when cargo is present)
#   pre-mark:   yes     -> pre-create the retry marker (proves success clears it)
#               garbage -> pre-create it with a NON-numeric count (proves the
#                          run-time normalization resets it before the bump)
# Populates: RC, ERR, CASE_HOME, CARGO_RECORD, PATH_CARGO_RECORD, SEED_RECORD,
#            LINK_RECORD, MARKER
run_case() {
  local name="$1" herdr_mode="$2" cargo_mode="$3" build_mode="${4:-ok}" pre_mark="${5:-no}"
  CASE_HOME="$work/$name/home"
  local bin="$work/$name/bin"
  CARGO_RECORD="$work/$name/cargo-abs-argv"
  PATH_CARGO_RECORD="$work/$name/cargo-path-argv"
  SEED_RECORD="$work/$name/seed-argv"
  LINK_RECORD="$work/$name/link-argv"
  MARKER="$CASE_HOME/.cache/herdr-plugin-build/$PLUGIN_ID.retry"
  local state="$work/$name/herdr-state"
  local plugin_dir="$CASE_HOME/.local/share/herdr/plugins/$PLUGIN_ID"
  mkdir -p "$bin" "$plugin_dir/target/release"

  make_herdr_stub "$bin" "$herdr_mode" "$state" "$LINK_RECORD" "$PLUGIN_ID"

  # Stand in for the compiled plugin binary the real `cargo build` would emit;
  # it records its argv so we can assert the script runs it as `<binary> seed`.
  cat >"$plugin_dir/target/release/last-workspace" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"$SEED_RECORD"
exit 0
STUB
  chmod +x "$plugin_dir/target/release/last-workspace"

  case "$pre_mark" in
    yes)
      mkdir -p "$(dirname "$MARKER")"
      printf '2\n' >"$MARKER"
      ;;
    garbage)
      # 12abc is deliberately arithmetic-hostile: without the run-time
      # normalization, $((12abc + 1)) is a syntax error that aborts the
      # rendered script under set -e (a bare identifier like 'garbage' would
      # evaluate to 0 and mask the missing guard).
      mkdir -p "$(dirname "$MARKER")"
      printf '12abc\n' >"$MARKER"
      ;;
    no) ;;
    *) fail "unknown pre-mark mode: $pre_mark" ;;
  esac

  local case_path
  case "$cargo_mode" in
    home)
      make_cargo_stub "$CASE_HOME/.cargo/bin/cargo" "$CARGO_RECORD" "$build_mode"
      case_path="$bin:$PATH_NO_CARGO"
      ;;
    pathonly)
      make_cargo_stub "$bin/cargo" "$PATH_CARGO_RECORD" "$build_mode"
      case_path="$bin:$PATH_NO_CARGO"
      ;;
    both)
      make_cargo_stub "$CASE_HOME/.cargo/bin/cargo" "$CARGO_RECORD" "$build_mode"
      make_cargo_stub "$bin/cargo" "$PATH_CARGO_RECORD" "$build_mode"
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

marker_present() { [[ -f $MARKER ]]; }

# --- R1: missing cargo (absent everywhere) is a RETRYABLE non-success ---------
run_case r1-none down none
[[ $RC -eq 0 ]] || fail "R1 no-cargo: expected exit 0 (must never abort apply), got $RC ($ERR)"
marker_present || fail "R1 no-cargo: retry marker not written (trigger would be consumed, no retry)"
[[ ! -e $CARGO_RECORD ]] || fail "R1 no-cargo: cargo ran despite being absent"
[[ ! -e $SEED_RECORD ]] || fail "R1 no-cargo: seed ran despite the build being skipped"

# --- R1: a PATH-only cargo counts as missing (absolute path authoritative) ----
run_case r1-pathonly down pathonly
[[ $RC -eq 0 ]] || fail "R1 pathonly: expected exit 0, got $RC ($ERR)"
marker_present || fail "R1 pathonly: retry marker not written for a PATH-only (ignored) cargo"
[[ ! -e $PATH_CARGO_RECORD ]] || fail "R1 pathonly: a PATH cargo was invoked ($(cat "$PATH_CARGO_RECORD"))"
[[ ! -e $SEED_RECORD ]] || fail "R1 pathonly: seed ran despite the build being skipped"

# --- R1: a GARBAGE marker count is reset at run time (normalization guard) ----
# The bump normalizes a non-numeric stored count to 0 before incrementing, so
# a corrupted marker yields a clean count of 1, not a shell arithmetic error.
run_case r1-garbage-marker down none ok garbage
[[ $RC -eq 0 ]] || fail "R1 garbage-marker: expected exit 0, got $RC ($ERR)"
marker_present || fail "R1 garbage-marker: retry marker missing after the bump"
[[ "$(cat "$MARKER")" == "1" ]] ||
  fail "R1 garbage-marker: a garbage count was not reset before the bump (got '$(cat "$MARKER")', expected 1)"

# --- R2: build ok but herdr server down -> registration UNVERIFIED (retryable) -
run_case r2-down down home
[[ $RC -eq 0 ]] || fail "R2 down: expected exit 0, got $RC ($ERR)"
[[ -s $CARGO_RECORD ]] || fail "R2 down: cargo did not build"
marker_present || fail "R2 down: registration failed but retry marker not written (trigger consumed)"

# --- R2: build ok but herdr returns an error envelope -> UNVERIFIED (retryable) -
run_case r2-errorjson errorjson home
[[ $RC -eq 0 ]] || fail "R2 errorjson: expected exit 0, got $RC ($ERR)"
marker_present || fail "R2 errorjson: error envelope not treated as retryable non-success"

# --- R3: build ok + link registers the exact plugin -> SUCCESS (marker cleared) -
run_case r3-linkok linkok home ok yes
[[ $RC -eq 0 ]] || fail "R3 linkok: expected exit 0, got $RC ($ERR)"
marker_present && fail "R3 linkok: retry marker still present after a verified registration (not cleared)"
[[ -s $LINK_RECORD ]] || fail "R3 linkok: plugin was not linked"
[[ -s $SEED_RECORD ]] || fail "R3 linkok: seed did not run after a successful build"

# --- R3: already registered -> SUCCESS, link NOT attempted (idempotent) --------
run_case r3-registered registered home
[[ $RC -eq 0 ]] || fail "R3 registered: expected exit 0, got $RC ($ERR)"
marker_present && fail "R3 registered: retry marker present despite the plugin already registered"
[[ ! -e $LINK_RECORD ]] || fail "R3 registered: link attempted for an already-registered plugin ($(cat "$LINK_RECORD"))"

# --- R4: a real cargo build failure fails LOUDLY (exit non-zero) ---------------
run_case r4-buildfail linkok home fail
[[ $RC -ne 0 ]] || fail "R4 buildfail: a compile failure must fail loudly (non-zero), got exit 0"
grep -qi 'build failed' <<<"$ERR" || fail "R4 buildfail: missing a loud build-failure message ($ERR)"

# --- F2: absolute cargo wins even when a PATH cargo also exists ----------------
run_case f2-both linkok both
[[ $RC -eq 0 ]] || fail "F2 both: expected exit 0, got $RC ($ERR)"
[[ -s $CARGO_RECORD ]] || fail "F2 both: ~/.cargo/bin/cargo was not invoked"
grep -q 'build' "$CARGO_RECORD" || fail "F2 both: absolute cargo not invoked to build ($(cat "$CARGO_RECORD"))"
# The exact build argv matters: --release places the binary where the seed and
# the plugin manifest expect it, and --locked keeps the vendored Cargo.lock
# authoritative so the lockfile cannot drift on apply.
grep -qw -- '--release' "$CARGO_RECORD" ||
  fail "F2 both: cargo argv lacks --release ($(cat "$CARGO_RECORD"))"
grep -qw -- '--locked' "$CARGO_RECORD" ||
  fail "F2 both: cargo argv lacks --locked ($(cat "$CARGO_RECORD"))"
[[ ! -e $PATH_CARGO_RECORD ]] || fail "F2 both: the PATH cargo was invoked instead ($(cat "$PATH_CARGO_RECORD"))"

printf 'PASS: after_55 separates build from registration, treats missing cargo and unverified registration as retryable (marker present, exit 0), clears the marker on a verified exact-id registration, and fails loudly on a real build error\n'
