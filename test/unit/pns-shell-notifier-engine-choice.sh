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
  printf 'printf "%%s\\n" "$@" >>"%s/calls"\n' "$scratch"
} >"$home/.local/libexec/pns/pns"
chmod +x "$home/.local/libexec/pns/pns"

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

fail() {
  echo "$1" >&2
  exit 1
}

# --- no config: the notification fires, the pulse does not -----------------
calls="$(fire 400)"
grep -qxF -- '--agent' <<<"$calls" || fail "the long tier must notify; got: $calls"
grep -qxF -- '--pane' <<<"$calls" || fail "the pane must ride along; got: $calls"
grep -qxF -- 'wW:p7' <<<"$calls" || fail "the pane id must ride along; got: $calls"
grep -qxF -- 'pulse' <<<"$calls" && fail "no config means no pulse; got: $calls"
grep -qxF -- '--local-only' <<<"$calls" && fail "no narrowing flag may appear; got: $calls"

# --- with the config, the pulse runs as the engine's subcommand ------------
mkdir -p "$home/.config/pns"
printf '[plugins.hue]\nenabled = true\n' >"$home/.config/pns/config.toml"
calls="$(fire 400)"
grep -qxF -- 'pulse' <<<"$calls" || fail "the config turns the pulse on; got: $calls"

# --- the tier boundaries, exactly ------------------------------------------
calls="$(fire 300)"
grep -qxF -- 'pulse' <<<"$calls" || fail "300s is the pulse tier; got: $calls"
calls="$(fire 299)"
grep -qxF -- 'pulse' <<<"$calls" && fail "299s must not pulse; got: $calls"
grep -qxF -- '--agent' <<<"$calls" || fail "299s still notifies; got: $calls"
calls="$(fire 30)"
grep -qxF -- '--agent' <<<"$calls" || fail "30s notifies; got: $calls"
calls="$(fire 29)"
[[ -z $calls ]] || fail "nothing may fire below 30s; got: $calls"

exit 0
