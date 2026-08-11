#!/usr/bin/env bash
# The shell notifier calls the engine binary, and the pulse follows the
# CONFIG: `pns pulse` runs only when ~/.config/pns/config.toml exists,
# because without an enabled hue table it would silently do nothing and the
# lights would just stop.
#
# The function is extracted from the RENDERED bashrc rather than a copy, so a
# repoint that edits one and not the other fails here.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$REPO_ROOT/dot_bashrc.tmpl" >"$scratch/bashrc" 2>/dev/null

sed -n '/^  __cmd_notify_precmd() {$/,/^  }$/p' "$scratch/bashrc" |
  sed 's/^  //' >"$scratch/notifier.sh"
[[ -s $scratch/notifier.sh ]] || {
  echo "could not extract __cmd_notify_precmd from the rendered bashrc" >&2
  exit 1
}

# A HOME with a space, because these paths are interpolated everywhere.
home="$scratch/home dir"
mkdir -p "$home/.local/libexec/pns"
{
  printf '#!/usr/bin/env bash\n'
  # shellcheck disable=SC2016  # CALLS_FILE must expand when the STUB runs
  printf 'printf "%%s\\n" "$@" >>"${CALLS_FILE:-%s/calls}"\n' "$scratch"
} >"$home/.local/libexec/pns/pns"
chmod +x "$home/.local/libexec/pns/pns"

# All six scenarios run inside ONE bash process, each writing its own calls
# file, with a single settle at the end: six spawns each with their own
# settle loop put the file over the repo's one-second rule.
run_scenarios() {
  HOME="$home" HERDR_PANE_ID=wW:p7 bash --noprofile --norc -c "
    source '$scratch/notifier.sh'
    fire() { # <elapsed> <calls-file>
      export CALLS_FILE=\"\$2\"
      SECONDS=\$1
      __cmd_notify_start=0
      __cmd_notify_name='sleep 999'
      (exit 0)
      __cmd_notify_precmd
    }
    fire 400 '$scratch/calls-noconfig'
    mkdir -p '$home/.config/pns'
    printf '[plugins.hue]\nenabled = true\n' >'$home/.config/pns/config.toml'
    fire 400 '$scratch/calls-config'
    fire 300 '$scratch/calls-300'
    fire 299 '$scratch/calls-299'
    fire 30 '$scratch/calls-30'
    fire 29 '$scratch/calls-29'
  " >/dev/null 2>&1 || true
  # One settle for every detached write the six scenarios spawned.
  sleep 0.15
}

fail() {
  echo "$1" >&2
  exit 1
}

run_scenarios

# --- no config: the notification fires, the pulse does not -----------------
calls="$(cat "$scratch/calls-noconfig" 2>/dev/null || true)"
grep -qxF -- '--agent' <<<"$calls" || fail "the long tier must notify; got: $calls"
grep -qxF -- '--pane' <<<"$calls" || fail "the pane must ride along; got: $calls"
grep -qxF -- 'wW:p7' <<<"$calls" || fail "the pane id must ride along; got: $calls"
grep -qxF -- 'pulse' <<<"$calls" && fail "no config means no pulse; got: $calls"
grep -qxF -- '--local-only' <<<"$calls" && fail "no narrowing flag may appear; got: $calls"

# --- with the config, the pulse runs as the engine's subcommand ------------
calls="$(cat "$scratch/calls-config" 2>/dev/null || true)"
grep -qxF -- 'pulse' <<<"$calls" || fail "the config turns the pulse on; got: $calls"

# --- the tier boundaries, exactly ------------------------------------------
calls="$(cat "$scratch/calls-300" 2>/dev/null || true)"
grep -qxF -- 'pulse' <<<"$calls" || fail "300s is the pulse tier; got: $calls"
calls="$(cat "$scratch/calls-299" 2>/dev/null || true)"
grep -qxF -- 'pulse' <<<"$calls" && fail "299s must not pulse; got: $calls"
grep -qxF -- '--agent' <<<"$calls" || fail "299s still notifies; got: $calls"
calls="$(cat "$scratch/calls-30" 2>/dev/null || true)"
grep -qxF -- '--agent' <<<"$calls" || fail "30s notifies; got: $calls"
calls="$(cat "$scratch/calls-29" 2>/dev/null || true)"
[[ -z $calls ]] || fail "nothing may fire below 30s; got: $calls"

exit 0
