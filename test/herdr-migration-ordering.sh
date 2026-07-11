#!/usr/bin/env bash
# herdr-migration-ordering.sh: run_onchange_before_10-system-packages must NOT
# let `brew bundle --cleanup` remove the old multiplexer (tmux/sesh) before
# herdr is installed AND currently healthy. `--cleanup` uninstalls anything
# installed but absent from the desired set, so once tmux/sesh leave the
# manifest a bare --cleanup on a not-yet-migrated machine would strip the only
# multiplexer the moment before herdr's install/verify; if that fails, the
# machine is stranded.
#
# The invariant has two halves:
#
#   1. TRIGGER: the stamp run_after_58 writes must be part of before_10's
#      RENDERED body (a presence boolean), because chezmoi re-runs a
#      run_onchange script only when its rendered content changes. Without
#      this, the stamp write at apply N would not re-fire before_10 at apply
#      N+1 and the teardown would wait for an unrelated package-data edit.
#   2. AUTHORITY: cleanup passes only when the stamp is present AND a LIVE
#      herdr health check succeeds in the same apply. A stale stamp (written
#      while healthy, herdr broken since) must NOT authorize the teardown.
#
# Short-circuit: when neither tmux nor sesh is installed (already migrated),
# cleanup always proceeds and the health check is skipped entirely.
#
# This renders the REAL before_10 and runs it with a stub Homebrew prefix
# (brew/uv/volta), a stub npm, and a stub herdr on PATH; nothing real is
# installed. The exact `brew bundle` argv and the stderr warnings are captured.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl"
STAMP_REL=".cache/herdr-migration/verified"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

for tool in chezmoi jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'SKIP: %s not on PATH; cannot render/run before_10\n' "$tool"
    exit 0
  fi
done
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

render_into() {
  local home="$1" out="$2"
  HOME="$home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$SCRIPT" >"$out" || fail "chezmoi failed to render $SCRIPT (HOME=$home)"
}

# --- render-level trigger oracle (stamp presence must change the render) -----
# chezmoi re-runs a run_onchange script only when its rendered content changes,
# so the stamp's PRESENCE must be interpolated into the body. Render once with
# the stamp absent and once with it present: the two bodies must differ. This
# is the exact regression oracle for "the stamp write re-fires before_10".
no_stamp_home="$work/render-no-stamp"
stamp_home="$work/render-stamp"
mkdir -p "$no_stamp_home" "$stamp_home/$(dirname "$STAMP_REL")"
: >"$stamp_home/$STAMP_REL"
render_into "$no_stamp_home" "$work/render-no-stamp.sh"
render_into "$stamp_home" "$work/render-stamp.sh"
if [[ ! -s $work/render-no-stamp.sh ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi
if cmp -s "$work/render-no-stamp.sh" "$work/render-stamp.sh"; then
  fail "rendered before_10 is identical with and without the stamp; the stamp write would never re-fire the run_onchange trigger (cleanup would wait for an unrelated package-data edit)"
fi

# The runtime cases below all execute the no-stamp render; the stamp check and
# the health check both happen at RUN time, so one rendered body exercises
# every case.
rendered="$work/render-no-stamp.sh"

# Stub Homebrew prefix: brew (tap/trust/list/bundle/autoupdate), uv, volta.
# brew `list <pkg>` reports "installed" only for names in $INSTALLED_PKGS;
# `bundle` records its argv and swallows the Brewfile on stdin.
build_stub_prefix() {
  local prefix="$1"
  mkdir -p "$prefix/bin"
  cat >"$prefix/bin/brew" <<'STUB'
#!/bin/bash
case "$1" in
  tap | trust | autoupdate) exit 0 ;;
  list)
    pkg="$2"
    for installed in $INSTALLED_PKGS; do
      [[ $pkg == "$installed" ]] && exit 0
    done
    exit 1 ;;
  bundle)
    printf '%s\n' "$*" >>"$BUNDLE_RECORD"
    cat >/dev/null
    exit 0 ;;
esac
exit 0
STUB
  printf '#!/bin/bash\nexit 0\n' >"$prefix/bin/uv"
  printf '#!/bin/bash\nexit 0\n' >"$prefix/bin/volta"
  chmod +x "$prefix/bin/brew" "$prefix/bin/uv" "$prefix/bin/volta"
}

# Stub herdr, two modes:
#   healthy -> version ok, both plugins registered+enabled+warning-free,
#              session running, config reload applied with no diagnostics
#   broken  -> every invocation fails (the binary does not run)
make_herdr_stub() {
  local dir="$1" mode="$2"
  cat >"$dir/herdr" <<STUB
#!/bin/bash
mode="$mode"
if [[ \$mode == broken ]]; then
  exit 1
fi
if [[ \$1 == --version ]]; then
  echo "herdr 0.7.0-test"
  exit 0
fi
sub="\$1 \$2"
case "\$sub" in
  "plugin list")
    # args: plugin list --plugin <id> --json
    id="\$4"
    printf '{"id":"cli:plugin","result":{"plugins":[{"plugin_id":"%s","enabled":true}],"type":"plugin_list"}}\n' "\$id"
    exit 0 ;;
  "session list")
    printf '{"sessions":[{"default":true,"name":"default","running":true}]}\n'
    exit 0 ;;
  "server reload-config")
    printf '{"id":"cli:server:reload-config","result":{"diagnostics":[],"status":"applied","type":"config_reload"}}\n'
    exit 0 ;;
esac
exit 0
STUB
  chmod +x "$dir/herdr"
}

# run_case <name> <installed-pkgs> <stamp?> <herdr-mode>
run_case() {
  local name="$1" installed="$2" stamp="$3" herdr_mode="$4"
  local case_home="$work/$name/home"
  local prefix="$work/$name/prefix"
  local path_bin="$work/$name/path-bin"
  BUNDLE_RECORD="$work/$name/bundle-argv"
  ERR_FILE="$work/$name/stderr"
  mkdir -p "$case_home/.config/herdr" "$path_bin"
  build_stub_prefix "$prefix"
  make_herdr_stub "$path_bin" "$herdr_mode"
  printf '#!/bin/bash\nexit 0\n' >"$path_bin/npm"
  chmod +x "$path_bin/npm"
  printf 'x = 1\n' >"$case_home/.config/herdr/config.toml"
  if [[ $stamp == stamp ]]; then
    mkdir -p "$case_home/$(dirname "$STAMP_REL")"
    : >"$case_home/$STAMP_REL"
  fi
  RC=0
  HOME="$case_home" HOMEBREW_PREFIX="$prefix" \
    PATH="$path_bin:$PATH" INSTALLED_PKGS="$installed" BUNDLE_RECORD="$BUNDLE_RECORD" \
    bash "$rendered" >/dev/null 2>"$ERR_FILE" || RC=$?
}

cleanup_used() { grep -qw -- '--cleanup' "$BUNDLE_RECORD"; }

# tmux still installed, no stamp -> defer cleanup, and SAY so (the deferral
# warning is part of the contract: silently withholding --cleanup would leave
# the operator with no clue why tmux/sesh survive).
run_case tmux-no-stamp "tmux" no-stamp healthy
[[ $RC -eq 0 ]] || fail "tmux-no-stamp: script errored (rc=$RC; stderr: $(cat "$ERR_FILE"))"
[[ -s $BUNDLE_RECORD ]] || fail "tmux-no-stamp: brew bundle was never invoked"
cleanup_used && fail "tmux-no-stamp: --cleanup passed though tmux is installed and herdr is unverified (would strand the machine)"
grep -q 'WITHOUT --cleanup' "$ERR_FILE" ||
  fail "tmux-no-stamp: the deferral warning was not printed (stderr: $(cat "$ERR_FILE"))"

# sesh still installed, no stamp -> defer cleanup.
run_case sesh-no-stamp "sesh" no-stamp healthy
cleanup_used && fail "sesh-no-stamp: --cleanup passed though sesh is installed and herdr is unverified"

# tmux installed, stamp present, herdr LIVE-healthy -> cleanup proceeds.
run_case tmux-stamp-healthy "tmux" stamp healthy
[[ $RC -eq 0 ]] || fail "tmux-stamp-healthy: script errored (rc=$RC; stderr: $(cat "$ERR_FILE"))"
cleanup_used || fail "tmux-stamp-healthy: --cleanup not passed though the stamp is present and herdr is live-healthy (migration would never complete)"

# tmux installed, stamp present, herdr NOW BROKEN -> the stale stamp must NOT
# authorize the teardown: the live health check is the cleanup authority.
run_case tmux-stamp-stale "tmux" stamp broken
[[ $RC -eq 0 ]] || fail "tmux-stamp-stale: script errored (rc=$RC; stderr: $(cat "$ERR_FILE"))"
cleanup_used && fail "tmux-stamp-stale: --cleanup passed on a STALE stamp though herdr is currently broken (would remove the only working multiplexer)"
grep -qi 'stale' "$ERR_FILE" ||
  fail "tmux-stamp-stale: no stale-stamp warning printed (stderr: $(cat "$ERR_FILE"))"

# No multiplexer installed -> cleanup proceeds regardless of stamp or herdr
# health (nothing to strand; the machine is already migrated). herdr broken in
# both cases proves the gate and the health check are skipped entirely.
run_case none-no-stamp "" no-stamp broken
cleanup_used || fail "none-no-stamp: --cleanup withheld though no multiplexer is installed (nothing to strand)"

run_case none-stamp "" stamp broken
cleanup_used || fail "none-stamp: --cleanup withheld though no multiplexer is installed"

printf 'PASS: before_10 renders the stamp presence into its trigger, defers cleanup while tmux/sesh is installed unless the stamp is present AND herdr passes a live health check, warns on deferral and on a stale stamp, and skips the gate when no multiplexer remains\n'
