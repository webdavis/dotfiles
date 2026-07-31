#!/usr/bin/env bash
# claude-enabled-plugins.sh, the settings modify-template's enabledPlugins dict
# must be a COMPLETE list of the plugins meant to stay on.
#
# WHY THIS EXISTS. modify_settings.json writes the dict with setValueAtPath,
# which REPLACES the value at that path rather than merging into it. So a plugin
# that is enabled live but absent from the dict is turned OFF by the next apply,
# with no message. Measured 2026-07-30: three plugins (codex@openai-codex,
# ponytail@ponytail, rust-analyzer-lsp@claude-plugins-official) were enabled on
# the machine and absent from the dict, so the cutover apply would have disabled
# all three. That is the failure this pins.
#
# WHAT IT CANNOT DO. It cannot read the live machine, because tests must not
# depend on one machine's state and CI has no ~/.claude/settings.json. So it
# pins the two properties that ARE static: the dict is syntactically well formed
# and every entry is enabled (never a stray `false`, which reads as a
# declaration but is the opposite of one), and the roster it declares matches a
# committed expected set that a human has to edit deliberately. Adding a plugin
# is then a two-line change, and forgetting the second line fails here rather
# than silently disabling something months later.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly MODIFY_TEMPLATE="$REPO_ROOT/private_dot_claude/modify_settings.json"

# The plugins this repository intends to keep enabled. Editing this list is the
# deliberate act; the template must agree with it exactly, in both directions.
readonly -a EXPECTED_ENABLED_PLUGINS=(
  'codex@openai-codex'
  'document-skills@anthropic-agent-skills'
  'frontend-design@claude-plugins-official'
  'playwright@claude-plugins-official'
  'ponytail@ponytail'
  'rust-analyzer-lsp@claude-plugins-official'
  'security-guidance@claude-plugins-official'
  'superpowers@claude-plugins-official'
  'swift-lsp@claude-plugins-official'
)

failures=0
fail() {
  printf 'FAIL: %s\n' "$1" >&2
  failures=$((failures + 1))
}

[[ -r $MODIFY_TEMPLATE ]] || {
  printf 'FAIL: cannot read %s\n' "$MODIFY_TEMPLATE" >&2
  exit 1
}

# The dict body: from the `$enabledPlugins := dict` line to the setValueAtPath
# that consumes it. Anchored on both ends so a reordering of the file cannot
# silently widen or empty the range.
#
# shellcheck disable=SC2016  # the single quotes are required: `$enabledPlugins`
# is a literal Go-template variable name being matched inside the file, not a
# shell expansion. Double-quoting it would expand to the empty string and the
# range would start at line 1.
plugin_block() {
  sed -n '/\$enabledPlugins := dict/,/setValueAtPath "enabledPlugins"/p' "$MODIFY_TEMPLATE"
}

block="$(plugin_block)"

# A block that matched nothing would make every later assertion vacuously true,
# which is the way this whole class of guard fails. Refuse that first.
if [[ -z $block ]]; then
  fail 'found no enabledPlugins dict in the modify-template; the assertions below would all pass vacuously'
  printf '\nclaude-enabled-plugins: %d failure(s)\n' "$failures" >&2
  exit 1
fi
if ! grep -q 'setValueAtPath "enabledPlugins"' <<<"$block"; then
  fail 'the enabledPlugins dict is not terminated by its setValueAtPath call; the extracted range is wrong'
fi

# Every declared entry, in the form "<name>" <value>.
declared_names=()
declared_disabled=()
while IFS= read -r line; do
  [[ $line =~ \"([^\"]+@[^\"]+)\"[[:space:]]+([a-z]+) ]] || continue
  declared_names+=("${BASH_REMATCH[1]}")
  [[ ${BASH_REMATCH[2]} == 'true' ]] || declared_disabled+=("${BASH_REMATCH[1]}")
done <<<"$block"

if ((${#declared_names[@]} == 0)); then
  fail 'the enabledPlugins dict declares no plugins at all'
fi

# A `false` here is worse than an omission: it reads like a declaration while
# doing the opposite, so it survives a casual review of the list.
if ((${#declared_disabled[@]} > 0)); then
  fail "the enabledPlugins dict sets these to something other than true: ${declared_disabled[*]}"
fi

# Both directions. A missing entry disables a working plugin at the next apply;
# an extra one enables something nobody chose.
expected_sorted="$(printf '%s\n' "${EXPECTED_ENABLED_PLUGINS[@]}" | sort)"
declared_sorted="$(printf '%s\n' "${declared_names[@]}" | sort)"

missing="$(comm -23 <(printf '%s\n' "$expected_sorted") <(printf '%s\n' "$declared_sorted"))"
extra="$(comm -13 <(printf '%s\n' "$expected_sorted") <(printf '%s\n' "$declared_sorted"))"

if [[ -n $missing ]]; then
  fail "expected these plugins to be declared enabled, and the template does not declare them (an apply would DISABLE them): $(tr '\n' ' ' <<<"$missing")"
fi
if [[ -n $extra ]]; then
  fail "the template declares plugins that are not in the expected set; add them to EXPECTED_ENABLED_PLUGINS deliberately or remove them: $(tr '\n' ' ' <<<"$extra")"
fi

if ((failures > 0)); then
  printf '\nclaude-enabled-plugins: %d failure(s)\n' "$failures" >&2
  exit 1
fi

printf 'claude-enabled-plugins: OK, %d plugins declared enabled and matching the expected set\n' \
  "${#declared_names[@]}"
