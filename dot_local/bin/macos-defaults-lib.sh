# shellcheck shell=bash
# macos-defaults-lib.sh, shared helpers for the macos-defaults-{apply,capture,
# drift} tools. Sourced, never executed, so it carries no shebang and no
# executable bit.
#
# Each tool sources it as
#   source "$(dirname "${BASH_SOURCE[0]}")/macos-defaults-lib.sh"
# which resolves in BOTH the chezmoi source tree (dot_local/bin/) and the applied
# ~/.local/bin/ layout: this file carries no executable_ or dot_ prefix, so chezmoi
# deploys it under the same basename its siblings are deployed beside.

# resolve_source_dir, print the chezmoi source directory for the CURRENT context.
#
# Resolution order, most specific first:
#   1. $MACOS_DEFAULTS_SOURCE_DIR when SET, an explicit caller override. Set but
#      empty is a caller error, not "unset", so it is rejected rather than skipped.
#   2. The chezmoi source tree containing the current directory, so a run from a
#      secondary worktree targets THAT worktree rather than the primary checkout.
#      It is routed through `chezmoi --source=<top> source-path` so chezmoi
#      normalizes the path.
#   3. Otherwise chezmoi's configured source directory.
#
# Every failure returns nonzero with a message rather than falling through to the
# next rule: falling back after a failed chezmoi call would silently retarget a
# different checkout, which is the class of bug this resolver exists to end.
#
# Two rules keep that promise, and both close a way an earlier version broke it:
#
#   The source tree is identified by its .chezmoiversion marker, NOT by the data
#   file. Those are different questions. A source tree whose macos_defaults.yaml is
#   absent is STILL this tree, and must report a missing data file for the tree the
#   caller is standing in. Keying on the data file made an absent file look like
#   "some unrelated directory" and silently resolved whichever other checkout did
#   have one, reintroducing the exact bug this resolver exists to end.
#
#   The worktree is resolved with git's context variables SCRUBBED. `git rev-parse`
#   honors $GIT_DIR and $GIT_WORK_TREE, so an exported value from a git hook or a
#   wrapper made the resolver describe a checkout the caller was not in. Unsetting
#   them inside the command substitution binds the answer to the physical directory.
#
# Residual, stated rather than hidden: if git fails for a reason OTHER than an
# inherited context variable, a corrupt repository being the realistic one, the
# tree cannot be identified and resolution falls to chezmoi's configured source.
# That case is not reproduced here and is left as the documented limit.
resolve_source_dir() {
  if [[ -n ${MACOS_DEFAULTS_SOURCE_DIR+x} ]]; then
    if [[ -z $MACOS_DEFAULTS_SOURCE_DIR ]]; then
      printf 'error: MACOS_DEFAULTS_SOURCE_DIR is set but empty; refusing to resolve another checkout\n' >&2
      return 1
    fi
    printf '%s\n' "$MACOS_DEFAULTS_SOURCE_DIR"
    return 0
  fi

  local worktree_top resolved
  worktree_top="$(
    unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE
    git rev-parse --show-toplevel 2>/dev/null
  )"
  if [[ -n $worktree_top && -f "$worktree_top/.chezmoiversion" ]]; then
    if ! resolved="$(chezmoi --source="$worktree_top" source-path)"; then
      printf 'error: chezmoi --source=%s source-path failed; refusing to fall back to another checkout\n' \
        "$worktree_top" >&2
      return 1
    fi
    printf '%s\n' "$resolved"
    return 0
  fi

  if ! resolved="$(chezmoi source-path)"; then
    printf 'error: chezmoi source-path failed; the chezmoi source directory is unknown\n' >&2
    return 1
  fi
  printf '%s\n' "$resolved"
}

# macos_defaults_data_file, print the resolved path to macos_defaults.yaml.
# Returns 2, the tools' shared "data file missing or unreadable" status, when the
# source directory cannot be resolved. An empty resolution is a failure too: it
# would otherwise compose into a plausible-looking /.chezmoidata/... path.
macos_defaults_data_file() {
  local source_dir
  source_dir="$(resolve_source_dir)" || return 2
  if [[ -z $source_dir ]]; then
    printf 'error: resolved an empty chezmoi source directory for macos_defaults.yaml\n' >&2
    return 2
  fi
  printf '%s/.chezmoidata/macos_defaults.yaml\n' "$source_dir"
}

# require_readable_data_file <path>, the shared readable-data-file guard. Returns
# 2 with a message naming the file, so the caller's exit status matches the "data
# file missing or unreadable" contract documented in each tool's header.
require_readable_data_file() { # <path>
  local data_file="$1"
  if [[ ! -r $data_file ]]; then
    printf 'error: cannot read %s\n' "$data_file" >&2
    return 2
  fi
}

# defaults_records_tsv <path>, emit each tracked record as one tab-separated line:
#   domain<TAB>key<TAB>type<TAB>value<TAB>host    (host empty when global).
# yq emits a single blank line for an empty array, which callers skip.
#
# KNOWN LIMITATION, carried unchanged from the pre-extraction code so this
# refactor stays behavior-preserving. Callers read with IFS=$'\t', and tab is IFS
# *whitespace*, so bash collapses runs of tabs. Only a TRAILING empty field
# survives: an empty host reads back empty as intended. An empty INTERIOR field
# does not. A record with an empty value would collapse its two adjacent tabs and
# shift host left into value, applying the host string as the setting's value
# against the global domain. No tracked record has an empty value today, so this
# is latent rather than live. The fix is a non-whitespace delimiter and is its own
# slice, which now changes this one function instead of every caller.
defaults_records_tsv() { # <path>
  local data_file="$1"
  yq eval -r '.macos.defaults[] | [.domain, .key, .type, .value, (.host // "")] | @tsv' "$data_file"
}

# defaults_records_unit_separated <path>, emit each tracked record as one line
# of SEVEN fields joined by the ASCII unit separator (0x1f):
#   domain, key, type, value, host, scope, plist_path
# host and plist_path are empty when absent; an ABSENT scope defaults to
# "user" here, so a scope that reaches a caller empty was explicitly empty in
# the record (a record error, rejected by validate_record_scope below). The
# unit separator is not IFS whitespace, so an empty INTERIOR field survives
# `IFS=$'\x1f' read` intact, unlike the tab-separated stream above, whose
# collapse is exactly why the new optional columns do not extend it. A value
# containing a literal 0x1f byte would still split; no macOS preference value
# carries one.
defaults_records_unit_separated() { # <path>
  local data_file="$1"
  local unit_separator=$'\x1f'
  yq eval -r ".macos.defaults[] | [.domain, .key, .type, .value, (.host // \"\"), (.scope // \"user\"), (.plist_path // \"\")] | join(\"$unit_separator\")" "$data_file"
}

# validate_record_scope <scope> <host> <plist_path>, print the validated scope.
# Rejects, with a message and nonzero status, every combination that would
# otherwise be silently misapplied:
#   - a scope other than user/system, including the set-but-empty scope ""
#     (defaults_records_unit_separated already turned an ABSENT field into
#     "user", so an empty scope here was explicitly empty in the record);
#   - scope system with a host: ByHost storage is per-user, the pair is
#     meaningless;
#   - scope user with a plist_path: the path is only honored on system
#     records, and accepting it would silently write the user domain instead
#     of the named file.
validate_record_scope() { # <scope> <host> <plist_path>
  local scope="$1" host="$2" plist_path="$3"
  case "$scope" in
    user | system) ;;
    *)
      printf 'error: unknown scope %q (expected user or system)\n' "$scope" >&2
      return 1
      ;;
  esac
  if [[ $scope == system && -n $host ]]; then
    printf 'error: scope system cannot be combined with host %q; ByHost storage is per-user\n' "$host" >&2
    return 1
  fi
  if [[ $scope == user && -n $plist_path ]]; then
    printf 'error: plist_path %q is only honored on scope system records\n' "$plist_path" >&2
    return 1
  fi
  printf '%s\n' "$scope"
}

# resolve_system_plist_path <domain> <plist_path>, print the plist path a
# system-scope record writes to and reads from. An empty declared path means
# the default, /Library/Preferences/<domain>. A declared path must be
# ABSOLUTE: a relative path would resolve against whatever directory the tool
# happens to run from, so it is rejected, never resolved.
resolve_system_plist_path() { # <domain> <plist_path>
  local domain="$1" plist_path="$2"
  if [[ -z $plist_path ]]; then
    printf '/Library/Preferences/%s\n' "$domain"
    return 0
  fi
  if [[ $plist_path != /* ]]; then
    printf 'error: relative plist_path %q (domain %s); an absolute path is required\n' \
      "$plist_path" "$domain" >&2
    return 1
  fi
  printf '%s\n' "$plist_path"
}

# system_defaults_write <plist_path> <key> <type> <value>, one system-scope
# write. /Library plists are root-owned, so the write goes through sudo;
# keeping it here keeps apply and any future caller on one code path.
system_defaults_write() { # <plist_path> <key> <type> <value>
  sudo defaults write "$1" "$2" "-$3" "$4"
}

# system_defaults_read_actual <plist_path> <key>, the three-outcome
# system-scope read for drift. Prints exactly one of:
#   - the live value, when `defaults read` succeeds;
#   - "<unset>", ONLY when defaults itself reports the domain/default pair
#     does not exist, the one failure that genuinely means "not set";
#   - "<unreadable>", up front when the plist file exists but this user cannot
#     read it (defaults would answer from a stale cache or misreport), and for
#     every other read failure. Unknown failures land here, never in
#     "<unset>": collapsing them would report drift against a value nobody
#     actually read, and skipping them would hide the record entirely.
# Always returns 0; the outcome is the printed marker. Documented limit: a
# plist whose PARENT directory blocks traversal cannot be file-checked, so
# that case rides on the stderr classification alone.
system_defaults_read_actual() { # <plist_path> <key>
  local plist_path="$1" key="$2"
  local file_candidate
  for file_candidate in "$plist_path" "$plist_path.plist"; do
    if [[ -e $file_candidate && ! -r $file_candidate ]]; then
      printf '<unreadable>'
      return 0
    fi
  done
  local value read_error_file read_status=0
  read_error_file="$(mktemp)"
  value="$(defaults read "$plist_path" "$key" 2>"$read_error_file")" || read_status=$?
  if [[ $read_status -eq 0 ]]; then
    rm -f "$read_error_file"
    printf '%s' "$value"
    return 0
  fi
  if grep -q 'does not exist' "$read_error_file"; then
    rm -f "$read_error_file"
    printf '<unset>'
    return 0
  fi
  rm -f "$read_error_file"
  printf '<unreadable>'
}
