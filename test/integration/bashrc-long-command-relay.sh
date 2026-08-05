#!/usr/bin/env bash
#
# The SHELL producer reaches relay: a long command finishing in an interactive
# shell must arrive at ~/.local/bin/relay.sh with its state and its pane.
#
# WHY THIS FILE EXISTS. relay is the progressive notifier: it decides presence
# (at the desk, banner only; away, add the phone push), it writes the Discord
# paper trail, and its banner is clickable (the click runs `herdr agent focus`
# on the pane the notification names). Every producer feeds it: the Claude and
# Codex hooks, the weekly unattended jobs, the osquery alerter. Each of those has
# a test. The shell producer, the long-running command notifier in ~/.bashrc, had
# none, and it is the one that lost its wiring: it called `alerter` directly, so
# a five-minute build finishing while nobody was at the desk raised a banner on
# an empty room, logged nothing, and left the pane unreachable.
#
# So what is pinned here is the CONTRACT, not the bash: a shell command that runs
# long enough reaches relay, carrying the state derived from its exit code and
# the pane it ran in, narrowed to the local channel at the first tier and fanned
# out at the second. A rewrite in another language satisfies this file by
# invoking relay the same way; nothing here reads as bash beyond the sandbox that
# drives it.
#
# Integration: the rendered ~/.bashrc notifier driven against a stubbed relay and
# a stubbed hue-pulse, which are its two boundaries.
#
# HOW THE STUBS ARE SYNCHRONIZED, since the notifier detaches every call it makes
# (`( ... & )`, so `fg`/`jobs` never see them) and a detached grandchild cannot be
# waited for: the driver is run with fd 9 duplicated onto the command
# substitution's pipe, the stubs record onto fd 9, and the substitution returns
# when the pipe reaches EOF, which happens only once the driver AND every process
# that inherited fd 9 has exited. That is a real wait, not a settle time, so a
# case that expects NOTHING is as deterministic as one that expects a record, and
# the file needs no sleeps or polling at all. Each stub writes its whole record
# with a single short `printf`, which is atomic on a pipe, so the two of them
# firing at once cannot interleave.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/dot_bashrc.tmpl"

RELAY_TARGET_PATH='.local/bin/relay.sh'
HUE_PULSE_TARGET_PATH='.local/bin/hue-pulse.sh'
PROJECT_DIRECTORY_NAME='fixture-project'
PANE='wW:p8'
# The two tiers, at their exact boundaries: the first is local-only, the second
# adds the fan-out and the light pulse.
LOCAL_TIER_SECONDS=30
FANOUT_TIER_SECONDS=300
# A clock-relative observation is bracketed by two reads of SECONDS and retried
# while they disagree, because the notifier reads that clock ITSELF: a tick
# between the driver's read and the notifier's makes the elapsed time one second
# longer than the case asked for, which at a tier boundary is the difference
# between firing and not. Same reasoning as test/unit/bashrc-brew-cache-self-heal.sh.
CLOCK_STABLE_OBSERVATION_ATTEMPTS=8
CLOCK_TICKED_MARKER='clock-ticked'

fail() {
  printf 'bashrc-long-command-relay: FAIL -- %s\n' "$*" >&2
  exit 1
}

# `! grep` in final position under `set -e` does not fail a test (errexit ignores
# an inverted status), so every negative assertion goes through this helper.
refute_contains() {
  local haystack="$1" needle="$2" description="$3"
  if grep -qF -- "$needle" <<<"$haystack"; then
    printf '=== records ===\n%s\n' "$haystack" >&2
    fail "$description (found '$needle')"
  fi
}

assert_contains() {
  local haystack="$1" needle="$2" description="$3"
  grep -qF -- "$needle" <<<"$haystack" || {
    printf '=== records ===\n%s\n' "$haystack" >&2
    fail "$description (missing '$needle')"
  }
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'bashrc-long-command-relay: SKIP (chezmoi not on PATH; cannot render dot_bashrc.tmpl)\n'
  exit 0
}
[[ -f $TEMPLATE ]] || fail "missing template: $TEMPLATE"

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT
home="$sandbox/home"
project="$sandbox/$PROJECT_DIRECTORY_NAME"
mkdir -p "$home/${RELAY_TARGET_PATH%/*}" "$project" "$sandbox/render-home"

rendered="$(HOME="$sandbox/render-home" CI=1 chezmoi --source "$REPO_ROOT" \
  execute-template --no-tty <"$TEMPLATE")" ||
  fail 'chezmoi failed to render dot_bashrc.tmpl'

# The notifier, sliced from its first assignment through the registration that
# hands it to bash-preexec.
notifier="$sandbox/notifier.sh"
awk '/^[[:space:]]*__cmd_notify_start=""/{inside = 1}
     inside
     /^[[:space:]]*precmd_functions\+=\(__cmd_notify_precmd\)/{exit}' <<<"$rendered" >"$notifier"
grep -qF '__cmd_notify_precmd()' "$notifier" ||
  fail 'could not slice the long-running command notifier out of the rendered ~/.bashrc'

# Both boundaries record one line per invocation: a tag, then every argument
# wrapped in angle brackets so a value containing spaces stays one token and a
# flag/value pairing can be asserted without pinning the order of the flags.
for stub in "$RELAY_TARGET_PATH" "$HUE_PULSE_TARGET_PATH"; do
  cat >"$home/$stub" <<STUB
#!/usr/bin/env bash
printf '${stub##*/} %s\n' "\$(printf '<%s>' "\$@")" >&9
STUB
  chmod +x "$home/$stub"
done

# Drive one command completion and return what the boundaries recorded.
#
# The driver sets the notifier's own state rather than sleeping: preexec stamps
# SECONDS, so an elapsed time is simply a start stamp that far back. `(exit N)`
# immediately before the call is what puts the simulated exit code in `$?`, which
# is the first thing the notifier reads.
driver="$sandbox/driver.sh"
cat >"$driver" <<'DRIVER'
# shellcheck disable=SC1090
. "$NOTIFIER_BLOCK"
__cmd_notify_name="$SIMULATED_COMMAND"
clock_before=$SECONDS
__cmd_notify_start=$((clock_before - SIMULATED_ELAPSED))
(exit "$SIMULATED_EXIT")
__cmd_notify_precmd
[[ $clock_before == "$SECONDS" ]] || printf '%s\n' "$CLOCK_TICKED_MARKER" >&9
DRIVER

run_command() {
  local command_line="$1" elapsed="$2" exit_code="$3" pane="${4-$PANE}" attempt records
  for ((attempt = 0; attempt < CLOCK_STABLE_OBSERVATION_ATTEMPTS; attempt++)); do
    records="$(
      cd "$project" &&
        env -i \
          PATH="${BASH%/*}:/usr/bin:/bin" \
          HOME="$home" \
          HERDR_PANE_ID="$pane" \
          NOTIFIER_BLOCK="$notifier" \
          SIMULATED_COMMAND="$command_line" \
          SIMULATED_ELAPSED="$elapsed" \
          SIMULATED_EXIT="$exit_code" \
          CLOCK_TICKED_MARKER="$CLOCK_TICKED_MARKER" \
          "$BASH" --noprofile --norc "$driver" 9>&1 >"$sandbox/terminal" 2>&1
    )"
    if [[ -s $sandbox/terminal ]]; then
      printf '=== driver output ===\n%s\n' "$(cat "$sandbox/terminal")" >&2
      fail "the notifier wrote to the terminal while handling '$command_line'"
    fi
    grep -qxF "$CLOCK_TICKED_MARKER" <<<"$records" || {
      printf '%s\n' "$records"
      return 0
    }
  done
  fail "the wall clock ticked on all $CLOCK_STABLE_OBSERVATION_ATTEMPTS attempts for '$command_line', so the notifier never saw the elapsed time the case asked for"
}

invocation_count() {
  local records="$1" tag="$2"
  grep -c "^$tag " <<<"$records" || true
}

# relay's parser treats a value-taking flag as having NO value when the next
# token is another recognized flag or the argv ends, warns, and DROPS the flag.
# At the first tier that means dropping --local-only and leaking a 30-second
# command to the phone and Discord. So every relay call is checked for a
# dangling flag, whatever else the case is about. Whether an unknown pane is
# passed as an empty value or left out entirely is the notifier's business;
# leaving the flag there with nothing after it is not.
readonly VALUE_TAKING_RELAY_FLAGS=' --agent --state --project --branch --detail --pane '
assert_relay_flags_carry_values() {
  local records="$1" label="$2" line index
  while IFS= read -r line; do
    [[ $line == 'relay.sh '* ]] || continue
    local -a tokens=()
    while [[ $line =~ \<([^\>]*)\> ]]; do
      tokens+=("${BASH_REMATCH[1]}")
      line="${line#*"${BASH_REMATCH[0]}"}"
    done
    for ((index = 0; index < ${#tokens[@]}; index++)); do
      [[ $VALUE_TAKING_RELAY_FLAGS == *" ${tokens[index]} "* ]] || continue
      ((index + 1 < ${#tokens[@]})) ||
        fail "$label: relay was handed a bare ${tokens[index]} at the end of the argv, which its parser warns about and drops"
      [[ ${tokens[index + 1]} != --* ]] ||
        fail "$label: relay was handed ${tokens[index]} with ${tokens[index + 1]} as its value, so one of the two is silently dropped"
    done
  done <<<"$records"
}

# --- A. the local tier: relay, narrowed to this machine ----------------------
records="$(run_command 'make build --jobs 8' "$LOCAL_TIER_SECONDS" 0)"
[[ "$(invocation_count "$records" relay.sh)" == 1 ]] ||
  fail "the local tier called relay $(invocation_count "$records" relay.sh) times, wanted exactly 1"
assert_relay_flags_carry_values "$records" 'the local tier'
assert_contains "$records" '<--local-only>' \
  'the local tier does not narrow relay to this machine, so a 30-second command pushes to the phone and Discord'
assert_contains "$records" '<--agent><shell>' \
  'the shell producer does not identify itself as the shell agent'
assert_contains "$records" '<--state><done>' \
  'a command that succeeded does not report the done state'
assert_contains "$records" "<--project><$PROJECT_DIRECTORY_NAME>" \
  'the notification does not carry the directory the command ran in'
assert_contains "$records" "<--pane><$PANE>" \
  'the notification does not carry the herdr pane, so its banner cannot focus the pane on click'
grep -qE '<--detail><make \([0-9]+s\)>' <<<"$records" ||
  fail "the detail does not name the command and how long it took: $records"
[[ "$(invocation_count "$records" hue-pulse.sh)" == 0 ]] ||
  fail 'the local tier pulsed the lights, which belongs to the five-minute tier alone'

# --- B. the fan-out tier: relay unnarrowed, plus the lights ------------------
records="$(run_command 'nix build' "$FANOUT_TIER_SECONDS" 0)"
[[ "$(invocation_count "$records" relay.sh)" == 1 ]] ||
  fail "the fan-out tier called relay $(invocation_count "$records" relay.sh) times, wanted exactly 1"
assert_relay_flags_carry_values "$records" 'the fan-out tier'
refute_contains "$records" '<--local-only>' \
  'the five-minute tier still narrows relay to this machine, so a long build never reaches the phone or Discord'
assert_contains "$records" '<--agent><shell>' \
  'the fan-out tier does not identify the shell agent'
assert_contains "$records" "<--pane><$PANE>" \
  'the fan-out tier drops the herdr pane'
assert_contains "$records" 'hue-pulse.sh <0>' \
  'the five-minute tier does not pulse the lights with the exit code'

# --- C. a failure carries the failed state and the exit code ----------------
records="$(run_command 'cargo test' "$FANOUT_TIER_SECONDS" 7)"
assert_contains "$records" '<--state><failed>' \
  'a command that exited non-zero still reports the done state'
grep -qE '<--detail><cargo \([0-9]+s, exit 7\)>' <<<"$records" ||
  fail "the detail of a failed command does not carry its exit code: $records"
assert_contains "$records" 'hue-pulse.sh <7>' \
  'the light pulse is not handed the exit code, so a failure pulses green'

# --- D. below the first tier, nothing is sent -------------------------------
records="$(run_command 'git status' "$((LOCAL_TIER_SECONDS - 1))" 0)"
[[ -z ${records//[[:space:]]/} ]] ||
  fail "a command finishing under ${LOCAL_TIER_SECONDS}s notified anyway: $records"

# --- E. an interactive TUI is skipped however long it ran -------------------
# The agent CLIs are in the skip list because they fire their own relay hooks;
# without the skip every Claude session would also raise a shell notification.
records="$(run_command 'claude --resume' "$FANOUT_TIER_SECONDS" 0)"
[[ -z ${records//[[:space:]]/} ]] ||
  fail "an interactive TUI notified after a long session: $records"

# --- F. outside herdr the call is still well formed -------------------------
# The pane is unknown here, which is the case that produces a dangling `--pane`
# if the notifier passes the flag with nothing behind it.
records="$(run_command 'make build' "$LOCAL_TIER_SECONDS" 0 '')"
assert_relay_flags_carry_values "$records" 'outside herdr'
assert_contains "$records" '<--local-only>' \
  'outside herdr the local tier stopped narrowing relay'
assert_contains "$records" '<--agent><shell>' \
  'outside herdr the notification is not sent at all'

# --- G. relay owns the notification; the shell does not raise its own -------
# The regression this file exists for: a direct `alerter` call is a banner that
# skipped presence routing, wrote no Discord entry, and could not be clicked to
# reach the pane.
refute_contains "$(cat "$notifier")" 'alerter' \
  'the notifier raises its own macOS banner beside relay, bypassing presence routing and the paper trail'

printf 'bashrc-long-command-relay: OK (both tiers reach relay with state, project and pane; local-only below five minutes, fan-out plus the light pulse above it; nothing under the first tier and nothing for an interactive TUI)\n'
