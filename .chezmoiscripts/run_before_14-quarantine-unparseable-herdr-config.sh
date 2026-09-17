#!/bin/bash
# Move an unreadable ~/.config/herdr/config.toml out of the way before the
# herdr config modify-template reads it.
#
# WHY. dot_config/herdr/modify_config.toml is a chezmoi modify-template: it
# receives the live ~/.config/herdr/config.toml on .chezmoi.stdin and hands it
# to fromToml, which HARD ERRORS on input that is not TOML (measured
# 2026-09-17 on chezmoi 2.72.1: `error calling fromToml: toml: line 1:
# expected '.' or '=', but got 'i' instead`). A modify-template that errors
# aborts the WHOLE apply rather than one target, so every later target and
# every run_after_ script is skipped, including
# run_after_05-osquery-known-good-manifests.sh, so a config.toml truncated
# mid-write (herdr killed while saving a settings-screen edit) stops the
# manifests refreshing and the pipeline audit pages CRIT on every tick. No
# template can catch it: Go's text/template has no recover, so the repair has
# to run BEFORE the template. This is the same shape as run_before_13 next
# door, which does it for ~/.codex/config.toml.
#
# WHAT IT DOES. An unreadable file is MOVED (never deleted) into
# ~/workspaces/backups and replaced with an EMPTY file, which is a valid TOML
# document, so the same apply rebuilds every stable field from source. A
# readable file is left byte-identical and an absent one is left absent, so
# the common case is a no-op. Idempotent: what a quarantine leaves behind is
# readable, so the next run does nothing.
#
# WHAT THE MOVE COSTS. Every FREE-DRIFT key the modify-template preserves (an
# undeclared [ui.sidebar.agents.rows_by_agent.<agent>] table, a live-only
# [[keys.command]] entry, [operatorextra]) is READ from the live file rather
# than declared in the template, so a quarantine drops all of it back to this
# repo's declared defaults. The warning below says so. The alternative is an
# apply that cannot run at all.
#
# Ordering: after 10-system-packages, which is where taplo comes from.

set -euo pipefail

config="$HOME/.config/herdr/config.toml"
backup_dir="$HOME/workspaces/backups"

warn() { printf 'quarantine-herdr-config: WARNING -- %s\n' "$*" >&2; }

[[ -f $config ]] || exit 0

# No parser, no verdict. A fresh machine reaches this before Homebrew has
# installed taplo, and a false positive would destroy a healthy config
# carrying every live keybinding and sidebar row, so "cannot tell" leaves the
# file alone.
command -v taplo >/dev/null 2>&1 || exit 0

# `taplo check` is syntax validation, and its verdicts agree with the
# template's parser on every shape that matters (measured 2026-09-17,
# mirroring run_before_13's own measurement on the Codex config): an empty
# file and a whitespace-only file both PASS, which is deliberate, since
# fromToml returns an empty map for those and the template treats that as a
# fresh machine. A duplicate key fails both. TOML has no stream concept, so
# the multi-root problem that forces `jq -s 'length <= 1'` in the sibling
# JSON script has no equivalent here. RESIDUAL: taplo and Go's TOML reader are
# different implementations, so a file one accepts and the other rejects
# would still abort the apply. No such shape is known; none was searched for.
#
# --no-auto-config IS LOAD BEARING. Without it taplo walks up from its
# working directory looking for a taplo.toml, and chezmoi runs this script
# with whatever working directory the apply inherited. A rule there naming a
# schema could make a file that PARSES exit 1, which would quarantine a
# healthy config, and a rule that excludes the file would make taplo check
# zero files and exit 0, which lets real garbage through. The verdict has to
# be syntax and nothing else.
if taplo check --no-auto-config "$config" >/dev/null 2>&1; then
  exit 0
fi

# ISO 8601 with hyphens inside the timestamp: BSD date has no -Is, and a colon
# in a filename is a poor idea on macOS anyway.
backup="$backup_dir/$(date -u +"%Y-%m-%dT%H-%M-%S").herdr-config-quarantined.backup.toml"

if ! mkdir -p "$backup_dir" || ! mv "$config" "$backup"; then
  warn "$config does not parse and could not be moved into $backup_dir."
  warn "The apply will fail in modify_config.toml until that file is repaired by hand."
  exit 0
fi

warn "$config did not parse. It was MOVED to $backup and replaced with an empty file."
warn "This apply rebuilds the managed fields from this repo's declared defaults. It does NOT restore live-only keybindings, sidebar rows, or other undeclared keys: recreate them by hand or in herdr's own settings screens."
: >"$config"
