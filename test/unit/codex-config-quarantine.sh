#!/usr/bin/env bash
# codex-config-quarantine.sh, the run_before quarantine script must move an
# unreadable ~/.codex/config.toml out of the way BEFORE the Codex config
# modify-template reads it, and must leave every readable one alone.
#
# WHY IT EXISTS. private_dot_codex/modify_private_config.toml receives the live
# config on .chezmoi.stdin and hands it to fromToml, which hard errors on input
# that is not TOML. A modify-template that errors aborts the WHOLE apply, so one
# corrupt byte in that file skips every other target and every run_after_ script
# in the run, the osquery known-good manifest refresh among them.
#
# WHY THE READABLE CASES OUTNUMBER THE CORRUPT ONES. A quarantine that fires on
# a file the template can read is worse than the bug it fixes: it moves a healthy
# config away and drops every hook approval, which only the live file carries. So
# every shape the template survives is asserted byte-identical afterwards,
# including the two that hold no key at all (empty, whitespace-only), which a
# stricter "must have content" predicate would quarantine wrongly.
#
# WHY THE DUPLICATE-KEY CASE IS HERE. It is the shape that looks fine to a reader
# and to a naive line-based check, and both parsers reject it, so it pins that
# the script asks a real TOML parser rather than pattern-matching.
#
# HOW IT STAYS HERMETIC. Every run gets its own throwaway HOME under one mktemp
# directory. The operator's ~/.codex/config.toml and ~/workspaces/backups are
# never read or written.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly SCRIPT="$REPO_ROOT/.chezmoiscripts/run_before_13-quarantine-unparseable-codex-config.sh"

# The repo's backup naming convention: timestamp first, hyphens inside the
# timestamp, a period before the name, `.backup` before the extension.
readonly BACKUP_NAME_PATTERN='^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}-[0-9]{2}-[0-9]{2}\.codex-config-quarantined\.backup\.toml$'

fail() {
  printf 'codex-config-quarantine: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $SCRIPT ]] || fail "the quarantine script is missing: $SCRIPT"
command -v taplo >/dev/null 2>&1 || fail "taplo is missing, so no case here would exercise a verdict"

# pwd -P because mktemp -d hands back a path through the /var symlink, and the
# taplo case below turns on whether a tool sees the config file as living under
# the directory it found its own configuration in.
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
# bytes intact, replaced with an empty file at 0600, reported loudly, and the
# apply left alive.
assert_quarantined() { # <name> <bytes>
  local name=$1 bytes=$2 home original mode
  home="$sandbox/$name"
  mkdir -p "$home/.codex"
  case_home="$home"
  original="$sandbox/$name.original-bytes"
  printf '%s' "$bytes" >"$home/.codex/config.toml"
  printf '%s' "$bytes" >"$original"

  run_script "$home"
  [[ $rc -eq 0 ]] || fail "$name: exit $rc, want 0 (a quarantine must not block the apply)"

  collect_backups "$home"
  [[ ${#backups[@]} -eq 1 ]] || fail "$name: ${#backups[@]} files in ~/workspaces/backups, want exactly 1"
  [[ $(basename "${backups[0]}") =~ $BACKUP_NAME_PATTERN ]] ||
    fail "$name: backup name $(basename "${backups[0]}") does not match the repo naming convention"
  cmp -s "$original" "${backups[0]}" || fail "$name: the backup does not hold the original bytes"

  [[ -f $home/.codex/config.toml ]] || fail "$name: config.toml was left absent instead of reset"
  [[ ! -s $home/.codex/config.toml ]] ||
    fail "$name: config.toml was not reset to an empty file"

  # The replacement is about to be filled with three KeePassXC secrets by the
  # apply that follows, and mv took the original's mode away with the file.
  mode="$(stat -f '%Lp' "$home/.codex/config.toml")"
  [[ $mode == 600 ]] || fail "$name: the replacement is mode $mode, want 600"

  grep -qF "${backups[0]}" <<<"$output" || fail "$name: the warning does not name the backup path"
  grep -qF '/hooks' <<<"$output" || fail "$name: the warning does not say hook trust has to be re-approved"
}

# A live file the template CAN read: byte-identical afterwards, and no backup
# directory brought into existence at all.
assert_untouched() { # <name> <bytes>
  local name=$1 bytes=$2 home original
  home="$sandbox/$name"
  mkdir -p "$home/.codex"
  original="$sandbox/$name.original-bytes"
  printf '%s' "$bytes" >"$home/.codex/config.toml"
  printf '%s' "$bytes" >"$original"

  run_script "$home"
  [[ $rc -eq 0 ]] || fail "$name: exit $rc, want 0"
  cmp -s "$original" "$home/.codex/config.toml" ||
    fail "$name: a config the template can read was modified"
  [[ ! -d $home/workspaces/backups ]] ||
    fail "$name: a config the template can read was quarantined"
}

# --- the shapes that abort the apply today ---------------------------------
assert_quarantined 'truncated-table-header' 'model = "gpt-5.6-sol"
[hooks.state."a:b:0:0"
'
assert_quarantined 'not-toml' 'this file is not toml
'
assert_quarantined 'duplicate-key' 'model = "a"
model = "b"
'
healed_home="$case_home"

# Idempotent: the file a quarantine leaves behind is readable, so a second run
# writes no second backup and changes nothing.
run_script "$healed_home"
[[ $rc -eq 0 ]] || fail "idempotence: exit $rc on the second run, want 0"
collect_backups "$healed_home"
[[ ${#backups[@]} -eq 1 ]] || fail "idempotence: a second run left ${#backups[@]} backups, want 1"
[[ ! -s $healed_home/.codex/config.toml ]] ||
  fail "idempotence: a second run changed the healed config.toml"

# --- the shapes the template already survives ------------------------------
# The hooks.state entry is the one this file exists to protect: it must survive
# a run byte for byte, because the template can only read trust back, never
# declare it.
assert_untouched 'plain-config' 'model = "gpt-5.6-sol"

[projects."/Users/stephen/workspaces/Ivy"]
trust_level = "trusted"

[hooks.state."/Users/stephen/.codex/hooks.json:stop:0:0"]
trusted_hash = "sha256:0000"
'
assert_untouched 'empty-file' ''
assert_untouched 'blank-file' '
   '

# --- no live file ----------------------------------------------------------
# Nothing to read, so nothing to write: the script must not conjure a config
# file, a .codex directory or a backup directory out of an absent one.
absent_home="$sandbox/absent-config-file"
mkdir -p "$absent_home/.codex"
run_script "$absent_home"
[[ $rc -eq 0 ]] || fail "absent config.toml: exit $rc, want 0"
[[ ! -e $absent_home/.codex/config.toml ]] || fail "absent config.toml: the script created one"
[[ ! -d $absent_home/workspaces/backups ]] || fail "absent config.toml: the script created a backup directory"

bare_home="$sandbox/absent-codex-directory"
mkdir -p "$bare_home"
run_script "$bare_home"
[[ $rc -eq 0 ]] || fail "absent .codex: exit $rc, want 0"
[[ ! -e $bare_home/.codex ]] || fail "absent .codex: the script created it"
[[ ! -d $bare_home/workspaces/backups ]] || fail "absent .codex: the script created a backup directory"

# --- no parser -------------------------------------------------------------
# A fresh machine reaches this script before Homebrew has installed taplo. With
# no verdict available the corrupt file must be LEFT ALONE rather than guessed
# at, because a false positive costs every hook approval on the machine.
noparser_home="$sandbox/no-taplo"
mkdir -p "$noparser_home/.codex" "$sandbox/empty-path"
printf 'this file is not toml\n' >"$noparser_home/.codex/config.toml"
rc=0
output="$(HOME="$noparser_home" PATH="$sandbox/empty-path" /bin/bash "$SCRIPT" 2>&1)" || rc=$?
[[ $rc -eq 0 ]] || fail "no parser: exit $rc, want 0"
[[ $(cat "$noparser_home/.codex/config.toml") == 'this file is not toml' ]] ||
  fail "no parser: the corrupt config was touched without a verdict"
[[ ! -d $noparser_home/workspaces/backups ]] ||
  fail "no parser: the script quarantined a file it could not check"

# --- a taplo config in the working directory -------------------------------
# The verdict must come from the file's syntax and nothing else. taplo searches
# from its working directory upwards for a taplo.toml, chezmoi runs this script
# with whatever working directory the apply inherited, and a rule found that way
# swings the verdict BOTH ways: a schema rule fails a config that parses, which
# would quarantine a healthy file and cost every hook approval on the machine,
# and an exclude rule collects no files at all and passes a config that does not
# parse. Both cases put the config under the same directory the taplo config
# sits in, because that is what a taplo rule's include patterns are resolved
# against, and it is the shape an apply run from the home directory has.
healthy_home="$sandbox/taplo-config-in-cwd-healthy"
mkdir -p "$healthy_home/.codex"
printf 'model = "gpt-5.6-sol"\n' >"$healthy_home/.codex/config.toml"
cat >"$healthy_home/taplo.toml" <<'SCHEMA_RULE_TAPLO_CONFIG'
[[rule]]
include = ["**/config.toml"]
schema.path = "/nonexistent-schema.json"
SCHEMA_RULE_TAPLO_CONFIG
run_script_in "$healthy_home" "$healthy_home"
[[ $rc -eq 0 ]] || fail "taplo schema rule in cwd: exit $rc, want 0"
[[ $(cat "$healthy_home/.codex/config.toml") == 'model = "gpt-5.6-sol"' ]] ||
  fail "taplo schema rule in cwd: a config the template can read was quarantined"
[[ ! -d $healthy_home/workspaces/backups ]] ||
  fail "taplo schema rule in cwd: a config the template can read was moved to the backup directory"

corrupt_home="$sandbox/taplo-config-in-cwd-corrupt"
mkdir -p "$corrupt_home/.codex"
printf 'this file is not toml\n' >"$corrupt_home/.codex/config.toml"
cat >"$corrupt_home/taplo.toml" <<'EXCLUDING_TAPLO_CONFIG'
include = ["never-matches/*.toml"]
EXCLUDING_TAPLO_CONFIG
run_script_in "$corrupt_home" "$corrupt_home"
[[ $rc -eq 0 ]] || fail "taplo exclude rule in cwd: exit $rc, want 0"
collect_backups "$corrupt_home"
[[ ${#backups[@]} -eq 1 ]] ||
  fail "taplo exclude rule in cwd: ${#backups[@]} backups, want 1 (the verdict followed the config, not the syntax)"

printf 'codex-config-quarantine: OK\n'
