#!/usr/bin/env bash
# herdr-migration-verify.sh: run_after_58-herdr-migration-verify runs
# post-target-update and owns the deferred tmux/sesh teardown that before_10
# withholds. Its state machine:
#
#   1. SHORT-CIRCUIT: if neither tmux nor sesh is installed there is nothing to
#      migrate. Make NO herdr contact at all (this also removes the routine
#      reload-config side effect on already-migrated machines) and exit 0.
#   2. Otherwise run the shared health check
#      (.chezmoitemplates/herdr-health-check.sh.tmpl) against the CURRENT
#      (just-installed) herdr. It requires ALL of: the binary runs; the session
#      server answers with a running session; config.toml passes the live
#      reload validation; both vendored plugins are registered by EXACT id,
#      enabled, and warning-free in an array-shaped list.
#   3. On full verification, perform the deferred cleanup itself: regenerate the
#      Brewfile from the same package data before_10 uses and run
#      `brew bundle cleanup --force` with it (the removal before_10 withheld).
#   4. On ANY verification failure, warn loudly (naming the interactive
#      activation step) and exit 0; the migration retries naturally next apply.
#
# The script never aborts the apply (exit 0 always, even when the cleanup
# fails) and holds no stamp -- it re-derives everything from live state each
# apply. It is exercised by rendering the REAL template and running it against a
# stub herdr and a stub brew on PATH with a scratch HOME.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_after_58-herdr-migration-verify.sh.tmpl"

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

# Stub herdr. Modes flip one health signal each to prove cleanup needs ALL of
# them; the healthy shape mirrors the live 0.7.0 CLI (enabled:true, no warnings
# key, id envelope on plugin/reload responses). Every invocation appends its
# argv to $HERDR_RECORD so a test can prove herdr was (or was not) contacted.
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
#   config-rejected-empty-diag -> reload-config reports status "rejected" with
#                       an EMPTY diagnostics array (decouples the status check
#                       from the diagnostics check; see the mutation case below)
make_herdr_stub() {
  local dir="$1" mode="$2"
  cat >"$dir/herdr" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"\$HERDR_RECORD"
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
    if [[ \$mode == config-rejected-empty-diag ]]; then
      printf '{"id":"cli:server:reload-config","result":{"diagnostics":[],"status":"rejected","type":"config_reload"}}\n'
      exit 0
    fi
    printf '{"id":"cli:server:reload-config","result":{"diagnostics":[],"status":"applied","type":"config_reload"}}\n'
    exit 0 ;;
esac
exit 0
STUB
  chmod +x "$dir/herdr"
}

# Stub brew: `list <pkg>` reports installed only for names in $INSTALLED_PKGS;
# `bundle` records its full argv and exits per $BREW_BUNDLE_RC (so a cleanup
# failure can be simulated). after_58 passes the Brewfile via --file, not stdin,
# so the stub must NOT read stdin (doing so would block on an unclosed fd).
make_brew_stub() {
  local dir="$1"
  cat >"$dir/brew" <<'STUB'
#!/bin/bash
case "$1" in
  list)
    pkg="$2"
    read -ra installed_packages <<<"$INSTALLED_PKGS"
    for installed in "${installed_packages[@]}"; do
      [[ $pkg == "$installed" ]] && exit 0
    done
    exit 1 ;;
  bundle)
    printf '%s\n' "$*" >>"$BUNDLE_RECORD"
    exit "${BREW_BUNDLE_RC:-0}" ;;
esac
exit 0
STUB
  chmod +x "$dir/brew"
}

# run_case <name> <herdr-mode> <installed-pkgs> <config?> [brew-bundle-rc]
run_case() {
  local name="$1" mode="$2" installed="$3" config="$4" bundle_rc="${5:-0}"
  CASE_HOME="$work/$name/home"
  local prefix="$work/$name/prefix"
  local path_bin="$work/$name/path-bin"
  HERDR_RECORD="$work/$name/herdr-argv"
  BUNDLE_RECORD="$work/$name/bundle-argv"
  ERR_FILE="$work/$name/stderr"
  OUT_FILE="$work/$name/stdout"
  mkdir -p "$prefix/bin" "$path_bin" "$CASE_HOME/.config/herdr"
  make_brew_stub "$prefix/bin"
  make_herdr_stub "$path_bin" "$mode"
  : >"$HERDR_RECORD"
  [[ $config == config ]] && printf 'x = 1\n' >"$CASE_HOME/.config/herdr/config.toml"
  RC=0
  HOME="$CASE_HOME" HOMEBREW_PREFIX="$prefix" PATH="$path_bin:$PATH" \
    INSTALLED_PKGS="$installed" HERDR_RECORD="$HERDR_RECORD" \
    BUNDLE_RECORD="$BUNDLE_RECORD" BREW_BUNDLE_RC="$bundle_rc" \
    bash "$rendered" >"$OUT_FILE" 2>"$ERR_FILE" || RC=$?
}

cleanup_ran() { [[ -f $BUNDLE_RECORD ]] && grep -q 'bundle cleanup --force' "$BUNDLE_RECORD"; }
herdr_contacted() { [[ -s $HERDR_RECORD ]]; }

# --- short-circuit: fresh/already-migrated machine, no multiplexer -----------
# No tmux/sesh installed: NO herdr contact, no cleanup, exit 0. herdr is broken
# to prove it is never invoked (the short-circuit precedes any health check).
run_case fresh-no-op binary-broken "wget" config
[[ $RC -eq 0 ]] || fail "fresh-no-op: expected exit 0, got $RC ($(cat "$ERR_FILE"))"
herdr_contacted && fail "fresh-no-op: herdr was contacted though no multiplexer is installed (short-circuit skipped)"
cleanup_ran && fail "fresh-no-op: brew bundle cleanup ran though nothing needs migrating"

# --- migration host, happy path ----------------------------------------------
# tmux installed + herdr fully healthy: verify, then run brew bundle cleanup
# --force to remove tmux/sesh. The success message is printed.
run_case migrate-happy healthy "tmux" config
[[ $RC -eq 0 ]] || fail "migrate-happy: expected exit 0, got $RC ($(cat "$ERR_FILE"))"
herdr_contacted || fail "migrate-happy: herdr was never contacted though tmux is installed"
cleanup_ran || fail "migrate-happy: brew bundle cleanup --force did not run though herdr is verified (migration would never complete)"
grep -q 'migration complete' "$OUT_FILE" ||
  fail "migrate-happy: no migration-complete message after a verified cleanup ($(cat "$OUT_FILE"))"

# --- serverless host: defers forever without ever cleaning -------------------
# tmux installed, herdr present but its session server is down (no interactive
# terminal ever started it): NOT verified, no cleanup, and the warning must name
# the interactive activation step. Exit 0 (retries next apply).
run_case serverless session-down "tmux" config
[[ $RC -eq 0 ]] || fail "serverless: expected exit 0, got $RC"
herdr_contacted || fail "serverless: herdr was never contacted though tmux is installed"
cleanup_ran && fail "serverless: brew bundle cleanup ran though the herdr server is down (would remove the only multiplexer)"
grep -qi 'interactive terminal' "$ERR_FILE" ||
  fail "serverless: the deferral warning does not name the interactive activation step ($(cat "$ERR_FILE"))"

# --- predicate rejection cases: multiplexer present, but herdr unhealthy ------
# Each unhealthy mode must block the cleanup. Cleanup running under any of these
# would remove the only working multiplexer against an unusable herdr.
for mode in binary-broken plugin-missing plugin-disabled plugin-warning \
  plugin-wrong-id plugin-duplicate plugin-mixed session-stopped config-invalid; do
  run_case "reject-$mode" "$mode" "tmux" config
  [[ $RC -eq 0 ]] || fail "reject-$mode: expected exit 0, got $RC ($(cat "$ERR_FILE"))"
  cleanup_ran && fail "reject-$mode: brew bundle cleanup ran though herdr is unhealthy ($mode)"
done

# config.toml absent, herdr otherwise healthy: NOT verified, no cleanup.
run_case reject-no-config healthy "tmux" no-config
[[ $RC -eq 0 ]] || fail "reject-no-config: expected exit 0, got $RC"
cleanup_ran && fail "reject-no-config: brew bundle cleanup ran though config.toml is absent"

# Reload reports status "rejected" but with an EMPTY diagnostics array. The
# config-invalid stub above couples a non-applied status with NON-empty
# diagnostics, so the diagnostics-length check alone would still reject it and
# a deleted `.result.status == "applied"` mutant would survive. This case
# decouples the two: with the status check deleted, empty diagnostics would
# read as a pass and cleanup would run against a REJECTED config. The predicate
# must still reject it (no cleanup), which kills that mutant.
run_case reject-rejected-empty-diag config-rejected-empty-diag "tmux" config
[[ $RC -eq 0 ]] || fail "reject-rejected-empty-diag: expected exit 0, got $RC"
cleanup_ran && fail "reject-rejected-empty-diag: brew bundle cleanup ran though reload-config reported status 'rejected' (only the empty diagnostics happened to match; the status check must still reject it)"

# --- never-abort: cleanup itself fails ---------------------------------------
# herdr verified but `brew bundle cleanup` exits non-zero: warn and exit 0, do
# NOT abort the apply. tmux/sesh may remain; the next apply retries.
run_case cleanup-fails healthy "tmux" config 5
[[ $RC -eq 0 ]] || fail "cleanup-fails: a failing brew bundle cleanup aborted the apply (rc=$RC; stderr: $(cat "$ERR_FILE"))"
cleanup_ran || fail "cleanup-fails: brew bundle cleanup was expected to be attempted"
[[ -s $ERR_FILE ]] || fail "cleanup-fails: no warning printed about the failed cleanup"

printf 'PASS: after_58 short-circuits with no herdr contact when no multiplexer is installed, verifies the current herdr (binary, running session, validated config, both plugins enabled+warning-free by exact id) before running brew bundle cleanup --force, defers with an activation-step warning on any unhealthy state or a serverless host, and never aborts the apply even when the cleanup fails\n'
