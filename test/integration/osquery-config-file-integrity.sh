#!/usr/bin/env bash
#
# File-integrity watches cover the alerting pipeline's own homes and the
# alerter's config directory. The rendered osquery.conf must:
#
#   - watch BOTH pipeline homes under one pipeline_integrity category (no
#     split): ~/.local/libexec/osquery (the pipeline scripts' home) AND
#     ~/.local/bin (root-of-trust operator tools: relay.sh, hue-pulse.sh,
#     the weekly upgrade, update-skills all run unattended from there);
#   - watch the alerter's config directory (~/.config/osquery, where the
#     page-launchd allowlist lives) as allowlist_file;
#   - hash pipeline_integrity (both homes) and ~/Library/LaunchAgents, so
#     pipeline-script and LaunchAgent events carry the sha256 the alerter's
#     (path, hash) tuple check needs;
#   - keep the ~/.ssh directory EVENT watch but carry no ssh hashes entry:
#     the hash maps are consumer-driven (the tuple check) and nothing reads
#     ssh hashes, while hashing all of ~/.ssh would hash churny files
#     (known_hosts rewrites on every connection) and private key material
#     into logs for no consumer.
#
# Render-driven: chezmoi renders osquery.conf exactly as at apply time with
# HOME pointed at a scratch dir, so the assertions also prove the paths come
# from {{ .chezmoi.homeDir }} and not a hardcoded home.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 1 && pwd)"
cd "$REPO_ROOT" || exit 1

if ! command -v chezmoi >/dev/null 2>&1; then
  printf 'SKIP: chezmoi not found (run inside the nix dev shell)\n'
  exit 0
fi

render_home="$(mktemp -d)"
trap 'rm -rf "$render_home"' EXIT
render() { HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$1"; }

fails=0
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  fails=$((fails + 1))
}

CONF_TEMPLATE=".chezmoitemplates/osquery/osquery.conf"

conf_json="$(render "$CONF_TEMPLATE")" || fail "osquery.conf failed to render"
jq empty <<<"$conf_json" 2>/dev/null || fail "rendered osquery.conf is not valid JSON"

# No em-dash anywhere in the shipped config.
if grep -q $'\xe2\x80\x94' <<<"$conf_json"; then
  fail "the rendered osquery.conf contains an em-dash"
fi

# has_path <map> <category> <path> -- the category's array contains the path.
has_path() {
  jq -e --arg c "$2" --arg p "$3" ".${1}[\$c] // [] | index(\$p) != null" <<<"$conf_json" >/dev/null
}

# Both pipeline homes, one category, in the EVENT-watch map...
has_path file_paths pipeline_integrity "$render_home/.local/libexec/osquery/%%" ||
  fail "file_paths.pipeline_integrity must watch ~/.local/libexec/osquery/%% (the pipeline scripts' home)"
has_path file_paths pipeline_integrity "$render_home/.local/bin/%%" ||
  fail "file_paths.pipeline_integrity must watch ~/.local/bin/%% (root-of-trust operator tools)"

# ...and both in the HASH map, so their events carry the sha256 the alerter's
# (path, hash) tuple check needs.
has_path file_paths_hashes pipeline_integrity "$render_home/.local/libexec/osquery/%%" ||
  fail "file_paths_hashes.pipeline_integrity must hash ~/.local/libexec/osquery/%%"
has_path file_paths_hashes pipeline_integrity "$render_home/.local/bin/%%" ||
  fail "file_paths_hashes.pipeline_integrity must hash ~/.local/bin/%%"

# The alerter's config directory is event-watched.
has_path file_paths allowlist_file "$render_home/.config/osquery/%%" ||
  fail "file_paths.allowlist_file must watch ~/.config/osquery/%% (the page-launchd allowlist's home)"

# User LaunchAgents are hashed (the tuple check's other consumer).
has_path file_paths_hashes launch_agents "$render_home/Library/LaunchAgents/%%" ||
  fail "file_paths_hashes.launch_agents must hash ~/Library/LaunchAgents/%%"

# The ~/.ssh EVENT watch stays...
has_path file_paths ssh "$render_home/.ssh/%%" ||
  fail "file_paths.ssh must keep the ~/.ssh/%% directory event watch"

# ...but no ssh hashes entry: nothing consumes ssh hashes, and hashing all of
# ~/.ssh would hash churny known_hosts and private key material for no reader.
if jq -e '.file_paths_hashes | has("ssh")' <<<"$conf_json" >/dev/null; then
  fail "file_paths_hashes must not carry an ssh entry (no consumer; churny/private content)"
fi

if ((fails > 0)); then
  printf '%d file-integrity watch assertion(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'PASS: file-integrity watches cover both pipeline homes, the alerter config dir, and hash only what the tuple check reads\n'
