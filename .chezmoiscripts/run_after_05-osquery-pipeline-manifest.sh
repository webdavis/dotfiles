#!/usr/bin/env bash
#
# run_after_05-osquery-pipeline-manifest.sh
# Refresh the root-owned pipeline-integrity manifest: the known-good sha256 of the
# osquery alerting pipeline's own scripts and plists. osqueryd (root) hashes those
# files; the alerter PAGES a file_events change whose (path, hash) is not in this
# manifest (tamper) and stays silent when it matches (a legitimate apply).
#
# NOT a template, deliberately. The mandated agent apply is
# `chezmoi apply --exclude=templates`, which skips template scripts but still
# applies the pipeline's plain executable_*.sh files. As a .tmpl this runner would
# never refresh the manifest on that path, so every agent apply would leave each
# updated pipeline file paging a false CRIT until someone ran a full interactive
# apply. Darwin is therefore gated at runtime, not with a Go-template guard.
#
# Runs in the EARLIEST after-phase slot (05): all target files are written before
# any after-script runs, so nothing is lost by going first, and the WatchPaths
# alerter judges a finding exactly once (its cursor advances), so the manifest must
# be current before the alerter can look at the change it just caused.
#
# The manifest is derived from chezmoi's INTENT, never from the tree it protects:
#   - the file SET comes from `chezmoi managed` (the source state), so a file an
#     attacker plants in the pipeline home is not managed, never enters the
#     manifest, and therefore pages forever - which is correct;
#   - each file's CONTENT hash comes from `chezmoi cat` (the source state rendered
#     as chezmoi would write it), so a managed file tampered on disk is signed with
#     its INTENDED hash and the tampered bytes then fail the tuple check and page.
# Nothing here reads or executes a user-writable DEPLOYED file.
# (`chezmoi status`/`verify` are not usable for this: nested inside an apply they
# fail with "timeout obtaining persistent state lock". `managed` and `cat` read the
# source state without that lock and work nested; this was verified empirically.)
#
# WHAT THIS DOES AND DOES NOT COVER. It detects tampering of the DEPLOYED tree
# after generation: bytes that differ from what chezmoi would write, and files that
# chezmoi does not manage at all. It does NOT defend against a compromised chezmoi
# SOURCE. The source is user-writable, and it is the authority an apply deploys
# from, so an attacker who edits a managed source file and then waits for (or
# races) a legitimate apply gets their bytes both deployed AND manifested, and this
# root install signs them. That is not fixable at this layer on a single-user
# machine, where the operator's own authority is what deploys; a git-dirty tripwire
# would not be a trust boundary either, since the same attacker can commit or
# rewrite local refs. The boundary this buys is post-deployment integrity, not
# supply-chain integrity of the source.
#
# Blind spot, recorded honestly: the manifest binds CONTENT only. An
# ATTRIBUTES_MODIFIED event (for example `chmod g+w` on a pipeline script) carries
# the unchanged hash, matches its tuple, and stays silent. A mode/owner column is a
# follow-up, not part of this change.
set -euo pipefail

[[ "$(uname)" == Darwin ]] || exit 0

# Keep this default in sync with PIPELINE_MANIFEST in
# dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh (the consumer).
# A test pins the two literals equal, because the producer and the consumer of a
# security-critical file must not agree only by copy-paste.
manifest="${OSQUERY_PIPELINE_MANIFEST:-/var/osquery/pipeline-known-good.sha256}"

home="${CHEZMOI_HOME_DIR:-$HOME}"

# chezmoi sets CHEZMOI_SOURCE_DIR for the scripts it runs; pin the nested calls to
# that same source so this cannot read a different checkout than the apply in
# progress. Absent (a direct invocation), fall back to the configured default.
chezmoi_args=()
[[ -n ${CHEZMOI_SOURCE_DIR:-} ]] && chezmoi_args+=(--source "$CHEZMOI_SOURCE_DIR")

# The pipeline file set, from managed intent: every managed file under the
# dedicated pipeline home, plus this host's own osquery LaunchAgents. This filter
# is the same set _pipeline_is_tracked matches and the same set osquery.conf
# watches; all three must stay identical or a watched-but-unmanifested file pages
# forever and a manifested-but-unwatched file is never checked.
#
# The listing is materialized to a file under an EXPLICIT status check rather than
# piped straight into the loop: a process substitution discards the producer's exit
# status, so a `chezmoi managed` that emitted some paths and THEN failed would hand
# the loop a PARTIAL set, pass the non-empty guard below, and root-install a
# manifest missing tuples over a complete one - making every later legitimate
# change to a dropped file page forever.
managed_list="$(mktemp)"
sorted_list="$(mktemp)"
trap 'rm -f "$managed_list" "$sorted_list" "${fresh:-}"' EXIT

if ! chezmoi "${chezmoi_args[@]}" managed --path-style=absolute --include=files >"$managed_list"; then
  printf 'osquery pipeline manifest: could not list managed files, refusing to rewrite the manifest\n' >&2
  exit 1
fi
if ! LC_ALL=C sort "$managed_list" >"$sorted_list"; then
  printf 'osquery pipeline manifest: could not sort the managed listing, refusing to rewrite the manifest\n' >&2
  exit 1
fi

paths=()
while IFS= read -r target; do
  case "$target" in
    "$home"/.local/libexec/osquery/*) paths+=("$target") ;;
    "$home"/Library/LaunchAgents/com.webdavis.osquery-*.plist) paths+=("$target") ;;
  esac
done <"$sorted_list"

fresh="$(mktemp)"

# shasum format ("<sha256>  <path>"), path-sorted above for a byte-reproducible
# manifest. The hash is captured into a VARIABLE first, deliberately: a command
# substitution used directly as a printf argument would discard a failing
# `chezmoi cat` (printf itself still succeeds) and emit a tuple with an empty hash,
# quietly corrupting the manifest. As an assignment its failure is the simple
# command's status, so set -e aborts and the previous manifest stays in force.
for target in "${paths[@]}"; do
  file_hash="$(chezmoi "${chezmoi_args[@]}" cat "$target" | shasum -a 256 | awk '{print $1}')"
  if [[ -z $file_hash ]]; then
    printf 'osquery pipeline manifest: could not hash %s, refusing to rewrite the manifest\n' "$target" >&2
    exit 1
  fi
  printf '%s  %s\n' "$file_hash" "$target"
done >"$fresh"

# Never let an empty render overwrite a good manifest.
if [[ ! -s $fresh ]]; then
  printf 'osquery pipeline manifest: refusing to install an EMPTY manifest (no managed pipeline files resolved)\n' >&2
  exit 1
fi

# The deployed manifest is root-owned 0644 (world-readable), so the compare needs
# no privilege; only a real content change warrants the sudo write. A missing
# manifest (fresh machine) compares unequal and installs.
if cmp -s "$fresh" "$manifest"; then
  exit 0
fi

# Fresh host: /var/osquery is created by the osquery setup script, which is a
# TEMPLATE and so is skipped by `chezmoi apply --exclude=templates` - the very
# command this runner is plain in order to run under. Create the manifest's parent
# ourselves rather than fail the apply and leave the host with no manifest at all.
# Only when it is actually missing, so a normal apply performs no extra privileged
# call, and idempotent either way.
manifest_dir="$(dirname "$manifest")"
if [[ ! -d $manifest_dir ]]; then
  sudo install -d -o root -g wheel -m 0755 "$manifest_dir"
fi

sudo install -o root -g wheel -m 0644 "$fresh" "$manifest"
