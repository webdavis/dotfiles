#!/usr/bin/env bash
# herdr-migration-verify.sh — run_after_58-herdr-migration-verify writes the
# herdr-verified stamp ONLY when herdr is fully proven as a multiplexer, and
# removes it otherwise. The stamp is the signal run_onchange_before_10 consults
# before it lets `brew bundle --cleanup` remove the old multiplexer (tmux/sesh):
# no stamp, no teardown. Verification requires ALL of:
#
#   1. the herdr binary is present and runnable
#   2. its config.toml is present
#   3. BOTH vendored plugins are registered (exact plugin id)
#   4. the herdr session server is reachable and reports a running session
#      (the strongest non-interactive proof a second session can be hosted — a
#      full spawn+attach needs a TTY the apply does not have)
#
# The script never aborts the apply (exit 0 always) and is idempotent. It is
# exercised by rendering the REAL template and running it against stub herdr on
# PATH with a scratch HOME.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_after_58-herdr-migration-verify.sh.tmpl"
STAMP_REL=".cache/herdr-migration/verified"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

for tool in chezmoi jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'SKIP: %s not on PATH; cannot render/run the verify script\n' "$tool"
    exit 0
  fi
done
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

rendered="$work/rendered.sh"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$SCRIPT" >"$rendered" || fail "chezmoi failed to render $SCRIPT"
rm -rf "$render_home"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# Stub herdr. Modes flip one health signal each to prove the stamp needs ALL of
# them. `plugins` lists which plugin ids `plugin list --plugin` reports.
#   healthy       -> version ok, both plugins, session running
#   plugin-missing-> only herdr-last-workspace reports (smart-nav absent)
#   session-down  -> `session list` fails (server unreachable)
#   binary-broken -> `herdr --version` fails
make_herdr_stub() {
  local dir="$1" mode="$2"
  cat >"$dir/herdr" <<STUB
#!/bin/bash
mode="$mode"
sub="\$1 \$2"
case "\$sub" in
  "--version ")
    [[ \$mode == binary-broken ]] && exit 1
    echo "herdr 0.7.0-preview.test"
    exit 0 ;;
esac
if [[ \$1 == --version ]]; then
  [[ \$mode == binary-broken ]] && exit 1
  echo "herdr 0.7.0-preview.test"; exit 0
fi
case "\$sub" in
  "session list")
    [[ \$mode == session-down ]] && exit 3
    printf '{"sessions":[{"default":true,"name":"default","running":true}]}\n'
    exit 0 ;;
  "plugin list")
    # args: plugin list --plugin <id> --json
    id="\$4"
    if [[ \$mode == plugin-missing && \$id == herdr-smart-nav ]]; then
      printf '{"result":{"plugins":[],"type":"plugin_list"}}\n'; exit 0
    fi
    printf '{"result":{"plugins":[{"plugin_id":"%s"}],"type":"plugin_list"}}\n' "\$id"
    exit 0 ;;
esac
exit 0
STUB
  chmod +x "$dir/herdr"
}

# run_case <name> <mode> <config?> <pre-stamp?>
run_case() {
  local name="$1" mode="$2" config="$3" pre_stamp="$4"
  CASE_HOME="$work/$name/home"
  local bin="$work/$name/bin"
  STAMP="$CASE_HOME/$STAMP_REL"
  mkdir -p "$bin" "$CASE_HOME/.config/herdr"
  make_herdr_stub "$bin" "$mode"
  [[ $config == config ]] && printf 'x = 1\n' >"$CASE_HOME/.config/herdr/config.toml"
  if [[ $pre_stamp == pre-stamp ]]; then
    mkdir -p "$(dirname "$STAMP")"
    : >"$STAMP"
  fi
  RC=0
  HOME="$CASE_HOME" PATH="$bin:$PATH" bash "$rendered" >/dev/null 2>&1 || RC=$?
}

stamp_present() { [[ -f $STAMP ]]; }

# Healthy: stamp written, exit 0.
run_case healthy healthy config no-stamp
[[ $RC -eq 0 ]] || fail "healthy: expected exit 0, got $RC"
stamp_present || fail "healthy: verified stamp was not written despite herdr fully healthy"

# Idempotent: a second run on a healthy host keeps the stamp, still exit 0.
run_case healthy-again healthy config pre-stamp
[[ $RC -eq 0 ]] || fail "healthy-again: expected exit 0, got $RC"
stamp_present || fail "healthy-again: stamp removed on a healthy second run (not idempotent)"

# One plugin missing: NOT verified — stamp absent, and a stale stamp is removed.
run_case plugin-missing plugin-missing config pre-stamp
[[ $RC -eq 0 ]] || fail "plugin-missing: expected exit 0, got $RC"
stamp_present && fail "plugin-missing: stamp present though a plugin is not registered"

# Session server down: NOT verified.
run_case session-down session-down config pre-stamp
[[ $RC -eq 0 ]] || fail "session-down: expected exit 0, got $RC"
stamp_present && fail "session-down: stamp present though the session server is unreachable"

# Config missing: NOT verified.
run_case no-config healthy no-config pre-stamp
[[ $RC -eq 0 ]] || fail "no-config: expected exit 0, got $RC"
stamp_present && fail "no-config: stamp present though config.toml is absent"

# Binary broken: NOT verified.
run_case binary-broken binary-broken config pre-stamp
[[ $RC -eq 0 ]] || fail "binary-broken: expected exit 0, got $RC"
stamp_present && fail "binary-broken: stamp present though the herdr binary does not run"

printf 'PASS: run_after_58 writes the herdr-verified stamp only when binary, config, both plugins, and the session server all check out, removes it otherwise, and never aborts the apply\n'
