#!/usr/bin/env bats
# The hue-pulse channel: the one destination that is handed an EXIT CODE rather
# than a JSON event, because its callers are the Claude Stop hook and the
# long-command notifier rather than relay.sh. SP3 normalizes that interface; the
# tests here pin what it does today.
#
# The split follows the rest of pns. Which colour an exit code means, and which
# openhue arguments put a snapshotted light back, are pure functions in
# helpers/event.sh and are called DIRECTLY here (no fork). Only the behaviors
# that ARE the process (the lock, the snapshot-then-restore round trip, the
# absence guards) spawn the channel.
#
# PATH is BUILT FROM NOTHING, one symlink per tool the channel is allowed to
# reach, rather than prepending a stub directory to the real one. Two reasons,
# both load-bearing: openhue is genuinely installed on this machine, so a stub
# the suite forgot to shadow would drive the operator's lights; and macOS 26
# ships /usr/bin/jq, so "jq is not installed" cannot be produced by dropping
# Homebrew off the front. A tool the channel grows a need for shows up as a
# loud failure here rather than as a silent host dependency.
#
# The stubbed `sleep` is what keeps the file fast: a real pulse waits 4.8s for
# the bulbs to ramp, and nothing here asserts on the waits.

setup_file() {
  BIN="$BATS_FILE_TMPDIR/bin"
  mkdir -p "$BIN"
  local tool
  # bash for the channel's own `env bash` shebang; the rest is what it runs.
  for tool in bash jq dirname mkdir mktemp head rm cat; do
    ln -sf "$(command -v "$tool")" "$BIN/$tool"
  done
  # A no-op `sleep`. Linked to true(1) BY PATH: `command -v true` answers with
  # the shell builtin, which would leave a dangling link, and a one-line script
  # would cost another shell per wait.
  ln -sf /usr/bin/true "$BIN/sleep"
  cat >"$BIN/openhue" <<'STUB'
#!/bin/sh
printf '%s\n' "$*" >>"$OPENHUE_ARGV"
case "$1 $2" in
  'get room') cat "$OPENHUE_ROOMS" ;;
  'get light') cat "$OPENHUE_LIGHTS" ;;
esac
STUB
  chmod +x "$BIN/openhue"

  # Three rooms, one of them empty, so "the room has no lights" and "there is no
  # such room" are distinct fixtures rather than the same one twice.
  OPENHUE_ROOMS="$BATS_FILE_TMPDIR/rooms.json"
  cat >"$OPENHUE_ROOMS" <<'JSON'
[
  {"Id": "studio-id", "Name": "3F - Studio"},
  {"Id": "kitchen-id", "Name": "Kitchen"},
  {"Id": "closet-id", "Name": "Closet"}
]
JSON
  # The studio holds one lit color-temperature light and one that is OFF, which
  # is the pair the restore has to tell apart. The kitchen light is the control:
  # a pulse of the studio must never reach it.
  OPENHUE_LIGHTS="$BATS_FILE_TMPDIR/lights.json"
  cat >"$OPENHUE_LIGHTS" <<'JSON'
[
  {"Id": "studio-ceiling",
   "Parent": {"Parent": {"Id": "studio-id"}},
   "HueData": {"on": {"on": true}, "dimming": {"brightness": 80},
               "color_temperature": {"mirek_valid": true, "mirek": 366},
               "color": {"xy": {"x": 0.42, "y": 0.4}}}},
  {"Id": "studio-lamp",
   "Parent": {"Parent": {"Id": "studio-id"}},
   "HueData": {"on": {"on": false}, "dimming": {"brightness": 42},
               "color_temperature": {"mirek_valid": false},
               "color": {"xy": {"x": 0.55, "y": 0.31}}}},
  {"Id": "kitchen-strip",
   "Parent": {"Parent": {"Id": "kitchen-id"}},
   "HueData": {"on": {"on": true}, "dimming": {"brightness": 100},
               "color_temperature": {"mirek_valid": false},
               "color": {"xy": {"x": 0.3, "y": 0.3}}}}
]
JSON
  export BIN OPENHUE_ROOMS OPENHUE_LIGHTS
}

setup() {
  PNS="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)/dot_local/libexec/pns"
  CHANNEL="$PNS/channels/executable_hue-pulse.sh"
  # shellcheck source=dot_local/libexec/pns/helpers/event.sh
  source "$PNS/helpers/event.sh"
  # The serialize lock is anchored under $HOME, so a per-test HOME is what keeps
  # one test's lock out of the next test's way.
  export HOME="$BATS_TEST_TMPDIR"
  export OPENHUE_ARGV="$BATS_TEST_TMPDIR/openhue.argv"
  : >"$OPENHUE_ARGV"
}

# Spawn the channel with ONLY the built tool PATH. The narrowing is applied to
# the CHANNEL rather than to the whole test, so the assertions below still have
# a working grep. $BIN is read at call time, which is what lets an absence test
# hand this a PATH with one tool taken out.
pulse() { PATH="$BIN" "$CHANNEL" "$@"; }

# One recorded openhue call, matched whole so a partial argument list cannot
# pass for the real one.
openhue_called() { grep -qxF -- "$1" "$OPENHUE_ARGV"; }

# How many calls CHANGED something. Every no-op asserts this is zero, because
# "exited 0" alone is also what a channel that pulsed the room and then tidied
# up would report.
write_call_count() { grep -c '^set ' "$OPENHUE_ARGV" || true; }

# A copy of the tool PATH with one tool taken out, so an absence test cannot
# disturb the shared one.
path_without() {
  local dir="$BATS_TEST_TMPDIR/bin-without-$1"
  cp -R "$BIN" "$dir"
  rm -f "$dir/$1"
  printf "%s" "$dir"
}

# --- pns_pulse_color -------------------------------------------------------

@test "a zero exit code pulses the green gamut corner" {
  [ "$(pns_pulse_color 0)" = "0.17 0.7 70" ]
}

@test "a non-zero exit code pulses the red gamut corner" {
  [ "$(pns_pulse_color 1)" = "0.6915 0.3083 100" ]
}

@test "an exit code that is not a number pulses red rather than aborting the pulse" {
  # `[[ $code -eq 0 ]]`, which this replaced, put the argument into arithmetic:
  # a word aborted the channel under `set -u` instead of choosing a colour, and
  # an empty one read as success. Unproven success is a failure here.
  [ "$(pns_pulse_color oops)" = "0.6915 0.3083 100" ]
}

# --- pns_restore_args ------------------------------------------------------

@test "a light in color-temperature mode is restored by its mirek value" {
  [ "$(pns_restore_args true 80 ct 366 '')" = "--on
--brightness
80
-t
366
--transition-time
500ms" ]
}

@test "a light in xy mode is restored by both coordinates" {
  [ "$(pns_restore_args true 80 xy 0.55 0.31)" = "--on
--brightness
80
-x
0.55
-y
0.31
--transition-time
500ms" ]
}

@test "a light that was off is restored off, never to a brightness" {
  # Sending a brightness would turn it back on, which is the outcome the
  # operator sees: the pulse ends and a lamp that was dark all evening is lit.
  [ "$(pns_restore_args false 42 xy 0.55 0.31)" = "--off
--transition-time
500ms" ]
}

# --- the pulse itself ------------------------------------------------------

@test "a success pulses the room green" {
  pulse 0
  openhue_called 'set room studio-id --on -x 0.17 -y 0.7 --brightness 70 --transition-time 1200ms'
}

@test "a failure pulses the room red" {
  pulse 1
  openhue_called 'set room studio-id --on -x 0.6915 -y 0.3083 --brightness 100 --transition-time 1200ms'
}

@test "HUE_PULSE_ROOMS chooses which room is pulsed" {
  HUE_PULSE_ROOMS='Kitchen' pulse 0
  openhue_called 'set room kitchen-id --on -x 0.17 -y 0.7 --brightness 70 --transition-time 1200ms'
}

@test "the restore puts back exactly the lights the pulsed room holds" {
  # One behavior with three edges: the lit light comes back lit at its own
  # colour, the dark one stays dark, and a light in another room is never
  # addressed at all. Asserted from one spawn because they are one traversal of
  # one snapshot, and a second full pulse costs more than the split is worth.
  pulse 0
  openhue_called 'set light studio-ceiling --on --brightness 80 -t 366 --transition-time 500ms'
  openhue_called 'set light studio-lamp --off --transition-time 500ms'
  run grep -qF 'kitchen-strip' "$OPENHUE_ARGV"
  [ "$status" -ne 0 ]
}

# --- the silent no-ops -----------------------------------------------------

# These two are the only tests here that can assert SILENCE, and it is worth
# knowing why: the lock takes fd 9 with `exec 9>>"$lock" 2>/dev/null`, and an
# `exec` carrying only redirections applies them to the SHELL, so everything
# after that line runs with stderr discarded whatever it writes. Both absence
# guards sit above it, so `[ -z "$output" ]` is a real assertion in these two
# and would be vacuous anywhere below.
#
# Both also pin the OUTCOME rather than the guard. Delete either `command -v`
# line and the run still ends the same way (openhue or jq missing makes the
# room lookup fail into its `|| true`, the room id comes back empty, and the
# channel exits 0 having changed nothing), so no test can tell the two apart
# from outside. The guards skip pointless work; they do not decide behavior.

@test "an uninstalled openhue is a silent no-op" {
  BIN="$(path_without openhue)" run pulse 0
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -s "$OPENHUE_ARGV" ]
}

@test "an uninstalled jq is a silent no-op" {
  BIN="$(path_without jq)" run pulse 0
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ "$(write_call_count)" -eq 0 ]
}

@test "a room name that matches nothing stops before anything else is asked of the bridge" {
  # Both halves are the one behavior: an unknown room ends the run there. The
  # light query is what says so, because the empty-snapshot guard further down
  # would swallow an unknown room too and leave "no light was touched" true for
  # the wrong reason.
  HUE_PULSE_ROOMS='No Such Room' run pulse 0
  [ "$status" -eq 0 ]
  [ "$(write_call_count)" -eq 0 ]
  run grep -qF 'get light' "$OPENHUE_ARGV"
  [ "$status" -ne 0 ]
}

@test "a room holding no lights is left alone rather than pulsed" {
  # The snapshot comes back empty, and a pulse with nothing to restore would
  # strand the room on whatever the last phase set.
  HUE_PULSE_ROOMS='Closet' run pulse 0
  [ "$status" -eq 0 ]
  [ "$(write_call_count)" -eq 0 ]
}

# --- the serialize lock ----------------------------------------------------

@test "a pulse already in flight is skipped rather than interleaved" {
  [[ -x /usr/bin/lockf ]] || skip "no /usr/bin/lockf: the kernel lock is a darwin-only guarantee"
  mkdir -p "$HOME/.local/state"
  # Hold the exact kernel lock a running pulse holds, from this process, for the
  # whole of the second pulse's run. `-t 0` makes the contention immediate, so
  # this needs no background process to race and no sleep to settle.
  exec 8>>"$HOME/.local/state/hue-pulse.lockf"
  /usr/bin/lockf -s -t 0 8
  run pulse 0
  exec 8>&-
  [ "$status" -eq 0 ]
  [ ! -s "$OPENHUE_ARGV" ]
}

@test "every configured room pulses in ONE openhue call, so they flash together" {
  # openhue takes up to ten rooms per call, so both rooms change in a single
  # request rather than two. Sequential calls would visibly stagger the flash,
  # and the point of pulsing the kitchen and the studio together is that they
  # read as one signal.
  HUE_PULSE_ROOMS=$'3F - Studio\nKitchen' pulse 0
  openhue_called 'set room studio-id kitchen-id --on -x 0.17 -y 0.7 --brightness 70 --transition-time 1200ms'
}

@test "a configured room that does not exist is skipped, and the rest still pulse" {
  HUE_PULSE_ROOMS=$'3F - Studio\nNo Such Room' pulse 0
  openhue_called 'set room studio-id --on -x 0.17 -y 0.7 --brightness 70 --transition-time 1200ms'
}

@test "lights in EVERY configured room are restored, not just the first" {
  HUE_PULSE_ROOMS=$'3F - Studio\nKitchen' pulse 0
  openhue_called 'set light kitchen-strip --on --brightness 100 -x 0.3 -y 0.3 --transition-time 500ms'
}
