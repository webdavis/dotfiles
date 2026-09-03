#!/bin/bash
# Move an unreadable ~/.codex/config.toml out of the way before the Codex config
# modify-template reads it.
#
# WHY. private_dot_codex/modify_private_config.toml is a chezmoi modify-template:
# it receives the live ~/.codex/config.toml on .chezmoi.stdin and hands it to
# fromToml, which HARD ERRORS on input that is not TOML (measured 2026-09-03 on
# chezmoi 2.72.1: `error calling fromToml: toml: line 1: expected '.' or '='`).
# A modify-template that errors aborts the WHOLE apply rather than one target,
# so every later target and every run_after_ script is skipped. That includes
# run_after_05-osquery-known-good-manifests.sh, so a config.toml truncated
# mid-write stops the manifests refreshing and the pipeline audit pages CRIT on
# every tick. No template can catch it: Go's text/template has no recover, so
# the repair has to run BEFORE the template. This is the same shape as
# run_before_12 next door, which does it for ~/.claude/settings.json.
#
# WHAT IT DOES. An unreadable file is MOVED (never deleted) into
# ~/workspaces/backups and replaced with an EMPTY file, which is a valid TOML
# document, so the same apply rebuilds every stable field from source. A
# readable file is left byte-identical and an absent one is left absent, so the
# common case is a no-op. Idempotent: what a quarantine leaves behind is
# readable, so the next run does nothing.
#
# WHAT THE MOVE COSTS. [hooks.state] is READ from the live file rather than
# declared in the template, because Codex hook trust is operator-only. A
# quarantine therefore drops every hook approval and Codex re-prompts for all of
# them. That is the safe direction to fail, and the warning below says so. Live
# project trust, the [plugins.*] roster and the desktop app's own settings go
# the same way. The alternative is an apply that cannot run at all.
#
# Ordering: after 10-system-packages, which is where taplo comes from.

set -euo pipefail

config="$HOME/.codex/config.toml"
backup_dir="$HOME/workspaces/backups"

warn() { printf 'quarantine-codex-config: WARNING -- %s\n' "$*" >&2; }

[[ -f $config ]] || exit 0

# No parser, no verdict. A fresh machine reaches this before Homebrew has
# installed taplo, and a false positive would destroy a healthy config carrying
# every hook approval the operator has granted, so "cannot tell" leaves the file
# alone.
command -v taplo >/dev/null 2>&1 || exit 0

# `taplo check` is syntax validation, and its verdicts agree with the template's
# parser on every shape that matters (measured 2026-09-03): an empty file and a
# whitespace-only file both PASS, which is deliberate, since fromToml returns an
# empty map for those and the template treats that as a fresh machine. A
# duplicate key fails both. TOML has no stream concept, so the multi-root
# problem that forces `jq -s 'length <= 1'` in the sibling script has no
# equivalent here. RESIDUAL: taplo and Go's TOML reader are different
# implementations, so a file one accepts and the other rejects would still abort
# the apply. No such shape is known; none was searched for.
#
# --no-auto-config IS LOAD BEARING. Without it taplo walks up from its working
# directory looking for a taplo.toml, and chezmoi runs this script with whatever
# working directory the apply inherited. Measured 2026-09-03 on taplo 0.10.0: a
# rule there naming a schema makes a file that PARSES exit 1, which would
# quarantine a healthy config and drop every hook approval on the machine, and a
# rule that excludes the file makes taplo check zero files and exit 0, which
# lets real garbage through. The verdict has to be syntax and nothing else.
if taplo check --no-auto-config "$config" >/dev/null 2>&1; then
  exit 0
fi

# ISO 8601 with hyphens inside the timestamp: BSD date has no -Is, and a colon in
# a filename is a poor idea on macOS anyway.
backup="$backup_dir/$(date -u +"%Y-%m-%dT%H-%M-%S").codex-config-quarantined.backup.toml"

if ! mkdir -p "$backup_dir" || ! mv "$config" "$backup"; then
  warn "$config does not parse and could not be moved into $backup_dir."
  warn "The apply will fail in modify_private_config.toml until that file is repaired by hand."
  exit 0
fi

warn "$config did not parse. It was MOVED to $backup and replaced with an empty file."
warn "This apply rebuilds the managed fields. It does NOT restore hook trust: open Codex and run /hooks to re-approve them."
# 0600 to match the target the template writes: this file carries three secrets
# once the apply fills it in, and mv took the original's mode away with it.
(umask 077 && : >"$config")
