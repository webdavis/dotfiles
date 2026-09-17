#!/usr/bin/env bash
#
# Run an apply and keep a transcript of it, so its outcome can be read
# afterwards instead of copied out of a terminal by hand.
#
# THE APPLY MUST NOT BE VERBOSE. `-v` prints a unified diff of every target it
# writes, and seventeen targets here render KeePassXC secrets, so a verbose
# transcript is a plaintext copy of them in the one file an agent is told to
# read. Measured 2026-09-16: `apply -v --dry-run` emitted 387 diff content
# lines, the same run without `-v` emitted none.
#
# NOTHING MAY REACH THE VAULT ON THE APPLY'S BEHALF. An earlier version
# recorded `chezmoi status` first with its output going only to the log:
# `status` renders templates, so it prompted for the passphrase into a file the
# operator cannot see, and the run hung.
#
# The filter is a BACKSTOP. With `-v` gone there should be nothing to withhold,
# and it stays because the cost of being wrong is unrecoverable.
#
# THE APPLY RUNS UNDER A PSEUDO-TERMINAL. chezmoi draws its passphrase prompt
# with a terminal widget that styles itself only when standard output is a
# terminal, so capturing through a pipe rendered the dimmed "password"
# placeholder as ordinary bright text. `script` gives the apply a real terminal
# and writes every byte to both the operator's screen and the capture, which
# restores the prompt chezmoi normally draws. A terminal also means colour and
# carriage returns in the capture, stripped on the way into the log.

set -uo pipefail

log_dir="${CHEZMOI_APPLY_LOG_DIR:-$HOME/.local/state/chezmoi-apply}"
repo_root="$(cd -- "${BASH_SOURCE[0]%/*}/.." && pwd)"

# Timestamp first, so a listing sorts chronologically. macOS ships BSD date,
# which has no --iso-8601, hence the explicit format. Colons are left out of
# the filename because they are not portable in a path.
stamp="$(date -u +"%Y-%m-%dT%H-%M-%SZ")"
log="$log_dir/$stamp.apply.log"

# The transcript names targets and carries error text, so everything this
# writes is created private. mkdir -p leaves an EXISTING directory's mode
# alone, which is why that one is still set explicitly.
umask 077
mkdir -p "$log_dir"
chmod 700 "$log_dir"

# The unfiltered capture, which lives only between the apply finishing and the
# filter running, and is removed on every exit path.
raw="$(mktemp "$log_dir/.$stamp.raw.XXXXXX")"
trap 'rm -f "$raw"' EXIT INT TERM

# Any line shaped like unified-diff content is withheld and counted. The
# '--- ' and '+++ ' prefixes are kept because they name PATHS, not content.
is_content() {
  case $1 in
    '--- '* | '+++ '*) return 1 ;;
    '+'* | '-'* | ' '*) return 0 ;;
    *) return 1 ;;
  esac
}

# Appends the capture to the log, withholding content lines and counting them.
# `|| [[ -n $line ]]` catches a final line with no trailing newline, which for
# an error message is the line that matters most. One sed strips the terminal
# escape sequences for the whole file, rather than a subprocess per line: the
# character class covers colour (ESC[0m), erase (ESC[2K) and the private
# cursor and mouse modes chezmoi's prompt uses (ESC[?25l, ESC[?2004h).
filter_into_log() {
  local source_file=$1 line withheld=0
  while IFS= read -r line || [[ -n $line ]]; do
    if is_content "$line"; then
      ((withheld++))
      continue
    fi
    if ((withheld > 0)); then
      printf '    [%d content line(s) withheld]\n' "$withheld" >>"$log"
      withheld=0
    fi
    printf '%s\n' "$line" >>"$log"
  done < <(sed -E $'s/\033\\[[0-9;?]*[a-zA-Z]//g; s/\r//g' "$source_file")
  if ((withheld > 0)); then
    printf '    [%d content line(s) withheld]\n' "$withheld" >>"$log"
  fi
}

{
  printf 'chezmoi apply\n'
  printf 'started:    %s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  printf 'repo:       %s\n' "$repo_root"
  printf 'git HEAD:   %s\n' "$(git -C "$repo_root" rev-parse --short HEAD 2>/dev/null || printf 'unknown')"
  printf 'git branch: %s\n' "$(git -C "$repo_root" rev-parse --abbrev-ref HEAD 2>/dev/null || printf 'unknown')"
  printf '\napply output:\n'
} >>"$log"

# `script` needs a terminal to attach the new one to. Without it (a test, or
# anything non-interactive) the plain capture is taken instead: there is no
# prompt to render in that case, and the transcript still has to be written.
if [[ -t 1 ]]; then
  script -q "$raw" chezmoi apply
  status=$?
else
  chezmoi apply 2>&1 | tee "$raw"
  status=${PIPESTATUS[0]}
fi

filter_into_log "$raw"

{
  printf '\nfinished:   %s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  printf 'exit_code:  %d\n' "$status"
  if ((status == 0)); then
    printf 'result:     OK\n'
  else
    printf 'result:     FAILED\n'
  fi
} >>"$log"

# A stable path to the newest run, so a reader needs no globbing. A symlink
# rather than a copy, so there is one set of bytes to protect.
ln -sfn "$log" "$log_dir/latest.apply.log"

printf '\napply exit %d, log: %s\n' "$status" "$log_dir/latest.apply.log" >&2
exit "$status"
