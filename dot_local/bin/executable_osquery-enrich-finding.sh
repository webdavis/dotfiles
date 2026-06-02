#!/usr/bin/env bash
#
# osquery-enrich-finding.sh — given a path drawn from an osquery finding (a
# launchd plist, an app/extension bundle, a Mach-O binary, or a watched config
# file), emit a SHORT single-line fact string to stdout describing its trust:
# code-signing authority, ad-hoc/unsigned state, and download (quarantine)
# origin. Read-only, no network — pure local inspection.
#
# The exit status is the machine signal the caller (osquery-results-alerter.sh)
# routes on:
#   0  — TRUSTED or not-applicable: a binary signed by Apple or a Developer ID,
#        a config file (signing N/A), or a launchd job whose payload is a script
#        run by a signed interpreter (surfaced with a note, but not escalated —
#        the user's own script LaunchAgents are this shape; the deferred
#        allowlist is what would let unknown ones escalate).
#   10 — UNTRUSTED / undeterminable code: unsigned, ad-hoc, signed-without-an-
#        authority, or a binary codesign cannot assess. Fail-LOUD — uncertainty
#        about *code* escalates (→ #priority), never silently trusts.
#
# Nothing is ever suppressed here; the caller always surfaces the finding. This
# only decides how LOUD it is. Usage:
#   facts=$(osquery-enrich-finding.sh "$path") || rc=$?

set -euo pipefail

path="${1:-}"
[ -n "$path" ] || exit 0

PLUTIL=/usr/bin/plutil
CODESIGN=/usr/bin/codesign

# Append ", downloaded" when the file carries a quarantine xattr (Gatekeeper's
# mark that it arrived via a browser/download, not the system). Best-effort.
quarantine_note() {
  local q
  q=$(xattr -p com.apple.quarantine "$1" 2>/dev/null) || return 0
  [ -n "$q" ] && printf ', downloaded'
  return 0
}

# Assess a Mach-O binary or bundle. Echoes a human label; returns 10 only for
# the untrusted cases (unsigned, ad-hoc, or signed with no authority) so they
# promote NOTICE -> CRIT. Returns 0 when a signing authority is present —
# Apple/Developer ID get a friendly label, any other named authority is shown
# verbatim ("signed: <authority>"). A present-but-unrecognized authority is NOT
# promoted (the authority still surfaces in the alert for the user to judge); a
# revoked/abused valid cert is out of scope for an offline codesign check.
assess_code() {
  local target="$1" out auth
  # codesign -dv exits non-zero on an unsigned object and prints to stderr.
  if ! out=$("$CODESIGN" -dv --verbose=2 "$target" 2>&1); then
    printf 'UNSIGNED'
    return 10
  fi
  if printf '%s' "$out" | grep -qi 'not signed'; then
    printf 'UNSIGNED'
    return 10
  fi
  if printf '%s' "$out" | grep -qi 'adhoc'; then
    printf 'ad-hoc signature (untrusted)'
    return 10
  fi
  auth=$(printf '%s\n' "$out" | awk -F= '/^Authority=/{print $2; exit}')
  if [ -z "$auth" ]; then
    printf 'signed, no authority (untrusted)'
    return 10
  fi
  case "$auth" in
    'Software Signing' | Apple*) printf 'signed: Apple' ;;
    'Developer ID Application: '*) printf 'signed: %s' "${auth#Developer ID Application: }" ;;
    *) printf 'signed: %s' "$auth" ;;
  esac
  return 0
}

# A launchd job whose program is one of these runs an attacker-controllable
# script payload — the signed interpreter tells us nothing about the script.
is_interpreter() {
  case "$(basename "$1")" in
    sh | bash | zsh | dash | ksh | python | python2 | python3 | perl | ruby | node | osascript | php | env) return 0 ;;
    *) return 1 ;;
  esac
}

case "$path" in
  *.plist)
    # launchd job: resolve the payload (Program, else ProgramArguments[0]).
    prog=$("$PLUTIL" -extract Program raw -o - "$path" 2>/dev/null) || prog=""
    [ -n "$prog" ] || prog=$("$PLUTIL" -extract ProgramArguments.0 raw -o - "$path" 2>/dev/null) || prog=""
    if [ -z "$prog" ]; then
      printf 'launchd job, no program resolved (untrusted)'
      exit 10
    fi
    if is_interpreter "$prog"; then
      # Find the first existing absolute-path argument — the script payload.
      script=""
      for i in 1 2 3 4 5; do
        a=$("$PLUTIL" -extract "ProgramArguments.$i" raw -o - "$path" 2>/dev/null) || a=""
        [ -n "$a" ] || continue
        case "$a" in
          /*) [ -f "$a" ] && {
            script="$a"
            break
          } ;;
        esac
      done
      if [ -n "$script" ]; then
        printf 'runs script %s via %s — payload unverified%s' \
          "$(basename "$script")" "$(basename "$prog")" "$(quarantine_note "$script")"
      else
        printf 'runs %s (interpreter) — payload unverified' "$(basename "$prog")"
      fi
      exit 0
    fi
    label=$(assess_code "$prog") && rc=0 || rc=$?
    printf '%s%s' "$label" "$(quarantine_note "$prog")"
    exit "$rc"
    ;;
  *.app | *.kext | *.systemextension | *.dext | *.appex)
    label=$(assess_code "$path") && rc=0 || rc=$?
    printf '%s%s' "$label" "$(quarantine_note "$path")"
    exit "$rc"
    ;;
  *)
    # A Mach-O binary (e.g. an unexpected setuid file) → assess it. Anything
    # else (sshd_config, sudoers, ssh keys) is not code → stat context only,
    # no escalation (those queries already classify their own severity).
    if [ -f "$path" ] && file "$path" 2>/dev/null | grep -qi 'mach-o'; then
      label=$(assess_code "$path") && rc=0 || rc=$?
      printf '%s%s' "$label" "$(quarantine_note "$path")"
      exit "$rc"
    fi
    if [ -e "$path" ]; then
      meta=$(stat -f 'owner %Su, mode %Sp, modified %Sm' -t '%Y-%m-%dT%H:%M:%SZ' "$path" 2>/dev/null) || meta=""
      [ -n "$meta" ] && printf '%s' "$meta"
    fi
    exit 0
    ;;
esac
