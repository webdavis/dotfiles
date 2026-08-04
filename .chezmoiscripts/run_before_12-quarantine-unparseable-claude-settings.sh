#!/bin/bash
# Move an unreadable ~/.claude/settings.json out of the way before the settings
# modify-template reads it.
#
# WHY. private_dot_claude/modify_settings.json is a chezmoi modify-template: it
# receives the live ~/.claude/settings.json on .chezmoi.stdin and hands it to
# fromJson, which HARD ERRORS on input that is not JSON. A modify-template that
# errors aborts the WHOLE apply rather than one target, so every later target and
# every run_after_ script is skipped and the corrupt file is left in place, which
# means permissions.deny is not restored either. No template can catch it:
# chezmoi's three JSON readers all fail the template on bad input and Go's
# text/template has no recover, so the repair has to run BEFORE the template.
# test/unit/claude-enabled-plugins.sh pins that limit from the template's side.
#
# WHAT IT DOES. An unreadable file is MOVED (never deleted) into
# ~/workspaces/backups and replaced with `{}`, so the same apply rebuilds every
# stable field from source. A readable file is left byte-identical and an absent
# one is left absent, so the common case is a no-op. Idempotent: what a
# quarantine leaves behind is readable, so the next run does nothing.
#
# WHAT THE MOVE COSTS. Per-plugin state (the boolean `claude plugin disable`
# writes, or a version pin) is READ from the live file rather than declared in
# the template, so a quarantine loses it and every declared plugin comes back
# enabled. The warning below says so. The alternative is an apply that cannot run
# at all.
#
# Ordering: after 10-system-packages, which is where jq comes from.

set -euo pipefail

settings="$HOME/.claude/settings.json"
backup_dir="$HOME/workspaces/backups"

warn() { printf 'quarantine-claude-settings: WARNING -- %s\n' "$*" >&2; }

[[ -f $settings ]] || exit 0

# No parser, no verdict. A fresh machine reaches this before Homebrew has
# installed jq, and a false positive would destroy a healthy settings file, so
# "cannot tell" leaves the file alone.
command -v jq >/dev/null 2>&1 || exit 0

# Readable means the file holds AT MOST ONE JSON value. `jq empty` alone is not
# that test: it accepts a STREAM of values, so `{"a": 1}{"b": 2}` passes it while
# Go's encoding/json rejects it (`invalid character '{' after top-level value`)
# and the template dies on exactly the file jq called fine. Slurping and counting
# is the single-value test. Zero values (an empty or whitespace-only file) count
# as readable on purpose: the template trims its stdin and treats that shape as
# an absent file. Every JSON value is accepted, an object or otherwise, because
# the template survives a whole-file `null` and a whole-file array too.
if jq -e -s 'length <= 1' <"$settings" >/dev/null 2>&1; then
  exit 0
fi

# ISO 8601 with hyphens inside the timestamp: BSD date has no -Is, and a colon in
# a filename is a poor idea on macOS anyway.
backup="$backup_dir/$(date -u +"%Y-%m-%dT%H-%M-%S").claude-settings-quarantined.backup.json"

if ! mkdir -p "$backup_dir" || ! mv "$settings" "$backup"; then
  warn "$settings does not parse and could not be moved into $backup_dir."
  warn "The apply will fail in modify_settings.json until that file is repaired by hand."
  exit 0
fi

warn "$settings did not parse. It was MOVED to $backup and replaced with an empty object."
warn "This apply rebuilds the managed fields. It does NOT restore per-plugin state: re-run 'claude plugin disable <id>' for anything that was disabled."
printf '{}\n' >"$settings"
