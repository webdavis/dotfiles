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
#   1. $MACOS_DEFAULTS_SOURCE_DIR when set, an explicit caller override.
#   2. The current git worktree's top level, when that top level carries a
#      .chezmoidata/macos_defaults.yaml, so a run from a secondary worktree targets
#      THAT worktree rather than the primary checkout. It is routed through
#      `chezmoi --source=<top> source-path` so chezmoi normalizes the path.
#   3. Otherwise chezmoi's configured source directory.
#
# Every failure returns nonzero with a message rather than falling through to the
# next rule: falling back after a failed chezmoi call would silently retarget a
# different checkout, which is the class of bug this resolver exists to end.
resolve_source_dir() {
  if [[ -n ${MACOS_DEFAULTS_SOURCE_DIR:-} ]]; then
    printf '%s\n' "$MACOS_DEFAULTS_SOURCE_DIR"
    return 0
  fi

  local worktree_top resolved
  if worktree_top="$(git rev-parse --show-toplevel 2>/dev/null)" &&
    [[ -f "$worktree_top/.chezmoidata/macos_defaults.yaml" ]]; then
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
