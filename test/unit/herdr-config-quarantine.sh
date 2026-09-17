#!/usr/bin/env bash
# herdr-config-quarantine.sh, the run_before quarantine script must move an
# unreadable ~/.config/herdr/config.toml out of the way BEFORE the herdr
# config modify-template reads it, and must leave every readable one alone.
#
# WHY IT EXISTS. dot_config/herdr/modify_config.toml receives the live config
# on .chezmoi.stdin and hands it to fromToml, which hard errors on input that
# is not TOML. A modify-template that errors aborts the WHOLE apply, so one
# corrupt byte in that file (herdr killed mid-write, or a hand edit gone
# wrong) skips every other target and every run_after_ script in the run, the
# osquery known-good manifest refresh among them.
#
# WHY THE READABLE CASES OUTNUMBER THE CORRUPT ONES. A quarantine that fires
# on a file the template can read is worse than the bug it fixes: it moves a
# healthy config away and drops every live-only keybinding and sidebar row,
# which only the live file carries. So every shape the template survives is
# asserted byte-identical afterwards, including the two that hold no key at
# all (empty, whitespace-only), which a stricter "must have content"
# predicate would quarantine wrongly.
#
# WHY THE DUPLICATE-KEY CASE IS HERE. It is the shape that looks fine to a
# reader and to a naive line-based check, and both parsers reject it, so it
# pins that the script asks a real TOML parser rather than pattern-matching.
#
# HOW IT STAYS HERMETIC. Every run gets its own throwaway HOME under one
# mktemp directory. The operator's ~/.config/herdr/config.toml and
# ~/workspaces/backups are never read or written.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly SCRIPT="$REPO_ROOT/.chezmoiscripts/run_before_14-quarantine-unparseable-herdr-config.sh"

# The repo's backup naming convention: timestamp first, hyphens inside the
# timestamp, a period before the name, `.backup` before the extension.
readonly BACKUP_NAME_PATTERN='^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}-[0-9]{2}-[0-9]{2}\.herdr-config-quarantined\.backup\.toml$'

fail() {
  printf 'herdr-config-quarantine: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $SCRIPT ]] || fail "the quarantine script is missing: $SCRIPT"
command -v taplo >/dev/null 2>&1 || fail "taplo is missing, so no case here would exercise a verdict"

# pwd -P because mktemp -d hands back a path through the /var symlink, and the
# taplo case below turns on whether a tool sees the config file as living
# under the directory it found its own configuration in.
sandbox="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$sandbox"' EXIT

rc=0
output=''
case_home=''
declare -a backups=()

run_script() { # <home>
  rc=0
  output="$(HOME="$1" bash "$SCRIPT" 2>&1)" || rc=$?
}

run_script_in() { # <working-directory> <home>
  rc=0
  output="$(cd "$1" && HOME="$2" bash "$SCRIPT" 2>&1)" || rc=$?
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
# bytes intact, replaced with an empty file, reported loudly, and the apply
# left alive.
assert_quarantined() { # <name> <bytes>
  local name=$1 bytes=$2 home original
  home="$sandbox/$name"
  mkdir -p "$home/.config/herdr"
  case_home="$home"
  original="$sandbox/$name.original-bytes"
  printf '%s' "$bytes" >"$home/.config/herdr/config.toml"
  printf '%s' "$bytes" >"$original"

  run_script "$home"
  [[ $rc -eq 0 ]] || fail "$name: exit $rc, want 0 (a quarantine must not block the apply)"

  collect_backups "$home"
  [[ ${#backups[@]} -eq 1 ]] || fail "$name: ${#backups[@]} files in ~/workspaces/backups, want exactly 1"
  [[ $(basename "${backups[0]}") =~ $BACKUP_NAME_PATTERN ]] ||
    fail "$name: backup name $(basename "${backups[0]}") does not match the repo naming convention"
  cmp -s "$original" "${backups[0]}" || fail "$name: the backup does not hold the original bytes"

  [[ -f $home/.config/herdr/config.toml ]] || fail "$name: config.toml was left absent instead of reset"
  [[ ! -s $home/.config/herdr/config.toml ]] ||
    fail "$name: config.toml was not reset to an empty file"

  grep -qF "${backups[0]}" <<<"$output" || fail "$name: the warning does not name the backup path"
  grep -qF 'live-only' <<<"$output" || fail "$name: the warning does not say live-only keys are lost"
}

# A live file the template CAN read: byte-identical afterwards, and no backup
# directory brought into existence at all.
assert_untouched() { # <name> <bytes>
  local name=$1 bytes=$2 home original
  home="$sandbox/$name"
  mkdir -p "$home/.config/herdr"
  original="$sandbox/$name.original-bytes"
  printf '%s' "$bytes" >"$home/.config/herdr/config.toml"
  printf '%s' "$bytes" >"$original"

  run_script "$home"
  [[ $rc -eq 0 ]] || fail "$name: exit $rc, want 0"
  cmp -s "$original" "$home/.config/herdr/config.toml" ||
    fail "$name: a config the template can read was modified"
  [[ ! -d $home/workspaces/backups ]] ||
    fail "$name: a config the template can read was quarantined"
}

# --- the shapes that abort the apply today ---------------------------------
assert_quarantined 'truncated-table-header' 'onboarding = false
[keys
'
assert_quarantined 'not-toml' 'this is not toml [[[
'
assert_quarantined 'duplicate-key' 'onboarding = false
onboarding = true
'
healed_home="$case_home"

# Idempotent: the file a quarantine leaves behind is readable, so a second run
# writes no second backup and changes nothing.
run_script "$healed_home"
[[ $rc -eq 0 ]] || fail "idempotence: exit $rc on the second run, want 0"
collect_backups "$healed_home"
[[ ${#backups[@]} -eq 1 ]] || fail "idempotence: a second run left ${#backups[@]} backups, want 1"
[[ ! -s $healed_home/.config/herdr/config.toml ]] ||
  fail "idempotence: a second run changed the healed config.toml"

# --- the shapes the template already survives ------------------------------
assert_untouched 'plain-config' 'onboarding = false

[ui.sidebar.agents.rows_by_agent.claude]
row = 1
'
assert_untouched 'empty-file' ''
assert_untouched 'blank-file' '
   '

# --- no live file ------------------------------------------------------------
# Nothing to read, so nothing to write: the script must not conjure a config
# file, a herdr directory or a backup directory out of an absent one.
absent_home="$sandbox/absent-config-file"
mkdir -p "$absent_home/.config/herdr"
run_script "$absent_home"
[[ $rc -eq 0 ]] || fail "absent config.toml: exit $rc, want 0"
[[ ! -e $absent_home/.config/herdr/config.toml ]] || fail "absent config.toml: the script created one"
[[ ! -d $absent_home/workspaces/backups ]] || fail "absent config.toml: the script created a backup directory"

bare_home="$sandbox/absent-herdr-directory"
mkdir -p "$bare_home/.config"
run_script "$bare_home"
[[ $rc -eq 0 ]] || fail "absent .config/herdr: exit $rc, want 0"
[[ ! -e $bare_home/.config/herdr ]] || fail "absent .config/herdr: the script created it"
[[ ! -d $bare_home/workspaces/backups ]] || fail "absent .config/herdr: the script created a backup directory"

# --- no parser -------------------------------------------------------------
# A fresh machine reaches this script before Homebrew has installed taplo.
# With no verdict available the corrupt file must be LEFT ALONE rather than
# guessed at, because a false positive costs every live-only key on the
# machine.
noparser_home="$sandbox/no-taplo"
mkdir -p "$noparser_home/.config/herdr" "$sandbox/empty-path"
printf 'this is not toml [[[\n' >"$noparser_home/.config/herdr/config.toml"
rc=0
output="$(HOME="$noparser_home" PATH="$sandbox/empty-path" /bin/bash "$SCRIPT" 2>&1)" || rc=$?
[[ $rc -eq 0 ]] || fail "no parser: exit $rc, want 0"
[[ $(cat "$noparser_home/.config/herdr/config.toml") == 'this is not toml [[[' ]] ||
  fail "no parser: the corrupt config was touched without a verdict"
[[ ! -d $noparser_home/workspaces/backups ]] ||
  fail "no parser: the script quarantined a file it could not check"

# --- a taplo config in the working directory --------------------------------
# The verdict must come from the file's syntax and nothing else. taplo
# searches from its working directory upwards for a taplo.toml, chezmoi runs
# this script with whatever working directory the apply inherited, and a rule
# found that way swings the verdict BOTH ways: a schema rule fails a config
# that parses, which would quarantine a healthy file and cost every live-only
# key on the machine, and an exclude rule collects no files at all and passes
# a config that does not parse. Both cases put the config under the same
# directory the taplo config sits in, because that is what a taplo rule's
# include patterns are resolved against, and it is the shape an apply run
# from the home directory has.
healthy_home="$sandbox/taplo-config-in-cwd-healthy"
mkdir -p "$healthy_home/.config/herdr"
printf 'onboarding = false\n' >"$healthy_home/.config/herdr/config.toml"
cat >"$healthy_home/taplo.toml" <<'SCHEMA_RULE_TAPLO_CONFIG'
[[rule]]
include = ["**/config.toml"]
schema.path = "/nonexistent-schema.json"
SCHEMA_RULE_TAPLO_CONFIG
run_script_in "$healthy_home" "$healthy_home"
[[ $rc -eq 0 ]] || fail "taplo schema rule in cwd: exit $rc, want 0"
[[ $(cat "$healthy_home/.config/herdr/config.toml") == 'onboarding = false' ]] ||
  fail "taplo schema rule in cwd: a config the template can read was quarantined"
[[ ! -d $healthy_home/workspaces/backups ]] ||
  fail "taplo schema rule in cwd: a config the template can read was moved to the backup directory"

corrupt_home="$sandbox/taplo-config-in-cwd-corrupt"
mkdir -p "$corrupt_home/.config/herdr"
printf 'this is not toml [[[\n' >"$corrupt_home/.config/herdr/config.toml"
cat >"$corrupt_home/taplo.toml" <<'EXCLUDING_TAPLO_CONFIG'
include = ["never-matches/*.toml"]
EXCLUDING_TAPLO_CONFIG
run_script_in "$corrupt_home" "$corrupt_home"
[[ $rc -eq 0 ]] || fail "taplo exclude rule in cwd: exit $rc, want 0"
collect_backups "$corrupt_home"
[[ ${#backups[@]} -eq 1 ]] ||
  fail "taplo exclude rule in cwd: ${#backups[@]} backups, want 1 (the verdict followed the config, not the syntax)"

printf 'herdr-config-quarantine: OK\n'
