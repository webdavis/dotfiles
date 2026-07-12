# shellcheck shell=bash
# macos-defaults-lib.sh -- shared helpers for the macos-defaults-{apply,capture,
# drift} tools. Sourced, never executed. Deployed to ~/.local/bin alongside the
# tools; each sources it via
#   source "$(dirname "${BASH_SOURCE[0]}")/macos-defaults-lib.sh"
# which resolves in BOTH the chezmoi source tree (dot_local/bin/) and the applied
# ~/.local/bin layout, because this lib carries no executable_/dot_ rename, so its
# basename is identical in both.

# resolve_source_dir -- print the chezmoi source directory for the CURRENT context.
#
# Fixes the worktree-writes-primary bug: the tools used to hardcode the primary
# checkout ("${HOME}/workspaces/Ivy/webdavis/dotfiles"), so a `just defaults-*` run
# from a SECONDARY git worktree wrote (or read) the PRIMARY tree's YAML instead of
# the worktree the operator is standing in. Resolution order:
#   1. $MACOS_DEFAULTS_SOURCE_DIR, when set: an explicit override.
#   2. The current git worktree's top-level, when it carries this repo's
#      .chezmoidata/macos_defaults.yaml -- so a run from a secondary worktree
#      targets THAT worktree. It is routed through `chezmoi --source=<top>
#      source-path` so chezmoi normalizes and validates the path.
#   3. Otherwise chezmoi's configured source dir (`chezmoi source-path`).
resolve_source_dir() {
  if [[ -n ${MACOS_DEFAULTS_SOURCE_DIR:-} ]]; then
    printf '%s\n' "$MACOS_DEFAULTS_SOURCE_DIR"
    return 0
  fi
  local top
  if top="$(git rev-parse --show-toplevel 2>/dev/null)" &&
    [[ -f "$top/.chezmoidata/macos_defaults.yaml" ]]; then
    chezmoi --source="$top" source-path
    return 0
  fi
  chezmoi source-path
}

# macos_defaults_data_file -- print the resolved path to macos_defaults.yaml.
macos_defaults_data_file() {
  local src
  src="$(resolve_source_dir)" || return 1
  printf '%s/.chezmoidata/macos_defaults.yaml\n' "$src"
}

# require_readable_data_file <path> -- exit 2 with a message if the file is not
# readable (the shared "data file missing or unreadable" guard).
require_readable_data_file() {
  local file="$1"
  if [[ ! -r $file ]]; then
    printf 'error: cannot read %s\n' "$file" >&2
    exit 2
  fi
}

# defaults_records_tsv <path> -- emit each tracked record as one TSV line:
#   domain<TAB>key<TAB>type<TAB>value<TAB>host   (host empty when global).
# yq emits a single blank line for an empty array; callers skip the empty row.
defaults_records_tsv() {
  local file="$1"
  yq eval -r '.macos.defaults[] | [.domain, .key, .type, .value, (.host // "")] | @tsv' "$file"
}
