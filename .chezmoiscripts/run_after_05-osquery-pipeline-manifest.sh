#!/usr/bin/env bash
#
# run_after_05-osquery-pipeline-manifest.sh
# Refresh the root-owned pipeline-integrity manifest: the known-good sha256, mode
# and owner of the osquery alerting pipeline's own scripts and plists. osqueryd
# (root) watches those files; the alerter PAGES a file_events change whose
# (path, hash, mode, uid) tuple is not in this manifest (tamper) and stays silent
# when it matches (a legitimate apply).
#
# Format, one whitespace-separated tuple per line with the PATH LAST so a path
# containing spaces is still read whole by `read -r hash mode uid path`:
#
#   <sha256> <mode> <uid> <path>
#
# mode is exactly four octal digits (0755); uid is decimal.
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
#     its INTENDED hash and the tampered bytes then fail the tuple check and page;
#   - each file's MODE comes from `chezmoi dump` (the same source state, reported as
#     the perm chezmoi would apply), so it is the mode encoded by the source
#     attributes - the executable_ and private_ prefixes - and NOT the mode the file
#     currently carries. A file an attacker chmod-ed on disk is therefore signed
#     with its intended mode, and the drifted mode then fails the tuple check, for
#     exactly the reason the content hash is taken from intent rather than a disk
#     hash. This was verified empirically: chezmoi deploys a file at the perm dump
#     reports (0755 for executable_, 0644 plain, 0700 for private_executable_),
#     unclamped by umask, so intent and a clean deployment agree. `dump` needs a
#     throwaway --persistent-state to run nested; see the call site.
#   - each file's OWNER is the uid this apply is running as. chezmoi has no owner
#     attribute; it writes every target file as the invoking user, so the uid that
#     is running is BY DEFINITION the intended owner. That is process identity, not
#     a property read out of the protected tree, so an attacker who has already
#     chown-ed a pipeline file cannot influence it. (If an operator ever applied as
#     root, the files really would be root-owned and the manifest would record that,
#     so the derivation stays self-consistent.)
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
# Nor is the root install a boundary against a user-level attacker on this host.
# The manifest is written root-owned 0644 so a process at the user's privilege
# level cannot rewrite it, and the consumer refuses a manifest that is not (see
# _pipeline_manifest_is_trustworthy in pipeline-verdict.sh). That raises the bar and
# stops an unprivileged process from whitelisting a file it just tampered, but the
# operator account here has passwordless sudo, so a process running AS the operator
# can escalate with no prompt and rewrite the manifest at will. Every unattended
# chezmoi script depends on that sudo configuration, this one included, so the limit
# is recorded rather than closed: root ownership buys a higher bar and a loud
# failure mode, not integrity against a determined user-level attacker.
#
# Blind spot, recorded honestly: the owning GROUP is not bound. chezmoi has no
# group intent to derive it from, and a chgrp alone cannot make a file writable
# that the bound mode does not already grant group write to, so the only case a
# group column would add on its own is chgrp plus chmod - which the mode column
# already pages.
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
dump_json="$(mktemp)"
# A THROWAWAY persistent state for the nested `chezmoi dump` below; see the comment
# at that call for why it cannot share the apply's.
dump_state_dir="$(mktemp -d)"
trap 'rm -f "$managed_list" "$sorted_list" "$dump_json" "${fresh:-}"; rm -rf "$dump_state_dir"' EXIT

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

# Resolve the SET before anything else consults it. `chezmoi dump` with NO target
# arguments dumps the ENTIRE target state, which would render every managed
# template - including the ones that call keepassxc - from an unattended apply. An
# empty path list must therefore abort here, before the dump, not only at the
# empty-manifest guard further down.
if [[ ${#paths[@]} -eq 0 ]]; then
  printf 'osquery pipeline manifest: no managed pipeline files resolved, refusing to rewrite the manifest\n' >&2
  exit 1
fi

# The INTENDED mode of every pipeline file, in one dump of the same source state.
# `chezmoi dump --format=json` reports each target as chezmoi would write it,
# including the perm its source attributes (executable_, private_) encode, so the
# mode column is intent for the same reason the hash column is.
#
# --persistent-state IS REQUIRED AND MUST NOT BE REMOVED. Unlike `managed` and
# `cat`, `dump` opens chezmoi's persistent state, and this script runs NESTED
# inside the apply that already holds that lock, so a plain `chezmoi dump` here
# fails with "timeout obtaining persistent state lock" on every real apply
# (verified empirically; the same trap the docblock records for `status`/`verify`).
# Pointing it at a throwaway state file gives the dump its own uncontended lock.
# The persistent state holds entry and script bookkeeping, not the source state, so
# a fresh one does not change the perm or contents reported for a file target.
#
# Materialized under an explicit status check for the same reason the managed
# listing is: a dump that emitted some entries and then failed must abort, never
# leave a path silently without a mode.
if ! chezmoi "${chezmoi_args[@]}" --persistent-state "$dump_state_dir/state.boltdb" \
  dump --format=json "${paths[@]}" >"$dump_json"; then
  printf 'osquery pipeline manifest: could not dump the managed pipeline files, refusing to rewrite the manifest\n' >&2
  exit 1
fi

# jq, not a hand-rolled parse: the dump is JSON, and jq is already a hard runtime
# dependency of the alerter this manifest protects. The perm is emitted FIRST and
# the (destination-relative) name LAST, so `read -r perm rel` reads a name
# containing spaces whole, the same discipline the manifest itself uses.
#
# Materialized to a file rather than read through a process substitution, for the
# reason recorded above the managed listing: a process substitution discards the
# producer's exit status, so a jq that emitted some pairs and then failed would
# hand the loop a partial map and the tuples it could not answer for would be
# silently missing their mode.
perm_pairs="$(mktemp)"
trap 'rm -f "$managed_list" "$sorted_list" "$dump_json" "$perm_pairs" "${fresh:-}"; rm -rf "$dump_state_dir"' EXIT
if ! jq -r 'to_entries[] | "\(.value.perm) \(.key)"' "$dump_json" >"$perm_pairs"; then
  printf 'osquery pipeline manifest: could not read the intended modes out of the dump, refusing to rewrite the manifest\n' >&2
  exit 1
fi

declare -A intended_perm=()
while read -r perm rel; do
  [[ -n $perm && -n $rel ]] || continue
  intended_perm["$home/$rel"]="$perm"
done <"$perm_pairs"
if [[ ${#intended_perm[@]} -eq 0 ]]; then
  printf 'osquery pipeline manifest: the dump yielded no modes, refusing to rewrite the manifest\n' >&2
  exit 1
fi

# The OWNER column: the uid this apply is running as, which is the uid chezmoi
# writes every target file as. Validated as digits so a surprising `id` can never
# put a non-numeric token in a security-critical column.
owner_uid="$(id -u)"
if [[ ! $owner_uid =~ ^[0-9]{1,10}$ ]]; then
  printf 'osquery pipeline manifest: id -u did not report a numeric uid, refusing to rewrite the manifest\n' >&2
  exit 1
fi

fresh="$(mktemp)"

# "<sha256> <mode> <uid> <path>", path-sorted above for a byte-reproducible
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
  # chezmoi reports perm as a DECIMAL integer (493 for 0755). Validated as digits
  # and range-bound to the twelve permission bits BEFORE the octal conversion, so a
  # value that is not a mode aborts instead of being formatted into something that
  # looks like one. `10#` forces base ten on both uses: a leading zero would
  # otherwise make bash read the digits as octal.
  target_perm="${intended_perm["$target"]:-}"
  if [[ ! $target_perm =~ ^[0-9]{1,4}$ ]] || ((10#$target_perm > 4095)); then
    printf 'osquery pipeline manifest: no usable intended mode for %s, refusing to rewrite the manifest\n' "$target" >&2
    exit 1
  fi
  printf -v file_mode '%04o' "$((10#$target_perm))"
  printf '%s %s %s %s\n' "$file_hash" "$file_mode" "$owner_uid" "$target"
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
