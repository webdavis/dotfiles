#!/usr/bin/env bash
# herdr-migration-verify.sh: run_after_58-herdr-migration-verify writes the
# herdr-verified stamp ONLY when herdr is fully proven as a multiplexer, and
# removes it otherwise. The stamp is the TRIGGER signal run_onchange_before_10
# consults (and re-checks live) before it lets `brew bundle --cleanup` remove
# the old multiplexer (tmux/sesh): no stamp, no teardown. The shared predicate
# (.chezmoitemplates/herdr-health-check.sh.tmpl) requires ALL of:
#
#   1. the herdr binary is present and runnable
#   2. its config.toml is present AND passes the live reload validation
#      (`herdr server reload-config` reports status "applied" with an empty
#      diagnostics array; herdr 0.7.0 offers no validate-only subcommand)
#   3. BOTH vendored plugins are registered with the EXACT plugin id, enabled,
#      and warning-free, in an array-shaped plugin list with no error envelope
#      and no duplicate entries
#   4. the session server answers with an array-shaped session list containing
#      at least one running:true entry and no error envelope
#
# Exact-id-only matching is NOT enough: a probe with an invalid config, a
# disabled warning-bearing plugin, and one running session used to earn the
# stamp (herdr keeps disabled plugins registered and lists invalid manifests
# with warnings), so the predicate must reject those unusable installations.
#
# The script never aborts the apply (exit 0 always, even when filesystem
# operations on the stamp fail) and is idempotent. It is exercised by
# rendering the REAL template and running it against a stub herdr on PATH
# with a scratch HOME.
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

# Stub herdr. Modes flip one health signal each to prove the stamp needs ALL
# of them; the healthy shape mirrors the live 0.7.0 CLI (enabled:true, no
# warnings key, id envelope on plugin/reload responses).
#   healthy          -> version ok, both plugins enabled+warning-free,
#                       session running, config reload applied cleanly
#   binary-broken    -> every invocation fails (`herdr --version` included)
#   plugin-missing   -> smart-nav absent from its plugin list
#   plugin-disabled  -> smart-nav registered but enabled:false
#   plugin-warning   -> smart-nav enabled but carrying a manifest warning
#   plugin-wrong-id  -> the list returns a DIFFERENT plugin_id than requested
#   plugin-duplicate -> two entries for the same plugin id
#   plugin-mixed     -> .result.plugins is an object, not an array
#   session-down     -> `session list` fails (server unreachable)
#   session-stopped  -> session list answers but no entry is running:true
#   config-invalid   -> reload-config reports diagnostics (config rejected)
make_herdr_stub() {
  local dir="$1" mode="$2"
  cat >"$dir/herdr" <<STUB
#!/bin/bash
mode="$mode"
if [[ \$mode == binary-broken ]]; then
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
    if [[ \$id == herdr-smart-nav ]]; then
      case "\$mode" in
        plugin-missing)
          printf '{"id":"cli:plugin","result":{"plugins":[],"type":"plugin_list"}}\n'
          exit 0 ;;
        plugin-disabled)
          printf '{"id":"cli:plugin","result":{"plugins":[{"plugin_id":"%s","enabled":false}],"type":"plugin_list"}}\n' "\$id"
          exit 0 ;;
        plugin-warning)
          printf '{"id":"cli:plugin","result":{"plugins":[{"plugin_id":"%s","enabled":true,"warnings":["invalid manifest field: events"]}],"type":"plugin_list"}}\n' "\$id"
          exit 0 ;;
        plugin-wrong-id)
          printf '{"id":"cli:plugin","result":{"plugins":[{"plugin_id":"herdr-imposter","enabled":true}],"type":"plugin_list"}}\n'
          exit 0 ;;
        plugin-duplicate)
          printf '{"id":"cli:plugin","result":{"plugins":[{"plugin_id":"%s","enabled":true},{"plugin_id":"%s","enabled":true}],"type":"plugin_list"}}\n' "\$id" "\$id"
          exit 0 ;;
        plugin-mixed)
          printf '{"id":"cli:plugin","result":{"plugins":{"plugin_id":"%s","enabled":true},"type":"plugin_list"}}\n' "\$id"
          exit 0 ;;
      esac
    fi
    printf '{"id":"cli:plugin","result":{"plugins":[{"plugin_id":"%s","enabled":true}],"type":"plugin_list"}}\n' "\$id"
    exit 0 ;;
  "session list")
    if [[ \$mode == session-down ]]; then
      exit 3
    fi
    if [[ \$mode == session-stopped ]]; then
      printf '{"sessions":[{"default":true,"name":"default","running":false}]}\n'
      exit 0
    fi
    printf '{"sessions":[{"default":true,"name":"default","running":true}]}\n'
    exit 0 ;;
  "server reload-config")
    if [[ \$mode == config-invalid ]]; then
      printf '{"id":"cli:server:reload-config","result":{"diagnostics":["unknown key: keybindingz"],"status":"rejected","type":"config_reload"}}\n'
      exit 0
    fi
    printf '{"id":"cli:server:reload-config","result":{"diagnostics":[],"status":"applied","type":"config_reload"}}\n'
    exit 0 ;;
esac
exit 0
STUB
  chmod +x "$dir/herdr"
}

# run_case <name> <mode> <config?> <pre-stamp?> [fs-sabotage]
#   fs-sabotage: dir-at-stamp     -> a DIRECTORY occupies the stamp path
#                unwritable-cache -> ~/.cache is read-only and the stamp
#                                    subdir does not exist (mkdir -p fails)
run_case() {
  local name="$1" mode="$2" config="$3" pre_stamp="$4" sabotage="${5:-none}"
  CASE_HOME="$work/$name/home"
  local bin="$work/$name/bin"
  STAMP="$CASE_HOME/$STAMP_REL"
  ERR_FILE="$work/$name/stderr"
  mkdir -p "$bin" "$CASE_HOME/.config/herdr"
  make_herdr_stub "$bin" "$mode"
  [[ $config == config ]] && printf 'x = 1\n' >"$CASE_HOME/.config/herdr/config.toml"
  if [[ $pre_stamp == pre-stamp ]]; then
    mkdir -p "$(dirname "$STAMP")"
    : >"$STAMP"
  fi
  case "$sabotage" in
    dir-at-stamp)
      mkdir -p "$STAMP"
      ;;
    unwritable-cache)
      mkdir -p "$CASE_HOME/.cache"
      chmod 555 "$CASE_HOME/.cache"
      ;;
    none) ;;
    *) fail "unknown sabotage mode: $sabotage" ;;
  esac
  RC=0
  HOME="$CASE_HOME" PATH="$bin:$PATH" bash "$rendered" >/dev/null 2>"$ERR_FILE" || RC=$?
  # Re-open a sabotaged read-only dir so the EXIT-trap cleanup can remove it.
  if [[ $sabotage == unwritable-cache ]]; then
    chmod 755 "$CASE_HOME/.cache"
  fi
}

stamp_present() { [[ -f $STAMP ]]; }

# Healthy: stamp written, exit 0.
run_case healthy healthy config no-stamp
[[ $RC -eq 0 ]] || fail "healthy: expected exit 0, got $RC ($(cat "$ERR_FILE"))"
stamp_present || fail "healthy: verified stamp was not written despite herdr fully healthy"

# Idempotent: a second run on a healthy host keeps the stamp, still exit 0.
run_case healthy-again healthy config pre-stamp
[[ $RC -eq 0 ]] || fail "healthy-again: expected exit 0, got $RC"
stamp_present || fail "healthy-again: stamp removed on a healthy second run (not idempotent)"

# One plugin missing: NOT verified; a stale stamp is removed.
run_case plugin-missing plugin-missing config pre-stamp
[[ $RC -eq 0 ]] || fail "plugin-missing: expected exit 0, got $RC"
stamp_present && fail "plugin-missing: stamp present though a plugin is not registered"

# Plugin registered but DISABLED: an unusable installation must not stamp.
run_case plugin-disabled plugin-disabled config pre-stamp
[[ $RC -eq 0 ]] || fail "plugin-disabled: expected exit 0, got $RC"
stamp_present && fail "plugin-disabled: stamp present though a required plugin is disabled"

# Plugin enabled but carrying a manifest WARNING: not proven usable.
run_case plugin-warning plugin-warning config pre-stamp
[[ $RC -eq 0 ]] || fail "plugin-warning: expected exit 0, got $RC"
stamp_present && fail "plugin-warning: stamp present though a required plugin carries warnings"

# The list answers with a DIFFERENT plugin id: exact-id equality must reject
# it (kills any relaxation of the id match to a substring or any-entry check).
run_case plugin-wrong-id plugin-wrong-id config pre-stamp
[[ $RC -eq 0 ]] || fail "plugin-wrong-id: expected exit 0, got $RC"
stamp_present && fail "plugin-wrong-id: stamp present though the list returned a different plugin id"

# Duplicate entries for one id: exactly-one is the contract.
run_case plugin-duplicate plugin-duplicate config pre-stamp
[[ $RC -eq 0 ]] || fail "plugin-duplicate: expected exit 0, got $RC"
stamp_present && fail "plugin-duplicate: stamp present though the plugin list carries duplicate entries"

# Object-shaped (mixed) plugins container: only the CLI's real array shape
# counts; anything else is malformed and must not verify.
run_case plugin-mixed plugin-mixed config pre-stamp
[[ $RC -eq 0 ]] || fail "plugin-mixed: expected exit 0, got $RC"
stamp_present && fail "plugin-mixed: stamp present though the plugins container is not an array"

# Session server down: NOT verified.
run_case session-down session-down config pre-stamp
[[ $RC -eq 0 ]] || fail "session-down: expected exit 0, got $RC"
stamp_present && fail "session-down: stamp present though the session server is unreachable"

# Session list answers but nothing is running: NOT verified.
run_case session-stopped session-stopped config pre-stamp
[[ $RC -eq 0 ]] || fail "session-stopped: expected exit 0, got $RC"
stamp_present && fail "session-stopped: stamp present though no session is running"

# Config missing: NOT verified.
run_case no-config healthy no-config pre-stamp
[[ $RC -eq 0 ]] || fail "no-config: expected exit 0, got $RC"
stamp_present && fail "no-config: stamp present though config.toml is absent"

# Config present but REJECTED by the live reload validation: NOT verified.
run_case config-invalid config-invalid config pre-stamp
[[ $RC -eq 0 ]] || fail "config-invalid: expected exit 0, got $RC"
stamp_present && fail "config-invalid: stamp present though herdr rejected the config with diagnostics"

# Binary broken: NOT verified.
run_case binary-broken binary-broken config pre-stamp
[[ $RC -eq 0 ]] || fail "binary-broken: expected exit 0, got $RC"
stamp_present && fail "binary-broken: stamp present though the herdr binary does not run"

# --- never-abort guarantees (the apply must survive filesystem sabotage) ----

# A DIRECTORY occupies the stamp path, herdr healthy: the truncate fails; the
# script must warn and exit 0, not abort the apply.
run_case dir-at-stamp-healthy healthy config no-stamp dir-at-stamp
[[ $RC -eq 0 ]] || fail "dir-at-stamp-healthy: a directory at the stamp path aborted the apply (rc=$RC; stderr: $(cat "$ERR_FILE"))"
[[ -s $ERR_FILE ]] || fail "dir-at-stamp-healthy: no warning printed about the unwritable stamp"

# A DIRECTORY occupies the stamp path, herdr broken: the rm fails; the script
# must warn and exit 0.
run_case dir-at-stamp-broken binary-broken config no-stamp dir-at-stamp
[[ $RC -eq 0 ]] || fail "dir-at-stamp-broken: a directory at the stamp path aborted the apply on the not-verified path (rc=$RC; stderr: $(cat "$ERR_FILE"))"
[[ -s $ERR_FILE ]] || fail "dir-at-stamp-broken: no warning printed about the unremovable stamp path"

# ~/.cache is read-only and the stamp subdir is missing, herdr healthy: the
# mkdir fails; the script must warn and exit 0.
run_case unwritable-cache healthy config no-stamp unwritable-cache
[[ $RC -eq 0 ]] || fail "unwritable-cache: an unwritable cache dir aborted the apply (rc=$RC; stderr: $(cat "$ERR_FILE"))"
[[ -s $ERR_FILE ]] || fail "unwritable-cache: no warning printed about the unwritable cache dir"

printf 'PASS: run_after_58 stamps only a fully healthy herdr (binary, validated config, both plugins enabled+warning-free by exact id in an array-shaped list, running session), rejects disabled/warning/wrong-id/duplicate/mixed/stopped/invalid-config states, and never aborts the apply even under filesystem sabotage\n'
