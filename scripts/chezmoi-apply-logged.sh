#!/usr/bin/env bash
# Logs each apply without its diff lines, because some deployed files hold secrets.
# The apply runs inside `script` so the passphrase prompt still gets a terminal.

set -uo pipefail

log_dir="${CHEZMOI_APPLY_LOG_DIR:-$HOME/.local/state/chezmoi-apply}"
repo_root="$(cd -- "${BASH_SOURCE[0]%/*}/.." && pwd)"

# Timestamp first so a listing sorts by time; dashes keep the path portable.
stamp="$(date -u +"%Y-%m-%dT%H-%M-%SZ")"
log="$log_dir/$stamp.apply.log"

# The transcript names targets and carries error text, so keep it private.
# mkdir -p leaves an existing directory's mode alone, so set it explicitly.
umask 077
mkdir -p "$log_dir"
chmod 700 "$log_dir"

# The unfiltered capture, removed on every exit path.
raw="$(mktemp "$log_dir/.$stamp.raw.XXXXXX")"
trap 'rm -f "$raw"' EXIT INT TERM

# Leaves out unified-diff content but keeps '--- ' and '+++ ' lines, because they name paths.
is_content() {
  case $1 in
    '--- '* | '+++ '*) return 1 ;;
    '+'* | '-'* | ' '*) return 0 ;;
    *) return 1 ;;
  esac
}

# `|| [[ -n $line ]]` catches a final line with no trailing newline. The sed strips terminal escape sequences.
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

# `script` needs a terminal to attach its new one to; without one, capture
# through a pipe.
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

ln -sfn "$log" "$log_dir/latest.apply.log"

printf '\napply exit %d, log: %s\n' "$status" "$log_dir/latest.apply.log" >&2
exit "$status"
