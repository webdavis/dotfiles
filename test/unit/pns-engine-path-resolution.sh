#!/usr/bin/env bash
# Every producer resolves its engine through one function, so the transition
# window is decided once rather than copied per call site. Two rules:
#
#   the ENGINE prefers the binary and falls back to the bash relay, because a
#   hook can fire between the apply that installs the binary and the retirement
#   that removes the bash;
#
#   the PULSE follows the CONFIG, not the binary, because the binary's pulse
#   mode returns silently without an enabled [plugins.hue] and the lights would
#   simply stop.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

# A home with a space, because a flat command string would word-split here.
HOME="$scratch/home dir"
export HOME
mkdir -p "$HOME/.local/libexec/pns/channels" "$HOME/.config/pns"

# shellcheck source=dot_local/libexec/pns/helpers/engine-path.sh
source "$REPO_ROOT/dot_local/libexec/pns/helpers/engine-path.sh"

binary="$HOME/.local/libexec/pns/pns"
relay="$HOME/.local/libexec/pns/relay.sh"
channel="$HOME/.local/libexec/pns/channels/hue-pulse.sh"
config="$HOME/.config/pns/config.toml"

fail() {
  echo "$1" >&2
  exit 1
}

pulse_line() { pns_pulse_command | tr '\0' '|'; }

# --- the engine ------------------------------------------------------------
printf '#!/usr/bin/env bash\n' >"$relay"
chmod +x "$relay"
[[ "$(pns_engine_path)" == "$relay" ]] ||
  fail "without the binary the bash relay must carry it, got: $(pns_engine_path)"

printf '#!/usr/bin/env bash\n' >"$binary"
chmod +x "$binary"
[[ "$(pns_engine_path)" == "$binary" ]] ||
  fail "the installed binary must win, got: $(pns_engine_path)"

# A binary that is present but NOT executable is not an engine: a partial
# install must fall back rather than produce a call that cannot run.
chmod -x "$binary"
[[ "$(pns_engine_path)" == "$relay" ]] ||
  fail "a non-executable binary must fall back, got: $(pns_engine_path)"
chmod +x "$binary"

[[ "$(pns_engine_path /custom/engine)" == /custom/engine ]] ||
  fail "an explicit override must win outright"

# --- the pulse -------------------------------------------------------------
[[ "$(pulse_line)" == "" ]] ||
  fail "with no channel and no config there are no lights, got: $(pulse_line)"

printf '#!/usr/bin/env bash\n' >"$channel"
chmod +x "$channel"
[[ "$(pulse_line)" == "$channel|" ]] ||
  fail "without a config the bash channel keeps the lights, got: $(pulse_line)"

printf '[plugins.hue]\nenabled = true\n' >"$config"
[[ "$(pulse_line)" == "$binary|pulse|" ]] ||
  fail "with a config the binary's pulse mode wins, got: $(pulse_line)"

# The subcommand must survive a home with a space: reading the command back
# as separate lines is what makes that possible.
mapfile -t -d '' parts < <(pns_pulse_command)
[[ ${#parts[@]} -eq 2 && ${parts[0]} == "$binary" && ${parts[1]} == pulse ]] ||
  fail "the pulse command must be two arguments, got: ${parts[*]}"

rm "$config"
[[ "$(pulse_line)" == "$channel|" ]] ||
  fail "removing the config returns the lights to the bash channel"

# --- neither candidate may be a directory or a non-executable file ---------
# -x alone accepts a searchable DIRECTORY, which would suppress every
# fallback and leave the caller trying to execute it.
printf '[plugins.hue]\nenabled = true\n' >"$config"
chmod -x "$binary"
[[ "$(pulse_line)" == "$channel|" ]] ||
  fail "a non-executable binary must not claim the pulse, got: $(pulse_line)"
chmod +x "$binary"

mv "$binary" "$binary.real"
mkdir -p "$binary"
[[ "$(pulse_line)" == "$channel|" ]] ||
  fail "a DIRECTORY at the binary path must not claim the pulse, got: $(pulse_line)"
[[ "$(pns_engine_path)" == "$relay" ]] ||
  fail "a DIRECTORY at the binary path must not claim the engine either"
rmdir "$binary"
mv "$binary.real" "$binary"
rm "$config"

chmod -x "$channel"
[[ "$(pulse_line)" == "" ]] ||
  fail "a non-executable channel is no pulse at all, got: $(pulse_line)"
chmod +x "$channel"

# --- a stripped environment with explicit overrides still resolves ---------
# Hooks run with HOME sometimes absent; the overrides are what such a caller
# has, and expanding an unset HOME under set -u would abort the hook.
( set -u; unset HOME
  PNS_CHANNELS_DIR="$(dirname "$channel")" pns_pulse_command >/dev/null ) ||
  fail "an unset HOME with an explicit channels dir must still resolve"

exit 0
