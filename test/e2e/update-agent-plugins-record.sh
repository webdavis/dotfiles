#!/usr/bin/env bash
#
# update-agent-plugins.sh: the whole weekly flow, with the `claude` CLI and
# relay.sh stubbed at the boundary. No real plugin is installed, updated,
# enabled or disabled, and ~/.claude is never read: HOME is a sandbox for every
# run in this file.
#
# WHAT THIS PINS, and why each one is here rather than assumed:
#
#   - A DISABLED plugin is skipped, not updated. `claude plugin update` on a
#     disabled plugin proceeds (exit 0) and overwrites the tree in place, which
#     destroys the exact artifact the operator contained. Since `disable` is the
#     only recovery verb this vertical has, an updater that walks past it would
#     take the recovery away.
#   - The UNKNOWABLE identity lane is reported as refreshed with the change
#     unknowable, and NEVER as changed. Those plugins declare the literal
#     version "unknown" and their lastUpdated bumps on a no-op refresh, so a
#     change line built from either would cry change every single week.
#   - A run that could not read the plugin inventory ATTEMPTS NOTHING and says
#     so. The inventory is where installed-ness and enabled-ness come from, so
#     without it the disabled-skip above cannot be honoured; guessing would put
#     the contained plugin back.
#   - The recurring defect this whole family exists to prevent: an artifact that
#     reports success for work that did not happen. Every path that emits an
#     entry is asked what it says when the run did nothing, failed, or could not
#     be attempted.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/bin/executable_update-agent-plugins.sh"

fail() {
  printf 'update-agent-plugins-record: FAIL -- %s\n' "$*" >&2
  exit 1
}

# An explicit refute, never `! grep`: an inverted status is invisible to set -e
# and the assertion then passes for every input forever.
refute() {
  local pattern="$1" haystack="$2" message="$3"
  if grep -qE "$pattern" <<<"$haystack"; then
    printf '=== haystack ===\n%s\n' "$haystack" >&2
    fail "$message"
  fi
}

[[ -f $HELPER ]] || fail "helper not found: $HELPER"

command -v jq >/dev/null 2>&1 || fail "jq is not on PATH; the claude stub needs it"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.local/bin" "$HOME/.agents"

RELAY_LOG="$tmp/relay.log"
export RELAY_LOG
: >"$RELAY_LOG"
# One line per call carrying the route, so an entry's multi-line body stays
# greppable and the ALERT route is distinguishable from the LOG route.
cat >"$HOME/.local/bin/relay.sh" <<'STUB'
#!/usr/bin/env bash
if { : >&9; } 2>/dev/null; then fd9=inherited; else fd9=closed; fi
printf 'CALL url=%s fd9=%s ARGV %s\n' "${RELAY_HERMES_URL:-<default>}" "$fd9" "$(printf '%s ' "$@" | tr '\n' ' ')" >>"$RELAY_LOG"
printf '%s\n' "${RELAY_STUB_OUTCOME:-relay: posted HTTP 200}"
exit 0
STUB
chmod +x "$HOME/.local/bin/relay.sh"

export UNATTENDED_LOG_HERMES_URL="http://hermes.test/webhooks/unattended-upgrades"

# ── The claude stub ────────────────────────────────────────────────────────
# Two JSON state files stand in for ~/.claude/plugins: PLUGIN_STATE is what
# `plugin list --json` answers, MARKETPLACE_STATE is what
# `plugin marketplace list --json` answers. Every invocation is appended to
# CLAUDE_CALL_LOG, which is how "never updated the disabled one" is asserted as
# an absence of a CALL rather than as an absence of a log line.
STUBS="$tmp/stubs"
mkdir -p "$STUBS"
cat >"$STUBS/claude" <<'STUB'
#!/usr/bin/env bash
# The serialize lock lives on fd 9. A claude child that inherits it would keep a
# kernel flock held after the run exited (a detached descendant outliving the
# CLI), deferring every later slot over a competing run that does not exist. Only
# log the inheritance, so a fixed run leaves no FD9-INHERITED line at all.
{ : >&9; } 2>/dev/null && printf 'FD9-INHERITED %s\n' "$1 $2" >>"$CLAUDE_CALL_LOG"
printf '%s\n' "$*" >>"$CLAUDE_CALL_LOG"
case "$1 $2" in
  "plugin list")
    [[ -n ${CLAUDE_LIST_FAIL:-} ]] && {
      printf 'Error: could not read installed plugins\n' >&2
      exit 1
    }
    [[ -n ${CLAUDE_LIST_GARBAGE:-} ]] && {
      printf 'not json at all\n'
      exit 0
    }
    # A JSON array of OBJECTS that carry NO usable id: it passes the
    # array-of-objects check yet every id/scope extraction comes back empty, so
    # the whole roster looks absent (before) or removed (after). Distinct from
    # CLAUDE_LIST_SHAPE (non-objects) because F18's object check does not catch it.
    [[ -n ${CLAUDE_LIST_OBJGARBAGE:-} ]] && {
      printf '[{"foo":"bar"},{"baz":1}]\n'
      exit 0
    }
    # A JSON array of NON-OBJECTS on the FIRST reading only: it passes the
    # updater's "is it an array" check and then breaks any .id lookup, so the
    # BEFORE snapshot fails while the AFTER one succeeds.
    if [[ -n ${CLAUDE_LIST_SHAPE_ONCE:-} && ! -e ${CLAUDE_LIST_SHAPE_MARKER:-/nonexistent} ]]; then
      : >"$CLAUDE_LIST_SHAPE_MARKER"
      printf '[1, 2]\n'
      exit 0
    fi
    cat "$PLUGIN_STATE"
    exit 0
    ;;
  "plugin marketplace")
    case "$3" in
      list)
        # Fail the marketplace-list read after the Nth successful one, so the
        # configured check (call 1) succeeds while the source-verify read (call 2)
        # fails or times out. This is how F37's fail-open-on-a-failed-verify-read
        # is reproduced: the first read proves the marketplace exists, the second
        # cannot prove its repo.
        if [[ -n ${CLAUDE_MARKETPLACE_LIST_FAIL_AFTER:-} ]]; then
          n=$(($(cat "$MKT_LIST_COUNT" 2>/dev/null || printf 0) + 1))
          printf '%s' "$n" >"$MKT_LIST_COUNT"
          if [[ $n -gt $CLAUDE_MARKETPLACE_LIST_FAIL_AFTER ]]; then
            printf 'Error: marketplace list timed out\n' >&2
            exit 1
          fi
        fi
        cat "$MARKETPLACE_STATE"
        exit 0
        ;;
      add)
        for bad in ${CLAUDE_FAIL_MARKETPLACE_ADD:-}; do
          [[ $4 == "$bad" ]] && {
            printf 'Error: cannot reach %s\n' "$4" >&2
            exit 1
          }
        done
        jq --arg r "$4" '. + [{name: ($r | split("/")[1]), source: "github", repo: $r}]' \
          "$MARKETPLACE_STATE" >"$MARKETPLACE_STATE.new" && mv "$MARKETPLACE_STATE.new" "$MARKETPLACE_STATE"
        printf 'Added marketplace %s\n' "$4"
        exit 0
        ;;
    esac
    ;;
  "plugin update")
    for hang in ${CLAUDE_UPDATE_HANG:-}; do
      [[ $3 == "$hang" ]] && {
        sleep "${CLAUDE_HANG_SECS:-10}"
        exit 0
      }
    done
    for bad in ${CLAUDE_FAIL_UPDATE:-}; do
      [[ $3 == "$bad" ]] && {
        printf 'Failed to update plugin "%s": Plugin not found\n' "$3" >&2
        exit 1
      }
    done
    for pair in ${CLAUDE_BUMP:-}; do
      if [[ $3 == "${pair%%=*}" ]]; then
        jq --arg id "$3" --arg v "${pair#*=}" \
          'map(if .id == $id then .version = $v else . end)' \
          "$PLUGIN_STATE" >"$PLUGIN_STATE.new" && mv "$PLUGIN_STATE.new" "$PLUGIN_STATE"
      fi
    done
    printf 'Plugin "%s" is already at the latest version\n' "$3"
    exit 0
    ;;
  "plugin install")
    for bad in ${CLAUDE_FAIL_INSTALL:-}; do
      [[ $3 == "$bad" ]] && {
        printf 'Failed to install plugin "%s"\n' "$3" >&2
        exit 1
      }
    done
    jq --arg id "$3" --arg v "${CLAUDE_INSTALL_VERSION:-9.9.9}" \
      '. + [{id: $id, version: $v, scope: "user", enabled: true, installPath: "/dev/null", lastUpdated: "2026-08-03T00:00:00.000Z"}]' \
      "$PLUGIN_STATE" >"$PLUGIN_STATE.new" && mv "$PLUGIN_STATE.new" "$PLUGIN_STATE"
    printf 'Installed plugin %s\n' "$3"
    exit 0
    ;;
esac
printf 'claude stub: unhandled: %s\n' "$*" >&2
exit 64
STUB
chmod +x "$STUBS/claude"

PLUGIN_STATE="$tmp/plugins.json"
MARKETPLACE_STATE="$tmp/marketplaces.json"
CLAUDE_CALL_LOG="$tmp/claude-calls.log"
MKT_LIST_COUNT="$tmp/mkt-list-count"
export PLUGIN_STATE MARKETPLACE_STATE CLAUDE_CALL_LOG MKT_LIST_COUNT

LOCK="$HOME/.agents/custom-agent-plugins-lock.json"
# A FIXTURE lock, not the committed one: this file is about the updater's
# behaviour, and pinning it to the live roster would make it fail every time a
# plugin is added or dropped.
write_lock() {
  cat >"$LOCK" <<'EOF'
{
  "version": 1,
  "marketplaces": {
    "mkt-a": { "source": "github", "repo": "owner/mkt-a" },
    "mkt-b": { "source": "github", "repo": "owner/mkt-b" }
  },
  "plugins": {
    "steady@mkt-a": { "marketplace": "mkt-a", "harnesses": ["claude-code"], "identityLane": "versioned" },
    "mover@mkt-a": { "marketplace": "mkt-a", "harnesses": ["claude-code"], "identityLane": "versioned" },
    "sha@mkt-a": { "marketplace": "mkt-a", "harnesses": ["claude-code"], "identityLane": "git-sha" },
    "opaque@mkt-a": { "marketplace": "mkt-a", "harnesses": ["claude-code"], "identityLane": "unknowable" },
    "contained@mkt-a": { "marketplace": "mkt-a", "harnesses": ["claude-code"], "identityLane": "versioned" },
    "containedopaque@mkt-a": { "marketplace": "mkt-a", "harnesses": ["claude-code"], "identityLane": "unknowable" },
    "absent@mkt-b": { "marketplace": "mkt-b", "harnesses": ["claude-code"], "identityLane": "versioned" }
  }
}
EOF
}

# The live state the stub answers with: five of the six tracked plugins
# installed, one of those DISABLED (the contained one), and one tracked plugin
# absent entirely so the install path runs. mkt-b is deliberately not configured,
# so the absent plugin cannot be installed until its marketplace is added.
reset_state() {
  write_lock
  cat >"$PLUGIN_STATE" <<'EOF'
[
  {"id":"steady@mkt-a","version":"1.0.0","scope":"user","enabled":true,"installPath":"/dev/null","lastUpdated":"2026-07-01T00:00:00.000Z"},
  {"id":"mover@mkt-a","version":"1.0.0","scope":"user","enabled":true,"installPath":"/dev/null","lastUpdated":"2026-07-01T00:00:00.000Z"},
  {"id":"sha@mkt-a","version":"abc123def456","scope":"user","enabled":true,"installPath":"/dev/null","lastUpdated":"2026-07-01T00:00:00.000Z"},
  {"id":"opaque@mkt-a","version":"unknown","scope":"user","enabled":true,"installPath":"/dev/null","lastUpdated":"2026-07-01T00:00:00.000Z"},
  {"id":"contained@mkt-a","version":"1.0.0","scope":"user","enabled":false,"installPath":"/dev/null","lastUpdated":"2026-07-01T00:00:00.000Z"},
  {"id":"containedopaque@mkt-a","version":"unknown","scope":"user","enabled":false,"installPath":"/dev/null","lastUpdated":"2026-07-01T00:00:00.000Z"},
  {"id":"untracked@mkt-a","version":"5.0.0","scope":"user","enabled":true,"installPath":"/dev/null","lastUpdated":"2026-07-01T00:00:00.000Z"}
]
EOF
  printf '[{"name":"mkt-a","source":"github","repo":"owner/mkt-a","installLocation":"/dev/null"}]\n' >"$MARKETPLACE_STATE"
  # ~/.claude/settings.json is the USER-scope containment source of truth: a plugin
  # is updated only if enabledPlugins[id] is true. The two contained fixtures are
  # false here; the rest are enabled. This is where disabled-ness is read from, NOT
  # the inventory's effective `enabled` (which is cwd-dependent).
  mkdir -p "$HOME/.claude"
  cat >"$HOME/.claude/settings.json" <<'EOF'
{
  "enabledPlugins": {
    "steady@mkt-a": true,
    "mover@mkt-a": true,
    "sha@mkt-a": true,
    "opaque@mkt-a": true,
    "contained@mkt-a": false,
    "containedopaque@mkt-a": false,
    "absent@mkt-b": true
  }
}
EOF
  : >"$CLAUDE_CALL_LOG"
  rm -rf "$HOME/.local/state"
  : >"$RELAY_LOG"
}

STATE_DIR="$HOME/.local/state/update-agent-plugins"
MARKER="$STATE_DIR/last-success-at"

RUN_OUTPUT=""
RUN_RC=0
lock_seq=0
# run_helper [args...] -- a fresh lock file per run so the serialize lock never
# self-contends across scenarios. UPDATE_AGENT_PLUGINS_FORCE bypasses the idle
# gate for every case that is not about the idle gate.
run_helper() {
  lock_seq=$((lock_seq + 1))
  : >"$RELAY_LOG"
  rm -f "$MKT_LIST_COUNT"
  RUN_OUTPUT="$(PATH="$STUBS:$PATH" \
    CLAUDE_MARKETPLACE_LIST_FAIL_AFTER="${CLAUDE_MARKETPLACE_LIST_FAIL_AFTER:-}" \
    UPDATE_AGENT_PLUGINS_FORCE="${UPDATE_AGENT_PLUGINS_FORCE:-1}" \
    UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.$lock_seq" \
    UPDATE_AGENT_PLUGINS_CALL_TIMEOUT="${UPDATE_AGENT_PLUGINS_CALL_TIMEOUT:-}" \
    CLAUDE_FAIL_UPDATE="${CLAUDE_FAIL_UPDATE:-}" \
    CLAUDE_UPDATE_HANG="${CLAUDE_UPDATE_HANG:-}" \
    CLAUDE_HANG_SECS="${CLAUDE_HANG_SECS:-}" \
    CLAUDE_FAIL_INSTALL="${CLAUDE_FAIL_INSTALL:-}" \
    CLAUDE_FAIL_MARKETPLACE_ADD="${CLAUDE_FAIL_MARKETPLACE_ADD:-}" \
    CLAUDE_BUMP="${CLAUDE_BUMP:-}" \
    CLAUDE_LIST_FAIL="${CLAUDE_LIST_FAIL:-}" \
    CLAUDE_LIST_GARBAGE="${CLAUDE_LIST_GARBAGE:-}" \
    CLAUDE_LIST_SHAPE_ONCE="${CLAUDE_LIST_SHAPE_ONCE:-}" \
    CLAUDE_LIST_SHAPE_MARKER="${CLAUDE_LIST_SHAPE_MARKER:-}" \
    CLAUDE_LIST_OBJGARBAGE="${CLAUDE_LIST_OBJGARBAGE:-}" \
    RELAY_STUB_OUTCOME="${RELAY_STUB_OUTCOME:-}" \
    bash "$HELPER" "$@" 2>&1)"
  RUN_RC=$?
}

log_entries() { grep -F "url=$UNATTENDED_LOG_HERMES_URL " "$RELAY_LOG" || true; }
alert_entries() { grep -F 'url=<default> ' "$RELAY_LOG" || true; }
log_entry_count() { log_entries | grep -c 'ARGV' || true; }
updated_ids() { sed -n 's/^plugin update //p' "$CLAUDE_CALL_LOG" || true; }

# ── 1. A clean SCHEDULED run posts one record with its class, host, run
#      timestamp and gap. On a machine with no recorded success the gap reads
#      NEVER, which is the state a fresh install is in. ────────────────────────
reset_state
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "a clean run exited $RUN_RC: $RUN_OUTPUT"
entries="$(log_entries)"
[[ -n $entries ]] || fail "a clean scheduled run posted no record: $RUN_OUTPUT"
grep -qF -- '--remote-only' <<<"$entries" ||
  fail "the record was not posted with --remote-only, so every weekly heartbeat would banner and buzz: $entries"
grep -qF -- '--state completed' <<<"$entries" || fail "the record did not carry the completed class: $entries"
grep -qF -- '--agent update-agent-plugins' <<<"$entries" || fail "the record does not name the job: $entries"
grep -qE 'run at [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z' <<<"$entries" ||
  fail "the record carries no ISO 8601 UTC run timestamp: $entries"
grep -qiF 'NEVER RECORDED' <<<"$entries" ||
  fail "the first record does not say that no successful run has been recorded here: $entries"
this_host="$(hostname -s 2>/dev/null || printf '%s' "${HOSTNAME:-unknown-host}")"
grep -qF -- "--project $this_host" <<<"$entries" ||
  fail "the record does not name the host it is about (expected --project $this_host): $entries"
refute 'url=<default>' "$(cat "$RELAY_LOG")" "a clean run sent an alert; the alert route is for things to act on"
refute 'fd9=inherited' "$(cat "$RELAY_LOG")" \
  "a relay call inherited the run's serialize-lock fd; a detached child would hold the lock after the run exited"
refute 'FD9-INHERITED' "$(cat "$CLAUDE_CALL_LOG")" \
  "a claude plugin call inherited the run's serialize-lock fd 9; a detached descendant of the CLI would keep the flock held after the run exited, deferring every later slot"
[[ -s $MARKER ]] || fail "a successful run did not record its timestamp at $MARKER"

# ── 2. The DISABLED plugin is skipped, and the skip is a fact about the CALLS,
#      not about the prose. An update on a disabled plugin exits 0 and
#      overwrites the tree, taking the operator's containment with it. ─────────
refute '^plugin update contained@mkt-a$' "$(cat "$CLAUDE_CALL_LOG")" \
  'the updater updated a DISABLED plugin, overwriting the tree the operator contained'
grep -qF 'skipped (disabled)' <<<"$entries" ||
  fail "the record does not report the disabled plugin as skipped: $entries"
# shellcheck disable=SC2016 # backticks are Discord code-span syntax, not a substitution
grep -qF 'contained@mkt-a' <<<"$entries" ||
  fail "the record does not NAME the skipped plugin, so the operator cannot see what is being left behind: $entries"
# ...and every ENABLED tracked plugin was updated, so the skip is narrow.
for id in steady@mkt-a mover@mkt-a sha@mkt-a opaque@mkt-a; do
  grep -qxF "plugin update $id" "$CLAUDE_CALL_LOG" ||
    fail "the updater did not update the enabled tracked plugin $id: $(cat "$CLAUDE_CALL_LOG")"
done
# An UNTRACKED installed plugin is never touched. The lock is a wanted set, not
# an inventory, and five undeclared plugins live on the real machine.
refute 'untracked@mkt-a' "$(cat "$CLAUDE_CALL_LOG")" \
  "the updater touched a plugin the lock does not track"

# ── 3. The UNKNOWABLE lane is reported as refreshed with the change unknowable,
#      and never as changed. lastUpdated bumps on a no-op refresh, so a change
#      signal read from it is noise, and a weekly cry-wolf line is worse than
#      no line at all. ──────────────────────────────────────────────────────────
grep -qiF 'change unknowable' <<<"$entries" ||
  fail "the record does not state that the unknowable lane's change cannot be known: $entries"
grep -qF 'opaque@mkt-a' <<<"$entries" ||
  fail "the record does not name the plugin whose change is unknowable: $entries"
unknowable_line="$(grep -o 'unknowable identity lane: [^|]*' <<<"$entries" || true)"
[[ -n $unknowable_line ]] || fail "the record has no unknowable-identity-lane section: $entries"
refute 'changed' "$unknowable_line" \
  "the unknowable lane reported a CHANGE, which it cannot know: $unknowable_line"
# The sentence says those plugins were REFRESHED, so it may only name plugins
# this run actually refreshed. A DISABLED plugin in the same lane was skipped,
# and counting it here would claim work that did not happen, which is the one
# thing every entry in this channel must never do.
grep -qF 'unknowable identity lane: 1 tracked plugin(s) refreshed' <<<"$unknowable_line" ||
  fail "the unknowable lane counted plugins it did not refresh: $unknowable_line"
refute 'containedopaque@mkt-a' "$unknowable_line" \
  "the unknowable lane named a DISABLED plugin as refreshed: $unknowable_line"
grep -qF 'containedopaque@mkt-a' <<<"$entries" ||
  fail "the disabled unknowable plugin is not named anywhere in the entry, so it vanished silently: $entries"

# ── 4. A tracked plugin that is NOT installed is installed, and its marketplace
#      is added first. Without this the vertical merges and does nothing on a
#      fresh machine. ────────────────────────────────────────────────────────────
grep -qxF 'plugin marketplace add owner/mkt-b' "$CLAUDE_CALL_LOG" ||
  fail "the absent plugin's marketplace was not added: $(cat "$CLAUDE_CALL_LOG")"
grep -qxF 'plugin install absent@mkt-b' "$CLAUDE_CALL_LOG" ||
  fail "the absent tracked plugin was not installed: $(cat "$CLAUDE_CALL_LOG")"
grep -qF 'installed' <<<"$entries" || fail "the record does not report the install: $entries"
# The CONFIGURED marketplace is not re-added: `marketplace add` on an existing
# name is not something this repo has measured, so the updater asks first.
refute 'plugin marketplace add owner/mkt-a' "$(cat "$CLAUDE_CALL_LOG")" \
  "the updater re-added a marketplace that was already configured"

# ── 5. The entry says a restart is needed. `claude plugin update`'s own help
#      says "restart required to apply", so an entry that implied the new code
#      was already live would be wrong. ────────────────────────────────────────
grep -qiF 'restart' <<<"$entries" ||
  fail "the record does not say the updated plugins are not live until the next session: $entries"

# ── 6. A run that changed NOTHING still produces an entry, and says so. On a
#      quiet week the gap figure is the only information the entry carries, so
#      suppressing it would throw away the reason the channel exists. This is a
#      SECOND run against the state the first one left: nothing is absent any
#      more and no version moves. The FIRST run legitimately reported an
#      install, which is why the quiet case needs its own run. ────────────────
# shellcheck disable=SC2016 # backticks are Discord code-span syntax, not a substitution
grep -qF '`absent@mkt-b` (added)' <<<"$entries" ||
  fail "the first run did not report the newly installed plugin as added: $entries"
rm -rf "$HOME/.local/state"
: >"$CLAUDE_CALL_LOG"
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "the quiet second run exited $RUN_RC: $RUN_OUTPUT"
entries="$(log_entries)"
grep -qE 'plugins with a knowable version: 0 of [0-9]+ tracked entries changed' <<<"$entries" ||
  fail "a run that moved no version did not report zero changes: $entries"
refute '\((added|removed)\)' "$entries" "a quiet run invented a change list: $entries"
refute 'plugin install ' "$(cat "$CLAUDE_CALL_LOG")" \
  "the second run re-installed a plugin that was already present"

# ── 7. A VERSION TRANSITION is named. Both knowable lanes report a real
#      identity, so unlike the unknowable one this record can be specific. ─────
reset_state
CLAUDE_BUMP="mover@mkt-a=2.0.0 sha@mkt-a=fed987654321" run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "the version-transition run exited $RUN_RC: $RUN_OUTPUT"
entries="$(log_entries)"
# shellcheck disable=SC2016 # backticks are Discord code-span syntax, not a substitution
grep -qF '`mover@mkt-a` `1.0.0` -> `2.0.0`' <<<"$entries" ||
  fail "an updated plugin's version transition was not reported: $entries"
# shellcheck disable=SC2016 # backticks are Discord code-span syntax, not a substitution
grep -qF '`sha@mkt-a` `abc123def456` -> `fed987654321`' <<<"$entries" ||
  fail "a git-sha-versioned plugin's transition was not reported: $entries"
refute 'steady@mkt-a. `1' "$entries" "an unchanged plugin was listed as changed: $entries"

# ── 7b. A week in which NO unknowable-lane plugin was refreshed says so, so a
#       reader knows every refresh above reports a real version. The zero case
#       is an entry path of its own, and an entry path nobody asks goes silent
#       without anyone noticing. Both unknowable fixture plugins sit disabled
#       here, so the lane has members and none of them was refreshed. ──────────
reset_state
jq '.enabledPlugins."opaque@mkt-a" = false' "$HOME/.claude/settings.json" >"$HOME/.claude/settings.json.tmp" &&
  mv "$HOME/.claude/settings.json.tmp" "$HOME/.claude/settings.json"
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "the zero-unknowable run exited $RUN_RC: $RUN_OUTPUT"
entries="$(log_entries)"
grep -qF 'unknowable identity lane: no tracked plugin was refreshed in it' <<<"$entries" ||
  fail "an entry with no unknowable refresh does not say so, so the lane goes silent instead of reporting clean: $entries"
refute 'change unknowable' "$entries" \
  "an entry with no unknowable refresh still claimed one: $entries"

# ── 7c. A KNOWABLE-lane plugin whose runtime version comes back "unknown" has an
#        identity this run could not read. Left in the comparison it reads equal
#        to itself on both snapshots and is counted in the clean "0 of N changed"
#        line, reporting a comparison that never happened for it. It must be named
#        as undetermined and excluded from the count, NOT rendered as a clean
#        entry (and NOT as the unknowable lane, which it does not belong to).
reset_state
jq 'map(if .id == "steady@mkt-a" then .version = "unknown" else . end)' "$PLUGIN_STATE" >"$PLUGIN_STATE.tmp" &&
  mv "$PLUGIN_STATE.tmp" "$PLUGIN_STATE"
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "the undetermined-version run exited $RUN_RC: $RUN_OUTPUT"
entries="$(log_entries)"
grep -qF 'knowable identity undetermined' <<<"$entries" ||
  fail "a knowable plugin reporting no version was not named as undetermined, so it read as a clean comparison: $entries"
grep -qF 'steady@mkt-a' <<<"$(grep -o 'knowable identity undetermined:[^|]*' <<<"$entries")" ||
  fail "the undetermined line does not name the plugin whose identity could not be read: $entries"
# ...and it does not appear in the unknowable-lane sentence: it declares a
# knowable lane, so mislabelling it there would hide a data problem as a property.
unknowable_line="$(grep -o 'unknowable identity lane:[^|]*' <<<"$entries" || true)"
refute 'steady@mkt-a' "$unknowable_line" \
  "an undetermined knowable plugin was mislabelled into the unknowable lane: $unknowable_line"

# ── 8. A FAILED update alerts on the EXISTING route (the priority channel) and
#      the record still goes out stating the count. A weekly job that fails and
#      tells nobody is the gap this whole family closes. ─────────────────────────
reset_state
CLAUDE_FAIL_UPDATE="mover@mkt-a" run_helper --scheduled
[[ $RUN_RC -ne 0 ]] || fail "a run with a failed update exited 0: $RUN_OUTPUT"
alerts="$(alert_entries)"
[[ -n $alerts ]] || fail "a failed plugin update sent NO alert: $RUN_OUTPUT"
grep -qF -- '--agent update-agent-plugins' <<<"$alerts" || fail "the alert does not name the job: $alerts"
refute '[-][-]remote-only' "$alerts" "the alert used the record route's flag; it must land in the priority channel"
grep -qF 'mover@mkt-a' <<<"$alerts" ||
  fail "the alert does not name WHICH plugin failed, so there is nothing to act on: $alerts"
entries="$(log_entries)"
grep -qE 'failed:[^.]*mover@mkt-a' <<<"$entries" ||
  fail "the record does not name the failed plugin in its failure list: $entries"
grep -qE 'plugins: [0-9]+ tracked[^.]*1 failed' <<<"$entries" ||
  fail "the record does not carry a failed count: $entries"
[[ ! -e $MARKER ]] || fail "a failing run recorded itself as the last SUCCESSFUL run, freezing the gap figure"
# The other plugins were still attempted: one failure must not abort the rest.
grep -qxF 'plugin update steady@mkt-a' "$CLAUDE_CALL_LOG" ||
  fail "one failed update aborted the remaining plugins: $(cat "$CLAUDE_CALL_LOG")"

# ── 8b. The failure alert does not make a FALSE state claim. An INSTALL failure
#        means the plugin is NOT present and had no prior version, so a blanket
#        "still at whatever version they held" was wrong for exactly the plugins
#        that failed to install. And a "not found in any marketplace" error can
#        be a missing plugin OR an unreachable marketplace, an ambiguity the alert
#        must surface rather than assert one of them. ────────────────────────────
reset_state
jq 'map(select(.id != "steady@mkt-a"))' "$PLUGIN_STATE" >"$PLUGIN_STATE.tmp" &&
  mv "$PLUGIN_STATE.tmp" "$PLUGIN_STATE"
CLAUDE_FAIL_INSTALL="steady@mkt-a" run_helper --scheduled
[[ $RUN_RC -ne 0 ]] || fail "an install failure exited 0: $RUN_OUTPUT"
alerts="$(alert_entries)"
[[ -n $alerts ]] || fail "an install failure sent no alert: $(cat "$RELAY_LOG")"
grep -qF 'steady@mkt-a' <<<"$alerts" ||
  fail "the install-failure alert does not name the plugin: $alerts"
refute 'still at whatever version they held; the rest were refreshed' "$alerts" \
  "the alert claims a plugin that failed to INSTALL is still at a prior version it never had: $alerts"
grep -qiE 'unreachable|could not reach' <<<"$alerts" ||
  fail "the alert does not flag that a not-found message can be an unreachable marketplace: $alerts"
grep -qiE 'not present|install failure' <<<"$alerts" ||
  fail "the alert does not distinguish an install failure (plugin absent) from an update failure: $alerts"

# ── 8c. The failure alert states only OUTCOMES IT OBSERVED (F43, F33 completed).
#        The old alert closed with "Every plugin that did not fail was refreshed",
#        which is a FALSE state claim: the two disabled fixtures were SKIPPED (not
#        refreshed) and the absent one was INSTALLED (not refreshed). A run with a
#        failed update, two skipped plugins and an install must not assert they
#        were all refreshed; it names the actual breakdown instead. ───────────────
reset_state
CLAUDE_FAIL_UPDATE="mover@mkt-a" run_helper --scheduled
[[ $RUN_RC -ne 0 ]] || fail "F43 setup: a failed update exited 0: $RUN_OUTPUT"
alerts="$(alert_entries)"
refute 'did not fail was refreshed' "$alerts" \
  "F43: the alert still claims every non-failed plugin was refreshed, false for the SKIPPED (disabled) and INSTALLED ones: $alerts"
grep -qiE 'skipped [0-9]|[0-9]+ skipped' <<<"$alerts" ||
  fail "F43: the failure alert does not account for the skipped (contained) plugins, so a reader assumes they were refreshed: $alerts"

# ── 9. A MANUAL run posts NO record. Otherwise a hand run makes a dead
#      LaunchAgent look alive, inverting the one signal the record carries. It
#      still alerts on failure: a failure is a failure whoever started it. ─────
reset_state
run_helper
[[ $RUN_RC -eq 0 ]] || fail "a clean manual run exited $RUN_RC: $RUN_OUTPUT"
[[ "$(log_entry_count)" -eq 0 ]] || fail "a MANUAL run posted a weekly record: $(log_entries)"
# ...and it does not advance last-success-at. That marker is the dead-LaunchAgent
# gap the scheduled record reports; a hand run that touched it would reset the gap
# and make a stalled agent look alive.
[[ ! -e $MARKER ]] || fail "a clean MANUAL run advanced last-success-at, resetting the dead-LaunchAgent gap figure"
reset_state
CLAUDE_FAIL_UPDATE="mover@mkt-a" run_helper
[[ "$(log_entry_count)" -eq 0 ]] || fail "a MANUAL failing run posted a weekly record: $(log_entries)"
[[ -n "$(alert_entries)" ]] || fail "a MANUAL failing run sent no alert"

# ── 10. One record per ISO week AND zero re-mutation on the second slot (F16).
#       24 hourly Monday slots would otherwise post 24 entries; worse, the claim
#       used to gate only the POST, so every later slot RE-RAN `claude plugin
#       update` on every plugin (overwriting each tree, re-installing a release
#       that landed after the first slot) and merely skipped the message. A slot
#       that finds the week already completed must run ZERO plugin mutations. ────
reset_state
run_helper --scheduled
[[ "$(log_entry_count)" -eq 1 ]] || fail "the first scheduled run posted $(log_entry_count) entries"
[[ -n "$(updated_ids)" ]] ||
  fail "the first scheduled run ran no updates, so the second-slot test would prove nothing"
: >"$RELAY_LOG"
: >"$CLAUDE_CALL_LOG"
run_helper --scheduled
[[ "$(log_entry_count)" -eq 0 ]] || fail "a second scheduled run in the same week posted again"
second_updates="$(updated_ids | tr '\n' ' ')"
[[ -z ${second_updates// /} ]] ||
  fail "a second same-week slot RE-RAN plugin updates instead of running zero: [$second_updates]"
refute '^plugin install ' "$(cat "$CLAUDE_CALL_LOG")" \
  "a second same-week slot re-ran installs: $(cat "$CLAUDE_CALL_LOG")"

# ── 10b. A FAILED slot does not consume the completed week; a later clean slot
#         recovers (F29). Slot 1 has an update failure and posts, but leaves the
#         completed slot free; slot 2 succeeds and posts a completed record and
#         retries the plugin that failed, instead of being silenced by slot 1's
#         spent claim leaving the channel showing failure all week. ─────────────
reset_state
CLAUDE_FAIL_UPDATE="mover@mkt-a" run_helper --scheduled
[[ $RUN_RC -ne 0 ]] || fail "the failing slot 1 exited 0: $RUN_OUTPUT"
[[ "$(log_entry_count)" -eq 1 ]] || fail "the failing slot 1 posted no record: $(log_entries)"
: >"$RELAY_LOG"
: >"$CLAUDE_CALL_LOG"
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "the recovering slot 2 exited $RUN_RC: $RUN_OUTPUT"
[[ "$(log_entry_count)" -eq 1 ]] ||
  fail "a later clean slot was silenced by the earlier failure's claim, so the channel stays showing failure (F29): $(log_entries)"
grep -qF -- '--state completed' <<<"$(log_entries)" ||
  fail "the recovering slot did not post a completed record: $(log_entries)"
grep -qxF 'plugin update mover@mkt-a' "$CLAUDE_CALL_LOG" ||
  fail "the recovering slot did not retry the previously failed plugin: $(cat "$CLAUDE_CALL_LOG")"

# ── 10c. Per-plugin success markers (F35): a recovering slot updates ONLY the
#         previously-failed plugin, never the ones that already succeeded. F29
#         released the completed claim on any failure so a later slot could
#         recover, but that let the later slot RE-ENTER the whole loop and re-run
#         `claude plugin update` on every plugin that had ALREADY succeeded
#         (re-fetching a release that landed after slot 1 and overwriting each
#         tree), reopening F16. Per-plugin markers gate the WORK: slot 1 fails
#         mover and marks the three enabled ones that succeed (and the install);
#         slot 2 runs `plugin update` for ONLY mover, touches none of the others,
#         and still posts a completed record. ───────────────────────────────────
reset_state
CLAUDE_FAIL_UPDATE="mover@mkt-a" run_helper --scheduled
[[ $RUN_RC -ne 0 ]] || fail "F35 slot 1 (with a failed update) exited 0: $RUN_OUTPUT"
for id in steady@mkt-a sha@mkt-a opaque@mkt-a; do
  grep -qxF "plugin update $id" "$CLAUDE_CALL_LOG" ||
    fail "F35 slot 1 did not update the enabled plugin $id: $(cat "$CLAUDE_CALL_LOG")"
done
: >"$RELAY_LOG"
: >"$CLAUDE_CALL_LOG"
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "F35 slot 2 (recovery) exited $RUN_RC: $RUN_OUTPUT"
slot2_updates="$(updated_ids | tr '\n' ' ')"
[[ $slot2_updates == "mover@mkt-a " ]] ||
  fail "F35 slot 2 must run plugin update for ONLY the previously-failed mover@mkt-a, got: [$slot2_updates]"
refute '^plugin install ' "$(cat "$CLAUDE_CALL_LOG")" \
  "F35 slot 2 re-installed a plugin an earlier slot already installed: $(cat "$CLAUDE_CALL_LOG")"
grep -qF -- '--state completed' <<<"$(log_entries)" ||
  fail "F35 slot 2 did not post a completed record after recovering: $(log_entries)"
grep -qE 'plugins: [0-9]+ tracked[^.]*[1-9][0-9]* already done earlier this week' <<<"$(log_entries)" ||
  fail "F35 slot 2 did not account for the already-succeeded plugins as already-done in its record: $(log_entries)"

# ── 11. A REFUSED record must not consume the week, and the operator hears
#       about it on the route that still works. relay exits 0 whatever the
#       gateway answered, so a 401 reads exactly like a delivered entry from
#       here. ────────────────────────────────────────────────────────────────────
reset_state
export RELAY_STUB_OUTCOME='relay: post FAILED HTTP 401'
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "a refused record broke the update it was reporting on (rc=$RUN_RC): $RUN_OUTPUT"
[[ "$(log_entry_count)" -eq 1 ]] || fail "the refused record did not reach the log route once: $(log_entries)"
grep -qF -- '--state log-channel-broken' <<<"$(alert_entries)" ||
  fail "a broken record channel raised no alert on the priority route: $(cat "$RELAY_LOG")"
: >"$RELAY_LOG"
run_helper --scheduled
[[ "$(log_entry_count)" -eq 1 ]] ||
  fail "a week whose record was refused did not retry; it stayed claimed with nothing sent"
refute 'log-channel-broken' "$(cat "$RELAY_LOG")" "the broken-channel alert repeated inside one week"
unset RELAY_STUB_OUTCOME
: >"$RELAY_LOG"
run_helper --scheduled
[[ "$(log_entry_count)" -eq 1 ]] || fail "the retrying run did not post its record: $RUN_OUTPUT"

# ── 12. An inventory this run could not READ means nothing is attempted. The
#       inventory is where installed-ness and enabled-ness come from, so
#       proceeding would update the plugin the operator disabled. The run says
#       so on both routes and exits non-zero. ─────────────────────────────────────
for mode in CLAUDE_LIST_FAIL CLAUDE_LIST_GARBAGE; do
  reset_state
  env_value=1
  if [[ $mode == CLAUDE_LIST_FAIL ]]; then
    CLAUDE_LIST_FAIL=$env_value run_helper --scheduled
  else
    CLAUDE_LIST_GARBAGE=$env_value run_helper --scheduled
  fi
  [[ $RUN_RC -ne 0 ]] || fail "[$mode] a run that could not read the inventory exited 0: $RUN_OUTPUT"
  refute '^plugin update ' "$(cat "$CLAUDE_CALL_LOG")" \
    "[$mode] the updater updated plugins without knowing which were disabled"
  refute '^plugin install ' "$(cat "$CLAUDE_CALL_LOG")" \
    "[$mode] the updater installed plugins without knowing which were present"
  entries="$(log_entries)"
  [[ -n $entries ]] || fail "[$mode] an unreadable inventory recorded nothing: $RUN_OUTPUT"
  grep -qF -- '--state deferred' <<<"$entries" ||
    fail "[$mode] a run that attempted nothing did not record a deferral: $entries"
  grep -qF 'claude plugin list --json' <<<"$entries" ||
    fail "[$mode] the record does not name the command that failed: $entries"
  refute '0 of 0' "$entries" "[$mode] an unreadable inventory rendered as a clean comparison of nothing: $entries"
  [[ -n "$(alert_entries)" ]] ||
    fail "[$mode] an unreadable inventory sent no alert; it will not fix itself: $(cat "$RELAY_LOG")"
  [[ ! -e $MARKER ]] || fail "[$mode] a run that attempted nothing recorded a successful run"
done

# ── 13. The AFTER reading is a different fact: the updates DID run, so the entry
#       is a completed one that says the comparison could not be made rather
#       than one claiming nothing changed. ─────────────────────────────────────
reset_state
cat >"$STUBS/claude.wrapper" <<'WRAP'
#!/usr/bin/env bash
# Fails `plugin list --json` on every reading AFTER the first, so the before
# snapshot succeeds and the after one does not.
if [[ "$1 $2" == "plugin list" ]]; then
  if [[ -e $AFTER_FAIL_MARKER ]]; then
    printf 'Error: broke mid-run\n' >&2
    printf '%s\n' "$*" >>"$CLAUDE_CALL_LOG"
    exit 1
  fi
  : >"$AFTER_FAIL_MARKER"
fi
exec "$REAL_CLAUDE" "$@"
WRAP
chmod +x "$STUBS/claude.wrapper"
mkdir -p "$tmp/afterfail"
cp "$STUBS/claude" "$tmp/afterfail/real-claude"
cp "$STUBS/claude.wrapper" "$tmp/afterfail/claude"
rm -f "$tmp/after-fail-marker"
RUN_OUTPUT="$(PATH="$tmp/afterfail:$PATH" REAL_CLAUDE="$tmp/afterfail/real-claude" \
  AFTER_FAIL_MARKER="$tmp/after-fail-marker" \
  UPDATE_AGENT_PLUGINS_FORCE=1 UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.afterfail" \
  bash "$HELPER" --scheduled 2>&1)"
after_rc=$?
entries="$(log_entries)"
grep -qF -- '--state completed' <<<"$entries" ||
  fail "a run whose AFTER reading failed did not record itself as completed (rc=$after_rc): $entries | $RUN_OUTPUT"
grep -qiE 'plugins with a knowable version: [^|]*(NOT COMPARED|could not)' <<<"$entries" ||
  fail "the record does not say the version comparison could not be made: $entries"
refute 'plugins with a knowable version: 0 of 0' "$entries" \
  "a failed AFTER reading rendered as a clean comparison of nothing: $entries"
refute '\((added|removed)\)' "$entries" \
  "a one-sided reading invented a whole-roster change list: $entries"
# The updates themselves still happened and are still reported.
grep -qxF 'plugin update steady@mkt-a' "$CLAUDE_CALL_LOG" ||
  fail "the updates did not run: $(cat "$CLAUDE_CALL_LOG")"

# ── 13b. An inventory ENTRY WITH NO id must not take the comparison down with
#        it. Measured 2026-08-03: `.id | in($tracked)` raises "Cannot check
#        whether object has a null key" on such a record, and a version snapshot
#        that dies leaves an EMPTY before file, so the change line reports every
#        tracked plugin as newly added. That is a whole-roster change list,
#        invented, on a week where nothing happened. ────────────────────────────
reset_state
jq '. + [{"version":"1.0.0","scope":"user","enabled":true}]' "$PLUGIN_STATE" >"$PLUGIN_STATE.tmp" &&
  mv "$PLUGIN_STATE.tmp" "$PLUGIN_STATE"
run_helper --scheduled
entries="$(log_entries)"
[[ -n $entries ]] || fail "an inventory carrying an id-less record posted no entry at all: $RUN_OUTPUT"
# shellcheck disable=SC2016 # backticks are Discord code-span syntax, not a substitution
refute '`steady@mkt-a` \(added\)' "$entries" \
  "an id-less inventory record made the comparison report already-installed plugins as newly added: $entries"
# The comparison must still be a REAL one. Measured before the fix: both
# snapshots died, both files came back empty, and the line rendered
# "0 of 0 tracked entries changed" -- a clean week, on a comparison that never
# happened, which is the exact defect this family exists to prevent.
refute 'plugins with a knowable version: 0 of 0' "$entries" \
  "an id-less inventory record collapsed the comparison into a clean-looking comparison of NOTHING: $entries"
grep -qF 'plugins with a knowable version: 1 of 5 tracked entries changed' <<<"$entries" ||
  fail "the comparison did not cover the five tracked plugins that declare a version: $entries"
grep -qxF 'plugin update steady@mkt-a' "$CLAUDE_CALL_LOG" ||
  fail "an id-less inventory record stopped the updates running: $(cat "$CLAUDE_CALL_LOG")"

# ── 13c. A BEFORE inventory that is a JSON array of NON-OBJECTS attempts
#        NOTHING. [1, 2] passed the bare "is it an array" check and then broke
#        every .id/.enabled extraction, so every tracked plugin looked absent and
#        the run tried to INSTALL the whole roster while it could not tell which
#        plugins the operator had DISABLED, overwriting a contained tree. A shape
#        it cannot read is fail-closed: attempt nothing, alert, record a deferral.
#        A schema change could really hand back such a body.
reset_state
rm -f "$tmp/shape-marker"
CLAUDE_LIST_SHAPE_ONCE=1 CLAUDE_LIST_SHAPE_MARKER="$tmp/shape-marker" run_helper --scheduled
[[ $RUN_RC -ne 0 ]] ||
  fail "a non-object BEFORE inventory exited 0 instead of attempting nothing: $RUN_OUTPUT"
refute '^plugin (update|install) ' "$(cat "$CLAUDE_CALL_LOG")" \
  "a non-object inventory triggered an install storm, or updated plugins without knowing which were disabled: $(cat "$CLAUDE_CALL_LOG")"
entries="$(log_entries)"
grep -qF -- '--state deferred' <<<"$entries" ||
  fail "a non-object BEFORE inventory did not record a deferral: $entries"
grep -qF 'claude plugin list --json' <<<"$entries" ||
  fail "the record does not name the command whose output could not be read: $entries"
[[ -n "$(alert_entries)" ]] || fail "a non-object inventory sent no alert: $(cat "$RELAY_LOG")"
[[ ! -e $MARKER ]] || fail "a run that attempted nothing recorded a successful run"

# ── 13c2. A BEFORE inventory that is an array of OBJECTS carrying no usable id
#         attempts NOTHING (F38). F18 rejected an array of NON-objects, but
#         [{"foo":"bar"},{"baz":1}] passed the object check and then every id/scope
#         extraction came back empty, so the whole tracked roster looked ABSENT and
#         the run tried to INSTALL all of it while it could not tell which plugins
#         the operator had DISABLED. An inventory with no readable member fails
#         closed: attempt nothing, alert, record a deferral, install NOTHING. ──────
reset_state
CLAUDE_LIST_OBJGARBAGE=1 run_helper --scheduled
[[ $RUN_RC -ne 0 ]] ||
  fail "an object-garbage BEFORE inventory exited 0 instead of attempting nothing: $RUN_OUTPUT"
refute '^plugin (update|install) ' "$(cat "$CLAUDE_CALL_LOG")" \
  "an inventory of objects with no id triggered an install storm, or updated plugins without knowing which were disabled: $(cat "$CLAUDE_CALL_LOG")"
entries="$(log_entries)"
grep -qF -- '--state deferred' <<<"$entries" ||
  fail "an object-garbage BEFORE inventory did not record a deferral: $entries"
[[ -n "$(alert_entries)" ]] || fail "an object-garbage inventory sent no alert: $(cat "$RELAY_LOG")"
[[ ! -e $MARKER ]] || fail "a run that attempted nothing recorded a successful run"

# ── 13d. A malformed AFTER snapshot reads NOT COMPARED, never a list of removals.
#        The updates ran and the before snapshot was valid, but the after read
#        came back [1, 2], an array of non-objects that passed the old array check
#        so after_ok stayed true, and the comparison against a valid before
#        rendered every tracked plugin as REMOVED. Rejecting the non-object after
#        makes the section say the comparison could not be made.
reset_state
cat >"$STUBS/claude.after-nonobject" <<'WRAP'
#!/usr/bin/env bash
# A JSON array of non-objects on every `plugin list` AFTER the first, so the
# before snapshot is valid and only the after one is malformed.
if [[ "$1 $2" == "plugin list" ]]; then
  if [[ -e $AFTER_NONOBJ_MARKER ]]; then
    printf '%s\n' "$*" >>"$CLAUDE_CALL_LOG"
    printf '[1, 2]\n'
    exit 0
  fi
  : >"$AFTER_NONOBJ_MARKER"
fi
exec "$REAL_CLAUDE" "$@"
WRAP
chmod +x "$STUBS/claude.after-nonobject"
mkdir -p "$tmp/afternonobj"
cp "$STUBS/claude" "$tmp/afternonobj/real-claude"
cp "$STUBS/claude.after-nonobject" "$tmp/afternonobj/claude"
rm -f "$tmp/after-nonobj-marker"
RUN_OUTPUT="$(PATH="$tmp/afternonobj:$PATH" REAL_CLAUDE="$tmp/afternonobj/real-claude" \
  AFTER_NONOBJ_MARKER="$tmp/after-nonobj-marker" \
  UPDATE_AGENT_PLUGINS_FORCE=1 UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.afternonobj" \
  bash "$HELPER" --scheduled 2>&1)"
entries="$(log_entries)"
grep -qF -- '--state completed' <<<"$entries" ||
  fail "a run whose AFTER snapshot was a non-object array did not record completed: $entries | $RUN_OUTPUT"
grep -qiE 'plugins with a knowable version: [^|]*(NOT COMPARED|could not)' <<<"$entries" ||
  fail "a malformed after snapshot did not read NOT COMPARED: $entries"
refute '\(removed\)' "$entries" \
  "a malformed after snapshot rendered the tracked plugins as removals: $entries"
grep -qxF 'plugin update steady@mkt-a' "$CLAUDE_CALL_LOG" ||
  fail "the updates did not run before the malformed after read: $(cat "$CLAUDE_CALL_LOG")"

# ── 13d2. A malformed OBJECT-shaped AFTER snapshot reads NOT COMPARED, never a
#         list of removals (F41 completes F28 for the object case). F28 rejected an
#         after read of NON-objects ([1, 2]); an after read of objects with no
#         usable id ([{"foo":1},{"bar":2}]) passed the object check so after_ok
#         stayed true, every tracked plugin dropped out of the after snapshot, and
#         the comparison against a valid before rendered them all as REMOVED. The
#         updates ran and the before was valid, so the section must say the
#         comparison could not be made, not invent a whole-roster removal. ─────────
reset_state
cat >"$STUBS/claude.after-objgarbage" <<'WRAP'
#!/usr/bin/env bash
# An array of objects with no id on every `plugin list` AFTER the first, so the
# before snapshot is valid and only the after one is malformed-but-object-shaped.
if [[ "$1 $2" == "plugin list" ]]; then
  if [[ -e $AFTER_OBJGARBAGE_MARKER ]]; then
    printf '%s\n' "$*" >>"$CLAUDE_CALL_LOG"
    printf '[{"foo":1},{"bar":2}]\n'
    exit 0
  fi
  : >"$AFTER_OBJGARBAGE_MARKER"
fi
exec "$REAL_CLAUDE" "$@"
WRAP
chmod +x "$STUBS/claude.after-objgarbage"
mkdir -p "$tmp/afterobjgarbage"
cp "$STUBS/claude" "$tmp/afterobjgarbage/real-claude"
cp "$STUBS/claude.after-objgarbage" "$tmp/afterobjgarbage/claude"
rm -f "$tmp/after-objgarbage-marker"
RUN_OUTPUT="$(PATH="$tmp/afterobjgarbage:$PATH" REAL_CLAUDE="$tmp/afterobjgarbage/real-claude" \
  AFTER_OBJGARBAGE_MARKER="$tmp/after-objgarbage-marker" \
  UPDATE_AGENT_PLUGINS_FORCE=1 UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.afterobjgarbage" \
  bash "$HELPER" --scheduled 2>&1)"
entries="$(log_entries)"
grep -qF -- '--state completed' <<<"$entries" ||
  fail "a run whose AFTER snapshot was an object-garbage array did not record completed: $entries | $RUN_OUTPUT"
grep -qiE 'plugins with a knowable version: [^|]*(NOT COMPARED|could not)' <<<"$entries" ||
  fail "an object-garbage after snapshot did not read NOT COMPARED: $entries"
refute '\(removed\)' "$entries" \
  "an object-garbage after snapshot rendered the tracked plugins as removals: $entries"
grep -qxF 'plugin update steady@mkt-a' "$CLAUDE_CALL_LOG" ||
  fail "the updates did not run before the object-garbage after read: $(cat "$CLAUDE_CALL_LOG")"

# ── 14. NO LOCK means nothing to update, and it means chezmoi has not applied.
#       Silence here would be indistinguishable from a clean week. ─────────────
reset_state
rm -f "$LOCK"
run_helper --scheduled
[[ $RUN_RC -ne 0 ]] || fail "a missing lock exited 0: $RUN_OUTPUT"
entries="$(log_entries)"
[[ -n $entries ]] || fail "a missing lock recorded nothing: $RUN_OUTPUT"
grep -qF -- '--state deferred' <<<"$entries" || fail "a missing lock did not record a deferral: $entries"
[[ -n "$(alert_entries)" ]] || fail "a missing lock sent no alert: $(cat "$RELAY_LOG")"
write_lock

# ── 14b. A lock present but carrying an EMPTY or null plugins map is a degraded
#        state, not a valid empty roster. {}, {"plugins":{}} and {"plugins":null}
#        each parse as "an object with a plugins map", and the run once processed
#        zero plugins, posted "0 tracked", wrote last-success-at and exited clean,
#        a successful week reported for a vertical managing nothing, which is what
#        a truncated deployed lock looks like. It must attempt nothing and say so.
for degraded in '{}' '{"plugins":{}}' '{"plugins":null}'; do
  reset_state
  printf '%s\n' "$degraded" >"$LOCK"
  run_helper --scheduled
  [[ $RUN_RC -ne 0 ]] || fail "a degraded lock ($degraded) exited 0: $RUN_OUTPUT"
  refute '^plugin (update|install) ' "$(cat "$CLAUDE_CALL_LOG")" \
    "a degraded lock ($degraded) still mutated plugins"
  entries="$(log_entries)"
  grep -qF -- '--state deferred' <<<"$entries" ||
    fail "a degraded lock ($degraded) did not record a deferral: $entries"
  refute 'plugins: 0 tracked' "$entries" \
    "a degraded lock ($degraded) posted a clean-looking zero-tracked completed record: $entries"
  [[ -n "$(alert_entries)" ]] || fail "a degraded lock ($degraded) sent no alert: $(cat "$RELAY_LOG")"
  [[ ! -e $MARKER ]] ||
    fail "a degraded lock ($degraded) advanced last-success-at, recording a clean week for a vertical that managed nothing"
done
write_lock

# ── 14c. A NON-EMPTY plugins map whose MEMBERS are malformed also refuses (F40,
#        F21 completed for the malformed-member case). A member that is not an
#        object, or one missing its marketplace, passed the length check and then
#        crashed the loop's `.value.marketplace` read, so the process substitution
#        came back empty, tracked_count stayed 0, and the run posted the same clean
#        zero-work success a truncated lock produces. Each such lock must attempt
#        nothing and say so, never record a healthy week. ─────────────────────────
for badmember in \
  '{"plugins":{"steady@mkt-a":"garbage-not-an-object"}}' \
  '{"plugins":{"steady@mkt-a":{"identityLane":"versioned"}}}' \
  '{"plugins":{"steady@mkt-a":{"marketplace":""}}}'; do
  reset_state
  printf '%s\n' "$badmember" >"$LOCK"
  run_helper --scheduled
  [[ $RUN_RC -ne 0 ]] || fail "a lock with a malformed member ($badmember) exited 0: $RUN_OUTPUT"
  refute '^plugin (update|install) ' "$(cat "$CLAUDE_CALL_LOG")" \
    "a lock with a malformed member ($badmember) still mutated plugins"
  entries="$(log_entries)"
  grep -qF -- '--state deferred' <<<"$entries" ||
    fail "a lock with a malformed member ($badmember) did not record a deferral: $entries"
  refute 'plugins: 0 tracked' "$entries" \
    "a lock with a malformed member ($badmember) posted a clean-looking zero-tracked completed record: $entries"
  [[ -n "$(alert_entries)" ]] || fail "a lock with a malformed member ($badmember) sent no alert: $(cat "$RELAY_LOG")"
  [[ ! -e $MARKER ]] ||
    fail "a lock with a malformed member ($badmember) advanced last-success-at, recording a clean week for a vertical that did zero work"
done
write_lock

# ── 15. The IDLE GATE defers on recent Claude activity, records the deferral,
#       and attempts nothing. A deferral is not a failure: nothing was tried, so
#       it is recorded rather than alerted, and the gap line in that entry is
#       what makes a starved gate legible. ─────────────────────────────────────
reset_state
ACTIVITY="$HOME/.claude/projects"
mkdir -p "$ACTIVITY"
: >"$ACTIVITY/live-session.jsonl"
RUN_OUTPUT="$(PATH="$STUBS:$PATH" UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.idle" \
  bash "$HELPER" --scheduled 2>&1)"
idle_rc=$?
[[ $idle_rc -eq 75 ]] || fail "a deferral exited $idle_rc, want 75 (EX_TEMPFAIL): $RUN_OUTPUT"
refute '^plugin (update|install) ' "$(cat "$CLAUDE_CALL_LOG")" \
  "the updater mutated plugins on a slot it had decided to defer"
entries="$(log_entries)"
grep -qF -- '--state deferred' <<<"$entries" || fail "the idle deferral recorded nothing: $entries"
grep -qiE 'last successful run' <<<"$entries" ||
  fail "the deferral entry carries no gap line, so a permanently starved gate would be invisible: $entries"
refute 'url=<default>' "$(cat "$RELAY_LOG")" "an idle deferral alerted; nothing was attempted"

# ...and the gate is BYPASSABLE: UPDATE_AGENT_PLUGINS_FORCE=1 proceeds past the
# same live transcript. Every scenario above this one leans on that bypass, so
# a bypass that silently broke would turn this whole file into a test of a gate
# nobody can open, which is task #95's shape.
reset_state
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "UPDATE_AGENT_PLUGINS_FORCE=1 did not bypass a live transcript (rc=$RUN_RC): $RUN_OUTPUT"
grep -qxF 'plugin update steady@mkt-a' "$CLAUDE_CALL_LOG" ||
  fail "the FORCE bypass did not actually run the updates: $(cat "$CLAUDE_CALL_LOG")"

# ...and an activity dir the probe cannot READ fails CLOSED. The probe cannot
# prove the machine idle, and the costs are asymmetric: a wrong defer is one
# week, a wrong proceed swaps a plugin tree under a session the probe could not
# see.
reset_state
chmod 000 "$ACTIVITY"
RUN_OUTPUT="$(PATH="$STUBS:$PATH" UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.unreadable" \
  bash "$HELPER" --scheduled 2>&1)"
unreadable_rc=$?
chmod 755 "$ACTIVITY"
[[ $unreadable_rc -eq 75 ]] ||
  fail "an unreadable activity dir did not fail closed (rc=$unreadable_rc, want 75): $RUN_OUTPUT"
refute '^plugin (update|install) ' "$(cat "$CLAUDE_CALL_LOG")" \
  "the updater mutated plugins behind an activity probe it could not read"

# ...and a machine whose Claude transcripts are all STALE proceeds. A gate that
# defers forever is task #95's failure mode, and it is the reason this one
# probes Claude alone.
reset_state
touch -t "$(date -v-2H +%Y%m%d%H%M.%S 2>/dev/null || date -d '2 hours ago' +%Y%m%d%H%M.%S)" \
  "$ACTIVITY/live-session.jsonl"
RUN_OUTPUT="$(PATH="$STUBS:$PATH" UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.stale" \
  bash "$HELPER" --scheduled 2>&1)"
stale_rc=$?
[[ $stale_rc -eq 0 ]] || fail "a machine idle for two hours still deferred (rc=$stale_rc): $RUN_OUTPUT"
grep -qxF 'plugin update steady@mkt-a' "$CLAUDE_CALL_LOG" ||
  fail "an idle machine did not run the updates: $(cat "$CLAUDE_CALL_LOG")"
rm -rf "$ACTIVITY"

# ...and the probe counts ONLY session transcripts (*.jsonl). A README, a stray
# .DS_Store or an editor tempfile freshly dropped in the projects tree is not a
# live session, so it must NOT defer the week; matching every file starved the
# update for a reason unrelated to any running session.
reset_state
mkdir -p "$ACTIVITY"
: >"$ACTIVITY/README.md"
: >"$ACTIVITY/.DS_Store"
RUN_OUTPUT="$(PATH="$STUBS:$PATH" UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.nontranscript" \
  bash "$HELPER" --scheduled 2>&1)"
nontranscript_rc=$?
[[ $nontranscript_rc -eq 0 ]] ||
  fail "a non-transcript file (README.md) deferred the week (rc=$nontranscript_rc): $RUN_OUTPUT"
grep -qxF 'plugin update steady@mkt-a' "$CLAUDE_CALL_LOG" ||
  fail "a README.md in the projects tree stopped the updates running: $(cat "$CLAUDE_CALL_LOG")"
# ...but a fresh transcript beside it still defers, so the narrowing did not
# defeat the gate.
reset_state
mkdir -p "$ACTIVITY"
: >"$ACTIVITY/README.md"
: >"$ACTIVITY/live-session.jsonl"
RUN_OUTPUT="$(PATH="$STUBS:$PATH" UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.transcript" \
  bash "$HELPER" --scheduled 2>&1)"
[[ $? -eq 75 ]] ||
  fail "a fresh .jsonl transcript did not defer once the probe was narrowed to transcripts: $RUN_OUTPUT"
refute '^plugin (update|install) ' "$(cat "$CLAUDE_CALL_LOG")" \
  "the updater mutated plugins on a slot a live transcript should have deferred"
rm -rf "$ACTIVITY"

# ── 16. LOCK CONTENTION is a deferral, not a failure. ───────────────────────
reset_state
: >"$tmp/held.lock"
lock_holder_out="$tmp/holder.out"
rm -f "$lock_holder_out"
(
  exec 9>>"$tmp/held.lock"
  /usr/bin/lockf -s -t 0 9 2>/dev/null || exit 1
  : >"$lock_holder_out"
  while [[ -e "$tmp/hold-me" ]]; do sleep 0.05; done
) &
holder_pid=$!
: >"$tmp/hold-me"
for ((i = 0; i < 100; i++)); do
  [[ -e $lock_holder_out ]] && break
  sleep 0.05
done
if [[ -e $lock_holder_out ]]; then
  PATH="$STUBS:$PATH" UPDATE_AGENT_PLUGINS_FORCE=1 UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/held.lock" \
    bash "$HELPER" --scheduled >/dev/null 2>&1
  contended_rc=$?
  rm -f "$tmp/hold-me"
  wait "$holder_pid" 2>/dev/null || true
  [[ $contended_rc -eq 75 ]] || fail "lock contention exited $contended_rc, want 75"
  entries="$(log_entries)"
  grep -qF -- '--state deferred' <<<"$entries" || fail "lock contention did not record a deferral: $entries"
  refute 'url=<default>' "$(cat "$RELAY_LOG")" "lock contention alerted; nothing was attempted"
else
  rm -f "$tmp/hold-me"
  wait "$holder_pid" 2>/dev/null || true
  fail "could not stage a held lock; the contention case did not run"
fi

# ── 17. An unknown argument is an ERROR. A typo'd marker in the plist would
#       otherwise run every week and quietly post nothing, which looks exactly
#       like a dead LaunchAgent. ─────────────────────────────────────────────────
reset_state
run_helper --schedluled
[[ $RUN_RC -ne 0 ]] || fail "an unknown argument exited 0"
grep -qiE 'usage|unknown' <<<"$RUN_OUTPUT" || fail "an unknown argument produced no usage message: $RUN_OUTPUT"
refute '^plugin ' "$(cat "$CLAUDE_CALL_LOG")" "an unknown argument still ran plugin commands"

# ── 18. A HUNG claude call is BOUNDED, not a wedge. Any plugin command that
#       reached a stuck network would otherwise hold the serialize lock forever,
#       so no later Monday slot could recover the single launchd job. A bounded
#       call becomes a failed plugin (which alerts), and the run returns. ───────
reset_state
hang_start="$(date +%s)"
CLAUDE_UPDATE_HANG="mover@mkt-a" CLAUDE_HANG_SECS=30 UPDATE_AGENT_PLUGINS_CALL_TIMEOUT=1 \
  run_helper --scheduled
hang_elapsed=$(($(date +%s) - hang_start))
[[ $RUN_RC -ne 0 ]] || fail "a hung update did not fail the run (rc=$RUN_RC): $RUN_OUTPUT"
[[ $hang_elapsed -lt 25 ]] ||
  fail "a hung claude call was not bounded (the run took ${hang_elapsed}s); the LaunchAgent would wedge and no later slot could recover the single launchd job"
grep -qxF 'plugin update steady@mkt-a' "$CLAUDE_CALL_LOG" ||
  fail "one hung plugin aborted the remaining updates: $(cat "$CLAUDE_CALL_LOG")"
grep -qF 'mover@mkt-a' <<<"$(alert_entries)" ||
  fail "a hung update sent no alert naming the plugin that hung: $(alert_entries)"

# ── 19. On the LAST scheduled slot a deferral is a STARVATION alert. A live
#        Claude session that defers every Monday slot leaves the week updating
#        NOTHING; the final slot must alert, because a gate that defers forever in
#        silence is task #95's exact failure. Earlier slots defer quietly. A date
#        stub pins which slot this is; the idle gate sees a fresh transcript. ────
reset_state
mkdir -p "$ACTIVITY"
: >"$ACTIVITY/live-session.jsonl"
mkdir -p "$tmp/lastslot"
cat >"$tmp/lastslot/date" <<'DSTUB'
#!/usr/bin/env bash
for a in "$@"; do
  case "$a" in
    +%u) printf '%s\n' "${FAKE_DOW:-1}"; exit 0 ;;
    +%H) printf '%s\n' "${FAKE_HOUR:-23}"; exit 0 ;;
  esac
done
exec /bin/date "$@"
DSTUB
chmod +x "$tmp/lastslot/date"
# An earlier slot (Monday 00:00) defers with NO starvation alert.
: >"$RELAY_LOG"
PATH="$tmp/lastslot:$STUBS:$PATH" FAKE_DOW=1 FAKE_HOUR=0 \
  UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.slot0" bash "$HELPER" --scheduled >/dev/null 2>&1
[[ $? -eq 75 ]] || true
refute 'deferred-exhausted' "$(alert_entries)" \
  "an early-slot deferral fired the starvation alert; only the last slot should: $(cat "$RELAY_LOG")"
# The last slot (Monday 23:00) defers AND alerts that the week starved.
: >"$RELAY_LOG"
PATH="$tmp/lastslot:$STUBS:$PATH" FAKE_DOW=1 FAKE_HOUR=23 \
  UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.slot23" bash "$HELPER" --scheduled >/dev/null 2>&1
last_rc=$?
[[ $last_rc -eq 75 ]] || fail "the last-slot deferral did not exit 75 (rc=$last_rc)"
grep -qF -- '--state deferred-exhausted' <<<"$(alert_entries)" ||
  fail "the last scheduled slot deferred with no starvation alert, so a week that updated nothing is invisible: $(cat "$RELAY_LOG")"
rm -rf "$ACTIVITY"

# ── 19b. Once the week has COMPLETED, the last-slot deferral must NOT fire the
#         starvation alert (F42). The starvation alert means "the week updated
#         NOTHING because every slot deferred"; a week that already completed did
#         update, so alerting on it is a false claim. An earlier slot completes the
#         week (writing the completed token); the last slot then finds a live
#         session and defers, but the alert must stay silent because the work is
#         already done. The date stub from test 19 pins the last slot. ────────────
reset_state
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "F42 setup: the completing slot did not complete (rc=$RUN_RC): $RUN_OUTPUT"
mkdir -p "$ACTIVITY"
: >"$ACTIVITY/live-session.jsonl"
: >"$RELAY_LOG"
PATH="$tmp/lastslot:$STUBS:$PATH" FAKE_DOW=1 FAKE_HOUR=23 \
  UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.f42" bash "$HELPER" --scheduled >/dev/null 2>&1
f42_rc=$?
[[ $f42_rc -eq 75 ]] || fail "F42: the last-slot deferral on a completed week did not exit 75 (rc=$f42_rc)"
refute 'deferred-exhausted' "$(alert_entries)" \
  "F42: the last slot fired the starvation alert for a week that had already COMPLETED, a false 'nothing updated' claim: $(cat "$RELAY_LOG")"
rm -rf "$ACTIVITY"

# ── 20. The idle gate is RE-PROBED before mutating (F26). The up-front check runs
#        before `claude plugin list --json`, which reaches the network; a Claude
#        session that starts in that window must not have its plugin tree swapped
#        out mid-turn. A wrapper starts a fresh transcript during the inventory
#        read, and the second probe catches it and defers. ────────────────────────
reset_state
mkdir -p "$ACTIVITY"
# A STALE transcript, so the up-front probe PROCEEDS.
touch -t "$(date -v-2H +%Y%m%d%H%M.%S 2>/dev/null || date -d '2 hours ago' +%Y%m%d%H%M.%S)" \
  "$ACTIVITY/old-session.jsonl"
mkdir -p "$tmp/midrun"
cp "$STUBS/claude" "$tmp/midrun/real-claude"
cat >"$tmp/midrun/claude" <<'WRAP'
#!/usr/bin/env bash
# On the first `plugin list` (the before inventory read) a Claude session
# "starts": drop a fresh transcript into the activity dir, then behave normally.
if [[ "$1 $2" == "plugin list" && ! -e $MIDRUN_MARK ]]; then
  : >"$MIDRUN_MARK"
  : >"$ACT/live-session.jsonl"
fi
exec "$REAL_CLAUDE" "$@"
WRAP
chmod +x "$tmp/midrun/claude"
rm -f "$tmp/midrun-mark"
RUN_OUTPUT="$(PATH="$tmp/midrun:$STUBS:$PATH" REAL_CLAUDE="$tmp/midrun/real-claude" \
  MIDRUN_MARK="$tmp/midrun-mark" ACT="$ACTIVITY" \
  UPDATE_AGENT_PLUGINS_LOCKFILE="$tmp/lock.midrun" bash "$HELPER" --scheduled 2>&1)"
midrun_rc=$?
[[ $midrun_rc -eq 75 ]] ||
  fail "a session that started during the inventory read was not caught by the re-probe (rc=$midrun_rc): $RUN_OUTPUT"
refute '^plugin (update|install) ' "$(cat "$CLAUDE_CALL_LOG")" \
  "the updater swapped a plugin tree under a session that started mid-run: $(cat "$CLAUDE_CALL_LOG")"
rm -rf "$ACTIVITY"

# ── 21. A CONFIGURED marketplace is not trusted by NAME alone. A marketplace
#        re-pointed at a different repo (name kept, source swapped) would be
#        installed from silently while the lock still reads as declared. Re-point
#        mkt-a and make steady absent: the install is REFUSED and names the
#        divergence, and the marketplace is never re-added or removed (F8). ──────
reset_state
jq 'map(select(.id != "steady@mkt-a"))' "$PLUGIN_STATE" >"$PLUGIN_STATE.tmp" &&
  mv "$PLUGIN_STATE.tmp" "$PLUGIN_STATE"
printf '[{"name":"mkt-a","source":"github","repo":"impostor/mkt-a","installLocation":"/dev/null"}]\n' >"$MARKETPLACE_STATE"
run_helper --scheduled
[[ $RUN_RC -ne 0 ]] || fail "a re-pointed marketplace did not fail the run: $RUN_OUTPUT"
refute '^plugin install steady@mkt-a$' "$(cat "$CLAUDE_CALL_LOG")" \
  "the updater installed from a marketplace whose live repo diverges from the lock: $(cat "$CLAUDE_CALL_LOG")"
refute 'plugin marketplace add [^ ]*mkt-a' "$(cat "$CLAUDE_CALL_LOG")" \
  "the updater re-added the re-pointed mkt-a marketplace it must never touch: $(cat "$CLAUDE_CALL_LOG")"
grep -qF 'impostor/mkt-a' <<<"$(alert_entries)" ||
  fail "the alert does not name the re-pointed repo, so there is nothing to act on: $(alert_entries)"

# ── 21b. A marketplace whose SOURCE-VERIFY read FAILS or times out must BLOCK the
#         install, not fail OPEN (F37 completes F8 for the error path). The verify
#         read is a SECOND `claude plugin marketplace list --json` after the
#         configured check; if it errors, an empty live repo cannot prove a match,
#         and the old code proceeded to install from a marketplace it never
#         verified. Make steady absent, let the configured check (call 1) pass and
#         the verify read (call 2) fail: the install is REFUSED and the plugin is
#         reported, never installed from an unverified marketplace. ───────────────
reset_state
jq 'map(select(.id != "steady@mkt-a"))' "$PLUGIN_STATE" >"$PLUGIN_STATE.tmp" &&
  mv "$PLUGIN_STATE.tmp" "$PLUGIN_STATE"
# Drop the other absent tracked plugin so steady is the ONLY one that reaches the
# marketplace-list reads: its configured check is call 1 (passes) and its
# source-verify read is call 2 (fails), which FAIL_AFTER=1 pins exactly.
jq 'del(.plugins."absent@mkt-b")' "$LOCK" >"$LOCK.tmp" && mv "$LOCK.tmp" "$LOCK"
CLAUDE_MARKETPLACE_LIST_FAIL_AFTER=1 run_helper --scheduled
[[ $RUN_RC -ne 0 ]] || fail "a failed marketplace source-verify read did not fail the run: $RUN_OUTPUT"
refute '^plugin install steady@mkt-a$' "$(cat "$CLAUDE_CALL_LOG")" \
  "the updater installed from a marketplace whose source it could NOT verify (the verify read failed): $(cat "$CLAUDE_CALL_LOG")"
refute 'plugin marketplace add [^ ]*mkt-a' "$(cat "$CLAUDE_CALL_LOG")" \
  "the updater re-added mkt-a when its verify read failed; it must never re-add or remove a marketplace: $(cat "$CLAUDE_CALL_LOG")"
grep -qE 'failed:[^.]*steady@mkt-a' <<<"$(log_entries)" ||
  fail "the record does not report steady@mkt-a as failed when its marketplace could not be verified: $(log_entries)"
grep -qiE 'verif|could not' <<<"$(alert_entries)" ||
  fail "the alert does not say the marketplace source could not be verified: $(alert_entries)"

# ── 22. Containment is read from ~/.claude/settings.json (user scope), NOT the
#        inventory's effective `enabled` (F17). The effective field is merged, so
#        a project that re-enables a user-disabled plugin makes the inventory
#        report enabled=true when the CLI runs there. Make the inventory say
#        contained is enabled while settings.json keeps it disabled: the updater
#        must STILL skip it, or a hand run from such a project would update the
#        very plugin the operator contained. ─────────────────────────────────────
reset_state
jq 'map(if .id == "contained@mkt-a" then .enabled = true else . end)' "$PLUGIN_STATE" >"$PLUGIN_STATE.tmp" &&
  mv "$PLUGIN_STATE.tmp" "$PLUGIN_STATE"
run_helper --scheduled
refute '^plugin update contained@mkt-a$' "$(cat "$CLAUDE_CALL_LOG")" \
  "the updater trusted the inventory's effective enabled and updated a user-disabled plugin, breaching containment: $(cat "$CLAUDE_CALL_LOG")"
grep -qF 'contained@mkt-a' <<<"$(log_entries)" ||
  fail "the record does not name the contained plugin as skipped: $(log_entries)"

# ── 23. A record MISSING `enabled` cannot sneak an update (F18a). Containment is
#        decided by settings.json, so the inventory's enabled field is irrelevant:
#        a record with no enabled is contained by settings, not defaulted to
#        enabled-and-updated as the old inventory read did. ──────────────────────
reset_state
jq 'map(if .id == "contained@mkt-a" then del(.enabled) else . end)' "$PLUGIN_STATE" >"$PLUGIN_STATE.tmp" &&
  mv "$PLUGIN_STATE.tmp" "$PLUGIN_STATE"
run_helper --scheduled
refute '^plugin update contained@mkt-a$' "$(cat "$CLAUDE_CALL_LOG")" \
  "a record missing the enabled field was updated, overwriting a contained plugin: $(cat "$CLAUDE_CALL_LOG")"

# ── 23b. A MALFORMED-but-readable settings.json attempts NOTHING (F39), and is
#         DISTINGUISHED from a genuinely all-disabled one. Containment is read from
#         enabledPlugins; the old `jq ... || true` read a corrupt file as an empty
#         enabled set, so every plugin was skipped and the week reported a clean
#         all-disabled no-op for a settings file this run could not actually read.
#         A file that does not parse must refuse and alert; a file that parses to an
#         empty enabledPlugins is a real all-disabled week and must still run clean.
reset_state
printf 'this is not valid json {{{\n' >"$HOME/.claude/settings.json"
run_helper --scheduled
[[ $RUN_RC -ne 0 ]] || fail "a malformed settings.json exited 0 instead of attempting nothing: $RUN_OUTPUT"
refute '^plugin (update|install) ' "$(cat "$CLAUDE_CALL_LOG")" \
  "a malformed settings.json still mutated plugins without knowing which were contained: $(cat "$CLAUDE_CALL_LOG")"
entries="$(log_entries)"
grep -qF -- '--state deferred' <<<"$entries" ||
  fail "a malformed settings.json did not record a deferral: $entries"
grep -qF 'settings' <<<"$entries" ||
  fail "the record does not name the settings file it could not read: $entries"
[[ -n "$(alert_entries)" ]] || fail "a malformed settings.json sent no alert: $(cat "$RELAY_LOG")"
[[ ! -e $MARKER ]] || fail "a run that attempted nothing recorded a successful run"
# ...and a genuinely all-disabled (valid, empty enabledPlugins) file is NOT a
# refusal: it runs clean, every installed plugin skipped as disabled.
reset_state
printf '{"enabledPlugins":{}}\n' >"$HOME/.claude/settings.json"
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "a genuinely all-disabled settings.json was refused like a malformed one (rc=$RUN_RC): $RUN_OUTPUT"
refute '^plugin update ' "$(cat "$CLAUDE_CALL_LOG")" \
  "an all-disabled settings.json still updated a plugin: $(cat "$CLAUDE_CALL_LOG")"
grep -qF -- '--state completed' <<<"$(log_entries)" ||
  fail "a genuinely all-disabled week did not complete: $(log_entries)"

# ── 24. A plugin installed only at PROJECT scope is not the USER installation
#        this job manages (F24). It is treated as absent and the user copy is
#        INSTALLED, instead of a user-scope update that fails against a copy that
#        is not there and never installs the user one. ───────────────────────────
reset_state
jq 'map(if .id == "steady@mkt-a" then .scope = "project" else . end)' "$PLUGIN_STATE" >"$PLUGIN_STATE.tmp" &&
  mv "$PLUGIN_STATE.tmp" "$PLUGIN_STATE"
run_helper --scheduled
grep -qxF 'plugin install steady@mkt-a' "$CLAUDE_CALL_LOG" ||
  fail "a project-only plugin was not installed at user scope: $(cat "$CLAUDE_CALL_LOG")"
refute '^plugin update steady@mkt-a$' "$(cat "$CLAUDE_CALL_LOG")" \
  "a project-only plugin was updated (a user-scope update against a copy that is not there) instead of installed: $(cat "$CLAUDE_CALL_LOG")"

printf 'update-agent-plugins-record: OK (a scheduled run records its class, host, run timestamp and gap; a disabled plugin is skipped by CALL and named, an untracked one is untouched, an absent one is installed after its marketplace is added; the unknowable lane reports refreshed-change-unknowable and never changed while both knowable lanes name their transition; a failed update alerts the priority route and does not consume the success marker; an inventory this run could not read attempts NOTHING and says so on both routes, while a failed AFTER reading still completes and says NOT COMPARED; a manual run records nothing, one record per week, a refused record retries and alerts once; the idle gate defers on live Claude activity and proceeds on a stale machine; lock contention defers; an unknown argument is an error)\n'
