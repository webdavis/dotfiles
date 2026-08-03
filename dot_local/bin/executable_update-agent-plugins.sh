#!/usr/bin/env bash
#
# update-agent-plugins.sh -- the weekly, unattended Claude Code plugin updater,
# run by the com.webdavis.update-agent-plugins LaunchAgent across 24 hourly
# Monday slots. It reads dot_agents/custom-agent-plugins-lock.json (applied to
# ~/.agents/), installs any tracked plugin that is absent, runs
# `claude plugin update <id>` on the rest, and posts one record per ISO week to
# the #unattended-upgrades channel through unattended-log-lib.sh.
#
# THIS IS A SEPARATE VERTICAL FROM SKILLS, by operator ruling: it shares no lock,
# no LaunchAgent and no test with update-skills.sh, so neither one's deferral
# logic can govern the other. The two do share unattended-log-lib.sh, which is
# about how an unattended job REPORTS and knows nothing about either subject.
#
# WHAT IT DOES NOT DO, stated here because a design that hides its limits reads
# as a stronger guarantee than it is:
#
#   * It does not classify WHAT changed in an update, and it does not escalate.
#     A digest over the installed tree is not stable across a NO-OP update
#     (runtime files churn inside the tree, hooks/ included), an MCP-server
#     plugin's executed code is not in the tree at all, and an escalation that
#     fires on essentially every real release is a channel nobody reads. The
#     security cost of updating third-party executable code unattended is
#     ACCEPTED, deliberately, because the alternative (pin, watch for drift,
#     update by hand) depends on the operator noticing a passive alert, and a
#     design that depends on that has already failed.
#   * It cannot roll back. `claude plugin update` overwrites the installed tree
#     in place, neither `update` nor `install` takes a --version, and Claude
#     Code sweeps its own plugin cache on a schedule this repo does not control.
#     The recovery verb is `claude plugin disable <id>`, which sticks against an
#     apply because the settings modify-template preserves a live `false` for a
#     declared id. That is narrower than it sounds: the disable stops the plugin
#     loading from USER settings, and a repo whose PROJECT settings enable the
#     same plugin still loads it there (precedence: user < project < local <
#     flag < policy).
#   * It never uninstalls and never removes a marketplace. Removal is a manual
#     act here, by operator ruling.
#
# Usage: update-agent-plugins.sh [--scheduled]
#   --scheduled  marks this as the LaunchAgent's run. ONLY a scheduled run posts
#                a weekly record, mirroring update-skills.sh and
#                homebrew-weekly-upgrade.sh: without the marker, a hand run on a
#                Wednesday would post a weekly entry and a dead LaunchAgent
#                would look alive, inverting the one signal the record carries.
#                Failures alert either way.
#
# Exit codes: 0 clean, 1 something failed or nothing could be attempted,
#             2 bad usage, 75 (EX_TEMPFAIL) deferred, retry on a later slot.
set -uo pipefail

CLAUDE_CLI="${UPDATE_AGENT_PLUGINS_CLAUDE:-claude}"
AGENT_PLUGINS_LOCK="${UPDATE_AGENT_PLUGINS_LOCK_PATH:-$HOME/.agents/custom-agent-plugins-lock.json}"
LOCKFILE="${UPDATE_AGENT_PLUGINS_LOCKFILE:-$HOME/.local/state/update-agent-plugins.lock}"
STATE_DIR="${UPDATE_AGENT_PLUGINS_STATE_DIR:-$HOME/.local/state/update-agent-plugins}"
LOG_SUCCESS_MARKER="$STATE_DIR/last-success-at"
LOG_WEEK_GUARD="$STATE_DIR/log-week-claims"
AGENT_NAME="update-agent-plugins"

# The relay script by ABSOLUTE path: the LaunchAgent's PATH does not carry
# ~/.local/bin, so a bare `relay.sh` would never be found under launchd and
# every alert would vanish exactly when it mattered.
RELAY="${UPDATE_AGENT_PLUGINS_RELAY:-$HOME/.local/bin/relay.sh}"

# THE IDLE GATE, and the tradeoff it makes. WRITTEN OUT because task #95 is what
# happens when one is inherited instead of decided: update-skills.sh's gate
# probes Claude, Codex AND hermes and has deferred every slot on this machine
# since it shipped, so its subject never updates at all.
#
# The hazard here is narrow and specific. `claude plugin update` mutates
# ~/.claude/plugins, and for a plugin whose declared version is the literal
# string "unknown" the install path never changes, so the update overwrites that
# tree IN PLACE (measured: a planted marker file was gone afterwards). A live
# Claude Code session reads plugin files from exactly there, and the CLI's own
# help says "restart required to apply". So a swap under a live session can hand
# a running turn half of one tree and half of another.
#
# Nothing else on this machine reads ~/.claude/plugins: it is Claude Code's own
# config directory, and Codex and hermes load their skills from ~/.agents. So
# Codex and hermes activity is evidence about a hazard that does not exist, and
# folding them in can only defer runs that were safe. Measured 2026-08-03 on
# this machine, in one reading: ~/.codex/sessions was last written 5123 seconds
# earlier and ~/.hermes/logs 704 seconds earlier, i.e. hermes alone would have
# deferred a slot the plugin hazard did not require deferring.
#
# This gate can still starve, and that is the accepted residue: a machine with a
# live Claude session in every one of the 24 Monday slots updates nothing. What
# makes that legible rather than silent is the record: a deferral posts an entry
# whose gap line reads "last successful run: ... (23d 0h ago)", so a starved gate
# announces itself in the newest message on the channel. That is precisely the
# signal #95's gate lacked.
CLAUDE_ACTIVITY_DIR="${UPDATE_AGENT_PLUGINS_ACTIVITY_DIR:-$HOME/.claude/projects}"
IDLE_THRESHOLD_SECONDS="${UPDATE_AGENT_PLUGINS_IDLE_THRESHOLD:-900}"
[[ $IDLE_THRESHOLD_SECONDS =~ ^[0-9]+$ ]] || IDLE_THRESHOLD_SECONDS=900
FORCE="${UPDATE_AGENT_PLUGINS_FORCE:-}"

# An unknown argument is an ERROR, never a silent fallthrough: a typo'd marker in
# the plist would otherwise run every week and quietly post nothing, which looks
# exactly like a dead LaunchAgent.
SCHEDULED=""
for arg in "$@"; do
  case "$arg" in
    --scheduled) SCHEDULED=1 ;;
    *)
      printf 'usage: update-agent-plugins.sh [--scheduled]\nupdate-agent-plugins: unknown argument: %s\n' "$arg" >&2
      exit 2
      ;;
  esac
done

# The weekly record. A missing library is LOUD and never fatal: updating matters
# more than bookkeeping, but a silently absent record is the invisibility the
# record exists to end.
UNATTENDED_LOG_LIB="$(dirname "${BASH_SOURCE[0]}")/unattended-log-lib.sh"
UNATTENDED_LOG_AVAILABLE=""
if [[ -r $UNATTENDED_LOG_LIB ]]; then
  # shellcheck source=dot_local/bin/unattended-log-lib.sh
  source "$UNATTENDED_LOG_LIB"
  UNATTENDED_LOG_AVAILABLE=1
else
  printf 'update-agent-plugins: WARNING %s is missing; no weekly record will be posted (run chezmoi apply)\n' \
    "$UNATTENDED_LOG_LIB" >&2
fi

# Captured at START-UP, from ONE clock reading, before anything can rewrite the
# marker: a gap read later would be this run's own timestamp.
LOG_ENTRY_HEADER=""
if [[ -n $UNATTENDED_LOG_AVAILABLE ]]; then
  LOG_ENTRY_HEADER="$(unattended_log_entry_header "$LOG_SUCCESS_MARKER")"
fi

# __agent_plugins_code <text> [max-characters] -- render third-party text as a
# Discord inline code span, capped.
#
# Plugin names come from this repo's lock, but a CLI error message does not: it
# is written by whoever published the plugin, and it lands in a channel whose
# whole value is that its contents read as trustworthy machine records.
# Unquoted, `[urgent: click here](https://evil.example)` renders as a CLICKABLE
# LINK the operator never authored. This is deliberately LOCAL rather than the
# library's own quoting helper: the alert path below runs whether or not the
# library loaded, and a sanitiser that is absent exactly when the library is
# missing would leave the one message that gets pushed to a phone unquoted.
__agent_plugins_code() {
  local text="${1//\`/}" limit="${2:-200}"
  text="$(printf '%s' "$text" | tr -d '[:cntrl:]')"
  [[ ${#text} -gt $limit ]] && text="${text:0:limit}..."
  # shellcheck disable=SC2016 # the backticks are Discord code-span syntax
  printf '`%s`' "$text"
}

# weekly_alert <state> <detail> -- the EXISTING relay route, so this lands in the
# priority channel beside every other alert on this machine. Best effort: a
# missing relay never fails the run, and a failure to notify is stated.
weekly_alert() {
  local state="$1" detail="$2"
  if [[ ! -x $RELAY ]]; then
    printf 'update-agent-plugins: relay.sh is not executable at %s; this alert was NOT delivered\n' "$RELAY" >&2
    return 0
  fi
  "$RELAY" --agent "$AGENT_NAME" --state "$state" \
    --project "$(unattended_log_host 2>/dev/null || printf 'unknown-host')" --detail "$detail" 9>&- || true
  return 0
}

# weekly_record <class> <body> -- the LOG route. Gated on --scheduled and on the
# weekly claim, which admits one entry per class per ISO week: one, or two in a
# week that defers before it completes. The claim is taken BEFORE the attempt so
# two overlapping slots cannot both post, and GIVEN BACK when the attempt failed
# so a week is never marked done with nothing sent.
weekly_record() {
  local class="$1" body="$2" detail
  [[ -n $SCHEDULED ]] || return 0
  [[ -n $UNATTENDED_LOG_AVAILABLE ]] || return 0
  if ! unattended_log_claim_week "$LOG_WEEK_GUARD" "$class"; then
    printf 'update-agent-plugins: this ISO week already has a %s-or-better record; not posting again\n' "$class"
    return 0
  fi
  detail="$(printf '%s\n%s' "$LOG_ENTRY_HEADER" "$body")"
  if ! UNATTENDED_LOG_RELAY="$RELAY" unattended_log_post "$AGENT_NAME" "$class" \
    "$(unattended_log_host)" "$detail"; then
    unattended_log_release_week "$LOG_WEEK_GUARD" "$class"
    printf 'update-agent-plugins: the weekly record was NOT delivered; this week stays unclaimed so a later run retries\n' >&2
    UNATTENDED_LOG_RELAY="$RELAY" unattended_log_alert_delivery_failure "$LOG_WEEK_GUARD" "$AGENT_NAME"
  fi
  return 0
}

# nothing_attempted <state> <what-failed> <detail> -- the one shape every path
# that could not even start shares: an ALERT (it will not fix itself) and a
# DEFERRED record (nothing was attempted, so it is not an upgrade failure), then
# exit 1. Silence here is indistinguishable from a clean week, which is the
# defect this whole record exists to end.
nothing_attempted() {
  local state="$1" what="$2" detail="$3"
  printf 'update-agent-plugins: %s\n' "$detail" >&2
  weekly_alert "$state" "$detail"
  weekly_record deferred "$(printf 'nothing was attempted: %s An alert was also raised on the priority route; that path is fire-and-forget, so its delivery was not observed.' "$detail")"
  printf 'update-agent-plugins: the failing input was %s\n' "$what" >&2
  exit 1
}

# Serialize: one plugin run at a time, via the KERNEL. The 24 Monday slots and an
# ad-hoc hand run must never run concurrent `claude plugin` mutations against the
# same ~/.claude/plugins. macOS ships /usr/bin/lockf (flock(2)-backed): open the
# lock on fd 9 and test-acquire non-blocking (exit 75 = EX_TEMPFAIL when another
# process holds it). The kernel releases it when the fd closes, so there is no
# stale-lock class. Non-darwin hosts proceed unlocked; the contending scheduled
# runs are darwin-only. (House precedent: update-skills.sh and
# homebrew-weekly-upgrade.sh use the same shape.)
acquire_lock() {
  [[ -x /usr/bin/lockf ]] || return 0
  mkdir -p "$(dirname "$LOCKFILE")" 2>/dev/null || return 1
  exec 9>>"$LOCKFILE" || return 1
  /usr/bin/lockf -s -t 0 9
}

# __agent_plugins_should_defer -- 0 = DEFER, 1 = PROCEED. See the gate rationale
# above. Fails CLOSED on an unreadable activity dir or a scan error: the cost of
# a wrong defer is one week, the cost of a wrong proceed is a corrupted live
# session.
#
# `find -newermt "-<n> seconds"` is the portable primitive here: BSD find (this
# host, and the CI runner) cannot parse `-newermt "@<epoch>"`, but BOTH BSD and
# the flake's GNU findutils accept the RELATIVE form (measured 2026-08-03 on
# both), so no cutoff sentinel file is needed. `-print -quit` stops at the first
# hit, so an active machine costs one stat.
__agent_plugins_should_defer() {
  local newer
  [[ -n $FORCE ]] && return 1
  [[ -d $CLAUDE_ACTIVITY_DIR ]] || return 1 # no transcripts here: nothing to protect
  [[ -r $CLAUDE_ACTIVITY_DIR && -x $CLAUDE_ACTIVITY_DIR ]] || return 0
  if ! newer="$(find "$CLAUDE_ACTIVITY_DIR" -type f -newermt "-$IDLE_THRESHOLD_SECONDS seconds" -print -quit 2>/dev/null)"; then
    return 0 # scan error, fail closed
  fi
  [[ -n $newer ]] && return 0
  return 1
}

# __agent_plugins_inventory <outfile> -- `claude plugin list --json` into a file.
# Non-zero when the command failed OR when what came back is not a JSON array:
# the record's other paths read this file, and a half-written or HTML-error body
# that happened to be saved would be parsed as "no plugins installed", which is
# a clean-looking answer to a question that was never answered.
__agent_plugins_inventory() {
  local out="$1"
  "$CLAUDE_CLI" plugin list --json >"$out" 2>/dev/null || return 1
  jq -e 'type == "array"' "$out" >/dev/null 2>&1 || return 1
  return 0
}

# __agent_plugins_versions <inventory> <lock> -- "<id><TAB><version>" lines for
# tracked plugins in a KNOWABLE identity lane, the input shape
# unattended_log_change_line reads.
#
# The unknowable lane is excluded here rather than filtered later, and that is
# the whole point: its declared version is the literal string "unknown", so
# every one of them would compare equal forever and read as a guarantee that
# nothing changed. What can be said about that lane is said in its own sentence.
# A record with NO id is SKIPPED, not fatal. Measured 2026-08-03 on jq 1.7:
# `null | in($tracked)` raises "Cannot check whether object has a null key", and
# because this function is a pipeline under `set -o pipefail` the snapshot then
# came back EMPTY. Both readings failing that way rendered the change line as
# "0 of 0 tracked entries changed", which is a clean week reported for a
# comparison that never ran. The `.id != null` guard has to come FIRST: jq's
# `and` short-circuits, so ordering it after the lookup does not help.
__agent_plugins_versions() {
  jq -r --slurpfile lockdoc "$2" '
    ($lockdoc[0].plugins // {}) as $tracked
    | map(select((.id != null)
        and (.id | in($tracked))
        and (($tracked[.id].identityLane // "") != "unknowable")))
    | .[] | [.id, (.version // "")] | @tsv' "$1" 2>/dev/null | sort
}

# ---------------------------------------------------------------------------
# TWO different facts, and they used to collapse into one elsewhere in this
# repo: `lockf` answers 75 when another process holds the lock, and anything
# else means this run could not even OPEN the file.
plugins_lock_rc=0
acquire_lock || plugins_lock_rc=$?
if [[ $plugins_lock_rc -eq 75 ]]; then
  printf 'update-agent-plugins: another run holds the lock; deferring (exit 75).\n' >&2
  weekly_record deferred "nothing was attempted: another update-agent-plugins run already holds the serialize lock, so this run deferred (exit 75)."
  exit 75
elif [[ $plugins_lock_rc -ne 0 ]]; then
  nothing_attempted lock-unavailable "$LOCKFILE" \
    "$(printf 'the serialize lock at %s could not be OPENED (rc %d), which is not another run holding it, so no plugin was touched. Check that the directory is writable.' "$LOCKFILE" "$plugins_lock_rc")"
fi

printf '=== update-agent-plugins %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# The lock file is the whole input. An absent one is not an empty roster: it
# means chezmoi has not applied, and a run that treated it as "nothing to do"
# would report a clean week forever on a machine where this vertical is dead.
if [[ ! -r $AGENT_PLUGINS_LOCK ]]; then
  nothing_attempted lock-file-missing "$AGENT_PLUGINS_LOCK" \
    "$(printf 'the plugin lock at %s is missing or unreadable, so this run has no idea what to update. This is what an unapplied chezmoi state looks like; run chezmoi apply.' "$AGENT_PLUGINS_LOCK")"
fi
if ! jq -e 'type == "object" and ((.plugins // {}) | type) == "object"' "$AGENT_PLUGINS_LOCK" >/dev/null 2>&1; then
  nothing_attempted lock-file-malformed "$AGENT_PLUGINS_LOCK" \
    "$(printf 'the plugin lock at %s does not parse as an object carrying a plugins map, so this run refused to guess what to update.' "$AGENT_PLUGINS_LOCK")"
fi

if __agent_plugins_should_defer; then
  printf 'update-agent-plugins: a Claude Code session wrote a transcript within the last %ss (or the probe failed); deferring (exit 75).\n' \
    "$IDLE_THRESHOLD_SECONDS" >&2
  weekly_record deferred "$(printf 'nothing was attempted: a Claude Code transcript under %s was written within the last %s seconds (or the probe could not be read, which fails closed), so this slot deferred rather than swapping a plugin tree under a live session. A later Monday slot retries; the gap line above is what a permanently deferring gate looks like.' \
    "$CLAUDE_ACTIVITY_DIR" "$IDLE_THRESHOLD_SECONDS")"
  exit 75
fi

workspace=""
if ! workspace="$(mktemp -d "${TMPDIR:-/tmp}/update-agent-plugins.XXXXXX" 2>/dev/null)" || [[ -z $workspace ]]; then
  nothing_attempted workspace-unavailable "mktemp -d" \
    'the run workspace could not be created (mktemp -d failed), so the plugin inventory could not even be read and nothing was touched.'
fi
trap 'rm -rf "$workspace"' EXIT

# THE INVENTORY IS A PREREQUISITE, not a nicety. It is where installed-ness and
# enabled-ness come from, and a run that cannot read it cannot honour the
# disabled-skip below: it would update the very plugin the operator contained,
# overwriting the tree the disable exists to preserve. So an unreadable
# inventory attempts NOTHING.
if ! __agent_plugins_inventory "$workspace/before.json"; then
  # shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
  nothing_attempted inventory-unreadable "claude plugin list --json" \
    'the installed-plugin inventory could not be read (`claude plugin list --json` failed or did not return a JSON array), so this run could not tell which plugins are present or which are DISABLED. It touched nothing rather than risk updating a plugin the operator deliberately disabled.'
fi

installed_ids="$(jq -r '.[].id // empty' "$workspace/before.json" 2>/dev/null)"
disabled_ids="$(jq -r '.[] | select(.enabled == false) | .id // empty' "$workspace/before.json" 2>/dev/null)"
before_ok=1
__agent_plugins_versions "$workspace/before.json" "$AGENT_PLUGINS_LOCK" >"$workspace/before.tsv" || before_ok=""

# __agent_plugins_marketplace_configured <name> -- 0 when the CLI already knows
# that marketplace. Asked rather than assumed: `marketplace add` on an existing
# name is behaviour this repo has not measured, and this vertical never removes
# a marketplace, so the safe move is to add only what is missing.
__agent_plugins_marketplace_configured() {
  "$CLAUDE_CLI" plugin marketplace list --json 2>/dev/null |
    jq -e --arg name "$1" 'any(.[]; .name == $name)' >/dev/null 2>&1
}

# Per-plugin outcomes, as ARRAYS (never space-joined strings): they are what
# makes the entry and the alert actionable.
refreshed_plugins=()
installed_plugins=()
skipped_plugins=()
failed_plugins=()
tracked_count=0
unknowable_plugins=()

# Read on fd 3: the loop body runs the `claude` CLI, and a command that consumes
# stdin would eat the rest of the plugin list.
while IFS=$'\t' read -r -u3 plugin_id marketplace lane; do
  [[ -n $plugin_id ]] || continue
  tracked_count=$((tracked_count + 1))

  if ! grep -qxF -- "$plugin_id" <<<"$installed_ids"; then
    # ABSENT: install it, adding its marketplace first when the CLI does not
    # know it. Without this the whole vertical merges and does nothing on a
    # fresh machine, which is the state every machine but this one is in.
    if ! __agent_plugins_marketplace_configured "$marketplace"; then
      marketplace_repo="$(jq -r --arg name "$marketplace" '.marketplaces[$name].repo // empty' "$AGENT_PLUGINS_LOCK" 2>/dev/null)"
      if [[ -z $marketplace_repo ]]; then
        failed_plugins+=("$plugin_id: its marketplace $marketplace has no repo in the lock, so it cannot be obtained")
        continue
      fi
      if ! marketplace_output="$("$CLAUDE_CLI" plugin marketplace add "$marketplace_repo" 2>&1)"; then
        failed_plugins+=("$plugin_id: marketplace add $marketplace_repo failed: $marketplace_output")
        continue
      fi
    fi
    if install_output="$("$CLAUDE_CLI" plugin install "$plugin_id" 2>&1)"; then
      installed_plugins+=("$plugin_id")
      printf '   installed: %s\n' "$plugin_id"
    else
      failed_plugins+=("$plugin_id: install failed: $install_output")
      printf '   FAILED to install: %s\n' "$plugin_id" >&2
    fi
    continue
  fi

  if grep -qxF -- "$plugin_id" <<<"$disabled_ids"; then
    # DISABLED: skipped, with the tradeoff stated. `claude plugin update` on a
    # disabled plugin PROCEEDS (exit 0) and overwrites the tree in place, so
    # updating it would destroy the exact artifact the operator contained, and
    # `disable` is the only recovery verb this vertical has. The cost of the
    # skip is that a contained plugin goes stale, which is what containment
    # means; re-enabling it puts it back in the weekly rotation.
    skipped_plugins+=("$plugin_id")
    printf '   skipped (disabled): %s\n' "$plugin_id"
    continue
  fi

  if update_output="$("$CLAUDE_CLI" plugin update "$plugin_id" 2>&1)"; then
    refreshed_plugins+=("$plugin_id")
    # Collected HERE and nowhere else: the unknowable sentence says those
    # plugins were REFRESHED, so it may only name ones this run actually
    # refreshed. Collecting them at the top of the loop counted the skipped and
    # the failed ones too, which is a claim about work that did not happen.
    [[ $lane == "unknowable" ]] && unknowable_plugins+=("$plugin_id")
    printf '   refreshed: %s\n' "$plugin_id"
  else
    failed_plugins+=("$plugin_id: update failed: $update_output")
    printf '   FAILED to update: %s\n' "$plugin_id" >&2
  fi
done 3< <(jq -r '.plugins // {} | to_entries[]
  | [.key, (.value.marketplace // ""), (.value.identityLane // "")] | @tsv' "$AGENT_PLUGINS_LOCK" 2>/dev/null | sort)

printf '=== done %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# The AFTER reading is a different fact from the before one: the updates already
# ran, so a failure here costs the COMPARISON, never the entry.
after_ok=1
__agent_plugins_inventory "$workspace/after-raw.json" || after_ok=""
if [[ -n $after_ok ]]; then
  __agent_plugins_versions "$workspace/after-raw.json" "$AGENT_PLUGINS_LOCK" >"$workspace/after.tsv"
else
  : >"$workspace/after.tsv"
fi

# ---------------------------------------------------------------------------
# The weekly RECORD, posted whether or not anything moved. A run that changed
# nothing is precisely where the gap figure is the only information the entry
# carries.
if [[ -n $UNATTENDED_LOG_AVAILABLE ]]; then
  record_lines=()
  record_lines+=("$(printf 'plugins: %d tracked -- %d refreshed, %d installed, %d skipped (disabled), %d failed.' \
    "$tracked_count" "${#refreshed_plugins[@]}" "${#installed_plugins[@]}" \
    "${#skipped_plugins[@]}" "${#failed_plugins[@]}")")

  if [[ ${#failed_plugins[@]} -gt 0 ]]; then
    failed_rendered=""
    for entry in "${failed_plugins[@]}"; do
      failed_rendered+="$(__agent_plugins_code "$entry"), "
    done
    record_lines+=("$(printf 'failed: %s' "${failed_rendered%, }")")
  fi
  if [[ ${#installed_plugins[@]} -gt 0 ]]; then
    installed_rendered=""
    for entry in "${installed_plugins[@]}"; do
      installed_rendered+="$(__agent_plugins_code "$entry"), "
    done
    record_lines+=("$(printf 'installed (they were absent): %s' "${installed_rendered%, }")")
  fi
  if [[ ${#skipped_plugins[@]} -gt 0 ]]; then
    skipped_rendered=""
    for entry in "${skipped_plugins[@]}"; do
      skipped_rendered+="$(__agent_plugins_code "$entry"), "
    done
    record_lines+=("$(printf 'skipped (disabled): %s. A disabled plugin is left alone on purpose: an update would proceed anyway and overwrite the tree, which is the artifact the disable exists to preserve.' \
      "${skipped_rendered%, }")")
  fi

  # ONE flag for the comparison, because half a comparison is not one: a
  # snapshot that failed on EITHER reading would otherwise be compared against a
  # good one and report the whole roster as added or removed.
  compare_ok="$after_ok"
  [[ -n $before_ok ]] || compare_ok=""
  record_lines+=("$(unattended_log_change_section "$compare_ok" \
    "$workspace/before.tsv" "$workspace/after.tsv" \
    'plugins with a knowable version' \
    'Versions are what claude plugin list --json reports for the tracked plugins that declare one, a release version or a commit sha. A plugin refreshed to the same version does not appear here.' \
    versions 'claude plugin list --json')")

  # The unknowable lane is derived from the OUTCOMES and the lock, never from a
  # snapshot: those plugins declare the literal version "unknown" and their
  # lastUpdated bumps on a no-op refresh, so both readings are noise. Saying
  # "refreshed, change unknowable" is the honest sentence; saying "changed"
  # would cry wolf every single week.
  if [[ ${#unknowable_plugins[@]} -eq 0 ]]; then
    record_lines+=('unknowable identity lane: no tracked plugin was refreshed in it, so every refresh above reports a real version.')
  else
    unknowable_rendered=""
    for entry in "${unknowable_plugins[@]}"; do
      unknowable_rendered+="$(__agent_plugins_code "$entry"), "
    done
    record_lines+=("$(printf 'unknowable identity lane: %d tracked plugin(s) refreshed, change unknowable (%s). Their declared version is the literal string "unknown" and their lastUpdated bumps on a no-op refresh, so this run cannot tell whether anything moved. The one-time remedy is a manual marketplace remove and re-add, which restamps them with git SHAs.' \
      "${#unknowable_plugins[@]}" "${unknowable_rendered%, }")")
  fi

  record_lines+=('Nothing above is live yet: claude plugin update says restart required to apply, so a running session keeps the code it started with until the next one begins.')

  weekly_record completed "$(printf '%s\n' "${record_lines[@]}")"
fi

if [[ ${#failed_plugins[@]} -gt 0 ]]; then
  printf '=== %d plugin(s) failed; see FAILED lines above ===\n' "${#failed_plugins[@]}" >&2
  alert_rendered=""
  for entry in "${failed_plugins[@]}"; do
    alert_rendered+="$(__agent_plugins_code "$entry"), "
  done
  weekly_alert plugin-update-failed \
    "$(printf 'The weekly agent-plugin run finished with %d failed plugin(s): %s. Those plugins are still at whatever version they held; the rest were refreshed. Full output: ~/.local/log/agent-plugins/update-agent-plugins.log' \
      "${#failed_plugins[@]}" "${alert_rendered%, }")"
  exit 1
fi

# A fully clean run is what "last successful run" has to mean, so the marker is
# written here and nowhere else. A failing run deliberately leaves it alone, so
# the gap keeps growing until a run actually succeeds. ONLY a scheduled run
# writes it: last-success-at is the dead-LaunchAgent gap figure the weekly record
# reports, and a hand run that advanced it would reset that gap and make a stalled
# LaunchAgent look alive, the exact inversion --scheduled exists to prevent.
[[ -n $SCHEDULED && -n $UNATTENDED_LOG_AVAILABLE ]] && unattended_log_mark_success "$LOG_SUCCESS_MARKER"
exit 0
