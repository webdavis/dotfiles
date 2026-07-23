#!/usr/bin/env bash
#
# generate-pipeline-manifest.sh emits the pipeline-integrity known-good manifest:
# one shasum-format line ("<sha256>  <abspath>") for every deployed pipeline file,
# derived by GLOBBING THE REAL TREE, never a hand-maintained list. The manifest is
# what pipeline-verdict.sh consults, so its coverage must equal the real pipeline
# and cannot drift.
#
# Scope (the approved core set): every file under ~/.local/libexec/osquery
# (recursively, so the results-alerter/ helpers are included) plus this host's own
# ~/Library/LaunchAgents/com.webdavis.osquery-*.plist agents. Non-osquery
# LaunchAgents and ~/.local/bin operator tools are OUT (the ~/.local/bin question
# is the held file-integrity-scope decision, not this generator's core scope).
#
# Unit test: run the generator against a fixture HOME whose pipeline tree we
# control, and pin: every pipeline file gets its correct shasum tuple; the output
# is path-sorted and byte-stable; a non-osquery agent and a ~/.local/bin tool are
# excluded; and adding a new libexec file adds a tuple (the no-drift property, the
# reason this is generated and not a list). No sudo, no install.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GENERATOR="$REPO_ROOT/dot_local/libexec/osquery/executable_generate-pipeline-manifest.sh"

fails=0
fail() {
  printf 'osquery-generate-pipeline-manifest: FAIL -- %s\n' "$*" >&2
  fails=$((fails + 1))
}

[[ -f $GENERATOR ]] || {
  printf 'osquery-generate-pipeline-manifest: FAIL -- missing generator: %s\n' "$GENERATOR" >&2
  exit 1
}
command -v shasum >/dev/null 2>&1 || {
  printf 'osquery-generate-pipeline-manifest: SKIP -- shasum is required\n'
  exit 0
}

home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT
libexec="$home/.local/libexec/osquery"
agents="$home/Library/LaunchAgents"
mkdir -p "$libexec/results-alerter" "$agents" "$home/.local/bin"

# The pipeline files the manifest MUST cover (a top-level script, a sourced helper
# in the subdir, and two own-agent plists). Fixed contents so the shasum is stable.
printf 'echo digest\n' >"$libexec/digest.sh"
printf 'echo entry\n' >"$libexec/results-alerter.sh"
printf 'true\n' >"$libexec/results-alerter/normalize.sh"
printf '<plist>digest</plist>\n' >"$agents/com.webdavis.osquery-digest.plist"
printf '<plist>alerter</plist>\n' >"$agents/com.webdavis.osquery-results-alerter.plist"

# Decoys the manifest MUST NOT cover: a non-osquery LaunchAgent and a ~/.local/bin
# operator tool (the ~/.local/bin scope is the held decision, out of the core set).
printf '<plist>atuin</plist>\n' >"$agents/com.webdavis.atuin-daemon.plist"
printf 'echo relay\n' >"$home/.local/bin/relay.sh"

manifest="$(HOME="$home" "$GENERATOR")" || fail "the generator exited non-zero"

# 1. Every pipeline file has its exact shasum tuple (the generator hashes the real
#    bytes, and covers the recursive helper under results-alerter/).
for rel in \
  ".local/libexec/osquery/digest.sh" \
  ".local/libexec/osquery/results-alerter.sh" \
  ".local/libexec/osquery/results-alerter/normalize.sh" \
  "Library/LaunchAgents/com.webdavis.osquery-digest.plist" \
  "Library/LaunchAgents/com.webdavis.osquery-results-alerter.plist"; do
  want="$(shasum -a 256 "$home/$rel")"
  grep -qxF -- "$want" <<<"$manifest" ||
    fail "missing or wrong tuple for $rel (expected line: $want)"
done

# 2. Path-sorted (LC_ALL=C by the path field) and byte-stable across runs.
paths="$(awk '{print $2}' <<<"$manifest")"
LC_ALL=C sort -c <<<"$paths" 2>/dev/null || fail "the manifest is not sorted by path"
again="$(HOME="$home" "$GENERATOR")" || fail "the generator exited non-zero on the second run"
[[ $manifest == "$again" ]] || fail "the manifest is not byte-stable across runs"

# 3. The non-osquery LaunchAgent is excluded (scoped to com.webdavis.osquery-* only).
grep -qF 'com.webdavis.atuin-daemon.plist' <<<"$manifest" &&
  fail "a non-osquery LaunchAgent (atuin) leaked into the manifest"

# 4. The ~/.local/bin operator tool is excluded (held scope, out of the core set).
grep -qF '/.local/bin/relay.sh' <<<"$manifest" &&
  fail "a ~/.local/bin operator tool (relay.sh) leaked into the manifest (core scope is libexec + own plists)"

# 5. No-drift: the set is DERIVED from the tree, not hardcoded. A new libexec file
#    appears in the manifest with no change to the generator.
printf 'echo new\n' >"$libexec/newly-added.sh"
after="$(HOME="$home" "$GENERATOR")" || fail "the generator exited non-zero after adding a file"
want_new="$(shasum -a 256 "$libexec/newly-added.sh")"
grep -qxF -- "$want_new" <<<"$after" ||
  fail "a newly-added libexec file was not picked up (the manifest is a hardcoded list, not generated)"

if [[ $fails -gt 0 ]]; then
  printf '%d check(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'osquery-generate-pipeline-manifest: OK (correct shasum tuples for every pipeline file incl. recursive helpers; path-sorted + byte-stable; non-osquery agent and ~/.local/bin tool excluded; new file auto-covered - no drift)\n'
