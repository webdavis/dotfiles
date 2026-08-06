# shellcheck shell=bash
# pns decision core: the pure functions relay.sh wraps with IO.
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
