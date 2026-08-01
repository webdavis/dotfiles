#!/usr/bin/env bash
# claude-enabled-plugins.sh, the settings modify-template must RENDER a complete
# enabledPlugins object.
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
# HOW IT STAYS HERMETIC. It never reads or writes the operator's
# ~/.claude/settings.json, which CI does not have and which no test may depend
# on. It applies the ONE managed target into a throwaway destination whose
# settings.json is a fixture written here, with chezmoi's config, persistent
# state and cache all redirected into the same sandbox.
#
# WHY IT APPLIES A COPIED SOURCE DIRECTORY RATHER THAN THIS CHECKOUT. Applying
# from the checkout itself pulls in .chezmoi.toml.tmpl, whose
# [hooks.read-source-state.pre] runs .install-password-manager.sh on every
# source-state read. That script runs `brew install --cask keepassxc` when
# keepassxc-cli is absent, which is exactly the state of a CI runner (measured
# 2026-07-31: with a de-homebrewed PATH the hook fails the run outright, and
# with brew present it would install a cask). A test may not install software.
# So the sandbox source holds copies of the only two source files that decide
# whether and how .claude/settings.json is produced: the modify-template and
# .chezmoiignore.
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

# Reports are `<record>:<value>:<name>`, value BEFORE name so that a name
# containing the delimiter lands whole in the final field and an empty value
# still holds its own column. Splitting these on whitespace would collapse an
# empty value and shift the name left.
readonly REPORT_FIELD_DELIMITER=':'
readonly PLUGIN_RECORD='plugin'
readonly PASSTHROUGH_RECORD='passthrough'

# Build the fixture through the template engine rather than by pasting values
# into a JSON string, so a name carrying a quote or a backslash is escaped by
# the JSON writer instead of producing a broken fixture that fails for the wrong
# reason.
# shellcheck disable=SC2016 # a Go template: $-names and {{ }} are template
# syntax evaluated by chezmoi, not shell expansions. Double quotes here would
# expand them to nothing.
readonly FIXTURE_SETTINGS_TEMPLATE='
{{- $fixture := dict
    (env "CLAUDE_PASSTHROUGH_SETTING_KEY") true
    "enabledPlugins" (dict (env "CLAUDE_LIVE_BUT_UNDECLARED_PLUGIN") true) -}}
{{ $fixture | toPrettyJson }}'

# One line per enabledPlugins entry plus one for the passthrough field. `index`
# rather than `.field` because chezmoi errors on a missing key with the field
# form, and an absent key must reach the assertions below (which name it) rather
# than die inside the template with a message about map entries.
# shellcheck disable=SC2016 # a Go template, as above.
readonly SETTINGS_REPORT_TEMPLATE='
{{- $settings := fromJson (env "CLAUDE_RENDERED_SETTINGS_JSON") -}}
{{- $delimiter := env "CLAUDE_REPORT_FIELD_DELIMITER" -}}
{{- range $name, $value := (index $settings "enabledPlugins") -}}
{{ printf "%s%s%v%s%s\n" (env "CLAUDE_PLUGIN_RECORD") $delimiter $value $delimiter $name -}}
{{ end -}}
{{ printf "%s%s%v%s%s\n" (env "CLAUDE_PASSTHROUGH_RECORD") $delimiter
   (index $settings (env "CLAUDE_PASSTHROUGH_SETTING_KEY")) $delimiter
   (env "CLAUDE_PASSTHROUGH_SETTING_KEY") -}}'

failures=0
rendered_plugins=()
fail() {
  printf 'FAIL: %s\n' "$1" >&2
  failures=$((failures + 1))
}

finish() {
  if ((failures > 0)); then
    printf '\nclaude-enabled-plugins: %d failure(s)\n' "$failures" >&2
    exit 1
  fi
  printf 'claude-enabled-plugins: OK, %d plugins rendered enabled and matching the expected set\n' \
    "${#rendered_plugins[@]}"
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
readonly SANDBOX_DESTINATION="$sandbox/destination"
readonly SANDBOX_SETTINGS="$SANDBOX_DESTINATION/$SETTINGS_TARGET_RELATIVE_PATH"
readonly SANDBOX_CONFIG="$sandbox/chezmoi.toml"
readonly SANDBOX_PERSISTENT_STATE="$sandbox/chezmoistate.boltdb"
readonly SANDBOX_CACHE="$sandbox/cache"

# chezmoi_isolated <args>... -- chezmoi with every piece of its state redirected
# into the sandbox. Each flag closes a specific leak, and all four are load
# bearing:
#
#   --source            without it chezmoi reads the operator's DEFAULT source
#                       directory, which is a different checkout on a different
#                       branch.
#   --config            without it chezmoi loads the operator's
#                       ~/.config/chezmoi/chezmoi.toml. That config declares
#                       [hooks.read-source-state.pre] running
#                       .install-password-manager.sh, which runs
#                       `brew install --cask keepassxc` when keepassxc-cli is
#                       absent. Measured 2026-07-31: the hook fires on a bare
#                       `chezmoi execute-template`, not just on apply. A test
#                       may not install software, and it may not behave one way
#                       on the operator's machine and another on a runner.
#   --persistent-state  without it chezmoi writes the operator's state database.
#   --cache             without it chezmoi writes the operator's cache.
chezmoi_isolated() {
  chezmoi \
    --source "$SANDBOX_SOURCE" \
    --config "$SANDBOX_CONFIG" \
    --persistent-state "$SANDBOX_PERSISTENT_STATE" \
    --cache "$SANDBOX_CACHE" \
    --no-tty \
    "$@"
}

# render_template <template> -- run one self-contained Go template through
# chezmoi. The template needs no source tree of its own; it reads its inputs
# from the environment.
render_template() {
  chezmoi_isolated execute-template "$1"
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

# write_fixture_settings -- the pre-apply target file the modify-template reads.
write_fixture_settings() {
  mkdir -p "$(dirname "$SANDBOX_SETTINGS")"
  CLAUDE_PASSTHROUGH_SETTING_KEY="$PASSTHROUGH_SETTING_KEY" \
    CLAUDE_LIVE_BUT_UNDECLARED_PLUGIN="$LIVE_BUT_UNDECLARED_PLUGIN" \
    render_template "$FIXTURE_SETTINGS_TEMPLATE" >"$SANDBOX_SETTINGS"
}

# apply_settings_target -- apply the ONE managed target into the sandbox, the
# way a real apply does it: the modify-template receives the fixture written
# above on .chezmoi.stdin. An ABSOLUTE target path keeps the result independent
# of the caller's working directory, against which chezmoi resolves a relative
# one. Externals are never refreshed, so this stays offline even if the source
# tree gains one later.
apply_settings_target() {
  chezmoi_isolated \
    --destination "$SANDBOX_DESTINATION" \
    --refresh-externals=never \
    --force \
    apply "$SANDBOX_SETTINGS"
}

# settings_report -- the rendered enabledPlugins entries and the passthrough
# field, one delimited record per line.
settings_report() {
  CLAUDE_RENDERED_SETTINGS_JSON="$(cat "$SANDBOX_SETTINGS")" \
  CLAUDE_REPORT_FIELD_DELIMITER="$REPORT_FIELD_DELIMITER" \
  CLAUDE_PLUGIN_RECORD="$PLUGIN_RECORD" \
  CLAUDE_PASSTHROUGH_RECORD="$PASSTHROUGH_RECORD" \
  CLAUDE_PASSTHROUGH_SETTING_KEY="$PASSTHROUGH_SETTING_KEY" \
    render_template "$SETTINGS_REPORT_TEMPLATE"
}

: >"$SANDBOX_CONFIG"
populate_sandbox_source
write_fixture_settings

# A failed apply is terminal: nothing downstream can be asserted about a file
# that was never written. This is what catches a template that parses as a
# complete dict but does not compile, and a target that stopped being managed
# (measured: chezmoi exits 1 with `not managed` when .chezmoiignore covers it).
apply_stderr="$sandbox/apply-stderr.txt"
if ! apply_settings_target 2>"$apply_stderr"; then
  fail "chezmoi could not apply $SETTINGS_TARGET_RELATIVE_PATH from this checkout: $(tr '\n' ' ' <"$apply_stderr")"
  finish
fi

report="$(settings_report)" || {
  fail "the applied $SETTINGS_TARGET_RELATIVE_PATH could not be read back as JSON; chezmoi's own error is on stderr above"
  finish
}

rendered_disabled_plugins=()
passthrough_value=''
passthrough_records=0
while IFS="$REPORT_FIELD_DELIMITER" read -r record value name; do
  [[ -n $record ]] || continue
  case "$record" in
    "$PLUGIN_RECORD")
      rendered_plugins+=("$name")
      [[ $value == 'true' ]] || rendered_disabled_plugins+=("$name=$value")
      ;;
    "$PASSTHROUGH_RECORD")
      passthrough_value="$value"
      passthrough_records=$((passthrough_records + 1))
      ;;
    *) fail "unrecognised report record '$record'; the extraction template and this parser disagree" ;;
  esac
done <<<"$report"

# --- harness integrity, before anything is concluded from the render ---------

# An apply that never read the fixture would drop the unmanaged field. If that
# happens, this test is measuring some other render and its verdict is worthless,
# so say that rather than reporting a plugin diff computed from the wrong file.
if ((passthrough_records != 1)); then
  fail "expected exactly 1 $PASSTHROUGH_RECORD record and got $passthrough_records; the extraction template is not reporting what this parser reads"
  finish
fi
if [[ $passthrough_value != 'true' ]]; then
  fail "the unmanaged field $PASSTHROUGH_SETTING_KEY did not survive the apply (rendered '$passthrough_value'); either the modify-template stopped passing unmanaged fields through, or it never received the fixture on .chezmoi.stdin and this test is not exercising the apply path"
  finish
fi

# An empty extraction makes every comparison below vacuously true, which is how
# this whole class of guard fails. Refuse it before comparing anything.
if ((${#rendered_plugins[@]} == 0)); then
  fail 'the applied settings declare no enabled plugins at all'
  finish
fi

# --- the render itself -------------------------------------------------------

# The write is whole-value, so a plugin enabled live and absent from the dict is
# gone after the apply. That is why the dict has to be complete, and it is the
# reason this test's expected set is a closed list rather than a minimum.
if printf '%s\n' "${rendered_plugins[@]}" | grep -qxF "$LIVE_BUT_UNDECLARED_PLUGIN"; then
  fail "the fixture's undeclared plugin $LIVE_BUT_UNDECLARED_PLUGIN survived the apply; enabledPlugins is no longer a whole-value write, so the assumptions in this test and in the template's comment are both stale"
fi

# A `false` is worse than an omission: it reads like a declaration while doing
# the opposite, so it survives a casual review of the list.
if ((${#rendered_disabled_plugins[@]} > 0)); then
  fail "the render sets these plugins to something other than true: ${rendered_disabled_plugins[*]}"
fi

# Both directions. A missing entry disables a working plugin at the next apply;
# an extra one enables something nobody chose.
expected_sorted="$(printf '%s\n' "${EXPECTED_ENABLED_PLUGINS[@]}" | sort)"
rendered_sorted="$(printf '%s\n' "${rendered_plugins[@]}" | sort)"

missing="$(comm -23 <(printf '%s\n' "$expected_sorted") <(printf '%s\n' "$rendered_sorted"))"
extra="$(comm -13 <(printf '%s\n' "$expected_sorted") <(printf '%s\n' "$rendered_sorted"))"

if [[ -n $missing ]]; then
  fail "expected these plugins to render enabled, and the template does not produce them (an apply would DISABLE them): $(tr '\n' ' ' <<<"$missing")"
fi
if [[ -n $extra ]]; then
  fail "the template renders plugins that are not in the expected set; add them to EXPECTED_ENABLED_PLUGINS deliberately or remove them: $(tr '\n' ' ' <<<"$extra")"
fi

finish
