#!/usr/bin/env bash
# claude-enabled-plugins.sh, the settings modify-template must RENDER the whole
# declared plugin roster, must PRESERVE the per-plugin state the live file
# already holds, and must apply cleanly for every live settings file that parses
# as JSON or is empty once trimmed, on every operating system this repository
# targets. The shapes it CANNOT survive are named and pinned here too, in
# UNPARSEABLE_LIVE_FILE_CASES, because leaving them unstated is what let an
# earlier version of this comment claim the render never fails.
#
# WHY THIS EXISTS, PART ONE: THE ROSTER MUST BE COMPLETE. modify_settings.json
# writes enabledPlugins with setValueAtPath, which REPLACES the value at that
# path rather than merging into it. So a plugin that is enabled live but absent
# from the declaration is turned OFF by the next apply, with no message.
# Measured 2026-07-30: three plugins (codex@openai-codex, ponytail@ponytail,
# rust-analyzer-lsp@claude-plugins-official) were enabled on the machine and
# absent from the declaration, so the next apply would have disabled all three.
#
# WHY THIS EXISTS, PART TWO: A DISABLE MUST SURVIVE. `claude plugin disable` is
# the only supported way to stop a plugin's code running, and it writes
# `"<id>": false` into a settings file. A declaration that forces every plugin
# to `true` silently revokes that, on every apply rather than only at a cutover
# (measured 2026-08-02: `--exclude=templates` does NOT skip a modify-template, a
# live `false` came back as `true` through `chezmoi apply --exclude=templates`).
# So the render's rule is: a live JSON boolean `false` for a DECLARED plugin is
# preserved, and every other shape renders `true`.
#
# WHY THE LIVE-FILE SHAPE IS A TEST DIMENSION. Preserving a live value means
# READING the live file, and reading it is where a modify-template dies.
# Measured 2026-08-02 on chezmoi 2.71.1: `index` on an untyped nil is a HARD
# error and `eq X false` errors whenever X is not a bool, so the obvious
# `if eq (index $livePlugins $name) false` makes `chezmoi apply` FAIL and write
# NO settings.json at all for an absent file, an empty file, `{}`, and
# `enabledPlugins: null`, and fail again on an array-valued entry, which is
# inside Claude Code's own schema union. A failed render loses the WHOLE file:
# permissions.deny, every hook, statusLine and every skillOverrides entry, not
# just the plugin list. Hence one case per realistic live-file shape, an apply
# that must SUCCEED in each, and the stable-field assertions below.
#
# WHERE THAT STOPS, AND WHY THIS FILE SAYS SO. A failed modify-template does not
# fail one target, it aborts the APPLY: every later target and every run_after_
# script is skipped, and the file that caused it is left exactly as it was, so
# permissions.deny is not restored either. The render survives every live file
# that PARSES, and every one that is empty once trimmed. It does not survive a
# live file that is non-empty and is not JSON, and no version of this template
# can: chezmoi's three JSON readers (fromJson, fromJsonc, fromYaml) all fail the
# template on bad input, and Go's text/template has no recover, so there is
# nothing to fall back FROM (measured 2026-08-02 on chezmoi 2.62.3 and 2.71.1;
# `try`, `catch` and every lenient-parse name probed are undefined). Repairing
# that needs something outside this template that runs BEFORE it. So the shapes
# that abort are not left unmentioned: UNPARSEABLE_LIVE_FILE_CASES applies each
# one, requires the failure, requires the apply's error to NAME the template so
# the operator is not left guessing, and requires the live file to come back
# byte-identical so the failure is inert rather than destructive. If someone
# fixes the limitation, those cases fail and say where to move them.
#
# WHY THE STABLE FIELDS ARE ASSERTED HERE AT ALL. The version of this file that
# shipped before 2026-08-02 asserted only enabledPlugins, and measured green at
# exit 0 against exactly the render-breaking template described above, because
# its single fixture happened to be the one shape that works. An assertion that
# the REST of the managed file survived is what closes that hole, so the three
# security-critical permissions.deny entries are named, every stable field's
# kind is pinned, and the whole stable block must come out identical whatever
# the live file looked like.
#
# WHY THAT ASSERTION REACHES INSIDE THE STABLE FIELDS. Kind, non-emptiness and
# cross-case invariance together see a field that VANISHED and a field that
# varies with the live file, and they see neither of those one level down.
# Measured by mutation 2026-08-02, against the version of this file that first
# added them: deleting permissions.allow, deleting permissions.defaultMode,
# deleting the whole PreToolUse audit hook, dropping statusLine.command,
# repointing statusLine.command at another script, setting cleanupPeriodDays to
# 1, switching autoUpdatesChannel to `latest` and flipping
# remoteControlAtStartup to false ALL passed, eight for eight. Every one leaves
# the top-level field present, a map, and non-empty, and every one leaves it
# identical across the ten cases, so nothing above can see any of them. The
# named sub-paths, the exact scalar values and the required hook commands below
# are what make those eight fail. skillOverrides is the one stable field whose
# contents genuinely are owned elsewhere (test/unit/skills-roster-fanout.sh
# pins them against dot_agents/custom-skill-lock.json, verified 2026-08-02 by
# mutating an entry away and again by changing one's verdict: that guard failed
# on both), so its entries are still counted here rather than named.
#
# WHY IT RENDERS INSTEAD OF READING THE SOURCE TEXT. The first version of this
# test matched the declaration in the template's SOURCE, which approves a
# template that does not render the set it appears to declare. Two shapes, both
# measured green under source matching on 2026-07-31: moving one entry into a
# `{{- /* ... */ -}}` comment placed after the declaration (source reads 9
# entries, the render emits 8, no message), and appending a stray `(` to an
# entry (source reads 9 entries, chezmoi dies with `unclosed left paren` and
# applies nothing at all). So this test applies the template for real and
# asserts on the resulting JSON.
#
# WHY EVERY FIXTURE IS ASSERTED BEFORE THE APPLY. Several assertions here are
# ABSENCES or preservations: the undeclared plugin must be GONE afterwards, the
# disabled one must still be `false`. Both pass vacuously against a fixture that
# never carried what the assertion is about. Measured 2026-08-01: empty the
# fixture's enabledPlugins and change the template to MERGE instead of replace,
# and the unguarded version reports `OK, 9 plugins rendered` at exit 0. So each
# case states the fixture it needs, that fixture is proved on disk first, and
# every verdict after the apply is conditional on it.
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

# Every operating system this repository targets. The declared roster is meant
# to be the same on all of them, so every case below runs once per operating
# system and gets its own verdict.
readonly -a TARGET_OPERATING_SYSTEMS=(
  'darwin'
  'linux'
)

# A field chezmoi does not manage. It must SURVIVE the apply in every case whose
# fixture carries it. If it does not, the fixture was never read on
# .chezmoi.stdin and every other assertion here is measuring a render this repo
# does not perform.
readonly PASSTHROUGH_SETTING_KEY='voiceEnabled'

# Plugins enabled or disabled live and absent from the declaration. Both must be
# GONE after the apply: the write is whole-value, which is why the declaration
# has to be complete, and asserting it means the reason for the completeness
# requirement is checked rather than commented. Two of them, one live-`true` and
# one live-`false`, so the removal is shown not to depend on the live value.
readonly LIVE_UNDECLARED_ENABLED_PLUGIN='ghost-plugin@no-such-marketplace'
readonly LIVE_UNDECLARED_DISABLED_PLUGIN='spectre-plugin@no-such-marketplace'

# The plugins this repository declares. Editing this list is the deliberate act;
# the RENDER must agree with it exactly, in both directions. The keys are
# `<name>@<marketplace>`, which is the form Claude Code writes into settings.json
# (verified 2026-08-02 against the live user file); the CLI prints the bare name
# on success, which is not the key.
readonly -a DECLARED_PLUGINS=(
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

# Roles played by individual declared plugins inside the fixtures below. Each
# one must be a member of DECLARED_PLUGINS, which assert_test_constants proves,
# because a typo here would silently turn its case into a test of an undeclared
# plugin and pass for the wrong reason.
readonly DISABLED_DECLARED_PLUGIN='superpowers@claude-plugins-official'
readonly ENABLED_DECLARED_PLUGIN='codex@openai-codex'
readonly ARRAY_VALUED_DECLARED_PLUGIN='playwright@claude-plugins-official'
readonly STRING_VALUED_DECLARED_PLUGIN='swift-lsp@claude-plugins-official'
readonly NUMBER_VALUED_DECLARED_PLUGIN='ponytail@ponytail'
readonly NULL_VALUED_DECLARED_PLUGIN='frontend-design@claude-plugins-official'

# One case per realistic shape of the live settings file. Each is a whole-file
# state, so none of them can be folded into another.
#
#   no-live-file               a fresh machine, ~/.claude/settings.json absent.
#   empty-live-file            a zero-byte file, what a truncated write leaves.
#   blank-live-file            a file holding nothing but whitespace, which is
#                              what an editor that saves an emptied buffer
#                              leaves and what `printf '\n' >` leaves. It is a
#                              SEPARATE shape from a zero-byte file: an empty
#                              string is falsy in a template and a whitespace
#                              one is not, so the zero-byte case alone leaves
#                              this reaching the JSON parser.
#   empty-json-object          `{}`, what a reset leaves.
#   null-enabled-plugins       the key present and JSON null.
#   array-enabled-plugins      the key present and holding a JSON ARRAY, so the
#                              plugin container is neither a map nor nil. This
#                              is the only case that separates a map guard from
#                              a nil check: `hasKey` is nil-safe but is a HARD
#                              error on a non-map (measured 2026-08-02, `wrong
#                              type for value; expected map[string]interface {};
#                              got []interface {}`), so without this case a
#                              template that drops the map guard renders green.
#   whole-file-json-null       the whole file is JSON null.
#   whole-file-json-array      the whole file is a JSON array, so the container
#                              the render indexes is not a map at any level.
#   undeclared-plugins-only    only plugins this repo does not declare, one
#                              live-true and one live-false.
#   declared-plugin-disabled   the case the fix exists for: a declared plugin
#                              disabled live, alongside one left enabled and one
#                              undeclared.
#   non-boolean-plugin-values  declared plugins carrying an array, a string, a
#                              number and a null, none of which is a disable.
readonly -a LIVE_FILE_CASES=(
  'no-live-file'
  'empty-live-file'
  'blank-live-file'
  'empty-json-object'
  'null-enabled-plugins'
  'array-enabled-plugins'
  'whole-file-json-null'
  'whole-file-json-array'
  'undeclared-plugins-only'
  'declared-plugin-disabled'
  'non-boolean-plugin-values'
)

# The live-file shapes that are non-empty and are NOT JSON. Each one aborts the
# whole apply, and each is pinned rather than omitted, because the cost of one
# of these is paid by every OTHER target and every run_after_ script in the same
# run, not by this file alone.
#
#   truncated-json-live-file   a write that stopped mid-object: a crash, a full
#                              disk, a killed process. The bytes are a prefix of
#                              a valid file.
#   trailing-garbage-live-file a complete JSON object with extra characters
#                              after it, what a bad merge or a double write
#                              leaves.
#   not-json-live-file         not JSON at any point, what a hand edit into the
#                              wrong file leaves.
readonly -a UNPARSEABLE_LIVE_FILE_CASES=(
  'truncated-json-live-file'
  'trailing-garbage-live-file'
  'not-json-live-file'
)

# The apply's error must name the source template that could not read the file.
# Without this the case would pass for ANY apply failure, including a broken
# sandbox, and the operator facing a real one would get a message that does not
# say which file to look at.
readonly UNPARSEABLE_APPLY_ERROR_FRAGMENT='modify_settings.json'

# Reports are `<record>:<kind>:<value>:<name>`, name LAST so that a name
# containing the delimiter lands whole in the final field and every earlier
# field still holds its own column. Splitting these on whitespace would collapse
# an empty value and shift the name left. A value CAN carry the delimiter, since
# statusLine.command and every hook command are absolute paths built from
# .chezmoi.homeDir and a home directory may contain anything: `read` puts the
# overflow into the trailing name field, so such a record loses its own name and
# is looked up by nobody, which FAILS the assertion that names the path rather
# than passing it. That is the reason the assertions below are written as
# lookups by name and not as counts of records seen.
readonly REPORT_FIELD_DELIMITER=':'
readonly PLUGIN_RECORD='plugin'
readonly PLUGIN_CONTAINER_RECORD='plugincontainer'
readonly PASSTHROUGH_RECORD='passthrough'
readonly STABLE_RECORD='stable'
readonly DENY_RECORD='deny'
readonly HOOK_COMMAND_RECORD='hookcommand'

# A plugin entry must be a JSON boolean, and `%v` prints the string "true" and
# the boolean true identically, so the render's TYPE is reported alongside the
# value and asserted with it. Read 2026-08-01 out of the shipped Claude Code
# 2.1.220 binary, whose settings schema declares
# `enabledPlugins: record(string, union([array(string), boolean, undefined]))`:
# a quoted "true" is not in that union, so it enables nothing, and a value-only
# check accepts it anyway. This repository declares booleans, so `bool` is what
# is pinned here rather than the schema's whole union.
readonly JSON_BOOLEAN_KIND='bool'
readonly JSON_TRUE_VALUE='true'
readonly JSON_FALSE_VALUE='false'

# The OTHER member of that union. Claude Code 2.1.220's schema calls the array
# form the "extended format with version constraints", so an array-valued entry
# is a plugin pinned to a reviewed release rather than a malformed boolean.
# Collapsing it to `true` would keep the plugin enabled and drop the pin, which
# returns it to unconstrained: for a plugin that ships lifecycle hooks (ponytail
# ships JavaScript ones) that widens what may execute, and it fails in the
# direction that runs MORE code. So the render carries it through the same way
# it carries a `false`, and this test pins the array's contents rather than only
# its kind. `%v` prints a Go slice as `[a b c]`, hence the rendered form below.
readonly JSON_ARRAY_KIND='slice'
readonly LIVE_VERSION_CONSTRAINT_ENTRY='1.4.2'
readonly LIVE_VERSION_CONSTRAINT_RENDERED='[1.4.2]'

# The stable fields that must survive every case, and the kind each must have.
# An absent field reports kind `invalid`, so this table is also the presence
# check: it is what catches a render that failed and wrote nothing, which is the
# failure mode the naive version of the fix produced on five of ten cases here.
# A DOTTED path names a field inside another one, walked one segment at a time
# with a map guard per segment, so a path whose parent is missing or is not a
# map reports `invalid` here rather than dying inside the report template. This
# table is also the report's own path list, read by settings_report, so the two
# cannot drift apart.
#
# Each stable record also carries a DETAIL: the entry count for a container, the
# value itself for a scalar. A container's count is deliberately not compared
# against a fixed number here (see STABLE_CONTAINER_FIELDS), but both forms are
# compared ACROSS cases, which is what catches a stable field whose content
# depends on the shape of the live file. Reporting a placeholder for scalars
# instead of their value let exactly that mutation live through a whole run.
readonly -a STABLE_FIELD_KINDS=(
  'permissions=map'
  'permissions.allow=slice'
  'permissions.deny=slice'
  'permissions.defaultMode=string'
  'hooks=map'
  'statusLine=map'
  'statusLine.type=string'
  'statusLine.command=string'
  'skillOverrides=map'
  'cleanupPeriodDays=int64'
  'autoUpdatesChannel=string'
  'remoteControlAtStartup=bool'
)

# Stable fields that must additionally be NON-EMPTY. Emptiness is asserted, and
# not an exact entry count, because a count would make this file a second source
# of truth for a list that grows on unrelated work. What replaces the count is
# not the emptiness check on its own: it is the named sub-paths above, the exact
# values below and the named deny and hook entries, each of which pins the part
# of the field that has a reason to be pinned.
readonly -a STABLE_CONTAINER_FIELDS=(
  'permissions'
  'permissions.allow'
  'permissions.deny'
  'hooks'
  'statusLine'
  'skillOverrides'
)

# The stable fields whose exact VALUE is the policy, so kind and presence say
# nothing on their own. `cleanupPeriodDays` at 36525 is what stops Claude Code
# deleting session history, `defaultMode` is the permission posture, and each of
# the others is a documented setting in CLAUDE.md whose whole content is one
# scalar. Split on the FIRST `=`, so a value may contain one.
readonly -a STABLE_FIELD_VALUES=(
  'permissions.defaultMode=bypassPermissions'
  'statusLine.type=command'
  'cleanupPeriodDays=36525'
  'autoUpdatesChannel=stable'
  'remoteControlAtStartup=true'
)

# statusLine.command holds an absolute path built from .chezmoi.homeDir, which
# differs between this machine and a CI runner, so the SUFFIX is what is pinned.
# It still fails when the command is repointed at another script, which is the
# mutation an exact-value table cannot be written for.
readonly STATUS_LINE_COMMAND_PATH='statusLine.command'
readonly STATUS_LINE_COMMAND_SUFFIX='/.claude/statusline-command.sh'

# The permissions.deny entries whose loss is the reason this file asserts
# anything outside enabledPlugins at all. These are named rather than counted,
# for the same reason as above, and they are the three CLAUDE.md calls out.
readonly -a REQUIRED_DENY_ENTRIES=(
  'Read(.env)'
  'Read(secrets/**)'
  'Read(.ssh/id_*)'
)

# The hook commands whose loss this file has to see, named by a distinctive
# fragment rather than by the whole command, because the commands carry an
# absolute home path and some carry flags that are not this file's business.
# These are the four hooks CLAUDE.md declares chezmoi-controlled: the session
# start marker, the Hue pulse, the permission-prompt alert and the Bash audit
# log. A count would not do: `hooks` keeps five event keys and a non-zero entry
# count when any single one of them is deleted.
readonly -a REQUIRED_HOOK_COMMAND_FRAGMENTS=(
  'claude-user-prompt-start.sh'
  'claude-stop-pulse.sh'
  'alerter '
  'claude-audit.sh'
)

# Build structured inputs through the template engine rather than by pasting
# values into a JSON string, so a name carrying a quote or a backslash is
# escaped by the JSON writer instead of producing a broken fixture that fails
# for the wrong reason. One template per content-bearing case, each sitting
# directly above the fixture records its case expects, so the two are read
# together.
# shellcheck disable=SC2016 # a Go template: $-names and {{ }} are template
# syntax evaluated by chezmoi, not shell expansions. Double quotes here would
# expand them to nothing.
readonly FIXTURE_NULL_ENABLED_PLUGINS_TEMPLATE='
{{- $fixture := dict
    (env "CLAUDE_PASSTHROUGH_SETTING_KEY") true
    "enabledPlugins" (fromJson "null") -}}
{{ $fixture | toPrettyJson }}'

# shellcheck disable=SC2016 # a Go template, as above.
readonly FIXTURE_ARRAY_ENABLED_PLUGINS_TEMPLATE='
{{- $fixture := dict
    (env "CLAUDE_PASSTHROUGH_SETTING_KEY") true
    "enabledPlugins" (list (env "CLAUDE_DISABLED_DECLARED_PLUGIN")) -}}
{{ $fixture | toPrettyJson }}'

# shellcheck disable=SC2016 # a Go template, as above.
readonly FIXTURE_UNDECLARED_PLUGINS_ONLY_TEMPLATE='
{{- $fixture := dict
    (env "CLAUDE_PASSTHROUGH_SETTING_KEY") true
    "enabledPlugins" (dict
      (env "CLAUDE_LIVE_UNDECLARED_ENABLED_PLUGIN") true
      (env "CLAUDE_LIVE_UNDECLARED_DISABLED_PLUGIN") false) -}}
{{ $fixture | toPrettyJson }}'

# shellcheck disable=SC2016 # a Go template, as above.
readonly FIXTURE_DECLARED_PLUGIN_DISABLED_TEMPLATE='
{{- $fixture := dict
    (env "CLAUDE_PASSTHROUGH_SETTING_KEY") true
    "enabledPlugins" (dict
      (env "CLAUDE_DISABLED_DECLARED_PLUGIN") false
      (env "CLAUDE_ENABLED_DECLARED_PLUGIN") true
      (env "CLAUDE_LIVE_UNDECLARED_ENABLED_PLUGIN") true) -}}
{{ $fixture | toPrettyJson }}'

# shellcheck disable=SC2016 # a Go template, as above.
readonly FIXTURE_NON_BOOLEAN_PLUGIN_VALUES_TEMPLATE='
{{- $fixture := dict
    (env "CLAUDE_PASSTHROUGH_SETTING_KEY") true
    "enabledPlugins" (dict
      (env "CLAUDE_ARRAY_VALUED_DECLARED_PLUGIN") (list (env "CLAUDE_LIVE_VERSION_CONSTRAINT_ENTRY"))
      (env "CLAUDE_STRING_VALUED_DECLARED_PLUGIN") "false"
      (env "CLAUDE_NUMBER_VALUED_DECLARED_PLUGIN") 0
      (env "CLAUDE_NULL_VALUED_DECLARED_PLUGIN") (fromJson "null")) -}}
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

# One line per enabledPlugins entry, one for the passthrough field, one per
# stable field path, one per permissions.deny entry and one per command found
# anywhere under hooks. The hook walk descends event -> matcher -> hook entry,
# kind-guarding each level, so a hooks map the render mangled produces fewer
# records instead of an error, and the assertion that names the missing command
# is what reports it.
#
# `index` rather than `.field` because chezmoi errors on a missing key with the
# field form, and an absent key must reach the assertions below (which name it)
# rather than die inside the template with a message about map entries. `kindOf`
# reports the JSON type: `bool` for true, `string` for "true", `invalid` for an
# absent key or a JSON null.
#
# EVERY container is kind-guarded before it is indexed. `index` on an untyped
# nil is a hard error, and this template runs against degenerate settings files
# on purpose (JSON null, a bare array, `{}`), so an unguarded index would kill
# the report and be reported as "could not read the file as JSON", which
# diagnoses the wrong thing. Guarded, an absent container yields `invalid`
# records that the assertions name.
# shellcheck disable=SC2016 # a Go template, as above.
readonly SETTINGS_REPORT_TEMPLATE='
{{- $settings := fromJson (env "CLAUDE_RENDERED_SETTINGS_JSON") -}}
{{- if ne (kindOf $settings) "map" -}}
{{-   $settings = dict -}}
{{- end -}}
{{- $delimiter := env "CLAUDE_REPORT_FIELD_DELIMITER" -}}
{{- $plugins := index $settings "enabledPlugins" -}}
{{- $pluginContainerKind := kindOf $plugins -}}
{{- $pluginContainerLength := 0 -}}
{{- if or (eq $pluginContainerKind "map") (eq $pluginContainerKind "slice") -}}
{{-   $pluginContainerLength = len $plugins -}}
{{- end -}}
{{ printf "%s%s%s%s%d%s%s\n" (env "CLAUDE_PLUGIN_CONTAINER_RECORD") $delimiter
   $pluginContainerKind $delimiter $pluginContainerLength $delimiter
   "enabledPlugins" -}}
{{- if ne $pluginContainerKind "map" -}}
{{-   $plugins = dict -}}
{{- end -}}
{{- range $name, $value := $plugins -}}
{{ printf "%s%s%s%s%v%s%s\n" (env "CLAUDE_PLUGIN_RECORD") $delimiter
   (kindOf $value) $delimiter $value $delimiter $name -}}
{{ end -}}
{{- $passthroughKey := env "CLAUDE_PASSTHROUGH_SETTING_KEY" -}}
{{- $passthroughValue := index $settings $passthroughKey -}}
{{ printf "%s%s%s%s%v%s%s\n" (env "CLAUDE_PASSTHROUGH_RECORD") $delimiter
   (kindOf $passthroughValue) $delimiter $passthroughValue $delimiter
   $passthroughKey -}}
{{- $absent := index dict "no-such-key" -}}
{{- range $path := splitList "\n" (env "CLAUDE_STABLE_FIELD_PATHS") -}}
{{-   $value := $settings -}}
{{-   range $segment := splitList "." $path -}}
{{-     if eq (kindOf $value) "map" -}}
{{-       $value = index $value $segment -}}
{{-     else -}}
{{-       $value = $absent -}}
{{-     end -}}
{{-   end -}}
{{-   $detail := printf "%v" $value -}}
{{-   if or (eq (kindOf $value) "map") (eq (kindOf $value) "slice") -}}
{{-     $detail = printf "%d" (len $value) -}}
{{-   end -}}
{{ printf "%s%s%s%s%s%s%s\n" (env "CLAUDE_STABLE_RECORD") $delimiter
   (kindOf $value) $delimiter $detail $delimiter $path -}}
{{ end -}}
{{- $permissions := index $settings "permissions" -}}
{{- if ne (kindOf $permissions) "map" -}}
{{-   $permissions = dict -}}
{{- end -}}
{{- $deny := index $permissions "deny" -}}
{{- if ne (kindOf $deny) "slice" -}}
{{-   $deny = list -}}
{{- end -}}
{{- range $index, $entry := $deny -}}
{{ printf "%s%s%s%s%v%s%d\n" (env "CLAUDE_DENY_RECORD") $delimiter
   (kindOf $entry) $delimiter $entry $delimiter $index -}}
{{ end -}}
{{- $hooks := index $settings "hooks" -}}
{{- if ne (kindOf $hooks) "map" -}}
{{-   $hooks = dict -}}
{{- end -}}
{{- range $eventName, $matchers := $hooks -}}
{{-   if eq (kindOf $matchers) "slice" -}}
{{-     range $matcher := $matchers -}}
{{-       if eq (kindOf $matcher) "map" -}}
{{-         $entries := index $matcher "hooks" -}}
{{-         if eq (kindOf $entries) "slice" -}}
{{-           range $entry := $entries -}}
{{-             if eq (kindOf $entry) "map" -}}
{{-               printf "%s%s%s%s%v%s%s\n" (env "CLAUDE_HOOK_COMMAND_RECORD")
                    $delimiter (kindOf (index $entry "command")) $delimiter
                    (index $entry "command") $delimiter $eventName -}}
{{-             end -}}
{{-           end -}}
{{-         end -}}
{{-       end -}}
{{-     end -}}
{{-   end -}}
{{- end -}}'

failures=0
verified_case_count=0
verified_unparseable_case_count=0
fail() {
  printf 'FAIL: %s\n' "$1" >&2
  failures=$((failures + 1))
}

finish() {
  local expected_case_count=$((${#LIVE_FILE_CASES[@]} * ${#TARGET_OPERATING_SYSTEMS[@]}))
  local expected_unparseable_count=$((${#UNPARSEABLE_LIVE_FILE_CASES[@]} * ${#TARGET_OPERATING_SYSTEMS[@]}))
  if ((failures == 0 && verified_case_count != expected_case_count)); then
    fail "only $verified_case_count of $expected_case_count case runs reached their assertions, so some case was skipped without a verdict"
  fi
  if ((failures == 0 && verified_unparseable_case_count != expected_unparseable_count)); then
    fail "only $verified_unparseable_case_count of $expected_unparseable_count unparseable-shape runs reached their assertions, so a shape that aborts the whole apply went unmeasured"
  fi
  if ((failures > 0)); then
    printf '\nclaude-enabled-plugins: %d failure(s)\n' "$failures" >&2
    exit 1
  fi
  printf 'claude-enabled-plugins: OK, %d declared plugins verified across %d live-file cases and %d unparseable shapes for each target OS (%s)\n' \
    "${#DECLARED_PLUGINS[@]}" "${#LIVE_FILE_CASES[@]}" \
    "${#UNPARSEABLE_LIVE_FILE_CASES[@]}" "${TARGET_OPERATING_SYSTEMS[*]}"
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

# Per-case sandbox paths. The layout lives in these functions and nowhere else.
case_config_path() {
  printf '%s/config/%s.json' "$sandbox" "$1"
}

case_destination_directory() {
  printf '%s/destination/%s/%s' "$sandbox" "$1" "$2"
}

case_settings_path() {
  printf '%s/%s' "$(case_destination_directory "$1" "$2")" "$SETTINGS_TARGET_RELATIVE_PATH"
}

case_apply_stderr_path() {
  printf '%s/apply-stderr-%s-%s.txt' "$sandbox" "$1" "$2"
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

# render_fixture_template <template> -- the fixture builders all read the same
# names out of the environment, so the export list lives here once.
render_fixture_template() {
  CLAUDE_PASSTHROUGH_SETTING_KEY="$PASSTHROUGH_SETTING_KEY" \
    CLAUDE_LIVE_UNDECLARED_ENABLED_PLUGIN="$LIVE_UNDECLARED_ENABLED_PLUGIN" \
    CLAUDE_LIVE_UNDECLARED_DISABLED_PLUGIN="$LIVE_UNDECLARED_DISABLED_PLUGIN" \
    CLAUDE_DISABLED_DECLARED_PLUGIN="$DISABLED_DECLARED_PLUGIN" \
    CLAUDE_ENABLED_DECLARED_PLUGIN="$ENABLED_DECLARED_PLUGIN" \
    CLAUDE_ARRAY_VALUED_DECLARED_PLUGIN="$ARRAY_VALUED_DECLARED_PLUGIN" \
    CLAUDE_STRING_VALUED_DECLARED_PLUGIN="$STRING_VALUED_DECLARED_PLUGIN" \
    CLAUDE_NUMBER_VALUED_DECLARED_PLUGIN="$NUMBER_VALUED_DECLARED_PLUGIN" \
    CLAUDE_NULL_VALUED_DECLARED_PLUGIN="$NULL_VALUED_DECLARED_PLUGIN" \
    CLAUDE_LIVE_VERSION_CONSTRAINT_ENTRY="$LIVE_VERSION_CONSTRAINT_ENTRY" \
    render_template "$1"
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
  config_path="$(case_config_path "$operating_system")"
  mkdir -p "$(dirname "$config_path")"
  CLAUDE_TARGET_OPERATING_SYSTEM="$operating_system" \
    render_template "$SANDBOX_CONFIG_TEMPLATE" >"$config_path"
}

# case_literal_fixture <case> -- the exact bytes a degenerate case writes, or
# nothing at all when the case is content-bearing or writes no file. These carry
# no interpolated names, so a literal is safe here and needs no render.
case_literal_fixture() {
  case "$1" in
    'empty-live-file') printf '' ;;
    # A space, a tab, a carriage return and two newlines: every character the
    # render's whitespace trim has to cover, in the one fixture, so a trim
    # narrowed to newlines alone fails here.
    'blank-live-file') printf ' \t\r\n \n' ;;
    'empty-json-object') printf '{}\n' ;;
    'whole-file-json-null') printf 'null\n' ;;
    'whole-file-json-array') printf '[1, 2]\n' ;;
    'truncated-json-live-file') printf '{"voiceEnabled": true, "enabledPlugins": {' ;;
    'trailing-garbage-live-file') printf '{"voiceEnabled": true}}}\n' ;;
    'not-json-live-file') printf 'this file is not json\n' ;;
    *) return 1 ;;
  esac
}

# case_fixture_template <case> -- the builder for a content-bearing case, or
# nothing when the case is degenerate.
case_fixture_template() {
  case "$1" in
    'null-enabled-plugins') printf '%s' "$FIXTURE_NULL_ENABLED_PLUGINS_TEMPLATE" ;;
    'array-enabled-plugins') printf '%s' "$FIXTURE_ARRAY_ENABLED_PLUGINS_TEMPLATE" ;;
    'undeclared-plugins-only') printf '%s' "$FIXTURE_UNDECLARED_PLUGINS_ONLY_TEMPLATE" ;;
    'declared-plugin-disabled') printf '%s' "$FIXTURE_DECLARED_PLUGIN_DISABLED_TEMPLATE" ;;
    'non-boolean-plugin-values') printf '%s' "$FIXTURE_NON_BOOLEAN_PLUGIN_VALUES_TEMPLATE" ;;
    *) return 1 ;;
  esac
}

# case_expected_fixture_plugin_container_kind <case> -- the kind the fixture's
# enabledPlugins VALUE must have. Without this the two cases whose container is
# not a map would prove nothing: their fixture holds no plugin entries either
# way, so an entry comparison alone passes for a fixture that never carried the
# shape the case is named after.
case_expected_fixture_plugin_container_kind() {
  case "$1" in
    'null-enabled-plugins') printf 'invalid\n' ;;
    'array-enabled-plugins') printf 'slice\n' ;;
    *) printf 'map\n' ;;
  esac
}

# case_expected_fixture_plugin_records <case> -- the enabledPlugins entries the
# fixture builder above must actually have produced, as `<kind>:<value>:<name>`
# records. This is what stops a case from passing vacuously: the post-apply
# verdict about a disabled, an undeclared or an array-valued plugin means
# nothing unless the fixture really carried one.
case_expected_fixture_plugin_records() {
  case "$1" in
    'undeclared-plugins-only')
      printf '%s%s%s%s%s\n' "$JSON_BOOLEAN_KIND" "$REPORT_FIELD_DELIMITER" \
        "$JSON_TRUE_VALUE" "$REPORT_FIELD_DELIMITER" "$LIVE_UNDECLARED_ENABLED_PLUGIN"
      printf '%s%s%s%s%s\n' "$JSON_BOOLEAN_KIND" "$REPORT_FIELD_DELIMITER" \
        "$JSON_FALSE_VALUE" "$REPORT_FIELD_DELIMITER" "$LIVE_UNDECLARED_DISABLED_PLUGIN"
      ;;
    'declared-plugin-disabled')
      printf '%s%s%s%s%s\n' "$JSON_BOOLEAN_KIND" "$REPORT_FIELD_DELIMITER" \
        "$JSON_FALSE_VALUE" "$REPORT_FIELD_DELIMITER" "$DISABLED_DECLARED_PLUGIN"
      printf '%s%s%s%s%s\n' "$JSON_BOOLEAN_KIND" "$REPORT_FIELD_DELIMITER" \
        "$JSON_TRUE_VALUE" "$REPORT_FIELD_DELIMITER" "$ENABLED_DECLARED_PLUGIN"
      printf '%s%s%s%s%s\n' "$JSON_BOOLEAN_KIND" "$REPORT_FIELD_DELIMITER" \
        "$JSON_TRUE_VALUE" "$REPORT_FIELD_DELIMITER" "$LIVE_UNDECLARED_ENABLED_PLUGIN"
      ;;
    'non-boolean-plugin-values')
      printf '%s%s%s%s%s\n' "$JSON_ARRAY_KIND" "$REPORT_FIELD_DELIMITER" \
        "$LIVE_VERSION_CONSTRAINT_RENDERED" "$REPORT_FIELD_DELIMITER" \
        "$ARRAY_VALUED_DECLARED_PLUGIN"
      printf 'string%s%s%s%s\n' "$REPORT_FIELD_DELIMITER" "$JSON_FALSE_VALUE" \
        "$REPORT_FIELD_DELIMITER" "$STRING_VALUED_DECLARED_PLUGIN"
      printf 'int64%s0%s%s\n' "$REPORT_FIELD_DELIMITER" \
        "$REPORT_FIELD_DELIMITER" "$NUMBER_VALUED_DECLARED_PLUGIN"
      printf 'invalid%s<nil>%s%s\n' "$REPORT_FIELD_DELIMITER" \
        "$REPORT_FIELD_DELIMITER" "$NULL_VALUED_DECLARED_PLUGIN"
      ;;
    *) : ;;
  esac
}

# case_expected_rendered_plugin_records <case> -- what the APPLIED file must
# hold, as the same `<kind>:<value>:<name>` records. A declared plugin renders
# the JSON boolean true unless the live file already held a value that is INSIDE
# Claude Code's union for this key, in which case that value is carried through
# unchanged: a boolean false (the disable) and an array (the version
# constraint). Nothing else is a value: a string, a number, a null and an absent
# key all render true, and an undeclared plugin is not in this set at all, so a
# survivor shows up as an extra record.
case_expected_rendered_plugin_records() {
  local requested_case="$1" plugin kind value
  for plugin in "${DECLARED_PLUGINS[@]}"; do
    kind="$JSON_BOOLEAN_KIND"
    value="$JSON_TRUE_VALUE"
    if [[ $requested_case == 'declared-plugin-disabled' && $plugin == "$DISABLED_DECLARED_PLUGIN" ]]; then
      value="$JSON_FALSE_VALUE"
    fi
    if [[ $requested_case == 'non-boolean-plugin-values' && $plugin == "$ARRAY_VALUED_DECLARED_PLUGIN" ]]; then
      kind="$JSON_ARRAY_KIND"
      value="$LIVE_VERSION_CONSTRAINT_RENDERED"
    fi
    printf '%s%s%s%s%s\n' "$kind" "$REPORT_FIELD_DELIMITER" \
      "$value" "$REPORT_FIELD_DELIMITER" "$plugin"
  done
}

# write_case_fixture <case> <operating-system> -- put the live settings file for
# one case on disk. Every case creates the .claude directory, because chezmoi
# will not create a parent while applying a single target path (measured
# 2026-08-02: `stat .../.claude: no such file or directory`), and because the
# shape under test is an absent FILE rather than an unmanaged home directory.
write_case_fixture() {
  local requested_case="$1" operating_system="$2" settings_path fixture_template
  settings_path="$(case_settings_path "$requested_case" "$operating_system")"
  mkdir -p "$(dirname "$settings_path")"
  if [[ $requested_case == 'no-live-file' ]]; then
    return 0
  fi
  if case_literal_fixture "$requested_case" >"$settings_path"; then
    return 0
  fi
  fixture_template="$(case_fixture_template "$requested_case")" || {
    fail "[$requested_case] no fixture is defined for this case, so it would test the previous case's file"
    finish
  }
  render_fixture_template "$fixture_template" >"$settings_path"
}

# stable_field_paths -- the paths out of STABLE_FIELD_KINDS, one per line. The
# report template reads exactly this list, so a path added to the table is
# reported without a second edit, and a path can no longer be reported without
# being asserted or asserted without being reported.
stable_field_paths() {
  local field_kind
  for field_kind in "${STABLE_FIELD_KINDS[@]}"; do
    printf '%s\n' "${field_kind%%=*}"
  done
}

# settings_report <case> <operating-system> -- the enabledPlugins entries, the
# passthrough field, the stable field paths, the permissions.deny entries and
# the hook commands of that case's settings file, one delimited record per line.
# Called on the fixture and again on the applied result, because the same
# questions are asked of both.
settings_report() {
  CLAUDE_RENDERED_SETTINGS_JSON="$(cat "$(case_settings_path "$1" "$2")")" \
  CLAUDE_REPORT_FIELD_DELIMITER="$REPORT_FIELD_DELIMITER" \
  CLAUDE_PLUGIN_RECORD="$PLUGIN_RECORD" \
  CLAUDE_PLUGIN_CONTAINER_RECORD="$PLUGIN_CONTAINER_RECORD" \
  CLAUDE_PASSTHROUGH_RECORD="$PASSTHROUGH_RECORD" \
  CLAUDE_STABLE_RECORD="$STABLE_RECORD" \
  CLAUDE_DENY_RECORD="$DENY_RECORD" \
  CLAUDE_HOOK_COMMAND_RECORD="$HOOK_COMMAND_RECORD" \
  CLAUDE_STABLE_FIELD_PATHS="$(stable_field_paths)" \
  CLAUDE_PASSTHROUGH_SETTING_KEY="$PASSTHROUGH_SETTING_KEY" \
    render_template "$SETTINGS_REPORT_TEMPLATE"
}

# Findings of the most recent parse_settings_report call. Bash cannot return an
# array, so the parser publishes under names that say where they came from.
parsed_plugin_records=()
parsed_plugin_names=()
parsed_plugin_container_kind=''
parsed_plugin_container_length=0
parsed_passthrough_kind=''
parsed_passthrough_value=''
parsed_passthrough_record_count=0
parsed_stable_records=()
parsed_deny_values=()
parsed_hook_commands=()

# parse_settings_report <report> -- split one report into the findings above.
parse_settings_report() {
  local record kind value name
  parsed_plugin_records=()
  parsed_plugin_names=()
  parsed_plugin_container_kind=''
  parsed_plugin_container_length=0
  parsed_passthrough_kind=''
  parsed_passthrough_value=''
  parsed_passthrough_record_count=0
  parsed_stable_records=()
  parsed_deny_values=()
  parsed_hook_commands=()
  while IFS="$REPORT_FIELD_DELIMITER" read -r record kind value name; do
    [[ -n $record ]] || continue
    case "$record" in
      "$PLUGIN_RECORD")
        parsed_plugin_names+=("$name")
        parsed_plugin_records+=("$kind$REPORT_FIELD_DELIMITER$value$REPORT_FIELD_DELIMITER$name")
        ;;
      "$PLUGIN_CONTAINER_RECORD")
        parsed_plugin_container_kind="$kind"
        parsed_plugin_container_length="$value"
        ;;
      "$PASSTHROUGH_RECORD")
        parsed_passthrough_kind="$kind"
        parsed_passthrough_value="$value"
        parsed_passthrough_record_count=$((parsed_passthrough_record_count + 1))
        ;;
      "$STABLE_RECORD")
        parsed_stable_records+=("$name$REPORT_FIELD_DELIMITER$kind$REPORT_FIELD_DELIMITER$value")
        ;;
      "$DENY_RECORD")
        parsed_deny_values+=("$value")
        ;;
      "$HOOK_COMMAND_RECORD")
        parsed_hook_commands+=("$name$REPORT_FIELD_DELIMITER$value")
        ;;
      *) fail "unrecognised report record '$record'; the extraction template and this parser disagree" ;;
    esac
  done <<<"$1"
}

# stable_field_record <path> -- the parsed `<path>:<kind>:<length>` record for
# one stable field, or nothing when the report never mentioned it.
stable_field_record() {
  local wanted_path="$1" record
  for record in ${parsed_stable_records[@]+"${parsed_stable_records[@]}"}; do
    [[ ${record%%"$REPORT_FIELD_DELIMITER"*} == "$wanted_path" ]] || continue
    printf '%s\n' "$record"
    return 0
  done
  return 1
}

# stable_field_detail <path> -- the DETAIL column of one stable field's record:
# the entry count for a container, the value itself for a scalar. Non-zero when
# the report never mentioned the path, so a caller cannot read a missing field
# as an empty value.
stable_field_detail() {
  local record
  record="$(stable_field_record "$1")" || return 1
  record="${record#*"$REPORT_FIELD_DELIMITER"}"
  printf '%s\n' "${record#*"$REPORT_FIELD_DELIMITER"}"
}

# sorted_lines <line>... -- the arguments as a sorted newline-joined block, or
# the empty string for no arguments. `sort` on an empty argument list would hang
# on stdin, so the empty case returns before reaching it.
sorted_lines() {
  (($# > 0)) || return 0
  printf '%s\n' "$@" | LC_ALL=C sort
}

# assert_test_constants -- this file's own constants, before anything uses them.
# A role plugin that is not declared, or an "undeclared" plugin that is, turns
# its case into a test of something else and passes for the wrong reason.
assert_test_constants() {
  local declared_plugins role_plugin
  declared_plugins="$(sorted_lines "${DECLARED_PLUGINS[@]}")"
  for role_plugin in "$DISABLED_DECLARED_PLUGIN" "$ENABLED_DECLARED_PLUGIN" \
    "$ARRAY_VALUED_DECLARED_PLUGIN" "$STRING_VALUED_DECLARED_PLUGIN" \
    "$NUMBER_VALUED_DECLARED_PLUGIN" "$NULL_VALUED_DECLARED_PLUGIN"; do
    grep -qxF "$role_plugin" <<<"$declared_plugins" ||
      fail "the fixture role plugin $role_plugin is not in DECLARED_PLUGINS, so its case would measure an undeclared plugin instead"
  done
  for role_plugin in "$LIVE_UNDECLARED_ENABLED_PLUGIN" "$LIVE_UNDECLARED_DISABLED_PLUGIN"; do
    if grep -qxF "$role_plugin" <<<"$declared_plugins"; then
      fail "$role_plugin is in DECLARED_PLUGINS, so the assertion that it is REMOVED contradicts the assertion that every declared plugin is rendered"
    fi
  done
  ((failures == 0)) || finish
}

# assert_passthrough_field <case> <operating-system> <stage> <diagnosis> -- the
# unmanaged field must be present exactly once and be the JSON boolean true.
# Terminal, because a settings file without it is not the file this test thinks
# it is measuring. Only for cases whose fixture carries it: a case whose live
# file is absent, empty or a bare `{}` has nothing to pass through.
assert_passthrough_field() {
  local requested_case="$1" operating_system="$2" stage="$3" diagnosis="$4"
  if ((parsed_passthrough_record_count != 1)); then
    fail "[$operating_system/$requested_case] the $stage settings produced $parsed_passthrough_record_count $PASSTHROUGH_RECORD records and not 1; the extraction template is not reporting what this parser reads"
    finish
  fi
  if [[ $parsed_passthrough_kind != "$JSON_BOOLEAN_KIND" || $parsed_passthrough_value != "$JSON_TRUE_VALUE" ]]; then
    fail "[$operating_system/$requested_case] the $stage settings do not carry the unmanaged field $PASSTHROUGH_SETTING_KEY as the JSON boolean true (kind '$parsed_passthrough_kind', value '$parsed_passthrough_value'); $diagnosis"
    finish
  fi
}

# case_carries_passthrough <case> -- true when the case's fixture is a JSON
# object this test wrote the unmanaged field into.
case_carries_passthrough() {
  case_fixture_template "$1" >/dev/null 2>&1
}

# assert_fixture_precondition <case> <operating-system> -- the file about to be
# applied over must be exactly the shape the case names. Degenerate cases are
# checked as bytes on disk, because they have no JSON to report on; the
# content-bearing ones are checked through the same report the applied file goes
# through, against the records the case declares.
assert_fixture_precondition() {
  local requested_case="$1" operating_system="$2"
  local settings_path fixture_report expected_records actual_records literal_fixture
  local expected_container_kind

  settings_path="$(case_settings_path "$requested_case" "$operating_system")"

  if [[ $requested_case == 'no-live-file' ]]; then
    if [[ -e $settings_path ]]; then
      fail "[$operating_system/$requested_case] the live settings file exists, so this case is not measuring an absent file"
      finish
    fi
    return 0
  fi

  [[ -f $settings_path ]] || {
    fail "[$operating_system/$requested_case] the fixture builder wrote no live settings file, so this case is measuring the wrong shape"
    finish
  }

  if literal_fixture="$(case_literal_fixture "$requested_case")"; then
    [[ "$(cat "$settings_path")" == "$literal_fixture" ]] || {
      fail "[$operating_system/$requested_case] the live settings file on disk is not the literal this case declares; something rewrote it before the apply"
      finish
    }
    return 0
  fi

  fixture_report="$(settings_report "$requested_case" "$operating_system")" || {
    fail "[$operating_system/$requested_case] the fixture this test just wrote is not readable as JSON, so the fixture builder is broken and no verdict below would mean anything"
    finish
  }
  parse_settings_report "$fixture_report"
  assert_passthrough_field "$requested_case" "$operating_system" 'pre-apply fixture' \
    'the fixture builder did not write it, so this test does not control the file it is about to apply over'

  expected_container_kind="$(case_expected_fixture_plugin_container_kind "$requested_case")"
  [[ $parsed_plugin_container_kind == "$expected_container_kind" ]] || {
    fail "[$operating_system/$requested_case] the pre-apply fixture's enabledPlugins has kind '$parsed_plugin_container_kind' and this case is about kind '$expected_container_kind'; the fixture builder is not producing the shape the case is named after"
    finish
  }

  expected_records="$(case_expected_fixture_plugin_records "$requested_case" | LC_ALL=C sort)"
  actual_records="$(sorted_lines ${parsed_plugin_records[@]+"${parsed_plugin_records[@]}"})"
  [[ $expected_records == "$actual_records" ]] || {
    fail "[$operating_system/$requested_case] the pre-apply fixture does not hold the enabledPlugins entries this case is about; expected [$(tr '\n' ' ' <<<"$expected_records")] and it holds [$(tr '\n' ' ' <<<"$actual_records")]. Every post-apply verdict here would otherwise pass whatever the template does"
    finish
  }
}

# apply_settings_target <case> <operating-system> -- apply the ONE managed
# target into that case's sandbox destination, the way a real apply does it: the
# modify-template receives the fixture written above on .chezmoi.stdin. An
# ABSOLUTE target path keeps the result independent of the caller's working
# directory, against which chezmoi resolves a relative one. Externals are never
# refreshed, so this stays offline even if the source tree gains one later.
apply_settings_target() {
  local requested_case="$1" operating_system="$2"
  chezmoi_isolated "$(case_config_path "$operating_system")" \
    --destination "$(case_destination_directory "$requested_case" "$operating_system")" \
    --refresh-externals=never \
    --force \
    apply "$(case_settings_path "$requested_case" "$operating_system")"
}

# assert_rendered_enabled_plugins <case> <operating-system> -- the plugin list
# the apply produced.
assert_rendered_enabled_plugins() {
  local requested_case="$1" operating_system="$2"
  local expected_records actual_records undeclared_plugin

  # An empty extraction makes every comparison below vacuously true, which is how
  # this whole class of guard fails. Refuse it before comparing anything, and
  # refuse a container that is not a map first, because the report replaces a
  # non-map one with an empty map and the entry comparison alone would then
  # blame the entries for a container that is the wrong type outright.
  if [[ $parsed_plugin_container_kind != 'map' ]]; then
    fail "[$operating_system/$requested_case] the applied settings hold enabledPlugins as kind '$parsed_plugin_container_kind' and not a map; only a JSON object enables anything"
    finish
  fi
  if ((${#parsed_plugin_names[@]} == 0)); then
    fail "[$operating_system/$requested_case] the applied settings declare no enabled plugins at all"
    finish
  fi
  if ((parsed_plugin_container_length != ${#DECLARED_PLUGINS[@]})); then
    fail "[$operating_system/$requested_case] the applied settings hold $parsed_plugin_container_length enabledPlugins entries and this repo declares ${#DECLARED_PLUGINS[@]}"
  fi

  # The write is whole-value, so a plugin enabled or disabled live and absent
  # from the declaration is gone after the apply. That is why the declaration
  # has to be complete, and it is the reason the expected set below is a closed
  # list rather than a minimum.
  for undeclared_plugin in "$LIVE_UNDECLARED_ENABLED_PLUGIN" "$LIVE_UNDECLARED_DISABLED_PLUGIN"; do
    if printf '%s\n' "${parsed_plugin_names[@]}" | grep -qxF "$undeclared_plugin"; then
      fail "[$operating_system/$requested_case] the fixture's undeclared plugin $undeclared_plugin survived the apply; enabledPlugins is no longer a whole-value write, so the assumptions in this test and in the template's comment are both stale"
    fi
  done

  # Kind, value and name for every entry, in both directions and in one
  # comparison. A missing entry disables a working plugin at the next apply; an
  # extra one enables something nobody chose; a `false` where `true` is expected
  # reads like a declaration while doing the opposite; a quoted "true" survives
  # a value-only comparison against the boolean while enabling nothing.
  expected_records="$(case_expected_rendered_plugin_records "$requested_case" | LC_ALL=C sort)"
  actual_records="$(sorted_lines ${parsed_plugin_records[@]+"${parsed_plugin_records[@]}"})"
  if [[ $expected_records != "$actual_records" ]]; then
    fail "[$operating_system/$requested_case] the rendered enabledPlugins entries are not the ones this case expects. Expected [$(tr '\n' ' ' <<<"$expected_records")] and the apply produced [$(tr '\n' ' ' <<<"$actual_records")]; each record is <kind>:<value>:<name>"
  fi
}

# assert_stable_fields_survived <case> <operating-system> -- everything the
# modify-template manages OUTSIDE enabledPlugins. This is the assertion whose
# absence let a template that writes NO FILE AT ALL on five of these nine cases
# measure green. Presence and non-degeneracy are the first layer; the named
# sub-paths, the exact scalar values, the named deny entries and the named hook
# commands are the second, because eight separate one-level-down deletions and
# value changes measured green against the first layer alone.
assert_stable_fields_survived() {
  local requested_case="$1" operating_system="$2"
  local field_kind path expected_kind record actual_kind actual_detail
  local field_value expected_value
  local required_entry deny_values required_fragment hook_commands

  for field_kind in "${STABLE_FIELD_KINDS[@]}"; do
    path="${field_kind%%=*}"
    expected_kind="${field_kind#*=}"
    record="$(stable_field_record "$path")" || record=''
    if [[ -z $record ]]; then
      fail "[$operating_system/$requested_case] the applied settings report no record for the stable field $path; either the extraction template and STABLE_FIELD_KINDS disagree, or the field's value contains a '$REPORT_FIELD_DELIMITER' and shifted its own record out of reach (measured 2026-08-02 with a home directory containing one: it fails here rather than passing)"
      continue
    fi
    actual_kind="${record#*"$REPORT_FIELD_DELIMITER"}"
    actual_detail="${actual_kind#*"$REPORT_FIELD_DELIMITER"}"
    actual_kind="${actual_kind%%"$REPORT_FIELD_DELIMITER"*}"
    if [[ $actual_kind != "$expected_kind" ]]; then
      fail "[$operating_system/$requested_case] the stable field $path has kind '$actual_kind' and not '$expected_kind'; kind 'invalid' means the apply wrote no such field, which is what a render that fails outright leaves behind"
      continue
    fi
    if printf '%s\n' "${STABLE_CONTAINER_FIELDS[@]}" | grep -qxF "$path"; then
      # A container's detail is its entry count. The numeric test comes first so
      # that a detail which is not a count fails as a report defect rather than
      # being coerced to zero by the arithmetic and reported as an empty field.
      if [[ ! $actual_detail =~ ^[0-9]+$ ]]; then
        fail "[$operating_system/$requested_case] the stable container field $path reports '$actual_detail' where an entry count belongs; the extraction template and this assertion disagree"
      elif ((actual_detail == 0)); then
        fail "[$operating_system/$requested_case] the stable field $path survived the apply but is EMPTY; a present-but-empty container is the same loss as a missing one"
      fi
    fi
  done

  # The stable fields that are one scalar each. Kind and presence pass unchanged
  # when the value itself is wrong, and a wrong value here is the whole setting:
  # cleanupPeriodDays at anything but 36525 puts session cleanup back on.
  for field_value in "${STABLE_FIELD_VALUES[@]}"; do
    path="${field_value%%=*}"
    expected_value="${field_value#*=}"
    actual_detail="$(stable_field_detail "$path")" || {
      fail "[$operating_system/$requested_case] the applied settings report no record for the stable field $path, so its value cannot be checked; STABLE_FIELD_VALUES names a path that STABLE_FIELD_KINDS does not"
      continue
    }
    [[ $actual_detail == "$expected_value" ]] ||
      fail "[$operating_system/$requested_case] the stable field $path holds '$actual_detail' and not '$expected_value'; this field's whole content is that value, so a render that keeps the key and changes the value has changed the policy"
  done

  actual_detail="$(stable_field_detail "$STATUS_LINE_COMMAND_PATH")" || {
    fail "[$operating_system/$requested_case] the applied settings report no record for $STATUS_LINE_COMMAND_PATH"
    actual_detail=''
  }
  [[ $actual_detail == *"$STATUS_LINE_COMMAND_SUFFIX" ]] ||
    fail "[$operating_system/$requested_case] $STATUS_LINE_COMMAND_PATH is '$actual_detail', which does not end in $STATUS_LINE_COMMAND_SUFFIX; the status line still has a command and it is no longer this repo's"

  deny_values="$(sorted_lines ${parsed_deny_values[@]+"${parsed_deny_values[@]}"})"
  for required_entry in "${REQUIRED_DENY_ENTRIES[@]}"; do
    grep -qxF "$required_entry" <<<"$deny_values" ||
      fail "[$operating_system/$requested_case] permissions.deny no longer holds $required_entry; that list is the deny policy that applies even under bypassPermissions, and losing it is the worst outcome of a render this test exists to prevent"
  done

  # Every command under hooks, whatever event it hangs off. A hook that is gone
  # runs nothing, and `hooks` stays a non-empty map of five event keys when any
  # one of them is deleted, so nothing above this line can see the loss.
  hook_commands="$(sorted_lines ${parsed_hook_commands[@]+"${parsed_hook_commands[@]}"})"
  for required_fragment in "${REQUIRED_HOOK_COMMAND_FRAGMENTS[@]}"; do
    grep -qF -- "$required_fragment" <<<"$hook_commands" ||
      fail "[$operating_system/$requested_case] no hook command holds '$required_fragment'; that hook is one CLAUDE.md declares chezmoi-controlled, and the apply either dropped its event or repointed it"
  done
}

# The stable block of the first case verified, and the case it came from. Every
# later case must produce the same block: the SHAPE of the live settings file
# must not change one byte of what this repo manages outside enabledPlugins.
reference_stable_block=''
reference_stable_block_source=''

assert_stable_block_is_invariant() {
  local requested_case="$1" operating_system="$2" stable_block
  stable_block="$(
    sorted_lines ${parsed_stable_records[@]+"${parsed_stable_records[@]}"}
    sorted_lines ${parsed_deny_values[@]+"${parsed_deny_values[@]}"}
    sorted_lines ${parsed_hook_commands[@]+"${parsed_hook_commands[@]}"}
  )"
  if [[ -z $reference_stable_block_source ]]; then
    reference_stable_block="$stable_block"
    reference_stable_block_source="$operating_system/$requested_case"
    return 0
  fi
  [[ $stable_block == "$reference_stable_block" ]] ||
    fail "[$operating_system/$requested_case] the managed fields outside enabledPlugins differ from the ones $reference_stable_block_source produced, so the SHAPE of the live settings file is changing what this repo enforces. Expected [$(tr '\n' ' ' <<<"$reference_stable_block")] and got [$(tr '\n' ' ' <<<"$stable_block")]"
}

# verify_case <case> <operating-system> -- one whole cycle: write the fixture,
# prove the fixture, apply, prove the render.
verify_case() {
  local requested_case="$1" operating_system="$2"
  local applied_report apply_stderr

  write_case_fixture "$requested_case" "$operating_system"
  assert_fixture_precondition "$requested_case" "$operating_system"

  # A failed apply is terminal for this case: nothing downstream can be asserted
  # about a file that was never written. This is what catches a template that
  # parses as a complete declaration but does not compile, a template that dies
  # on the live file's shape, and a target that stopped being managed (measured:
  # chezmoi exits 1 with `not managed` when .chezmoiignore covers it).
  apply_stderr="$(case_apply_stderr_path "$requested_case" "$operating_system")"
  if ! apply_settings_target "$requested_case" "$operating_system" 2>"$apply_stderr"; then
    fail "[$operating_system/$requested_case] chezmoi could not apply $SETTINGS_TARGET_RELATIVE_PATH from this checkout: $(tr '\n' ' ' <"$apply_stderr")"
    return 0
  fi

  applied_report="$(settings_report "$requested_case" "$operating_system")" || {
    fail "[$operating_system/$requested_case] the applied $SETTINGS_TARGET_RELATIVE_PATH could not be read back as JSON; chezmoi's own error is on stderr above"
    return 0
  }
  parse_settings_report "$applied_report"

  if case_carries_passthrough "$requested_case"; then
    assert_passthrough_field "$requested_case" "$operating_system" 'applied' \
      'either the modify-template stopped passing unmanaged fields through, or it never received the fixture on .chezmoi.stdin and this test is not exercising the apply path'
  fi
  assert_rendered_enabled_plugins "$requested_case" "$operating_system"
  assert_stable_fields_survived "$requested_case" "$operating_system"
  assert_stable_block_is_invariant "$requested_case" "$operating_system"
  verified_case_count=$((verified_case_count + 1))
}

# file_bytes <path> -- a file's contents with a sentinel appended, so that a
# comparison sees trailing newlines. Command substitution strips them, and
# "the failed apply appended a newline" is exactly the kind of partial write
# this is here to catch.
file_bytes() {
  cat "$1"
  printf 'X'
}

# verify_unparseable_case <case> <operating-system> -- one live-file shape the
# render cannot survive. The apply MUST fail, its error MUST name the template
# so an operator can act on it, and the live file MUST come back byte-identical,
# because these bytes may be the only copy of settings the operator wants back.
# A PASS here is a pinned limitation, not an endorsement.
verify_unparseable_case() {
  local requested_case="$1" operating_system="$2"
  local settings_path apply_stderr bytes_before bytes_after

  write_case_fixture "$requested_case" "$operating_system"
  assert_fixture_precondition "$requested_case" "$operating_system"

  settings_path="$(case_settings_path "$requested_case" "$operating_system")"
  bytes_before="$(file_bytes "$settings_path")"

  apply_stderr="$(case_apply_stderr_path "$requested_case" "$operating_system")"
  if apply_settings_target "$requested_case" "$operating_system" 2>"$apply_stderr"; then
    fail "[$operating_system/$requested_case] the apply SUCCEEDED against a live settings file that is not JSON. Nothing is broken, the template got BETTER than what this case pins: move this case into LIVE_FILE_CASES with the records it should render, and correct the KNOWN LIMIT comment in private_dot_claude/modify_settings.json and the matching paragraph in CLAUDE.md"
    return 0
  fi

  grep -qF -- "$UNPARSEABLE_APPLY_ERROR_FRAGMENT" "$apply_stderr" ||
    fail "[$operating_system/$requested_case] the apply failed without naming $UNPARSEABLE_APPLY_ERROR_FRAGMENT anywhere in its error, so this case cannot tell a live file it could not read from a sandbox it could not build, and an operator hitting the real thing is told nothing about which file to fix: $(tr '\n' ' ' <"$apply_stderr")"

  if [[ ! -f $settings_path ]]; then
    fail "[$operating_system/$requested_case] the failed apply REMOVED the live settings file; the failure is meant to be inert, and those bytes were the only copy of whatever the operator had"
    verified_unparseable_case_count=$((verified_unparseable_case_count + 1))
    return 0
  fi

  bytes_after="$(file_bytes "$settings_path")"
  [[ $bytes_before == "$bytes_after" ]] ||
    fail "[$operating_system/$requested_case] the failed apply CHANGED the live settings file; a partial write here destroys the only copy of whatever the operator had"

  verified_unparseable_case_count=$((verified_unparseable_case_count + 1))
}

: >"$BOOTSTRAP_CONFIG"
assert_test_constants
populate_sandbox_source

for target_operating_system in "${TARGET_OPERATING_SYSTEMS[@]}"; do
  write_sandbox_config "$target_operating_system"
  for live_file_case in "${LIVE_FILE_CASES[@]}"; do
    verify_case "$live_file_case" "$target_operating_system"
  done
  for unparseable_live_file_case in "${UNPARSEABLE_LIVE_FILE_CASES[@]}"; do
    verify_unparseable_case "$unparseable_live_file_case" "$target_operating_system"
  done
done

finish
