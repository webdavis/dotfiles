#!/usr/bin/env bash
# The shell notifier must reach the RUST engine once the binary is installed
# and the BASH engine until then: an apply installs the binary under a shell
# that is already running, and that shell must not lose its notifications to
# a path that does not exist yet. The pulse is separate, because the engine's
# pulse mode needs a pns config the machine may not have yet.
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

# A HOME with a space, because a flat command string would word-split here and
# silently drop the call.
home="$scratch/home dir"

stub() { # <path> <label>
  mkdir -p "$(dirname "$1")"
  # One line per ARGUMENT, so a broken argument boundary cannot read the same
  # as separate arguments.
  {
    printf '#!/usr/bin/env bash\n'
    printf 'printf "%s <<%%s>>\\n" "$@" >>"%s/calls"\n' "$2" "$scratch"
  } >"$1"
  chmod +x "$1"
}

stub "$home/.local/libexec/pns/relay.sh" BASH-RELAY
stub "$home/.local/libexec/pns/channels/hue-pulse.sh" BASH-PULSE

# fire <elapsed>: run one notification and collect what the stubs recorded.
# The calls are DETACHED subshells the shell cannot wait for, so this settles
# on a quiet period rather than stopping at the first write: stopping early
# would let a later unwanted call escape the negative assertions.
fire() {
  : >"$scratch/calls"
  HOME="$home" SECONDS="$1" HERDR_PANE_ID=wW:p7 bash --noprofile --norc -c "
    source '$scratch/notifier.sh'
    __cmd_notify_start=0
    __cmd_notify_name='sleep 999'
    (exit 0)
    __cmd_notify_precmd
  " >/dev/null 2>&1 || true
  local previous="" current="" stable=0
  for _ in $(seq 1 40); do
    sleep 0.05
    current="$(cat "$scratch/calls" 2>/dev/null || true)"
    if [[ $current == "$previous" ]]; then
      stable=$((stable + 1))
      [[ $stable -ge 4 ]] && break
    else
      stable=0
    fi
    previous="$current"
  done
  printf '%s' "$current"
}

expect_call() { # <label> <calls> <arg>...
  local label="$1" calls="$2"
  shift 2
  local argument
  for argument in "$@"; do
    grep -qxF "$label <<$argument>>" <<<"$calls" || {
      echo "expected $label to be handed <<$argument>>" >&2
      echo "got:" >&2
      printf '%s\n' "$calls" >&2
      exit 1
    }
  done
}

refute_label() { # <label> <calls>
  if grep -q "^$1 " <<<"$2"; then
    echo "$1 must not have run" >&2
    printf '%s\n' "$2" >&2
    exit 1
  fi
}

# --- the binary is not installed yet: the bash engine still carries it -----
calls="$(fire 400)"
expect_call BASH-RELAY "$calls" --agent shell --state 'done' --pane wW:p7
expect_call BASH-PULSE "$calls" 0

# --- the binary is installed: it carries the notification ------------------
# The pulse stays with the bash channel, because no pns config exists here.
stub "$home/.local/libexec/pns/pns" BINARY
calls="$(fire 400)"
expect_call BINARY "$calls" --agent shell --state 'done' --project "${PWD##*/}" \
  --detail "sleep (400s)" --pane wW:p7
refute_label BASH-RELAY "$calls"
if grep -qxF 'BINARY <<--local-only>>' <<<"$calls"; then
  echo "a narrowing flag would silently drop every long-command phone push" >&2
  exit 1
fi
expect_call BASH-PULSE "$calls" 0

# --- with a pns config, the pulse becomes the binary's own subcommand ------
mkdir -p "$home/.config/pns"
printf '[plugins.hue]\nenabled = true\n' >"$home/.config/pns/config.toml"
calls="$(fire 400)"
expect_call BINARY "$calls" pulse 0
refute_label BASH-PULSE "$calls"
rm -r "$home/.config/pns"

# --- the tier boundaries, exactly ------------------------------------------
calls="$(fire 300)"
expect_call BASH-PULSE "$calls" 0
calls="$(fire 299)"
refute_label BASH-PULSE "$calls"
expect_call BINARY "$calls" --agent shell
calls="$(fire 30)"
expect_call BINARY "$calls" --agent shell
refute_label BASH-PULSE "$calls"
calls="$(fire 29)"
[[ -z $calls ]] || {
  echo "nothing may fire below the 30 second tier" >&2
  printf '%s\n' "$calls" >&2
  exit 1
}

exit 0
