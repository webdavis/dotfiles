#!/usr/bin/env bash
#
# firewall-gatekeeper-monitor.sh, polled every 60s by a launchd StartInterval
# agent. The security-posture monitor: it reads the live firewall (alf),
# Gatekeeper, AND screen-lock state via osqueryi in the gui/501 user session,
# plus every control declared in posture-controls.json (the chezmoi render of
# .chezmoidata/macos_posture_controls.yaml: FileVault, SIP, automatic login,
# the Guest account, and whatever later slices declare), compares against the
# previous run's baseline, and pages CRIT only on a protection turning OFF or
# a declared control deviating from its declared value. Silent in steady state.
#
# R2-3: screen-lock-off detection lives HERE, not in the root-daemon pack. The
# screenlock osquery table is scoped to the logged-in user, so the ROOT osqueryd
# daemon (no user session) never returns a screenlock row (the pack's screenlock
# queries were dead). This poller runs as a gui/501 user LaunchAgent whose
# osqueryi DOES have the user session, so it is the correct place to read it.

set -euo pipefail

STATE="${OSQUERY_POSTURE_STATE:-$HOME/.local/state/osquery-posture-state.json}"
GAP="$STATE.gap"                 # page-once marker for a monitoring gap (R2-9)
PERSIST_GAP="$STATE.persist-gap" # page-once marker for a baseline-persist failure
OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"
# The declared posture controls, rendered by chezmoi from
# .chezmoidata/macos_posture_controls.yaml. The poller reads the FILE rather
# than carrying the control list in its body, so adding a control is a data
# change. It lives beside this script so the pipeline-integrity watch and the
# known-good manifest cover it: the file decides WHAT gets monitored, so it is
# part of the monitor's body.
CONTROLS_FILE="${OSQUERY_POSTURE_CONTROLS:-$HOME/.local/libexec/osquery/posture-controls.json}"
# Absolute probe paths by default: the LaunchAgent PATH is minimal, and a
# status probe that silently resolved to something unexpected would be an
# untrustworthy read. The env overrides are the test seam.
FDESETUP="${OSQUERY_POSTURE_FDESETUP:-/usr/bin/fdesetup}"
CSRUTIL="${OSQUERY_POSTURE_CSRUTIL:-/usr/bin/csrutil}"
SYSADMINCTL="${OSQUERY_POSTURE_SYSADMINCTL:-/usr/sbin/sysadminctl}"
DEFAULTS="${OSQUERY_POSTURE_DEFAULTS:-/usr/bin/defaults}"
PGREP="${OSQUERY_POSTURE_PGREP:-/usr/bin/pgrep}"
PLUTIL="${OSQUERY_POSTURE_PLUTIL:-/usr/bin/plutil}"
READLINK="${OSQUERY_POSTURE_READLINK:-/usr/bin/readlink}"
# LuLu's rules archive, world-readable (0644 root:wheel, verified 2026-07-27),
# so this user-agent poller reads it unprivileged. READ-ONLY: the conversion
# below goes to stdout; the archive file itself is never written.
LULU_RULES_FILE="${OSQUERY_POSTURE_LULU_RULES:-/Library/Objective-See/LuLu/rules.plist}"
# LuLu's BASE preferences file, read (read-only, the same conversion-to-stdout
# discipline) by the active-profile guard below: when this file's
# currentProfile key names a profile, LuLu consults the profile's own
# preferences and rules instead of the base files.
LULU_PREFERENCES_FILE="${OSQUERY_POSTURE_LULU_PREFERENCES:-/Library/Objective-See/LuLu/preferences.plist}"

# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"

mkdir -p "$(dirname "$STATE")"

# Never leave a partial temp baseline behind, on any exit path (a mid-write
# failure, or the empty-read guard below).
trap 'rm -f "$STATE.tmp"' EXIT

# sanitize <text> -- neutralize a value before it reaches a notification body:
# newlines/CR/tabs flatten to spaces; backslash, backtick, dollar, and both
# quote characters are removed (apostrophes because the downstream render jq is
# bash single-quoted); the result is length-capped. System-read text is DATA,
# never structure. The normalized enum values a reader returns bypass this only
# because they are this script's own fixed constants, never probe output.
sanitize() {
  local text="${1//$'\n'/ }"
  text=${text//$'\r'/ }
  text=${text//$'\t'/ }
  text=${text//\\/}
  text=${text//\`/}
  text=${text//\$/}
  text=${text//\'/}
  text=${text//\"/}
  printf '%s' "${text:0:160}"
}

# sanitize_span <text> -- sanitize, then wrap in a Discord inline-code span:
# the same chokepoint treatment render-page.sh gives attacker-influenceable
# fields (backticks stripped so the span cannot be broken out of, newlines and
# tabs squashed, length-capped, wrapped in backticks). Character-stripping
# alone leaves markdown STRUCTURE intact -- emphasis, [links](...), @mentions
# survive sanitize as plain characters -- so every value that crosses into a
# notification body goes through here and renders inert inside the span.
sanitize_span() {
  # shellcheck disable=SC2016 # literal Discord inline-code backticks, no expansion intended
  printf '`%s`' "$(sanitize "$1")"
}

# Bound every probe so a WEDGED tool (a hung table or a stuck status call never
# closing stdout) becomes a monitoring gap, not silent blindness. Without a
# deadline, `|| true` handles an EXIT but not a HANG, launchd skips ticks while
# the process lives, and the uptime-watchdog cannot catch it (its probe queries
# a different table that answers while a posture read hangs). gtimeout
# preferred, timeout fallback (the codebase convention); if neither is on PATH,
# degrade to an unbounded read (no worse than before; the darwin fleet has
# one). The bound is PER PROBE (20s default, env-overridable): a tick running
# the combined query plus four control probes all wedged can total ~100s, past
# the 60s interval, in which case launchd (which never overlaps runs) simply
# starts the next tick late. The bound's job is to guarantee the tick ENDS and
# the gap gate pages, not to keep a worst-case tick under the interval. On
# timeout the probe is killed and exits nonzero, which classifies as
# indeterminate (or, for the combined osqueryi read, collapses to empty), and
# the gap gate pages.
posture_timeout_bin="$(command -v gtimeout || command -v timeout || true)"
run_bounded() {
  if [[ -n $posture_timeout_bin ]]; then
    "$posture_timeout_bin" "${OSQUERY_POSTURE_TIMEOUT:-20}" "$@"
  else
    "$@"
  fi
}

# Read the built-in posture trio in a single combined query (one osqueryi
# startup per tick, not one per protection). screenlock is folded in per R2-3.
# The exit status is captured, never erased: a FAILED osqueryi can still print
# healthy-looking JSON, and believing it would advance the baseline on
# untrustworthy data. Any nonzero exit, like any parse failure, empties the
# read, so the whole trio routes to the monitoring-gap gate below -- the same
# indeterminate-on-nonzero discipline the declared-control readers follow.
posture_rc=0
posture_raw=$(run_bounded "$OSQUERYI" --json "
  SELECT
    (SELECT global_state FROM alf) AS firewall,
    (SELECT assessments_enabled FROM gatekeeper) AS gatekeeper,
    (SELECT enabled FROM screenlock) AS screenlock
" 2>/dev/null) || posture_rc=$?
posture=""
if [[ $posture_rc -eq 0 ]]; then
  posture=$(jq -c '.[0] // empty' <<<"$posture_raw" 2>/dev/null) || posture=""
fi

cur_fw=$(jq -r '.firewall // empty' <<<"$posture" 2>/dev/null || echo "")
cur_gk=$(jq -r '.gatekeeper // empty' <<<"$posture" 2>/dev/null || echo "")
cur_sl=$(jq -r '.screenlock // empty' <<<"$posture" 2>/dev/null || echo "")

# ---- declared posture controls ----------------------------------------------

# reader_domain <reader> -> the reader's space-separated value domain, empty
# for an unknown reader. This table, the template's $readerDomains, and
# read_control below name the same reader set; the render test and the
# poller-vs-data agreement test hold them together.
reader_domain() {
  case "$1" in
    fdesetup_status | defaults_autologin) printf 'on off' ;;
    csrutil_status | sysadminctl_guest) printf 'enabled disabled' ;;
    pgrep_oversight | pgrep_lulu_extension) printf 'running stopped' ;;
    lulu_rule_present | lulu_rule_resolved_present) printf 'present absent' ;;
  esac
}

# reader_requires_target <reader> -- status 0 when the reader consumes a
# per-record target (the absolute binary path whose LuLu rule must exist).
# This table, the template's $readerTargets, and read_control below must
# agree; the render test and the agreement test hold them together.
reader_requires_target() {
  case "$1" in
    lulu_rule_present | lulu_rule_resolved_present) return 0 ;;
    *) return 1 ;;
  esac
}

# reader_reads_lulu_base_rules <reader> -- status 0 when the reader consults
# LuLu's BASE rules archive and therefore depends on no LuLu profile being
# active (see the profile guard below). Coincides with the target-requiring
# set today, but the two predicates answer different questions and must be
# free to diverge.
reader_reads_lulu_base_rules() {
  case "$1" in
    lulu_rule_present | lulu_rule_resolved_present) return 0 ;;
    *) return 1 ;;
  esac
}

# classify_probe <output> <rc> <needle> <value> [<needle> <value>]...
# The indeterminate-on-nonzero discipline (absorbed from the retired
# apply-time posture reminder): a probe that exits NONZERO is INDETERMINATE
# regardless of what it printed, because a failed probe's output is
# untrustworthy; it may print healthy-looking text yet have failed. A
# zero-exit probe's needle hits must all agree on EXACTLY ONE value; no hit,
# or hits mapping to conflicting values, is indeterminate too (an ambiguous
# probe is never guessed at). Needles are exact message forms enumerated from
# each binary's strings, so no needle is a substring of a different value's
# form.
classify_probe() {
  local output="$1" rc="$2"
  local needle value matched=""
  shift 2
  if [[ $rc -ne 0 ]]; then
    printf 'indeterminate'
    return 0
  fi
  while [[ $# -ge 2 ]]; do
    needle="$1"
    value="$2"
    shift 2
    if [[ $output == *"$needle"* ]]; then
      if [[ -z $matched ]]; then
        matched="$value"
      elif [[ $matched != "$value" ]]; then
        matched="conflict:" # ':' cannot appear in a domain value
      fi
    fi
  done
  if [[ -n $matched && $matched != "conflict:" ]]; then
    printf '%s' "$matched"
  else
    printf 'indeterminate'
  fi
}

# classify_pgrep <output> <rc> -- the shared classification for a pgrep
# process probe (documented statuses: 0 matched, 1 no match, 2 invalid
# options), with the status/output pairing symmetric: exit 0 with a
# WELL-FORMED pid list on stdout (digits, one pid per line, pgrep's only
# success output) is running; the no-match exit 1 with no output at all is
# stopped; every other outcome (usage or internal error, a timeout kill, a
# status/output mismatch in either direction) is indeterminate. A probe whose
# status and output disagree is never believed, whatever it printed.
classify_pgrep() {
  local output="$1" rc="$2"
  local pid_list_pattern=$'^[0-9]+(\n[0-9]+)*$'
  if [[ $rc -eq 0 && $output =~ $pid_list_pattern ]]; then
    printf 'running'
  elif [[ $rc -eq 1 && -z $output ]]; then
    printf 'stopped'
  else
    printf 'indeterminate'
  fi
}

# The active-profile guard. LuLu 4.x reads its preferences AND rules from an
# ACTIVE PROFILE directory when the base preferences file names one
# (Preferences.m getCurrentProfile reads the currentProfile key from the BASE
# file; Rules.m getPath prefers <currentProfile>/rules.plist). The rule
# readers below read the BASE archive, so while a profile is active they
# would be reading a file LuLu no longer consults: a stale base mention must
# never satisfy a control whose deciding rule set lives elsewhere. The guard
# converts the base preferences to stdout ONCE per tick, lazily, and any
# outcome other than a readable document with NO currentProfile key makes
# every lulu_rule read indeterminate (which the gap gate pages, naming the
# cause). Key PRESENCE is the trigger, fail-closed: nothing establishes what
# an empty currentProfile would mean, so only its absence is trusted.
lulu_profile_checked=0
lulu_profile_state="no_profile"
lulu_base_rules_authoritative() {
  if [[ $lulu_profile_checked -eq 0 ]]; then
    lulu_profile_checked=1
    local preferences_xml="" rc=0
    preferences_xml=$(run_bounded "$PLUTIL" -convert xml1 -o - "$LULU_PREFERENCES_FILE" 2>/dev/null) || rc=$?
    if [[ $rc -ne 0 || -z $preferences_xml ]]; then
      lulu_profile_state="indeterminate"
    elif grep -qF '<key>currentProfile</key>' <<<"$preferences_xml"; then
      lulu_profile_state="profile_active"
    else
      lulu_profile_state="no_profile"
    fi
  fi
  [[ $lulu_profile_state == "no_profile" ]]
}

# lulu_rule_in_archive <absolute-binary-path> -- the EXISTENCE-ONLY LuLu rule
# check: present when LuLu's rules archive mentions the path, absent when a
# readable archive does not, indeterminate when the archive cannot be read.
# The archive is an NSKeyedArchiver archive of LuLu's private Rule class, so
# this deliberately claims no more than "a rule mentioning this binary
# exists": the rule's action (allow vs block) is not recoverable without
# reimplementing the private keyed-archive layout, and this check must not
# pretend it did. The read is `plutil -convert xml1 -o -` (conversion to
# stdout, the file is never written), and the match is the exact <string>
# element in its XML-escaped form, so a longer path containing the target can
# never satisfy it. A zero-exit conversion that printed nothing is a
# status/output mismatch and stays indeterminate (plutil always prints the
# converted document on success), never absent: absent is a claim about a
# document that was actually read.
lulu_rule_in_archive() {
  local binary_path="$1" archive_xml="" rc=0 escaped_path
  archive_xml=$(run_bounded "$PLUTIL" -convert xml1 -o - "$LULU_RULES_FILE" 2>/dev/null) || rc=$?
  if [[ $rc -ne 0 || -z $archive_xml ]]; then
    printf 'indeterminate'
    return 0
  fi
  escaped_path=${binary_path//&/&amp;}
  escaped_path=${escaped_path//</&lt;}
  escaped_path=${escaped_path//>/&gt;}
  if grep -qF "<string>$escaped_path</string>" <<<"$archive_xml"; then
    printf 'present'
  else
    printf 'absent'
  fi
}

# read_control <reader> <target> -> the normalized value, or "indeterminate".
# One read-only status invocation of the authoritative tool per control,
# bounded like the combined query, with stdout and stderr captured together
# (sysadminctl and defaults report on stderr; verified on the target machine).
# The target argument is consumed only by the lulu_rule readers (load_controls
# validates the pairing). Raw probe text NEVER leaves this function: only the
# fixed normalized values do.
#
# Reader choices (each verified from the poller's gui/501 session context):
#   - fdesetup, NOT osquery's disk_encryption table: FileVault lives on the
#     APFS data volumes, so the obvious /dev/disk0 query reports 0 on a
#     FileVault-on machine, a false negative that would page a healthy system.
#   - csrutil: agrees with osquery's sip_config; the authoritative tool needs
#     no osqueryi startup.
#   - defaults read of loginwindow's autoLoginUser for automatic login: the
#     control means "auto-login is not DECLARED", so the reader asks the
#     declaration itself. `sysadminctl -autologin status` was rejected: it
#     reports the EFFECTIVE state and prints "Automatic login is disabled
#     because FileVault is enabled." (in the binary's message strings) even
#     when autoLoginUser IS set, so a configured auto-login would read healthy
#     until FileVault went off and it silently activated.
#   - sysadminctl for the Guest account: it reports the state on stderr and
#     exits 0 in both states.
read_control() {
  local reader="$1" target="${2:-}" output="" rc=0
  case "$reader" in
    fdesetup_status)
      # Every zero-exit status form enumerated from the binary's strings
      # (macOS 26.2): the plain On./Off. pair plus the restart-transition
      # variants. "Off, but will be enabled after the next restart." is OFF:
      # deferred enablement means the data is not yet encrypted, a real
      # exposure until the restart completes; "On, but needs to be restarted
      # to finish." reports the protection on. Error-prefixed forms exit
      # nonzero and stay indeterminate.
      output=$(run_bounded "$FDESETUP" status 2>&1) || rc=$?
      classify_probe "$output" "$rc" \
        "FileVault is On." on \
        "FileVault is On, but needs to be restarted to finish." on \
        "FileVault is Off." off \
        "FileVault is Off, but will be enabled after the next restart." off \
        "FileVault is Off, but needs to be restarted to finish." off
      ;;
    csrutil_status)
      output=$(run_bounded "$CSRUTIL" status 2>&1) || rc=$?
      classify_probe "$output" "$rc" \
        "System Integrity Protection status: enabled." enabled \
        "System Integrity Protection status: disabled." disabled
      ;;
    defaults_autologin)
      # Declared intent, three outcomes. Exit 0 means the autoLoginUser key
      # EXISTS: a user is declared for automatic login (on), whatever the
      # value. `defaults read` exits nonzero BOTH for an absent key (the
      # healthy state) and for a hard read failure, so only the canonical
      # does-not-exist diagnostic (which also covers a wholly absent
      # loginwindow domain: nothing declared either way) maps to off; any
      # other nonzero, a timeout kill included, is indeterminate. Absent is
      # thereby distinguished from unreadable, never conflated.
      output=$(run_bounded "$DEFAULTS" read /Library/Preferences/com.apple.loginwindow autoLoginUser 2>&1) || rc=$?
      if [[ $rc -eq 0 ]]; then
        printf 'on'
      elif [[ $output == *"autoLoginUser) does not exist"* ]]; then
        printf 'off'
      else
        printf 'indeterminate'
      fi
      ;;
    sysadminctl_guest)
      output=$(run_bounded "$SYSADMINCTL" -guestAccount status 2>&1) || rc=$?
      classify_probe "$output" "$rc" "Guest account enabled." enabled "Guest account disabled." disabled
      ;;
    pgrep_oversight)
      # A live process-table read: OverSight's monitor is the application
      # process itself. The installed bundle carries exactly one executable,
      # Contents/MacOS/OverSight (CFBundleExecutable "OverSight",
      # CFBundleIdentifier com.objective-see.oversight), and no LoginItems/
      # or XPCServices/ helpers (verified 2026-07-27), so an exact-name match
      # scoped to this user's session (-x -U) IS the running state.
      # Deliberately NOT a bundle-presence check: /Applications/OverSight.app
      # sits on disk whether or not the monitor is watching anything, so
      # presence would report healthy for a quit monitor. classify_probe
      # cannot express pgrep, whose documented healthy-off form is a status,
      # not a needle (man page: exit 0 matched, 1 no match, 2 invalid
      # options): exit 0 with a WELL-FORMED pid list on stdout (digits, one
      # pid per line, nothing else -- pgrep's only success output) is
      # running; the no-match exit 1 with no output at all is stopped; every
      # other outcome (usage or internal error, a timeout kill, a
      # status/output mismatch in either direction) is indeterminate, the
      # same untrustworthy-failure discipline as the readers above -- a
      # probe whose status and output disagree is never believed, whatever
      # it printed. The pairing is symmetric: exit 0 requires well-formed
      # pid output exactly as exit 1 requires empty output.
      output=$(run_bounded "$PGREP" -x -U "$UID" OverSight 2>&1) || rc=$?
      classify_pgrep "$output" "$rc"
      ;;
    pgrep_lulu_extension)
      # The LuLu network extension is a ROOT process (-U 0): the process
      # table is world-readable, so the user-agent poller reads it
      # unprivileged, and the root scope means no user process can
      # impersonate the extension. The exact 32-character name match was
      # verified live 2026-07-27 (pid 636). Deliberately a process probe,
      # not `systemextensionsctl list`: the list reports REGISTRATION state,
      # which persists even when the extension process is gone. What pgrep
      # proves is only that the process EXISTS: a wedged process can exist
      # and filter nothing, so this probe narrows the registration-versus-
      # running gap without closing the running-versus-filtering one.
      output=$(run_bounded "$PGREP" -x -U 0 com.objective-see.lulu.extension 2>&1) || rc=$?
      classify_pgrep "$output" "$rc"
      ;;
    lulu_rule_present)
      if ! lulu_base_rules_authoritative; then
        printf 'indeterminate'
        return 0
      fi
      lulu_rule_in_archive "$target"
      ;;
    lulu_rule_resolved_present)
      # The profile guard first: with an active (or unconfirmable) profile
      # the base archive is not the deciding rule set, so neither the
      # resolution nor the archive read should even run.
      if ! lulu_base_rules_authoritative; then
        printf 'indeterminate'
        return 0
      fi
      # Resolve the declared launcher FIRST, then require the archive to
      # mention the RESOLVED binary: LuLu keys rules on the executing
      # Mach-O, so a rule on a launcher symlink's own path protects nothing.
      # An unresolvable launcher is indeterminate (nothing was read), never
      # absent (absent is a claim about a rule set actually searched).
      local resolved_target="" resolve_rc=0
      resolved_target=$(run_bounded "$READLINK" -f "$target" 2>/dev/null) || resolve_rc=$?
      if [[ $resolve_rc -ne 0 || -z $resolved_target ]]; then
        printf 'indeterminate'
        return 0
      fi
      lulu_rule_in_archive "$resolved_target"
      ;;
    *)
      # Unreachable behind load_controls' reader validation; fail closed
      # anyway rather than fabricate a value.
      printf 'indeterminate'
      ;;
  esac
}

# load_controls -- read and validate the declared-controls file into the
# control_* arrays, fail-closed BEFORE any probe runs: an unreadable file, a
# non-verify tier, an unknown reader, a malformed or colliding id, an
# out-of-domain expect, or a missing description sets controls_problem (which
# the gap gate pages) and stops. The poller must never guess what to monitor.
# The template already refuses these at render time; this re-validation
# defends the deployed file itself.
control_ids=()
control_descriptions=()
control_readers=()
control_expects=()
control_targets=()
control_remedies=()
controls_problem=""
load_controls() {
  local count index record id tier reader expect target description remedy domain
  if [[ ! -f $CONTROLS_FILE ]]; then
    controls_problem="posture-controls file missing at $(sanitize_span "$CONTROLS_FILE")"
    return 0
  fi
  # Slurp (-s) so the WHOLE file must be exactly ONE top-level array: a
  # multi-document file (say, two arrays back to back) parses per document,
  # would emit one length per document, poison the loop arithmetic, and
  # silently monitor zero controls. The integer guard is belt-and-braces for
  # the same reason: count feeds bash arithmetic, so anything but one plain
  # integer is refused before it can be evaluated.
  if ! count=$(jq -ser 'if (length == 1 and (.[0] | type == "array")) then (.[0] | length) else error("not one array") end' <"$CONTROLS_FILE" 2>/dev/null); then
    controls_problem="the posture-controls file is not a JSON array"
    return 0
  fi
  if ! [[ $count =~ ^[0-9]+$ ]]; then
    controls_problem="the posture-controls file is not a JSON array"
    return 0
  fi
  for ((index = 0; index < count; index++)); do
    record=$(jq -c ".[$index]" <"$CONTROLS_FILE" 2>/dev/null || echo "")
    id=$(jq -r '.id // empty' <<<"$record" 2>/dev/null || echo "")
    tier=$(jq -r '.tier // empty' <<<"$record" 2>/dev/null || echo "")
    reader=$(jq -r '.reader // empty' <<<"$record" 2>/dev/null || echo "")
    expect=$(jq -r '.expect // empty' <<<"$record" 2>/dev/null || echo "")
    target=$(jq -r '.target // empty' <<<"$record" 2>/dev/null || echo "")
    description=$(sanitize "$(jq -r '.description // empty' <<<"$record" 2>/dev/null || echo "")")
    remedy=$(sanitize "$(jq -r '.remedy // empty' <<<"$record" 2>/dev/null || echo "")")
    if ! [[ $id =~ ^[a-z0-9_]+$ ]]; then
      controls_problem="posture-controls record $index has a missing or malformed id"
      return 0
    fi
    # ids become baseline field names, so they may appear once and must not
    # shadow the built-in trio.
    case " firewall gatekeeper screenlock ${control_ids[*]-} " in
      *" $id "*)
        controls_problem="posture-controls record [$id] collides with another monitored field"
        return 0
        ;;
    esac
    if [[ $tier != "verify" ]]; then
      controls_problem="posture-controls record [$id] declares tier $(sanitize_span "$tier"), not verify; the poller only reads controls, so the record does not belong in its file"
      return 0
    fi
    domain=$(reader_domain "$reader")
    if [[ -z $domain ]]; then
      controls_problem="posture-controls record [$id] names unknown reader $(sanitize_span "$reader")"
      return 0
    fi
    case " $domain " in
      *" $expect "*) ;;
      *)
        controls_problem="posture-controls record [$id] expects $(sanitize_span "$expect"), outside the $reader domain ($domain)"
        return 0
        ;;
    esac
    # The target pairing, both directions: a lulu_rule reader without an
    # absolute target has nothing to check, and a target on any other reader
    # is silently ignored data, which is how a mislabeled record hides. A
    # multi-line target is refused because the archive match is line-based:
    # a pattern spanning lines would match archive lines it never named.
    if reader_requires_target "$reader"; then
      if [[ -z $target ]]; then
        controls_problem="posture-controls record [$id] names reader $reader, which requires a target (the absolute binary path whose rule must exist)"
        return 0
      fi
      if [[ $target != /* ]]; then
        controls_problem="posture-controls record [$id] target $(sanitize_span "$target") must be an absolute path"
        return 0
      fi
      if [[ $target == *$'\n'* || $target == *$'\x1f'* ]]; then
        controls_problem="posture-controls record [$id] target contains a newline or a unit separator"
        return 0
      fi
    elif [[ -n $target ]]; then
      controls_problem="posture-controls record [$id] declares a target its reader $reader does not consume"
      return 0
    fi
    if [[ -z $description ]]; then
      controls_problem="posture-controls record [$id] has no description"
      return 0
    fi
    control_ids+=("$id")
    control_descriptions+=("$description")
    control_readers+=("$reader")
    control_expects+=("$expect")
    control_targets+=("$target")
    control_remedies+=("$remedy")
  done
  return 0
}
load_controls
if [[ -n $controls_problem ]]; then
  # A refused file is refused WHOLE: records loaded before the offender must
  # not be consulted by any later step, or a half-validated file would be
  # half-monitored.
  control_ids=()
  control_descriptions=()
  control_readers=()
  control_expects=()
  control_targets=()
  control_remedies=()
fi

# Read every declared control, only behind a clean load: a refused file must
# page BEFORE any probe runs, and a probe driven by an invalid record would be
# a read nothing declared.
control_values=()
if [[ -z $controls_problem ]]; then
  # The profile guard runs HERE, in the parent shell, before any read:
  # read_control runs inside a command substitution, so state set there dies
  # with the subshell; a guard first triggered inside one could neither
  # memoize (one preferences read per tick) nor reach the gap report with
  # its cause. Each read_control subshell inherits the settled state by
  # fork, so its own guard call is a pure lookup.
  for control_index in "${!control_ids[@]}"; do
    if reader_reads_lulu_base_rules "${control_readers[$control_index]}"; then
      lulu_base_rules_authoritative || true
      break
    fi
  done
  for control_index in "${!control_ids[@]}"; do
    control_values+=("$(read_control "${control_readers[$control_index]}" "${control_targets[$control_index]}")")
  done
fi

# page_gap_once <marker_path> <members> <title> <body> -- the shared
# page-once-via-marker discipline for the monitoring-gap and persistence-gap
# paths, refined to page-once-PER-MEMBER: the marker stores the space-separated
# set of gapped members already paged for, so an ONGOING gap stays quiet while
# a NEW member gapping during it still pages (one broken probe must never
# silence word of a second one breaking). Same notify-before-persist contract
# as page_crit_and_persist: send_alert FIRST, record the member set ONLY on
# success (best effort: a marker in an unwritable dir cannot be written, so
# the page may re-fire). When every current member is already covered, refresh
# the marker to the CURRENT set so a member that recovers and later re-gaps
# pages again. Return 0 when paged or already covered, nonzero when send_alert
# could not store the page (so a persisting condition re-pages). The per-path
# bodies, recovery-clear placement, and caller exit code stay OUTSIDE this
# helper.
page_gap_once() {
  local marker="$1" members="$2" title="$3" body="$4"
  local covered="" member_list=() member new_member=0
  read -ra member_list <<<"$members"
  if [[ -f $marker ]]; then
    covered=" $(cat "$marker" 2>/dev/null || true) "
    for member in "${member_list[@]}"; do
      if [[ $covered != *" $member "* ]]; then
        new_member=1
      fi
    done
  else
    new_member=1
  fi
  if [[ $new_member -eq 0 ]]; then
    printf '%s\n' "$members" >"$marker" 2>/dev/null || true
    return 0
  fi
  if send_alert CRIT "$title" "$body" "Sosumi"; then
    printf '%s\n' "$members" >"$marker" 2>/dev/null || true
    return 0
  fi
  return 1
}

# Validate any existing baseline BEFORE trusting it (and before write_state
# overwrites it): it must be owner-only (mode 600) AND parse to three in-domain
# built-in scalars (same domains as the gap gate below). A group/world-readable,
# corrupt, or out-of-domain baseline is not trustworthy (it could be planted to
# mask a disabled protection, or fabricate a transition), so it is treated as no
# prior baseline. GNU-first stat, BSD fallback.
prev_valid=0
prev_fw=""
prev_gk=""
prev_sl=""
prev_json=""
if [[ -f $STATE ]]; then
  st_mode=$(stat -c '%a' "$STATE" 2>/dev/null || stat -f '%Lp' "$STATE" 2>/dev/null || echo "")
  prev_json=$(cat "$STATE" 2>/dev/null || echo "")
  prev_fw=$(jq -r '.firewall // empty' <<<"$prev_json" 2>/dev/null || echo "")
  prev_gk=$(jq -r '.gatekeeper // empty' <<<"$prev_json" 2>/dev/null || echo "")
  prev_sl=$(jq -r '.screenlock // empty' <<<"$prev_json" 2>/dev/null || echo "")
  if [[ $st_mode == "600" && $prev_fw =~ ^[012]$ && $prev_gk =~ ^[01]$ && $prev_sl =~ ^[01]$ ]]; then
    prev_valid=1
  fi
fi

# R2-9 monitoring gap, extended to the declared controls and made PER MEMBER.
# The members: the built-in trio (ONE combined probe, trusted or distrusted as
# a unit), the controls file, and each declared control. A gapped member's
# state is UNKNOWN, not safe: page (once per member, via the marker's member
# set), do NOT persist its read (it would poison the baseline), and do NOT
# compare it (it would fabricate a transition). But a gap on one member must
# NEVER blind the others: every member that read cleanly is still compared,
# paged, and persisted below, with only the gapped members' baseline fields
# preserved from the prior. A monitor that went blind on unrelated controls
# because one probe broke would lose a real regression during the outage --
# permanently, because the baseline moves on after the deviant control
# recovers. A built-in scalar missing or out of its exact domain (firewall
# 0/1/2, Gatekeeper 0/1, screenlock 0/1) gaps the trio; an empty or failed
# osqueryi read leaves all three empty and lands there too. If send_alert
# cannot store the gap page (it still fires a last-resort local banner), log
# and exit nonzero so a PERSISTING gap re-pages next tick. (Values cross into
# the body only through sanitize_span: raw system-read text is data, never
# structure, and never reaches the page whole or outside an inline-code span.)
trio_clean=1
gap_detail=""
gap_members=""
if ! [[ $cur_fw =~ ^[012]$ && $cur_gk =~ ^[01]$ && $cur_sl =~ ^[01]$ ]]; then
  trio_clean=0
  gap_members="posture_query"
  gap_detail="the posture query returned an unreadable value (firewall=$(sanitize_span "$cur_fw") gatekeeper=$(sanitize_span "$cur_gk") screenlock=$(sanitize_span "$cur_sl"))"
fi
if [[ -n $controls_problem ]]; then
  gap_members+="${gap_members:+ }controls_file"
  gap_detail+="${gap_detail:+; }$controls_problem"
fi
indeterminate_ids=""
for control_index in "${!control_ids[@]}"; do
  if [[ ${control_values[$control_index]} == "indeterminate" ]]; then
    indeterminate_ids+="${indeterminate_ids:+ }${control_ids[$control_index]}"
  fi
done
if [[ -n $indeterminate_ids ]]; then
  gap_members+="${gap_members:+ }$indeterminate_ids"
  gap_detail+="${gap_detail:+; }indeterminate posture control read(s): [$indeterminate_ids]"
fi
# The profile guard's cause, spelled out beside the ids it blinded: an
# indeterminate whose reason is "LuLu is consulting different files" needs a
# different response (deactivate the profile, or teach the monitor the
# profile paths) than a failed probe. Fixed script text, never probe output.
if [[ $lulu_profile_checked -eq 1 && $lulu_profile_state == "profile_active" ]]; then
  gap_detail+="${gap_detail:+; }a LuLu profile is ACTIVE (the base preferences carry a currentProfile key), so LuLu is consulting the profile's own files and the base rules archive these controls read is not the one deciding traffic"
elif [[ $lulu_profile_checked -eq 1 && $lulu_profile_state == "indeterminate" ]]; then
  gap_detail+="${gap_detail:+; }the LuLu base preferences could not be read to confirm no profile is active"
fi
if [[ -n $gap_detail ]]; then
  gap_body="**Security-posture monitoring gap**"$'\n'"- $gap_detail: the security posture there is currently UNKNOWN."$'\n'"- A blind monitor cannot see a protection turn off. Did osqueryi, a posture probe, or the LaunchAgent break? **Check now.**"$'\n'"- Diagnose: run the posture query and the control probes by hand, then re-check."
  if ! page_gap_once "$GAP" "$gap_members" "🔴 **CRITICAL**" "$gap_body"; then
    printf 'firewall-gatekeeper-monitor: send_alert could not queue the monitoring-gap page; no marker written, retrying next tick\n' >&2
    exit 1
  fi
else
  # A fully clean read cleared every gap (recovery): drop the marker so a
  # future gap pages again. Done before the normal transition/persist logic.
  rm -f "$GAP" 2>/dev/null || true
fi

# With the trio unreadable AND no trusted prior baseline there is nothing to
# anchor a comparison or a persist to: any baseline written now would carry no
# valid trio and be distrusted next tick, so comparing the declared controls
# would re-page the same first observation every tick. The gap page above
# already said the posture is unknown; stop here and retry next tick.
if [[ $trio_clean -eq 0 && $prev_valid -eq 0 ]]; then
  exit 0
fi

# Per-control priors, each trusted independently: a control's prior field must
# exist in a trusted baseline, sit inside its reader's domain, AND have been
# recorded under the SAME declared expect (the baseline stores it as
# "<id>:expect" beside each value; ':' cannot appear in an id, so the pair can
# never collide). Any miss means that ONE control has no trusted prior
# (first-observation semantics for it) while the others keep theirs. This is
# what makes adding a control to the data file a quiet upgrade on a healthy
# machine instead of a page storm or a global baseline reset. An out-of-domain
# prior is never compared: comparing it would fabricate a transition, and it
# never reaches a page. A prior recorded under a DIFFERENT expect is never
# compared either: an operator tightening a declaration over a steady-deviant
# value would otherwise read as steady-deviant and stay silent, turning a
# hardening change into a silent no-op.
control_prevs=()
for control_index in "${!control_ids[@]}"; do
  control_prev=""
  if [[ $prev_valid -eq 1 ]]; then
    control_prev=$(jq -r --arg key "${control_ids[$control_index]}" '.[$key] // empty' <<<"$prev_json" 2>/dev/null || echo "")
    control_prev_expect=$(jq -r --arg key "${control_ids[$control_index]}" '.[$key + ":expect"] // empty' <<<"$prev_json" 2>/dev/null || echo "")
    control_prev_target=$(jq -r --arg key "${control_ids[$control_index]}" '.[$key + ":target"] // empty' <<<"$prev_json" 2>/dev/null || echo "")
    domain=$(reader_domain "${control_readers[$control_index]}")
    case " $domain " in
      *" $control_prev "*) ;;
      *) control_prev="" ;;
    esac
    if [[ $control_prev_expect != "${control_expects[$control_index]}" ]]; then
      control_prev=""
    fi
    # A prior recorded under a DIFFERENT target is never compared, for the
    # same reason as a different expect: an operator repointing a rule
    # control at a new binary declared a new intent, and the old baseline
    # would read a steady-deviant new target as already-paged. Targetless
    # readers store no :target key, so both sides are empty and match.
    if [[ $control_prev_target != "${control_targets[$control_index]}" ]]; then
      control_prev=""
    fi
  fi
  control_prevs+=("$control_prev")
done

# The baseline to persist: for every member that read cleanly, the value read
# THIS tick; for every gapped member, the trusted prior field preserved
# unchanged, so a member's real transitions are still detected across another
# member's outage. With a refused controls file the declared set is unknown,
# so a trusted prior's fields are preserved wholesale under the fresh trio.
# Normalized enum values only, in-domain by construction.
if [[ $trio_clean -eq 1 ]]; then
  baseline_json="$posture"
else
  baseline_json=$(jq -cn --arg fw "$prev_fw" --arg gk "$prev_gk" --arg sl "$prev_sl" \
    '{firewall: $fw, gatekeeper: $gk, screenlock: $sl}')
fi
if [[ -n $controls_problem && $prev_valid -eq 1 ]]; then
  baseline_json=$(jq -c --argjson trio "$baseline_json" '. + $trio' <<<"$prev_json")
fi
for control_index in "${!control_ids[@]}"; do
  control_field="${control_values[$control_index]}"
  if [[ $control_field == "indeterminate" ]]; then
    control_field="${control_prevs[$control_index]}" # trusted prior, may be empty
  fi
  if [[ -n $control_field ]]; then
    # The value AND the declaration it was recorded under (expect, and the
    # target for the targeted readers): the pairing is what lets the next
    # run detect a changed declaration and re-arm the control.
    baseline_json=$(jq -c \
      --arg key "${control_ids[$control_index]}" \
      --arg value "$control_field" \
      --arg expect "${control_expects[$control_index]}" \
      '. + {($key): $value, ($key + ":expect"): $expect}' <<<"$baseline_json")
    if [[ -n ${control_targets[$control_index]} ]]; then
      baseline_json=$(jq -c \
        --arg key "${control_ids[$control_index]}" \
        --arg target "${control_targets[$control_index]}" \
        '. + {($key + ":target"): $target}' <<<"$baseline_json")
    fi
  fi
done

# Persist the current posture owner-only (0600) so a later run can trust its own
# baseline. Written via a private temp file plus an atomic rename. Ordering for an
# OFF transition is notify-before-persist (see below): the baseline advances ONLY
# after send_alert durably enqueues the page. In steady state (no transition) it
# just refreshes the baseline.
write_state() {
  (
    umask 077
    printf '%s\n' "$baseline_json" >"$STATE.tmp"
  ) && mv -f "$STATE.tmp" "$STATE" && chmod 600 "$STATE"
}

# Persist the baseline, and make a persistence FAILURE loud rather than silent. If
# write_state fails the monitor is degraded: it cannot advance its baseline, so a
# stale baseline could silently mask the next real change (a stale prev=OFF reads
# the next real OFF as steady-off, silent, permanently). Page a degraded-monitor
# gap ONCE via its own marker, then exit nonzero so launchd retries and the stale
# baseline is never silently trusted. On success clear the marker (recovery). The
# marker lives in the state dir, so if that dir is itself unwritable the marker
# cannot be written and the degraded page may re-fire, acceptable for a serious
# ongoing fault (loud beats silently trusting a stale baseline).
persist_baseline() {
  if write_state; then
    rm -f "$PERSIST_GAP" 2>/dev/null || true
    return 0
  fi
  degraded_body="**Security-posture monitor degraded**"$'\n'"- The posture monitor could not persist its baseline: it cannot advance state, so a stale baseline could mask the next real change and blind the monitor."$'\n'"- Check the state directory free space and permissions. **Check now.**"
  page_gap_once "$PERSIST_GAP" "baseline_persist" "🔴 **CRITICAL**" "$degraded_body" || true
  exit 1
}

# Emit one CRIT page built from the given blocks, then advance the baseline.
# Notify-before-persist: send_alert is write-ahead durable (it stores the page
# before any network attempt and, if it cannot even store, fires a last-resort
# local banner), so a page is never silently dropped. The baseline advances only
# after send_alert succeeds; on a send_alert store-failure the baseline is left
# as-is and the poller exits nonzero, so a PERSISTING condition re-pages next tick
# and recovers its durable/remote copy (a transient that clears before the retry
# was still surfaced locally by the banner). Shared by the first-observation and
# transition paths so both obey one durability contract.
page_crit_and_persist() {
  local body title
  body=$(printf '%s\n\n' "$@")
  body=${body%$'\n\n'}
  title="🔴 **CRITICAL**"
  if [[ $# -gt 1 ]]; then title="🔴 **CRITICAL** · $#"; fi
  if ! send_alert CRIT "$title" "$body" "Sosumi"; then
    printf 'firewall-gatekeeper-monitor: send_alert could not queue the CRIT page; baseline not advanced, retrying next tick\n' >&2
    exit 1
  fi
  persist_baseline
}

# control_block <index> <first_observation:0|1> -- the CRIT block for a
# declared control deviating from its declared value. Everything in it is
# either this script's own text, a normalized enum value, or a sanitized
# record field wrapped in an inline-code span (description and remedy come
# from the data file, so they are data, never notification structure); raw
# probe output never appears.
control_block() {
  local control_index="$1" first_observation="$2"
  local description="\`${control_descriptions[$control_index]}\`"
  local expect="${control_expects[$control_index]}"
  local value="${control_values[$control_index]}"
  local remedy="${control_remedies[$control_index]}"
  local block
  if [[ $first_observation -eq 1 ]]; then
    block="**${description}: ${value} at first observation, declared ${expect}**"$'\n'"- **Now:** **${value}**"$'\n'"- The monitor has no prior baseline for this control and it already deviates from its declared value, a pre-existing exposure. Did you change it? If not, **investigate now**."
  else
    block="**${description}: now ${value}, declared ${expect}**"$'\n'"- **Was:** ${control_prevs[$control_index]}"$'\n'"- **Now:** **${value}**"$'\n'"- Did you change this? If not, something else did, **investigate now**."
  fi
  if [[ -n $remedy ]]; then
    block+=$'\n'"- \`${remedy}\`"
  fi
  printf '%s' "$block"
}

# No trustworthy prior baseline (first run, or a lost/deleted/planted/corrupt
# state file). DIVERGENCE from c69baab's silent first-run seed (F4, banked from
# the slice-6 alerter review): the alerter log-onlys firewall/Gatekeeper
# off-events and relies on THIS poller to page them, so a protection ALREADY off
# with no prior baseline would otherwise be silently accepted and go unpaged
# forever. Page each already-off protection and each already-deviant declared
# control as a first-observation exposure (screenlock too: it is poller-only,
# and an already-off lock is a real exposure), with the same
# notify-before-persist durability. If everything is healthy, seed silently.
if [[ $prev_valid -eq 0 ]]; then
  first_obs_blocks=()
  if [[ $cur_fw == "0" ]]; then
    first_obs_blocks+=("**Firewall is OFF (first observation)**"$'\n'"- **Now:** **OFF**"$'\n'"- The monitor has no prior baseline and the firewall is already off, a pre-existing exposure. Did you turn it off? If not, **investigate now**."$'\n'"- Re-enable it: System Settings → Network → Firewall")
  fi
  if [[ $cur_gk == "0" ]]; then
    first_obs_blocks+=("**Gatekeeper is OFF (first observation)**"$'\n'"- **Now:** **DISABLED**"$'\n'"- The monitor has no prior baseline and Gatekeeper is already disabled, a pre-existing exposure. Did you turn it off? If not, **investigate now**."$'\n'"- Re-enable it: System Settings → Privacy & Security (spctl cannot enable Gatekeeper from the CLI on macOS 15+)")
  fi
  if [[ $cur_sl == "0" ]]; then
    first_obs_blocks+=("**Screen lock is OFF (first observation)**"$'\n'"- **Now:** **OFF**"$'\n'"- The monitor has no prior baseline and the screen-lock password requirement is already off, anyone at the machine has access. Did you turn it off? If not, **investigate now**."$'\n'"- Re-enable it: System Settings → Lock Screen → Require password")
  fi
  for control_index in "${!control_ids[@]}"; do
    if [[ ${control_values[$control_index]} == "indeterminate" ]]; then
      continue # gapped member: paged above, never compared
    fi
    if [[ ${control_values[$control_index]} != "${control_expects[$control_index]}" ]]; then
      first_obs_blocks+=("$(control_block "$control_index" 1)")
    fi
  done
  if [[ ${#first_obs_blocks[@]} -eq 0 ]]; then
    persist_baseline # healthy first observation: seed the baseline silently
    exit 0
  fi
  page_crit_and_persist "${first_obs_blocks[@]}"
  exit 0
fi

# Human-readable state text for the Was: line of each transition block.
fw_to_text() {
  case "$1" in
    0) echo "OFF" ;;
    1) echo "on (allow signed)" ;;
    2) echo "on (block all)" ;;
    *) echo "?($1)" ;;
  esac
}
gk_to_text() {
  case "$1" in
    0) echo "DISABLED" ;;
    1) echo "enabled" ;;
    *) echo "?($1)" ;;
  esac
}
sl_to_text() {
  case "$1" in
    0) echo "OFF" ;;
    1) echo "on" ;;
    *) echo "?($1)" ;;
  esac
}

# A trusted baseline exists: page CRIT only on a protection turning OFF or a
# declared control leaving its declared value. A re-enable (a return to the
# declared/on state) is good news, not actionable, and there is no notice
# channel, so it is silent. Each block mirrors the results-alerter
# protection-off shape: bold header, Was/Now state, then a decision-first next
# step.
crit_blocks=()
if [[ $trio_clean -eq 1 ]]; then
  if [[ $cur_fw != "$prev_fw" && $cur_fw == "0" ]]; then
    crit_blocks+=("**Firewall turned OFF**"$'\n'"- **Was:** $(fw_to_text "$prev_fw")"$'\n'"- **Now:** **OFF**"$'\n'"- Did you turn this off? If not, something else did, **investigate now**."$'\n'"- Re-enable it: System Settings → Network → Firewall")
  fi
  if [[ $cur_gk != "$prev_gk" && $cur_gk == "0" ]]; then
    crit_blocks+=("**Gatekeeper turned OFF**"$'\n'"- **Was:** $(gk_to_text "$prev_gk")"$'\n'"- **Now:** **DISABLED**"$'\n'"- Did you turn this off? If not, something else did, **investigate now**."$'\n'"- Re-enable it: System Settings → Privacy & Security (spctl cannot enable Gatekeeper from the CLI on macOS 15+)")
  fi
  if [[ $cur_sl != "$prev_sl" && $cur_sl == "0" ]]; then
    crit_blocks+=("**Screen lock turned OFF**"$'\n'"- **Was:** $(sl_to_text "$prev_sl")"$'\n'"- **Now:** **OFF**"$'\n'"- Did you turn this off? If not, something else did, **investigate now**."$'\n'"- Re-enable it: System Settings → Lock Screen → Require password")
  fi
fi
# Declared controls: page on a REGRESSION (the prior held the declared value,
# the current does not) and on a first observation of a deviation (no trusted
# prior for this one control). Steady-deviant is silent (the regression already
# paged once; the baseline is the page-once marker), and a return to the
# declared value is silent recovery that re-arms the marker via the persist
# below.
for control_index in "${!control_ids[@]}"; do
  control_value="${control_values[$control_index]}"
  control_expect="${control_expects[$control_index]}"
  control_prev="${control_prevs[$control_index]}"
  if [[ $control_value == "indeterminate" ]]; then
    continue # gapped member: paged above, never compared
  fi
  if [[ $control_value == "$control_expect" ]]; then
    continue
  fi
  if [[ -z $control_prev ]]; then
    crit_blocks+=("$(control_block "$control_index" 1)")
  elif [[ $control_prev == "$control_expect" ]]; then
    crit_blocks+=("$(control_block "$control_index" 0)")
  fi
done

# No deviation (steady state or a recovery): refresh the baseline, silent.
if [[ ${#crit_blocks[@]} -eq 0 ]]; then
  persist_baseline
  exit 0
fi

# A deviation: page (notify-before-persist), then advance the baseline. One
# page for the tick, even when several protections deviated together.
page_crit_and_persist "${crit_blocks[@]}"
