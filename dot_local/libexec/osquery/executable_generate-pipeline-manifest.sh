#!/usr/bin/env bash
#
# generate-pipeline-manifest.sh - print the pipeline-integrity known-good manifest
# to stdout: one shasum line ("<sha256>  <abspath>") for every deployed file that
# makes up the osquery alerting pipeline. results-alerter.sh consults this manifest
# (via pipeline-verdict.sh) to tell a legitimate chezmoi apply from a tamper, so the
# manifest's coverage must equal the real pipeline exactly.
#
# The set is DERIVED from the real deployed tree, never a hand-maintained list: a
# new pipeline script or plist is covered automatically, so the manifest cannot
# drift out of sync with what ships (the brew-shellenv-cache regeneration lesson).
# Because it hashes the RENDERED, deployed files, a data-driven plist (the digest
# agent's StartCalendarInterval) is captured as it actually landed on disk, with no
# re-key bookkeeping.
#
# Scope: every file under ~/.local/libexec/osquery (recursively, so the
# results-alerter/ helpers are covered) plus this host's own
# ~/Library/LaunchAgents/com.webdavis.osquery-*.plist agents. Fail-safe: an
# unreadable file aborts (non-zero), so the caller leaves the prior manifest in
# place and the changed file fails the tuple check and PAGES rather than being
# silently vouched for.
set -euo pipefail

libexec_root="$HOME/.local/libexec/osquery"
agents_dir="$HOME/Library/LaunchAgents"

# Enumerate the pipeline files NUL-delimited (a path is never word-split), sort by
# path under a stable C collation for a byte-reproducible manifest, then hash each.
{
  [[ -d $libexec_root ]] && find "$libexec_root" -type f -print0
  shopt -s nullglob
  for plist in "$agents_dir"/com.webdavis.osquery-*.plist; do
    printf '%s\0' "$plist"
  done
  shopt -u nullglob
} | LC_ALL=C sort -z | while IFS= read -r -d '' file; do
  shasum -a 256 "$file"
done
