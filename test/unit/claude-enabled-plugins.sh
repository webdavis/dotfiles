#!/usr/bin/env bash
# claude-enabled-plugins.sh, the settings modify-template must RENDER a complete
# enabledPlugins object, on every operating system this repository targets.
#
# WHY THIS EXISTS. modify_settings.json writes enabledPlugins with
# setValueAtPath, which REPLACES the value at that path rather than merging into
# it. So a plugin that is enabled live but absent from the dict is turned OFF by
# the next apply, with no message. Measured 2026-07-30: three plugins
# (codex@openai-codex, ponytail@ponytail, rust-analyzer-lsp@claude-plugins-official)
# were enabled on the machine and absent from the dict, so the next apply would
# have disabled all three. That is the failure this pins.
#
# WHY IT RENDERS INSTEAD OF READING THE SOURCE TEXT. The first version of this
# test matched the dict in the template's SOURCE, which approves a template that
# does not render the set it appears to declare. Two shapes, both measured green
# under source matching on 2026-07-31: moving one entry into a `{{- /* ... */ -}}`
# comment placed after the dict (source reads 9 entries, the render emits 8, no
# message), and appending a stray `(` to an entry (source reads 9 entries,
# chezmoi dies with `unclosed left paren` and applies nothing at all). So this
# test applies the template for real and asserts on the resulting JSON.
#
# WHY THE FIXTURE IS ASSERTED BEFORE THE APPLY. The sharpest assertion here is
# an ABSENCE: the plugin that is enabled live and undeclared must be GONE
# afterwards. Absence is also what you get when the fixture never carried that
# plugin, so an unchecked fixture turns the assertion into a tautology. Measured
# 2026-08-01: empty the fixture's enabledPlugins and change the template to
# MERGE instead of replace, and the unguarded version reports `OK, 9 plugins
# rendered` at exit 0. That is precisely the production regression this file
# exists to prevent. So the fixture's own contents are asserted first, and every
# verdict after the apply is conditional on that.
#
# WHY IT RENDERS ONCE PER TARGET OPERATING SYSTEM. The declaration is a
# template, so an entry can sit behind an `if eq .chezmoi.os` guard. Measured
# 2026-08-01: gating one entry on darwin keeps a darwin host green while a Linux
# apply of the same source drops that plugin, which disables it. A test that
# renders only for its own host cannot see that, and CI runs on macOS only.
# chezmoi reads the operating system from template data, so the sandbox config
# sets `data.chezmoi.os` per render. Measured on chezmoi 2.62.3 (what the flake
# and CI provide) and 2.71.1 (Homebrew, what a developer shell has): both honour
# it, including while evaluating .chezmoiignore. The newer `--override-data`
# flag does the same thing and is NOT usable here, 2.62.3 rejects it as an
# unknown flag. Only `os` is overridden, so this pins OS-CONDITIONAL
# DECLARATIONS rather than simulating a Linux machine.
#
# HOW IT STAYS HERMETIC. It never reads or writes the operator's
# ~/.claude/settings.json, which CI does not have and which no test may depend
# on. It applies the ONE managed target into a throwaway destination whose
# settings.json is a fixture written here, with chezmoi's config, persistent
# state and cache all redirected into the same sandbox.
#
# WHAT --config BUYS AND WHAT THE COPIED SOURCE BUYS. Two separate defences, and
# an earlier version of this comment credited the wrong one. --config is the
# hook boundary: the operator's ~/.config/chezmoi/chezmoi.toml declares
# [hooks.read-source-state.pre] running .install-password-manager.sh, which runs
# `brew install --cask keepassxc` when keepassxc-cli is absent, and that hook
# fires on ANY source-state read, from a copied source directory just as readily
# as from this checkout (measured 2026-08-01, chezmoi --debug logs
# `Run cmd=.../.install-password-manager.sh` for the copied source). Pointing
# --config at a sandbox config is what stops it. The source's own
# .chezmoi.toml.tmpl is not read as configuration outside `chezmoi init`, so it
# was never the cause: applying the FULL checkout with an explicit sandbox
# --config fires no hook at all, though that template declares one. What the
# copied source buys is narrowing: the sandbox source state manages 2 entries
# (the .claude directory and the target inside it) where the checkout manages
# 301, so an unrelated source entry cannot change or break this test.
#
# WHY chezmoi AND NOT jq PARSES THE RESULT. chezmoi is the one tool this test
# cannot do without, and the flake's `run` shell (which CI uses) provides it.
# That shell does not provide jq, so parsing the render with chezmoi's own
# template engine keeps the dependency list at exactly one.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT

# The managed target, relative to a destination directory.
readonly SETTINGS_TARGET_RELATIVE_PATH='.claude/settings.json'

# The source files copied into the sandbox source directory, relative to this
# checkout. The modify-template produces the target; .chezmoiignore decides
# whether it is managed at all (measured: chezmoi exits 1 with `not managed`
# when the ignore list covers the target, so dropping the file from the target
# state is caught here rather than shipping as a silent no-op).
readonly -a SANDBOX_SOURCE_FILES=(
  'private_dot_claude/modify_settings.json'
  '.chezmoiignore'
)

# Every operating system this repository targets. The enabled set is meant to be
# the same on all of them, so each one gets its own apply and its own verdict.
readonly -a TARGET_OPERATING_SYSTEMS=(
  'darwin'
  'linux'
)

# The fixture target file the modify-template reads on .chezmoi.stdin. Two
# deliberate contents, each pinning a property of the merge:
#
#   PASSTHROUGH_SETTING_KEY   a field chezmoi does not manage. It must SURVIVE
#                             the apply. If it does not, the fixture was never
#                             read on stdin and every other assertion here is
#                             measuring a render this repo does not perform.
#   LIVE_BUT_UNDECLARED_PLUGIN  a plugin enabled live and absent from the dict.
#                             It must be GONE after the apply. That is the
#                             whole-value replace this test exists to bound, and
#                             asserting it means the reason for the completeness
#                             requirement is checked rather than commented.
readonly PASSTHROUGH_SETTING_KEY='voiceEnabled'
readonly LIVE_BUT_UNDECLARED_PLUGIN='ghost-plugin@no-such-marketplace'

# The plugins this repository intends to keep enabled. Editing this list is the
# deliberate act; the RENDER must agree with it exactly, in both directions.
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

# Reports are `<record>:<kind>:<value>:<name>`, name LAST so that a name
# containing the delimiter lands whole in the final field and every earlier
# field still holds its own column. Splitting these on whitespace would collapse
# an empty value and shift the name left. A value can only carry the delimiter
# when its kind is not `bool`, which already fails the assertions below, so the
# ordering never turns a real defect into a pass.
readonly REPORT_FIELD_DELIMITER=':'
readonly PLUGIN_RECORD='plugin'
readonly PASSTHROUGH_RECORD='passthrough'

# An entry must be the JSON boolean true, and `%v` prints the string "true" and
# the boolean true identically, so the render's TYPE is reported alongside the
# value and asserted with it. Read 2026-08-01 out of the shipped Claude Code
# 2.1.220 binary, whose settings schema declares
# `enabledPlugins: record(string, union([array(string), boolean, undefined]))`:
# a quoted "true" is not in that union, so it enables nothing, and a value-only
# check accepts it anyway. This repository declares booleans, so `bool` is what
# is pinned here rather than the schema's whole union.
readonly JSON_BOOLEAN_KIND='bool'
readonly JSON_TRUE_VALUE='true'

# Build structured inputs through the template engine rather than by pasting
# values into a JSON string, so a name carrying a quote or a backslash is
# escaped by the JSON writer instead of producing a broken fixture that fails
# for the wrong reason.
# shellcheck disable=SC2016 # a Go template: $-names and {{ }} are template
# syntax evaluated by chezmoi, not shell expansions. Double quotes here would
# expand them to nothing.
readonly FIXTURE_SETTINGS_TEMPLATE='
{{- $fixture := dict
    (env "CLAUDE_PASSTHROUGH_SETTING_KEY") true
    "enabledPlugins" (dict (env "CLAUDE_LIVE_BUT_UNDECLARED_PLUGIN") true) -}}
{{ $fixture | toPrettyJson }}'

# The sandbox chezmoi config for one render. `data.chezmoi.os` is what makes the
# apply happen for a target operating system other than the host's; the file is
# otherwise empty, which is what keeps the operator's read-source-state hook out
# of this test. JSON rather than TOML so chezmoi's own writer produces it and
# the format follows from the .json extension.
# shellcheck disable=SC2016 # a Go template, as above.
readonly SANDBOX_CONFIG_TEMPLATE='
{{- dict "data" (dict "chezmoi"
    (dict "os" (env "CLAUDE_TARGET_OPERATING_SYSTEM"))) | toJson -}}'

# One line per enabledPlugins entry plus one for the passthrough field. `index`
# rather than `.field` because chezmoi errors on a missing key with the field
# form, and an absent key must reach the assertions below (which name it) rather
# than die inside the template with a message about map entries. `kindOf`
# reports the JSON type: `bool` for true, `string` for "true", `invalid` for an
# absent key.
# shellcheck disable=SC2016 # a Go template, as above.
readonly SETTINGS_REPORT_TEMPLATE='
{{- $settings := fromJson (env "CLAUDE_RENDERED_SETTINGS_JSON") -}}
{{- $delimiter := env "CLAUDE_REPORT_FIELD_DELIMITER" -}}
{{- $passthroughKey := env "CLAUDE_PASSTHROUGH_SETTING_KEY" -}}
{{- $passthroughValue := index $settings $passthroughKey -}}
{{- range $name, $value := (index $settings "enabledPlugins") -}}
{{ printf "%s%s%s%s%v%s%s\n" (env "CLAUDE_PLUGIN_RECORD") $delimiter
   (kindOf $value) $delimiter $value $delimiter $name -}}
{{ end -}}
{{ printf "%s%s%s%s%v%s%s\n" (env "CLAUDE_PASSTHROUGH_RECORD") $delimiter
   (kindOf $passthroughValue) $delimiter $passthroughValue $delimiter
   $passthroughKey -}}'

failures=0
verified_plugin_count=0
fail() {
  printf 'FAIL: %s\n' "$1" >&2
  failures=$((failures + 1))
}

finish() {
  if ((failures > 0)); then
    printf '\nclaude-enabled-plugins: %d failure(s)\n' "$failures" >&2
    exit 1
  fi
  printf 'claude-enabled-plugins: OK, %d plugins rendered enabled and matching the expected set for each target OS (%s)\n' \
    "$verified_plugin_count" "${TARGET_OPERATING_SYSTEMS[*]}"
  exit 0
}

# chezmoi is not optional here. Skipping when it is absent would turn this guard
# off silently, which is the same class of failure it exists to catch; the
# flake's `run` shell ships chezmoi, so a hard failure names a real setup problem.
command -v chezmoi >/dev/null 2>&1 || {
  printf 'FAIL: chezmoi is not on PATH, so the modify-template cannot be rendered. Run inside the flake run shell: nix develop .#run\n' >&2
  exit 1
}

sandbox="$(mktemp -d)"
readonly sandbox
trap 'rm -rf "$sandbox"' EXIT

readonly SANDBOX_SOURCE="$sandbox/source"
readonly SANDBOX_PERSISTENT_STATE="$sandbox/chezmoistate.boltdb"
readonly SANDBOX_CACHE="$sandbox/cache"

# The config used for rendering ad-hoc templates, which read their inputs from
# the environment and never touch the source state or the operating system. It
# is empty on purpose: it exists only so that --config never resolves to the
# operator's config.
readonly BOOTSTRAP_CONFIG="$sandbox/bootstrap-chezmoi.toml"

# Per-operating-system sandbox paths. The layout lives in these three functions
# and nowhere else.
os_config_path() {
  printf '%s/config/%s.json' "$sandbox" "$1"
}

os_destination_directory() {
  printf '%s/destination/%s' "$sandbox" "$1"
}

os_settings_path() {
  printf '%s/%s' "$(os_destination_directory "$1")" "$SETTINGS_TARGET_RELATIVE_PATH"
}

os_apply_stderr_path() {
  printf '%s/apply-stderr-%s.txt' "$sandbox" "$1"
}

# chezmoi_isolated <config-path> <args>... -- chezmoi with every piece of its
# state redirected into the sandbox. Each flag closes a specific leak, and all
# four are load bearing:
#
#   --source            without it chezmoi reads the operator's DEFAULT source
#                       directory, which is a different checkout on a different
#                       branch.
#   --config            without it chezmoi loads the operator's
#                       ~/.config/chezmoi/chezmoi.toml. That config declares
#                       [hooks.read-source-state.pre] running
#                       .install-password-manager.sh, which runs
#                       `brew install --cask keepassxc` when keepassxc-cli is
#                       absent. Measured 2026-08-01: the hook fires on a bare
#                       `chezmoi execute-template`, not just on apply, and a
#                       copied source directory does not stop it. A test may not
#                       install software, and it may not behave one way on the
#                       operator's machine and another on a runner.
#   --persistent-state  without it chezmoi writes the operator's state database.
#   --cache             without it chezmoi writes the operator's cache.
chezmoi_isolated() {
  local config_path="$1"
  shift
  chezmoi \
    --source "$SANDBOX_SOURCE" \
    --config "$config_path" \
    --persistent-state "$SANDBOX_PERSISTENT_STATE" \
    --cache "$SANDBOX_CACHE" \
    --no-tty \
    "$@"
}

# render_template <template> -- run one self-contained Go template through
# chezmoi. The template needs no source tree and no target operating system: it
# reads its inputs from the environment.
render_template() {
  chezmoi_isolated "$BOOTSTRAP_CONFIG" execute-template "$1"
}

# populate_sandbox_source -- copy the source files under test into the sandbox
# source directory. A missing one is a hard failure: an absent modify-template
# would otherwise leave the target unmanaged, which reads as a chezmoi error
# about the wrong thing.
populate_sandbox_source() {
  local relative_path
  for relative_path in "${SANDBOX_SOURCE_FILES[@]}"; do
    [[ -r "$REPO_ROOT/$relative_path" ]] || {
      printf 'FAIL: cannot read %s\n' "$REPO_ROOT/$relative_path" >&2
      exit 1
    }
    mkdir -p "$SANDBOX_SOURCE/$(dirname "$relative_path")"
    cp "$REPO_ROOT/$relative_path" "$SANDBOX_SOURCE/$relative_path"
  done
}

# write_sandbox_config <operating-system> -- the chezmoi config that makes the
# next apply render for that operating system.
write_sandbox_config() {
  local operating_system="$1" config_path
  config_path="$(os_config_path "$operating_system")"
  mkdir -p "$(dirname "$config_path")"
  CLAUDE_TARGET_OPERATING_SYSTEM="$operating_system" \
    render_template "$SANDBOX_CONFIG_TEMPLATE" >"$config_path"
}

# write_fixture_settings <operating-system> -- the pre-apply target file the
# modify-template reads.
write_fixture_settings() {
  local settings_path
  settings_path="$(os_settings_path "$1")"
  mkdir -p "$(dirname "$settings_path")"
  CLAUDE_PASSTHROUGH_SETTING_KEY="$PASSTHROUGH_SETTING_KEY" \
    CLAUDE_LIVE_BUT_UNDECLARED_PLUGIN="$LIVE_BUT_UNDECLARED_PLUGIN" \
    render_template "$FIXTURE_SETTINGS_TEMPLATE" >"$settings_path"
}

# apply_settings_target <operating-system> -- apply the ONE managed target into
# that operating system's sandbox destination, the way a real apply does it: the
# modify-template receives the fixture written above on .chezmoi.stdin. An
# ABSOLUTE target path keeps the result independent of the caller's working
# directory, against which chezmoi resolves a relative one. Externals are never
# refreshed, so this stays offline even if the source tree gains one later.
apply_settings_target() {
  local operating_system="$1"
  chezmoi_isolated "$(os_config_path "$operating_system")" \
    --destination "$(os_destination_directory "$operating_system")" \
    --refresh-externals=never \
    --force \
    apply "$(os_settings_path "$operating_system")"
}

# settings_report <operating-system> -- the enabledPlugins entries and the
# passthrough field of that operating system's settings file, one delimited
# record per line. Called twice per operating system, once on the fixture and
# once on the applied result, because the same questions are asked of both.
settings_report() {
  CLAUDE_RENDERED_SETTINGS_JSON="$(cat "$(os_settings_path "$1")")" \
  CLAUDE_REPORT_FIELD_DELIMITER="$REPORT_FIELD_DELIMITER" \
  CLAUDE_PLUGIN_RECORD="$PLUGIN_RECORD" \
  CLAUDE_PASSTHROUGH_RECORD="$PASSTHROUGH_RECORD" \
  CLAUDE_PASSTHROUGH_SETTING_KEY="$PASSTHROUGH_SETTING_KEY" \
    render_template "$SETTINGS_REPORT_TEMPLATE"
}

# Findings of the most recent parse_settings_report call. Bash cannot return an
# array, so the parser publishes under names that say where they came from.
parsed_plugin_names=()
parsed_non_boolean_true_plugins=()
parsed_passthrough_kind=''
parsed_passthrough_value=''
parsed_passthrough_record_count=0

# parse_settings_report <report> -- split one report into the findings above.
parse_settings_report() {
  local record kind value name
  parsed_plugin_names=()
  parsed_non_boolean_true_plugins=()
  parsed_passthrough_kind=''
  parsed_passthrough_value=''
  parsed_passthrough_record_count=0
  while IFS="$REPORT_FIELD_DELIMITER" read -r record kind value name; do
    [[ -n $record ]] || continue
    case "$record" in
      "$PLUGIN_RECORD")
        parsed_plugin_names+=("$name")
        [[ $kind == "$JSON_BOOLEAN_KIND" && $value == "$JSON_TRUE_VALUE" ]] ||
          parsed_non_boolean_true_plugins+=("$name=$kind($value)")
        ;;
      "$PASSTHROUGH_RECORD")
        parsed_passthrough_kind="$kind"
        parsed_passthrough_value="$value"
        parsed_passthrough_record_count=$((parsed_passthrough_record_count + 1))
        ;;
      *) fail "unrecognised report record '$record'; the extraction template and this parser disagree" ;;
    esac
  done <<<"$1"
}

# assert_passthrough_field <operating-system> <stage> <diagnosis> -- the
# unmanaged field must be present exactly once and be the JSON boolean true.
# Terminal, because a settings file without it is not the file this test thinks
# it is measuring.
assert_passthrough_field() {
  local operating_system="$1" stage="$2" diagnosis="$3"
  if ((parsed_passthrough_record_count != 1)); then
    fail "[$operating_system] the $stage settings produced $parsed_passthrough_record_count $PASSTHROUGH_RECORD records and not 1; the extraction template is not reporting what this parser reads"
    finish
  fi
  if [[ $parsed_passthrough_kind != "$JSON_BOOLEAN_KIND" || $parsed_passthrough_value != "$JSON_TRUE_VALUE" ]]; then
    fail "[$operating_system] the $stage settings do not carry the unmanaged field $PASSTHROUGH_SETTING_KEY as the JSON boolean true (kind '$parsed_passthrough_kind', value '$parsed_passthrough_value'); $diagnosis"
    finish
  fi
}

# assert_fixture_precondition <operating-system> -- the pre-apply fixture must
# be exactly what the post-apply assertions assume. Without this the absence
# check after the apply passes for a fixture that never carried the undeclared
# plugin, which is how a merge-instead-of-replace template renders green.
assert_fixture_precondition() {
  local operating_system="$1"
  assert_passthrough_field "$operating_system" 'pre-apply fixture' \
    'the fixture builder did not write it, so this test does not control the file it is about to apply over'
  if ((${#parsed_plugin_names[@]} != 1)) ||
    [[ ${parsed_plugin_names[0]} != "$LIVE_BUT_UNDECLARED_PLUGIN" ]]; then
    fail "[$operating_system] the pre-apply fixture must enable exactly one plugin, $LIVE_BUT_UNDECLARED_PLUGIN, and it enables [${parsed_plugin_names[*]-}]; the post-apply check that this plugin is GONE would otherwise pass whatever the template does, including a merge that keeps every live plugin"
    finish
  fi
  if ((${#parsed_non_boolean_true_plugins[@]} > 0)); then
    fail "[$operating_system] the pre-apply fixture does not enable $LIVE_BUT_UNDECLARED_PLUGIN with the JSON boolean true: ${parsed_non_boolean_true_plugins[*]}"
    finish
  fi
}

# assert_rendered_enabled_plugins <operating-system> -- the render itself.
assert_rendered_enabled_plugins() {
  local operating_system="$1"
  local expected_sorted rendered_sorted missing extra

  assert_passthrough_field "$operating_system" 'applied' \
    'either the modify-template stopped passing unmanaged fields through, or it never received the fixture on .chezmoi.stdin and this test is not exercising the apply path'

  # An empty extraction makes every comparison below vacuously true, which is how
  # this whole class of guard fails. Refuse it before comparing anything.
  if ((${#parsed_plugin_names[@]} == 0)); then
    fail "[$operating_system] the applied settings declare no enabled plugins at all"
    finish
  fi

  # The write is whole-value, so a plugin enabled live and absent from the dict
  # is gone after the apply. That is why the dict has to be complete, and it is
  # the reason this test's expected set is a closed list rather than a minimum.
  if printf '%s\n' "${parsed_plugin_names[@]}" | grep -qxF "$LIVE_BUT_UNDECLARED_PLUGIN"; then
    fail "[$operating_system] the fixture's undeclared plugin $LIVE_BUT_UNDECLARED_PLUGIN survived the apply; enabledPlugins is no longer a whole-value write, so the assumptions in this test and in the template's comment are both stale"
  fi

  # A `false` is worse than an omission: it reads like a declaration while doing
  # the opposite. A quoted "true" is worse still, since it also SURVIVES a `%v`
  # comparison against the boolean.
  if ((${#parsed_non_boolean_true_plugins[@]} > 0)); then
    fail "[$operating_system] the render sets these plugins to something other than the JSON boolean true: ${parsed_non_boolean_true_plugins[*]}"
  fi

  # Both directions. A missing entry disables a working plugin at the next apply;
  # an extra one enables something nobody chose.
  expected_sorted="$(printf '%s\n' "${EXPECTED_ENABLED_PLUGINS[@]}" | sort)"
  rendered_sorted="$(printf '%s\n' "${parsed_plugin_names[@]}" | sort)"
  missing="$(comm -23 <(printf '%s\n' "$expected_sorted") <(printf '%s\n' "$rendered_sorted"))"
  extra="$(comm -13 <(printf '%s\n' "$expected_sorted") <(printf '%s\n' "$rendered_sorted"))"

  if [[ -n $missing ]]; then
    fail "[$operating_system] expected these plugins to render enabled, and the template does not produce them (an apply would DISABLE them): $(tr '\n' ' ' <<<"$missing")"
  fi
  if [[ -n $extra ]]; then
    fail "[$operating_system] the template renders plugins that are not in the expected set; add them to EXPECTED_ENABLED_PLUGINS deliberately or remove them: $(tr '\n' ' ' <<<"$extra")"
  fi

  verified_plugin_count="${#parsed_plugin_names[@]}"
}

# verify_render_for_operating_system <operating-system> -- one whole cycle:
# write the fixture, prove the fixture, apply, prove the render.
verify_render_for_operating_system() {
  local operating_system="$1"
  local fixture_report applied_report apply_stderr

  write_sandbox_config "$operating_system"
  write_fixture_settings "$operating_system"

  fixture_report="$(settings_report "$operating_system")" || {
    fail "[$operating_system] the fixture this test just wrote is not readable as JSON, so the fixture builder is broken and no verdict below would mean anything"
    finish
  }
  parse_settings_report "$fixture_report"
  assert_fixture_precondition "$operating_system"

  # A failed apply is terminal: nothing downstream can be asserted about a file
  # that was never written. This is what catches a template that parses as a
  # complete dict but does not compile, and a target that stopped being managed
  # (measured: chezmoi exits 1 with `not managed` when .chezmoiignore covers it).
  apply_stderr="$(os_apply_stderr_path "$operating_system")"
  if ! apply_settings_target "$operating_system" 2>"$apply_stderr"; then
    fail "[$operating_system] chezmoi could not apply $SETTINGS_TARGET_RELATIVE_PATH from this checkout: $(tr '\n' ' ' <"$apply_stderr")"
    finish
  fi

  applied_report="$(settings_report "$operating_system")" || {
    fail "[$operating_system] the applied $SETTINGS_TARGET_RELATIVE_PATH could not be read back as JSON; chezmoi's own error is on stderr above"
    finish
  }
  parse_settings_report "$applied_report"
  assert_rendered_enabled_plugins "$operating_system"
}

: >"$BOOTSTRAP_CONFIG"
populate_sandbox_source

for target_operating_system in "${TARGET_OPERATING_SYSTEMS[@]}"; do
  verify_render_for_operating_system "$target_operating_system"
done

finish
