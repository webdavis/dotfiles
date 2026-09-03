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

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT

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

printf 'codex-config-quarantine: OK\n'
