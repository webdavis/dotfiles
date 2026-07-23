#!/usr/bin/env bash
#
# tailscale-monitor.sh, polled every 60s by a launchd StartInterval agent. The
# public-exposure monitor: it reads `tailscale funnel status --json`, classifies
# whether a Funnel is exposing a local service to the PUBLIC internet, compares
# against the previous run's baseline, and pages CRIT only on a funnel turning ON
# (an off->on transition, a first-observation active funnel, or a monitoring gap).
# Silent in steady state and when a funnel is closed.
#
# Funnel traffic tunnels through tailscaled, so osquery cannot see it as a
# listening port; polling the CLI is the only way to catch it.
#
# The funnel signal is read from --json, not the human text: an active PUBLIC
# funnel is an AllowFunnel entry set true (tailscale ipn/serve.go, "AllowFunnel is
# the set of SNI:port values for which funnel traffic is allowed"), which a
# tailnet-only `serve` never sets. So a private serve does not false-page, and the
# read does not couple to the CLI's human wording.
#
# R2-5: a MONITORING GAP of a public-exposure detector is itself CRIT (a blind
# funnel monitor is an undetectable public-exposure risk). So a missing binary, a
# failed status command, empty output, or malformed JSON is NOT swallowed into a
# false "inactive": it pages once (via a .gap marker), preserves the prior valid
# funnel baseline, and never advances state on a blind read. Every page is a CRIT
# page (non-empty sound), so it reaches the remote #priority channel and pings.

set -euo pipefail

STATE="${OSQUERY_TAILSCALE_STATE:-$HOME/.local/state/osquery-tailscale-funnel.json}"
GAP="$STATE.gap"                 # page-once marker for a monitoring gap (R2-5)
PERSIST_GAP="$STATE.persist-gap" # page-once marker for a baseline-persist failure

# Resolution order: explicit override -> PATH (the headless homebrew formula this
# machine runs) -> the GUI-app path (the future tailscale-app cask). See CLAUDE.md
# the Tailscale section.
TAILSCALE="${OSQUERY_TAILSCALE_BIN:-$(command -v tailscale || echo /Applications/Tailscale.app/Contents/MacOS/Tailscale)}"

# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"

mkdir -p "$(dirname "$STATE")"

# Never leave a partial temp baseline behind, on any exit path.
trap 'rm -f "$STATE.tmp"' EXIT

CRIT_TITLE='🔴 **CRITICAL**'
# The BLIND-gap body shares one lead line + closer; the caller supplies the middle
# reason. Kept apostrophe-free for the alerting stack.
GAP_LEAD='**Tailscale funnel monitoring is BLIND - public-exposure paging is not running.**'
GAP_CLOSE='- A funnel could be opened to the PUBLIC internet without a page while this is blind. **Fix now.**'
# A literal backtick, so a command name renders as Discord inline-code. Built as a
# variable because shfmt -s would single-quote a no-expansion string and SC2016
# would then flag the bare backticks (the monolith hit this same conflict).
bt='`'

# write_state <funnel-value> -- atomically persist the baseline owner-only (0600),
# via a private temp file plus an atomic rename.
write_state() {
  (
    umask 077
    jq -cn --arg funnel "$1" '{funnel: $funnel}' >"$STATE.tmp"
  ) && mv -f "$STATE.tmp" "$STATE" && chmod 600 "$STATE"
}

# page_gap_once <marker> <body> -- the shared page-once-via-marker discipline for
# both the monitoring gap and the persist gap. If the marker is absent, send_alert
# FIRST (a CRIT page tier) and write the marker ONLY on success; return 0 when
# paged or already marked, nonzero when send_alert could not store the page (so a
# persisting condition re-pages). Best effort: a marker in an unwritable dir cannot
# be written, so the page may re-fire, acceptable for a serious ongoing fault.
page_gap_once() {
  local marker="$1" body="$2"
  if [[ -f $marker ]]; then
    return 0
  fi
  if send_alert CRIT "$CRIT_TITLE" "$body" "Sosumi"; then
    : >"$marker" 2>/dev/null || true
    return 0
  fi
  return 1
}

# gap_and_exit <reason-line> -- page the monitoring gap once (page-once via GAP)
# and exit. On a send_alert store-failure, write no marker and exit nonzero so the
# next tick re-pages. The body is static text plus the caller's reason: it never
# renders the raw (attacker-influenceable) CLI output.
gap_and_exit() {
  local body
  body="$GAP_LEAD"$'\n'"$1"$'\n'"$GAP_CLOSE"
  if ! page_gap_once "$GAP" "$body"; then
    printf 'tailscale-monitor: send_alert could not queue the monitoring-gap page; no marker written, retrying next tick\n' >&2
    exit 1
  fi
  exit 0
}

# persist_baseline <funnel-value> -- persist the baseline, making a persistence
# FAILURE loud rather than silent. A failed write cannot advance the baseline, so a
# stale baseline could later mask a real re-exposure (a stale prev=active reads the
# next real active as steady, silent, forever). Page a degraded-monitor gap ONCE
# via its own marker, then exit nonzero so launchd retries. On success clear the
# marker (recovery).
persist_baseline() {
  if write_state "$1"; then
    rm -f "$PERSIST_GAP" 2>/dev/null || true
    return 0
  fi
  local body
  body="**Tailscale funnel monitor degraded.**"$'\n'"- The funnel monitor could not persist its baseline: it cannot advance state, so a stale baseline could mask the next real public exposure and blind the monitor."$'\n'"- Check the state directory free space and permissions. **Check now.**"
  page_gap_once "$PERSIST_GAP" "$body" || true
  exit 1
}

# render_exposure <funnel-json> -- the public-exposure page body. The exposed
# SNI:port values come from AllowFunnel and are ATTACKER-INFLUENCEABLE (an attacker
# who opened the funnel controls the string), so each crosses into the body as
# INERT data through the same chokepoint render-page.sh uses: backticks stripped
# (they end an inline-code span), \r\n\t squashed to spaces (a newline also breaks
# the span and could forge a markdown line), then length-capped and wrapped in a
# Discord inline-code span. Display-only.
render_exposure() {
  local exposed
  exposed=$(printf '%s' "$1" | jq -r '
    def code:
      (gsub("`"; "") | gsub("[\r\n\t]"; " ")) as $v
      | "`" + (if ($v | length) > 200 then ($v[0:200] + "…(truncated)") else $v end) + "`";
    [.. | objects | .AllowFunnel // empty | to_entries[] | select(.value == true) | .key]
    | unique
    | if length == 0 then "`(unknown)`" else (map("- " + code) | join("\n")) end
  ' 2>/dev/null || printf '%s' '(unknown)')
  printf '%s\n' \
    "**Tailscale Funnel is exposing a local service to the PUBLIC internet.**" \
    "- Did you set this up? If not, close it now: **tailscale funnel reset**" \
    "- Exposed to the public internet:" \
    "$exposed"
}

# page_exposure_and_persist <funnel-json> -- emit one CRIT public-exposure page,
# then advance the baseline to active. Notify-before-persist: send_alert is
# write-ahead durable, so the baseline advances ONLY after the page is durably
# enqueued; on a send_alert store-failure the baseline is left as-is and the
# monitor exits nonzero, so a PERSISTING exposure re-pages next tick.
page_exposure_and_persist() {
  local body
  body=$(render_exposure "$1")
  if ! send_alert CRIT "$CRIT_TITLE" "$body" "Sosumi"; then
    printf 'tailscale-monitor: send_alert could not queue the funnel-exposure page; baseline not advanced, retrying next tick\n' >&2
    exit 1
  fi
  persist_baseline active
}

# Read the prior baseline. Only active/inactive is a valid funnel baseline; a
# corrupt/absent value is treated as no trustworthy baseline (R2-5b).
prev_funnel=""
if [[ -f $STATE ]]; then
  prev_funnel=$(jq -r '.funnel // empty' <"$STATE" 2>/dev/null || echo "")
fi
case "$prev_funnel" in
  active | inactive) ;;
  *) prev_funnel="" ;;
esac

# A missing binary is a monitoring gap (the regression that left this monitor dead
# on the formula install). CRIT so it reaches the remote channel, page-once. The
# body names the local binary PATH (operator/env-controlled, not network data).
if [[ ! -x $TAILSCALE ]]; then
  printf 'WARN: no tailscale binary (%s) - funnel monitoring is blind\n' "$TAILSCALE" >&2
  gap_and_exit "- No tailscale binary found at [$TAILSCALE]."
fi

# Run the status command, capturing its exit code (do NOT || true a failure into a
# false inactive). Bound it so a WEDGED tailscaled (the CLI blocks on the local API
# socket) becomes a monitoring gap, not silent blindness: without a bound, launchd
# skips ticks while the process lives and the monitor never pages. gtimeout
# preferred, timeout fallback (the codebase convention); if neither is on PATH,
# degrade to an unbounded read (no worse than before). The bound is well under the
# 60s tick and env-overridable. On timeout the CLI is killed (nonzero rc), which
# the gap gate below pages.
status_command=("$TAILSCALE" funnel status --json)
timeout_bin="$(command -v gtimeout || command -v timeout || true)"
if [[ -n $timeout_bin ]]; then
  status_command=("$timeout_bin" "${OSQUERY_TAILSCALE_TIMEOUT:-10}" "${status_command[@]}")
fi
rc=0
funnel_json=$("${status_command[@]}" 2>/dev/null) || rc=$?

if [[ $rc -ne 0 ]]; then
  gap_and_exit "- ${bt}tailscale funnel status --json${bt} exited $rc, so the funnel state is unreadable."
fi
if [[ -z $funnel_json ]]; then
  gap_and_exit "- ${bt}tailscale funnel status --json${bt} returned no output, so the funnel state is unreadable."
fi
# Malformed (non-JSON) output is a gap, not a silent inactive. jq empty validates
# JSON (any value, including {} and null) and fails only on a PARSE error.
if ! printf '%s' "$funnel_json" | jq empty >/dev/null 2>&1; then
  gap_and_exit "- ${bt}tailscale funnel status --json${bt} returned output that is not valid JSON, so the funnel state is unreadable."
fi

# Valid JSON: classify into active / inactive / gap. A PUBLIC funnel is active iff
# an AllowFunnel entry is boolean true at ANY depth (top-level, a Foreground
# session, or a Service), so a tailnet-only serve (Web/TCP without AllowFunnel) is
# correctly inactive. AllowFunnel is map[HostPort]bool: an entry value that is NOT a
# boolean, or an AllowFunnel that is not a map, is an UNEXPECTED shape and resolves
# to a gap (fail-safe), never a silent inactive that could miss a real exposure.
classification=$(printf '%s' "$funnel_json" | jq -r '
  [.. | objects | .AllowFunnel // empty] as $funnels
  | if ($funnels | any(type != "object")) then "gap"
    else ([$funnels[] | .[]?]) as $values
      | if ($values | any(type != "boolean")) then "gap"
        elif ($values | any(. == true)) then "active"
        else "inactive" end
    end
' 2>/dev/null || printf 'error')
case "$classification" in
  active) cur="active" ;;
  inactive) cur="inactive" ;;
  *)
    # "gap" (an unexpected AllowFunnel shape) or an unclassifiable/empty result: a
    # public-exposure detector treats an unreadable funnel state as a gap, never a
    # silent inactive (R2-5).
    gap_and_exit "- the funnel status JSON has an unexpected AllowFunnel shape, so the funnel state is unclassifiable and unreadable."
    ;;
esac

# A valid read cleared the gap (recovery): drop the marker so a future gap pages again.
rm -f "$GAP" 2>/dev/null || true

# Page on a fresh off->on transition, a first-observation active funnel (no prior
# baseline), or an active read on recovery from a blind window (prev was cleared).
# A steady active funnel (prev already active) is silent, and an on->off (funnel
# closed) is silent too. R2-5: an active reading after a gap MUST page, never be
# accepted as a silent new baseline.
if [[ $cur == "active" && $prev_funnel != "active" ]]; then
  page_exposure_and_persist "$funnel_json"
  exit 0
fi

# No transition to page: refresh the baseline (steady active, steady inactive, or a
# funnel just closed). A persistence failure here is a loud degraded-monitor gap.
persist_baseline "$cur"
