#!/usr/bin/env bash
#
# File-integrity watches cover the alerting pipeline's own homes and the
# alerter's config directory. The rendered osquery.conf must:
#
#   - watch ONLY the dedicated pipeline home under pipeline_integrity:
#     ~/.local/libexec/osquery. The entire osquery delivery path lives there
#     (send_alert's local banner and the curl-to-Hermes webhook are both under
#     it), so the pipeline manifest covers exactly what is watched. ~/.local/bin
#     is NOT under pipeline_integrity: it has its own category and its own
#     manifest, so neither list is made responsible for the other's files;
#   - watch ~/.local/bin under its OWN managed_bin category, so the
#     chezmoi-managed scripts that run unattended there (update-skills.sh,
#     homebrew-weekly-upgrade.sh, the claude-* hooks) generate events at all.
#     The verdict tracks only the paths the managed-bin manifest lists, so the
#     self-updating third-party shims sharing the directory stay silent;
#   - NOT hash ~/.local/bin. The hash maps are consumer-driven and the verdict
#     re-reads the file at judgment time (the event digest is explicitly not a
#     trust input), while the directory holds several hundred megabytes of
#     third-party binaries (zig, packer, mise, herdr, yt-dlp) that osqueryd would
#     otherwise sha256 on every change;
#   - watch the alerter's config directory (~/.config/osquery, where the
#     page-launchd allowlist lives) as allowlist_file;
#   - hash pipeline_integrity (the dedicated home) and ~/Library/LaunchAgents,
#     so pipeline-script and LaunchAgent events carry the sha256 the alerter's
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

# The dedicated pipeline home is watched in the EVENT map...
has_path file_paths pipeline_integrity "$render_home/.local/libexec/osquery/%%" ||
  fail "file_paths.pipeline_integrity must watch ~/.local/libexec/osquery/%% (the osquery delivery path's home)"
# ...and hashed, so its events carry the sha256 the alerter's (path, hash) check needs.
has_path file_paths_hashes pipeline_integrity "$render_home/.local/libexec/osquery/%%" ||
  fail "file_paths_hashes.pipeline_integrity must hash ~/.local/libexec/osquery/%%"

# ~/.local/bin is NOT under pipeline_integrity, in either map. It has its own
# category and its own manifest; folding it in here would put unrelated operator
# tools inside the file whose whole framing is the pipeline's own integrity.
if has_path file_paths pipeline_integrity "$render_home/.local/bin/%%"; then
  fail "file_paths.pipeline_integrity must NOT watch ~/.local/bin (it has its own category and manifest)"
fi
if has_path file_paths_hashes pipeline_integrity "$render_home/.local/bin/%%"; then
  fail "file_paths_hashes.pipeline_integrity must NOT hash ~/.local/bin"
fi

# ...but ~/.local/bin IS watched, under managed_bin. Without an event the verdict
# never runs, and the chezmoi-managed scripts that fire unattended from
# LaunchAgents and shell hooks would have no integrity coverage at all.
has_path file_paths managed_bin "$render_home/.local/bin/%%" ||
  fail "file_paths.managed_bin must watch ~/.local/bin/%% (the managed unattended operator scripts live there)"

# ...and deliberately NOT hashed. The verdict re-reads the file at judgment time
# and treats the event digest as untrusted, so a hash entry buys nothing, while
# ~/.local/bin holds hundreds of megabytes of third-party binaries (zig, packer,
# mise, herdr, yt-dlp) that osqueryd would sha256 on every change.
if has_path file_paths_hashes managed_bin "$render_home/.local/bin/%%"; then
  fail "file_paths_hashes must NOT hash ~/.local/bin (no consumer; hundreds of MB of third-party binaries)"
fi

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
printf 'PASS: file-integrity watches cover the dedicated pipeline home, ~/.local/bin under its own managed_bin category, and the alerter config dir, hashing only what the tuple check reads\n'
