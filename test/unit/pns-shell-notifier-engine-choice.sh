#!/usr/bin/env bash
# The shell notifier must reach the RUST engine once the binary is installed
# and the BASH engine until then: an apply installs the binary under a shell
# that is already running, and that shell must not lose its notifications to
# a path that does not exist yet. The pulse follows the same rule.
#
# The function is extracted from the RENDERED bashrc rather than a copy, so a
# repoint that edits one and not the other fails here.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$REPO_ROOT/dot_bashrc.tmpl" >"$scratch/bashrc" 2>/dev/null

# The notifier function alone, dedented out of its interactive-shell block.
sed -n '/^  __cmd_notify_precmd() {$/,/^  }$/p' "$scratch/bashrc" |
  sed 's/^  //' >"$scratch/notifier.sh"
[[ -s $scratch/notifier.sh ]] || {
  echo "could not extract __cmd_notify_precmd from the rendered bashrc" >&2
  exit 1
}

stub() { # <path> <label>
  mkdir -p "$(dirname "$1")"
  printf '#!/usr/bin/env bash\nprintf "%%s %%s\\n" "%s" "$*" >>"%s/calls"\n' "$2" "$scratch" >"$1"
  chmod +x "$1"
}

home="$scratch/home"
stub "$home/.local/libexec/pns/relay.sh" BASH-RELAY
stub "$home/.local/libexec/pns/channels/hue-pulse.sh" BASH-PULSE

# fire <elapsed>: run one notification with that duration and collect the
# calls the stubs recorded.
fire() {
  : >"$scratch/calls"
  HOME="$home" SECONDS="$1" bash --noprofile --norc -c "
    source '$scratch/notifier.sh'
    __cmd_notify_start=0
    __cmd_notify_name='sleep 999'
    (exit 0)
    __cmd_notify_precmd
    wait
  " >/dev/null 2>&1 || true
  # The calls are backgrounded subshells; give them a moment to land.
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    [[ -s $scratch/calls ]] && break
    sleep 0.1
  done
  sort "$scratch/calls" 2>/dev/null || true
}

# --- the binary is not installed yet: the bash engine still carries it -----
calls="$(fire 400)"
grep -q '^BASH-RELAY --agent shell' <<<"$calls" || {
  echo "without the binary the bash engine must still be called; got: $calls" >&2
  exit 1
}
grep -q '^BASH-PULSE 0' <<<"$calls" || {
  echo "without the binary the bash pulse must still fire; got: $calls" >&2
  exit 1
}

# --- the binary is installed: it wins, and the pulse is its subcommand -----
stub "$home/.local/libexec/pns/pns" BINARY
calls="$(fire 400)"
grep -q '^BINARY --agent shell' <<<"$calls" || {
  echo "the installed binary must carry the notification; got: $calls" >&2
  exit 1
}
grep -q '^BINARY pulse 0' <<<"$calls" || {
  echo "the pulse must be the binary's own subcommand; got: $calls" >&2
  exit 1
}
grep -q '^BASH' <<<"$calls" && {
  echo "no bash script may run once the binary is installed; got: $calls" >&2
  exit 1
}

# --- the 30s tier notifies without pulsing ---------------------------------
calls="$(fire 60)"
grep -q '^BINARY --agent shell' <<<"$calls" || {
  echo "the 30s tier must notify; got: $calls" >&2
  exit 1
}
grep -q 'pulse' <<<"$calls" && {
  echo "the 30s tier must not pulse the lights; got: $calls" >&2
  exit 1
}

exit 0
