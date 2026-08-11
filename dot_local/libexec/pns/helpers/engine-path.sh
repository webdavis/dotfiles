# shellcheck shell=bash
# WHICH ENGINE a producer should call, resolved in one place.
#
# TRANSITIONAL BY DESIGN. Every producer prefers the Rust binary and falls
# back to the bash engine until an apply installs it, because a hook or a
# LaunchAgent can fire in the window between the two. When the retirement
# slice removes the bash engine this file goes with it and the call sites
# name the binary directly.
#
# The pulse resolves SEPARATELY from the engine: the binary's pulse mode
# needs an enabled [plugins.hue] carrying a bridge and key, so until that
# config exists the bash channel is the only thing that can light the room.

# pns_engine_path
# The engine a producer should exec. An override wins outright, so a caller
# with its own opinion (a test, a one-off) is never second-guessed.
pns_engine_path() {
  local override="${1:-}"
  if [[ -n $override ]]; then
    printf '%s' "$override"
    return 0
  fi
  # A regular file, not merely something with the execute bit: a DIRECTORY at
  # that path satisfies -x and would suppress the fallback entirely.
  local binary="${PNS_ENGINE_BIN:-${HOME:-}/.local/libexec/pns/pns}"
  if [[ -f $binary && -x $binary ]]; then
    printf '%s' "$binary"
    return 0
  fi
  printf '%s' "${HOME:-}/.local/libexec/pns/relay.sh"
}

# pns_pulse_command
# The command that pulses the lights, as NUL-separated ARGUMENTS on stdout:
# the binary carries a subcommand, a flat string would word-split on a home
# directory with a space, and a newline separator would split a path that
# contains one. Prints nothing when neither form is available, which the
# caller reads as "no lights on this machine".
pns_pulse_command() {
  local binary="${PNS_ENGINE_BIN:-${HOME:-}/.local/libexec/pns/pns}"
  local config="${PNS_CONFIG_FILE:-${HOME:-}/.config/pns/config.toml}"
  if [[ -f $binary && -x $binary && -f $config ]]; then
    printf '%s\0' "$binary" pulse
    return 0
  fi
  local channel="${PNS_CHANNELS_DIR:-${HOME:-}/.local/libexec/pns/channels}/hue-pulse.sh"
  [[ -f $channel && -x $channel ]] && printf '%s\0' "$channel"
  return 0
}
