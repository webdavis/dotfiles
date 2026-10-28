#!/usr/bin/env bash
# Refuses the agent data files when a plugin or skill would reach one harness twice.
# Every rule spans both files, so it reads both from the repository root.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly plugins_file="$repo_root/.chezmoidata/agent-plugins.yaml"
readonly skills_file="$repo_root/.chezmoidata/agent-skills.yaml"

problems=0

problem() {
  printf 'agent-data-validate: %s\n' "$*" >&2
  problems=$((problems + 1))
}

duplicates_in() {
  sort | uniq -d
}

plugins_json() {
  yq -o json '.plugins.harnesses' "$plugins_file"
}

skills_json() {
  yq -o json '.skills.harnesses' "$skills_file"
}

check_marketplace_suffixes() {
  local line
  while IFS= read -r line; do
    problem "plugin ids under marketplace $line carry different @suffixes; the suffix is the marketplace name"
  done < <(plugins_json | jq -r '
    [.claude, .codex, .codex_and_claude] | add | .[]
    | select(([.plugins[] | split("@") | last] | unique | length) > 1) | .marketplace')
}

check_duplicate_plugin_ids() {
  local id
  while IFS= read -r id; do
    problem "plugin id $id is listed twice in agent-plugins.yaml"
  done < <(plugins_json | jq -r '([.claude, .codex, .codex_and_claude] | add | .[].plugins[]), .claude_builtins[]' | duplicates_in)
}

check_duplicate_marketplaces() {
  local harness groups repo
  for harness in claude codex; do
    groups="[.$harness, .codex_and_claude] | add"
    while IFS= read -r repo; do
      problem "marketplace $repo reaches $harness twice in agent-plugins.yaml"
    done < <(plugins_json | jq -r "$groups | .[].marketplace" | duplicates_in)
  done
}

check_duplicate_skill_sources() {
  local entry
  while IFS= read -r entry; do
    problem "$entry is listed twice in agent-skills.yaml"
  done < <(skills_json | jq -r '
    [.claude, .codex, .codex_and_claude] | map(to_entries[] | .value[]) | .[]
    | (.repo // .slug // .skill // empty)' | duplicates_in)
}

check_duplicate_hermes_identifiers() {
  local identifier
  while IFS= read -r identifier; do
    problem "Hermes identifier $identifier appears in two profiles in agent-skills.yaml"
  done < <(skills_json | jq -r '.hermes | to_entries[] | .value[]' | duplicates_in)
}

# A whole-repository npx entry (owner/repo, no path) and a marketplace from the
# same repository would install the same skills twice into one harness.
check_npx_against_marketplaces() {
  local harness repo
  for harness in claude codex; do
    while IFS= read -r repo; do
      problem "$repo reaches $harness as a plugin and as an npx install"
    done < <(
      {
        plugins_json | jq -r "[.$harness, .codex_and_claude] | add | .[].marketplace" | sort -u
        skills_json | jq -r "[.$harness, .codex_and_claude] | map(.npx // [] | .[]) | .[].repo | select((split(\"/\") | length) == 2)" | sort -u
      } | duplicates_in
    )
  done
}

main() {
  local file
  for file in "$plugins_file" "$skills_file"; do
    if [[ ! -f $file ]]; then
      problem "$file is missing"
      exit 1
    fi
  done
  check_marketplace_suffixes
  check_duplicate_plugin_ids
  check_duplicate_marketplaces
  check_duplicate_skill_sources
  check_duplicate_hermes_identifiers
  check_npx_against_marketplaces
  if ((problems > 0)); then
    exit 1
  fi
}

main "$@"
