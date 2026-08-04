#!/usr/bin/env bash
# claude-settings-quarantine.sh, the run_before quarantine script must move an
# unreadable ~/.claude/settings.json out of the way BEFORE the settings
# modify-template reads it, and must leave every readable one alone.
#
# WHY IT EXISTS. private_dot_claude/modify_settings.json receives the live
# settings file on .chezmoi.stdin and hands it to fromJson, which hard errors on
# input that is not JSON. A modify-template that errors aborts the WHOLE apply,
# so one corrupt byte in that file skips every other target and every run_after_
# script in the run. test/unit/claude-enabled-plugins.sh pins that limit from the
# template's side (UNPARSEABLE_LIVE_FILE_CASES); this test pins the repair.
#
# WHY THE READABLE CASES OUTNUMBER THE CORRUPT ONES. A quarantine that fires on
# a file the template can read is worse than the bug it fixes: it moves a
# healthy settings file away and drops the per-plugin state that only the live
# file carries. So every shape the template survives is asserted byte-identical
# afterwards, including the three that are not JSON OBJECTS (empty,
# whitespace-only, a whole-file JSON array), which a stricter "must be an
# object" predicate would quarantine wrongly.
#
# WHY A MULTI-ROOT CASE IS HERE. `jq empty` accepts a STREAM of JSON values, so
# `{"a": 1}{"b": 2}` passes it while Go's encoding/json (what the template uses)
# rejects it. That shape is the one an implementation reaching for the obvious
# `jq empty` gets wrong, so it gets its own case.
#
# HOW IT STAYS HERMETIC. Every run gets its own throwaway HOME under one mktemp
# directory. The operator's ~/.claude/settings.json and ~/workspaces/backups are
# never read or written.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly SCRIPT="$REPO_ROOT/.chezmoiscripts/run_before_12-quarantine-unparseable-claude-settings.sh"

# The repo's backup naming convention: timestamp first, hyphens inside the
# timestamp, a period before the name, `.backup` before the extension.
readonly BACKUP_NAME_PATTERN='^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}-[0-9]{2}-[0-9]{2}\.claude-settings-quarantined\.backup\.json$'

fail() {
  printf 'claude-settings-quarantine: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $SCRIPT ]] || fail "the quarantine script is missing: $SCRIPT"

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT

# Set by the helpers below; read by every case. case_home carries the home
# directory a case ran in, so no assertion has to live inside a command
# substitution (fail's exit would only leave the subshell there).
rc=0
output=''
case_home=''
declare -a backups=()

run_script() { # <home>
  rc=0
  output="$(HOME="$1" bash "$SCRIPT" 2>&1)" || rc=$?
}

collect_backups() { # <home>
  local candidate
  backups=()
  shopt -s nullglob
  for candidate in "$1"/workspaces/backups/*; do
    backups+=("$candidate")
  done
  shopt -u nullglob
}

# A live file the template CANNOT read: moved to ~/workspaces/backups with its
# bytes intact, replaced with `{}`, reported loudly, and the apply left alive.
assert_quarantined() { # <name> <bytes>
  local name=$1 bytes=$2 home original
  home="$sandbox/$name"
  mkdir -p "$home/.claude"
  case_home="$home"
  original="$sandbox/$name.original-bytes"
  printf '%s' "$bytes" >"$home/.claude/settings.json"
  printf '%s' "$bytes" >"$original"

  run_script "$home"
  [[ $rc -eq 0 ]] || fail "$name: exit $rc, want 0 (a quarantine must not block the apply)"

  collect_backups "$home"
  [[ ${#backups[@]} -eq 1 ]] || fail "$name: ${#backups[@]} files in ~/workspaces/backups, want exactly 1"
  [[ $(basename "${backups[0]}") =~ $BACKUP_NAME_PATTERN ]] ||
    fail "$name: backup name $(basename "${backups[0]}") does not match the repo naming convention"
  cmp -s "$original" "${backups[0]}" || fail "$name: the backup does not hold the original bytes"

  [[ -f $home/.claude/settings.json ]] || fail "$name: settings.json was left absent instead of reset"
  [[ $(cat "$home/.claude/settings.json") == '{}' ]] ||
    fail "$name: settings.json was not reset to {} (got $(cat "$home/.claude/settings.json"))"

  grep -qF "${backups[0]}" <<<"$output" || fail "$name: the warning does not name the backup path"
}

# A live file the template CAN read: byte-identical afterwards, and no backup
# directory brought into existence at all.
assert_untouched() { # <name> <bytes>
  local name=$1 bytes=$2 home original
  home="$sandbox/$name"
  mkdir -p "$home/.claude"
  original="$sandbox/$name.original-bytes"
  printf '%s' "$bytes" >"$home/.claude/settings.json"
  printf '%s' "$bytes" >"$original"

  run_script "$home"
  [[ $rc -eq 0 ]] || fail "$name: exit $rc, want 0"
  cmp -s "$original" "$home/.claude/settings.json" ||
    fail "$name: a settings file the template can read was modified"
  [[ ! -d $home/workspaces/backups ]] ||
    fail "$name: a settings file the template can read was quarantined"
}

# --- the shapes that abort the apply today ---------------------------------
# The first three are UNPARSEABLE_LIVE_FILE_CASES from
# test/unit/claude-enabled-plugins.sh, byte for byte.
assert_quarantined 'truncated-json' '{"voiceEnabled": true, "enabledPlugins": {'
assert_quarantined 'trailing-garbage' '{"voiceEnabled": true}}}
'
assert_quarantined 'not-json' 'this file is not json
'
assert_quarantined 'multi-root-json' '{"a": 1}{"b": 2}'
healed_home="$case_home"

# Idempotent: the file a quarantine leaves behind is readable, so a second run
# writes no second backup and changes nothing.
run_script "$healed_home"
[[ $rc -eq 0 ]] || fail "idempotence: exit $rc on the second run, want 0"
collect_backups "$healed_home"
[[ ${#backups[@]} -eq 1 ]] || fail "idempotence: a second run left ${#backups[@]} backups, want 1"
[[ $(cat "$healed_home/.claude/settings.json") == '{}' ]] ||
  fail "idempotence: a second run changed the healed settings.json"

# --- the shapes the template already survives ------------------------------
assert_untouched 'plain-object' '{
  "voiceEnabled": true,
  "enabledPlugins": { "codex@openai-codex": false }
}
'
assert_untouched 'empty-json-object' '{}'
assert_untouched 'empty-file' ''
assert_untouched 'blank-file' '
   '
assert_untouched 'whole-file-json-array' '[1, 2]'

# --- no live file ----------------------------------------------------------
# Nothing to read, so nothing to write: the script must not conjure a settings
# file, a .claude directory or a backup directory out of an absent one.
absent_home="$sandbox/absent-settings-file"
mkdir -p "$absent_home/.claude"
run_script "$absent_home"
[[ $rc -eq 0 ]] || fail "absent settings.json: exit $rc, want 0"
[[ ! -e $absent_home/.claude/settings.json ]] || fail "absent settings.json: the script created one"
[[ ! -d $absent_home/workspaces/backups ]] || fail "absent settings.json: the script created a backup directory"

bare_home="$sandbox/absent-claude-directory"
mkdir -p "$bare_home"
run_script "$bare_home"
[[ $rc -eq 0 ]] || fail "absent .claude: exit $rc, want 0"
[[ ! -e $bare_home/.claude ]] || fail "absent .claude: the script created it"
[[ ! -d $bare_home/workspaces/backups ]] || fail "absent .claude: the script created a backup directory"

printf 'claude-settings-quarantine: OK\n'
