#!/usr/bin/env bash
# moshi-gate: a presence-gated pass-through to moshi-hook.
#
#   usage: moshi-gate.sh <harness>-hook      the payload arrives on stdin
#
# WHY THIS EXISTS. moshi-hook holds the WebSocket that carries an approve or a
# deny back from the phone, which a one-way push structurally cannot do; pns
# holds the presence gate, which moshi has none of. So pns registers as the
# harness hook and FORWARDS here rather than competing with moshi: at the
# keyboard the phone stays quiet and the harness prompts as usual, away the
# card goes up. Two callers: hooks/relay-agent.sh on a blocking event, and the
# generated pi/omp extensions, whose one `helperBinary` line is repointed here.
#
# IT IS A PIPE, NOT AN INTERPRETER. stdin is never read in this script; `exec`
# hands the stream to moshi untouched, and moshi's stdout and exit code become
# this script's own. A gate that read the payload to look at it and forgot to
# write it back would leave moshi parsing an empty stream, and moshi then
# silently does nothing at all.
#
# EXIT 0 MEANS "NOT FORWARDED" on every path that declines (no moshi installed,
# operator at the keyboard, a subcommand this script will not vouch for), which
# is the harness's "no opinion, prompt as usual". The forwarded path is the one
# place in pns where a non-zero exit is correct rather than a bug: there the
# exit code is the operator's decision, and swallowing it would answer for them.
set -euo pipefail

sub="${1:-}"
# The subcommand arrives from a file moshi GENERATES (pi and omp spawn
# `helperBinary pi-hook`) and from a harness name pns reads out of its own
# environment, while moshi-hook's top-level positional is a PATH. An unvetted
# word here is this repo handing a third-party binary a filesystem argument
# nobody chose. Shape only, not a roster: the harness list is moshi's and grows.
[[ $sub =~ ^[a-z]+-hook$ ]] || exit 0

moshi="${MOSHI_HOOK_BIN:-/opt/homebrew/bin/moshi-hook}"
command -v "$moshi" >/dev/null 2>&1 || exit 0

helpers="${PNS_HELPERS_DIR:-${BASH_SOURCE[0]%/*}/../helpers}"
# A core that is not there forwards NOTHING: without the verdict this script
# cannot tell where the operator is, and pushing anyway is exactly the failure
# the gate exists to prevent.
# shellcheck source=dot_local/libexec/pns/helpers/event.sh
source "$helpers/event.sh" || exit 0
# shellcheck source=dot_local/libexec/pns/helpers/presence.sh
source "$helpers/presence.sh" || exit 0

# The same reading, thresholds and fail-open rules relay.sh gates its phone
# channel with. relay's two narrowing flags are relay's own and never apply
# here, so they are passed empty.
pns_wants_phone "$(pns_idle_secs)" "${RELAY_DESK_IDLE_SECS:-600}" "" "" "${RELAY_FORCE_PHONE:-}" || exit 0

exec "$moshi" "$sub"
