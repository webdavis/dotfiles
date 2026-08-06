# shellcheck shell=bash
# pns decision core: the pure functions relay.sh and its hooks wrap with IO.
#
# WHY THIS IS SEPARATE. Everything here is a total function of its arguments:
# no network, no files, no clock, no environment. That is what makes it
# testable one behavior at a time, in microseconds, without stubbing a
# subprocess. relay.sh keeps the impure half (reading the idle probe, spawning
# channels) and this keeps the decisions, which is also the split SP3 carries
# into Rust: a pure core with a thin IO shell around it.
#
# Nothing here prints diagnostics or exits. A caller decides what to do with a
# verdict.

# pns_title <agent> <state> <project>
# The one-line heading a channel with a title field uses.
pns_title() {
  local agent="${1:-}" state="${2:-}" project="${3:-}"
  printf '%s · %s%s' "${agent:-relay}" "${state:-done}" "${project:+ · $project}"
}

# pns_message <branch> <detail> <state>
# The body: the summary itself, branch-prefixed. Deliberately NOT a repeat of
# the state and project the title already carries, so a channel with a short
# preview spends it on content rather than boilerplate.
pns_message() {
  local branch="${1:-}" detail="${2:-}" state="${3:-}"
  printf '%s%s' "${branch:+($branch) }" "${detail:-${state:-done}}"
}

# pns_flatten_reply <text> [max_chars]
# An agent turn reduced to the one line a summary prompt and a notification can
# carry: every run of spaces, tabs, carriage returns and newlines becomes ONE
# space, both ends are trimmed, and at most <max_chars> survive.
#
# THE TAIL IS WHAT SURVIVES, not the head. A turn states its conclusion at the
# end, and the beginning is setup whoever gets the notification already
# watched. The cap is applied only when the text is over it, because bash's
# negative offset on a SHORT string yields the empty string rather than the
# whole one, so an unconditional cut would blank every ordinary turn.
#
# Word splitting does the flattening, rather than a `tr | tr | sed` pipeline,
# because IFS here holds exactly the four characters that pipeline rewrote and
# squeezed, so it is the same transformation for no processes at all on a path
# that already forks jq and python. Globbing is disabled across the split
# (and only restored if the caller did not already want it off) so a turn that
# mentions `*` is not expanded against the filesystem.
pns_flatten_reply() {
  local text="${1:-}" max="${2:-8000}" globbing_was_off=""
  local IFS=$' \t\r\n'
  [[ $- == *f* ]] && globbing_was_off=1
  set -f
  # The unquoted split IS the operation here, not an oversight: SC2206 asks for
  # the quoting that would defeat it. Its other half, globbing, is what the
  # `set -f` above answers.
  # shellcheck disable=SC2206
  local -a words=($text)
  [[ -n $globbing_was_off ]] || set +f
  IFS=' '
  text="${words[*]-}"
  [[ ${#text} -le $max ]] || text="${text: -max}"
  printf '%s' "$text"
}

# pns_wants_phone <idle_secs> <desk_idle> <local_only> <remote_only> <force>
# 0 when the phone push should fire.
#
# FAIL OPEN ON ANY UNCERTAINTY. A garbled or absent idle reading means presence
# is UNKNOWN, and unknown must mean "treat as away" so a push is never silently
# dropped. Both values are validated as plain decimal digits BEFORE any
# arithmetic: a non-numeric threshold would otherwise abort the comparison under
# `set -u` in the caller, and a garbled probe line coerces to 0, which reads as
# "actively typing" and suppresses the push. Either flag suppresses the phone
# outright, and the force override beats presence but not the flags.
pns_wants_phone() {
  local idle="${1:-}" desk="${2:-}" local_only="${3:-}" remote_only="${4:-}" force="${5:-}"
  [[ -n $local_only || -n $remote_only ]] && return 1
  [[ -n $force ]] && return 0
  [[ $idle =~ ^[0-9]+$ && $desk =~ ^[0-9]+$ ]] || return 0
  ((idle < desk)) && return 1
  return 0
}

# pns_channel_plan <local_only> <remote_only> <want_phone>
#
# KNOWN LIMIT, and it is the one SP3 must close: this function NAMES its
# channels, so adding a destination means editing core policy rather than only
# dropping a file in channels/. That is an open/closed violation and it is the
# same coupling that would make an extracted crate useless to anyone whose
# stack is not moshi plus hermes. Closing it needs channels to declare their
# own routing (a manifest, or a `--policy` query the core collects), which is
# the plugin REGISTRATION mechanism. Building that in bash and again in Rust is
# the duplication this split exists to avoid, so the limit is named here rather
# than half-solved.
# One "<channel> <mode>" line per channel that should fire, in delivery order.
# Empty output means nothing fires, which is a legitimate verdict the caller
# has to report rather than pass over in silence.
#
# --remote-only is the LOG path (the weekly unattended jobs): hermes alone, and
# SYNCHRONOUSLY, because an undelivered log entry is invisible in a way an
# undelivered alert is not. --local-only is its mirror and keeps the banner.
# Giving both suppresses everything, which is why the caller must say so.
pns_channel_plan() {
  local local_only="${1:-}" remote_only="${2:-}" want_phone="${3:-}"
  if [[ -n $local_only && -n $remote_only ]]; then
    return 0
  fi
  if [[ -n $remote_only ]]; then
    printf 'hermes sync\n'
    return 0
  fi
  [[ -n $want_phone && -z $local_only ]] && printf 'moshi async\n'
  [[ -z $local_only ]] && printf 'hermes async\n'
  printf 'macos-banner async\n'
  return 0
}

# pns_pane_is_safe <pane>
# 0 when a pane id may be interpolated into terminal-notifier's -execute, which
# takes a SHELL STRING. A pane carrying `; curl ... | sh` would otherwise run
# when the operator clicks the banner, and the value comes from $HERDR_PANE_ID,
# which pns does not own.
pns_pane_is_safe() {
  [[ ${1:-} =~ ^[A-Za-z0-9._-]+$ ]]
}

# pns_session_id_is_safe <id>
# 0 when a harness-supplied session id may be used as a FILENAME. The id
# arrives inside the hook's JSON payload and is interpolated into a path, so a
# value carrying `/` or `..` would write the marker outside its directory.
pns_session_id_is_safe() {
  [[ ${1:-} =~ ^[A-Za-z0-9._-]+$ && ${1:-} != *..* ]]
}

# pns_session_was_long <elapsed_secs> <threshold_secs>
# 0 when a session ran long enough to be worth a light pulse. A non-numeric
# elapsed (an unreadable or corrupt marker) is NOT long: unlike a dropped phone
# push, a missed pulse costs nothing, so this one fails CLOSED rather than
# flashing the room on garbage.
pns_session_was_long() {
  local elapsed="${1:-}" threshold="${2:-300}"
  [[ $elapsed =~ ^[0-9]+$ && $threshold =~ ^[0-9]+$ ]] || return 1
  ((elapsed >= threshold))
}

# pns_pulse_color <exit_code>
# The "x y peak" triple a light pulse runs at: the deep green gamut corner for
# a success, the deep red one for anything else, plus the peak brightness that
# colour is pulsed at. Green washes toward white at full brightness
# (Bezold-Brücke), so it peaks lower and lets the green primary dominate; red
# stays saturated at 100.
#
# The coordinates are CIE xy gamut corners rather than RGB because Hue clamps
# RGB into its gamut and desaturates hard; xy bypasses that conversion.
#
# ANYTHING that is not all zeroes is a failure, garbage included. The
# comparison is textual on purpose: `-eq` would feed an unvalidated argument to
# arithmetic, where a word like `oops` aborts the caller under `set -u` rather
# than choosing a colour, which is the crash pns_session_was_long already had
# to grow a guard for.
pns_pulse_color() {
  if [[ ${1:-0} =~ ^0+$ ]]; then
    printf '0.17 0.7 70\n'
  else
    printf '0.6915 0.3083 100\n'
  fi
}

# pns_restore_args <on_state> <brightness> <ct|xy> <v1> <v2>
# The openhue arguments that put ONE light back the way a snapshot found it,
# ONE PER LINE so the caller reads them into an array; a space-joined string
# would split any value that ever carries a space.
#
# A light that was off is restored off and told nothing else. Sending it a
# brightness would turn it on, which is the failure a user actually sees: the
# pulse ends and a lamp that was dark all evening is now lit.
pns_restore_args() {
  local on_state="${1:-}" brightness="${2:-}" mode="${3:-}" v1="${4:-}" v2="${5:-}"
  if [[ $on_state != true ]]; then
    printf '%s\n' --off --transition-time 500ms
    return 0
  fi
  printf '%s\n' --on --brightness "$brightness"
  if [[ $mode == ct ]]; then
    printf '%s\n' -t "$v1"
  else
    printf '%s\n' -x "$v1" -y "$v2"
  fi
  printf '%s\n' --transition-time 500ms
}
