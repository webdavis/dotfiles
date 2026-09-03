#!/usr/bin/env bash
#
# run_after_05-osquery-known-good-manifests.sh
# Refresh the root-owned known-good manifests: the recorded sha256, mode and owner
# of every file osquery's file-integrity watches judge. osqueryd (root) watches
# those files; the alerter PAGES a file_events change whose (path, hash, mode, uid)
# tuple is not in the governing manifest (tamper) and stays silent when it matches
# (a legitimate apply).
#
# Format, one whitespace-separated tuple per line with the PATH LAST so a path
# containing spaces is still read whole by `read -r hash mode uid path`:
#
#   <sha256> <mode> <uid> <path>
#
# mode is exactly four octal digits (0755); uid is decimal.
#
# TWO manifests, deliberately separate, because they cover two different trust
# domains with two different default-deny rules:
#
#   pipeline-known-good.sha256    the osquery alerting pipeline's OWN scripts and
#                                 plists (~/.local/libexec/osquery and our own
#                                 LaunchAgents). This is the monitor's body. The
#                                 whole directory is tracked, so a file PLANTED
#                                 there is unmanaged, unmanifested, and pages
#                                 forever, which is what we want of the monitor.
#
#   managed-bin-known-good.sha256 the chezmoi-MANAGED scripts under ~/.local/bin.
#                                 These are not pipeline files, but most of them
#                                 run UNATTENDED (LaunchAgents and shell hooks fire
#                                 update-skills.sh, uu and
#                                 the claude-* hooks with nobody watching), so a
#                                 tamper there executes on a timer. Only the
#                                 MANIFESTED paths are tracked there, because the
#                                 same directory holds third-party shims (herdr,
#                                 mise, bob, hermes, yt-dlp, and symlinks into pipx
#                                 and uv tool dirs) that update themselves; those
#                                 are not chezmoi's to vouch for and must stay
#                                 silent rather than page on every self-update.
#
# They are separate FILES rather than one list because the osquery pipeline
# manifest's single responsibility is the pipeline's own integrity (operator
# ruling, slice 15), and because a corrupt or stale bin manifest must not be able
# to falsify the monitor's judgment of itself. The generation logic is shared here
# rather than copied into a second runner: the partial-listing guard, the empty
# guard, the intent hashing and the privileged install are security-critical, and a
# second copy would be a second thing to drift.
#
# ARM ORDER IS LOAD-BEARING. The pipeline arm runs FIRST and installs before the
# bin arm starts, so a failure in the bin arm cannot leave the pipeline manifest
# stale. errexit then aborts the runner, which fails the apply loudly.
#
# NOT a template. This runner refreshes the manifests, so it must run on every
# apply that can change a manifested file. Keeping it plain removes any dependence
# on which entry types a given apply processes.
# Darwin is therefore gated at runtime, not with a Go-template guard.
#
# Runs in the EARLIEST after-phase slot (05): all target files are written before
# any after-script runs, so nothing is lost by going first, and the WatchPaths
# alerter judges a finding exactly once (its cursor advances), so the manifests
# must be current before the alerter can look at the change it just caused.
#
# The manifests are derived from chezmoi's INTENT, never from the tree they
# protect:
#   - the file SET comes from `chezmoi managed` (the source state), so a file an
#     attacker plants in a covered directory is not managed, never enters a
#     manifest, and is therefore either paged forever (the pipeline home) or left
#     to the untracked-neighbor path (~/.local/bin) - never blessed;
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
#     chown-ed a covered file cannot influence it. (If an operator ever applied as
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
# Each manifest is written root-owned 0644 so a process at the user's privilege
# level cannot rewrite it, and the consumer refuses a manifest that is not (see
# _pipeline_manifest_is_trustworthy in pipeline-verdict.sh). That raises the bar and
# stops an unprivileged process from whitelisting a file it just tampered, but the
# operator account here has passwordless sudo, so a process running AS the operator
# can escalate with no prompt and rewrite a manifest at will. Every unattended
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

# Keep these defaults in sync with PIPELINE_MANIFEST and MANAGED_BIN_MANIFEST in
# dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh (the consumer).
# Tests pin the literals equal, because the producer and the consumers of a
# security-critical file must not agree only by copy-paste.
pipeline_manifest="${OSQUERY_PIPELINE_MANIFEST:-/var/osquery/pipeline-known-good.sha256}"
managed_bin_manifest="${OSQUERY_MANAGED_BIN_MANIFEST:-/var/osquery/managed-bin-known-good.sha256}"

home="${CHEZMOI_HOME_DIR:-$HOME}"

# chezmoi sets CHEZMOI_SOURCE_DIR for the scripts it runs; pin the nested calls to
# that same source so this cannot read a different checkout than the apply in
# progress. Absent (a direct invocation), fall back to the configured default.
chezmoi_args=()
[[ -n ${CHEZMOI_SOURCE_DIR:-} ]] && chezmoi_args+=(--source "$CHEZMOI_SOURCE_DIR")

# ONE managed listing and ONE dump serve both arms: they are the expensive calls,
# and running them twice would also open a window in which the two manifests were
# built from different source states.
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
perm_pairs="$(mktemp)"
fresh="$(mktemp)"
# A THROWAWAY persistent state for the nested `chezmoi dump` below; see the comment
# at that call for why it cannot share the apply's.
dump_state_dir="$(mktemp -d)"
trap 'rm -f "$managed_list" "$sorted_list" "$dump_json" "$perm_pairs" "$fresh"; rm -rf "$dump_state_dir"' EXIT

if ! chezmoi "${chezmoi_args[@]}" managed --path-style=absolute --include=files >"$managed_list"; then
  printf 'osquery known-good manifests: could not list managed files, refusing to rewrite any manifest\n' >&2
  exit 1
fi
if ! LC_ALL=C sort "$managed_list" >"$sorted_list"; then
  printf 'osquery known-good manifests: could not sort the managed listing, refusing to rewrite any manifest\n' >&2
  exit 1
fi

# --- the arm file sets, from managed intent ----------------------------------
# These filters are one leg of the three-way agreement: the others are the
# osquery.conf WATCH set and _pipeline_is_tracked in pipeline-verdict.sh, and an
# integration test drives all three against one fixture so they cannot drift apart
# silently. The WATCH leg is the loosest of the three: osquery watches directories,
# so it reports neighbors of the covered files too, and _pipeline_is_tracked is what
# classifies those as untracked. What must match exactly is this filter and the
# tracked set, or a watched-and-tracked file the manifest can never contain pages
# forever and a manifested file nothing watches is never checked.
#
# The bin arm is NON-RECURSIVE on purpose: ~/.local/bin holds tools, and a managed
# file in a SUBDIRECTORY of it would be a different kind of thing that nothing has
# asked to cover. There are none today.
#
# The libexec arm IS recursive, because nesting is that tree's design: internal
# tools are grouped by owner (pns/, unattended-upgrades/, macos-defaults/), a tool
# with private helpers gets its own directory, and shared code sits in helpers/.
# A `case` pattern's `*` matches slashes, so one arm covers every depth. It sits
# AFTER the osquery arm so the pipeline keeps its own manifest.
pipeline_paths=()
managed_bin_paths=()
while IFS= read -r target; do
  case "$target" in
    "$home"/.local/libexec/osquery/*) pipeline_paths+=("$target") ;;
    "$home"/Library/LaunchAgents/com.webdavis.osquery-*.plist) pipeline_paths+=("$target") ;;
    # The page-launchd allowlist joins the PIPELINE arm, named as ONE EXACT FILE
    # rather than by its directory. It decides whether an unknown user LaunchAgent
    # pages, so it is infrastructure the alerter judges, and the verdict routes any
    # path outside ~/.local/bin to the pipeline manifest, so this is the only arm
    # that can vouch for it. The exact path matters: ~/.config/osquery also holds
    # webhook-secret, the daemon config, packs/ and the allowlist writer's lock
    # file, and a directory pattern would bind a secret's digest into this
    # world-readable root-owned manifest and sign a lock file that is recreated on
    # every curation run.
    "$home"/.config/osquery/page-launchd-allowlist.txt) pipeline_paths+=("$target") ;;
    "$home"/.local/bin/*/*) : ;; # a managed file in a subdirectory: not covered
    "$home"/.local/bin/*) managed_bin_paths+=("$target") ;;
    "$home"/.local/libexec/*) managed_bin_paths+=("$target") ;;
  esac
done <"$sorted_list"

# Resolve BOTH sets before anything else consults them. `chezmoi dump` with NO
# target arguments dumps the ENTIRE target state, which would render every managed
# template - including the ones that call keepassxc - from an unattended apply. An
# empty path list must therefore abort here, before the dump, not only at the
# empty-manifest guard further down.
if [[ ${#pipeline_paths[@]} -eq 0 ]]; then
  printf 'osquery known-good manifests: no managed pipeline files resolved, refusing to rewrite any manifest\n' >&2
  exit 1
fi
if [[ ${#managed_bin_paths[@]} -eq 0 ]]; then
  printf 'osquery known-good manifests: no managed ~/.local/bin files resolved, refusing to rewrite any manifest\n' >&2
  exit 1
fi

# The INTENDED mode of every covered file, in ONE dump of the same source state.
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
#
# The throwaway state is SEEDED with the config template's hash first. A fresh
# state has no configState record, and chezmoi compares that nil against the
# real template's hash on every apply-family command, so the un-seeded dump
# printed "config file template has changed, run chezmoi init" on EVERY apply,
# a warning no `chezmoi init` could ever clear because this state file is
# recreated each run. Diagnosed 2026-08-05 from a --debug apply (the warning
# surfaced at this script's start) and chezmoi v2.72.0 source (applyArgs in
# internal/cmd/config.go); proven both ways: unseeded dump warns, seeded is
# silent. Best-effort on purpose: a failed seed only means the cosmetic warning
# returns, never a failed manifest rewrite.
config_template="${CHEZMOI_SOURCE_DIR:-$HOME/workspaces/Ivy/webdavis/dotfiles}/.chezmoi.toml.tmpl"
if [[ -r $config_template ]]; then
  config_template_sha256="$(shasum -a 256 "$config_template" | awk '{print $1}')"
  chezmoi "${chezmoi_args[@]}" --persistent-state "$dump_state_dir/state.boltdb" \
    state set --bucket=configState --key=configState \
    --value="{\"configTemplateContentsSHA256\":\"$config_template_sha256\"}" 2>/dev/null || true
fi
if ! chezmoi "${chezmoi_args[@]}" --persistent-state "$dump_state_dir/state.boltdb" \
  dump --format=json "${pipeline_paths[@]}" "${managed_bin_paths[@]}" >"$dump_json"; then
  printf 'osquery known-good manifests: could not dump the managed files, refusing to rewrite any manifest\n' >&2
  exit 1
fi

# jq, not a hand-rolled parse: the dump is JSON, and jq is already a hard runtime
# dependency of the alerter these manifests protect. The perm is emitted FIRST and
# the (destination-relative) name LAST, so `read -r perm rel` reads a name
# containing spaces whole, the same discipline the manifests themselves use.
#
# Materialized to a file rather than read through a process substitution, for the
# reason recorded above the managed listing: a process substitution discards the
# producer's exit status, so a jq that emitted some pairs and then failed would
# hand the loop a partial map and the tuples it could not answer for would be
# silently missing their mode.
if ! jq -r 'to_entries[] | "\(.value.perm) \(.key)"' "$dump_json" >"$perm_pairs"; then
  printf 'osquery known-good manifests: could not read the intended modes out of the dump, refusing to rewrite any manifest\n' >&2
  exit 1
fi

declare -A intended_perm=()
while read -r perm rel; do
  [[ -n $perm && -n $rel ]] || continue
  intended_perm["$home/$rel"]="$perm"
done <"$perm_pairs"
if [[ ${#intended_perm[@]} -eq 0 ]]; then
  printf 'osquery known-good manifests: the dump yielded no modes, refusing to rewrite any manifest\n' >&2
  exit 1
fi

# The OWNER column: the uid this apply is running as, which is the uid chezmoi
# writes every target file as. Validated as digits so a surprising `id` can never
# put a non-numeric token in a security-critical column.
owner_uid="$(id -u)"
if [[ ! $owner_uid =~ ^[0-9]{1,10}$ ]]; then
  printf 'osquery known-good manifests: id -u did not report a numeric uid, refusing to rewrite any manifest\n' >&2
  exit 1
fi

# refresh_manifest <label> <manifest-path> <paths-array-name>
#
# Build this arm's manifest from the shared intent and install it when it differs
# from what is deployed. Aborts the runner on any refusal, leaving the previously
# installed manifest in force.
#
# errexit is live inside here because this is called as a PLAIN command. Do not
# "improve" a call site into `refresh_manifest ... || something`: bash suppresses
# errexit for the whole body of a function that is part of a `||` list, and the
# explicit checks below would then be the only protection left.
#
# The path array arrives by NAME through a nameref. Every local here is prefixed
# with the function name so a caller's array can never collide with one and make
# the nameref alias this function's own variable instead (the failure mode
# test/test-system/nameref-guards.sh exists for), and the two call sites pass
# names that carry no such prefix.
refresh_manifest() {
  local refresh_manifest_label="$1" refresh_manifest_dest="$2"
  local -n refresh_manifest_paths="$3"
  local refresh_manifest_target refresh_manifest_hash refresh_manifest_perm
  local refresh_manifest_mode refresh_manifest_dir

  # "<sha256> <mode> <uid> <path>", path-sorted above for a byte-reproducible
  # manifest, and the path LAST so a reader's final field takes the remainder whole
  # even for a path holding spaces.
  #
  # The hash is captured into a VARIABLE first, deliberately: a command
  # substitution used directly as a printf argument would discard a failing
  # `chezmoi cat` (printf itself still succeeds) and emit a tuple with an empty
  # hash, quietly corrupting the manifest. Its status is then checked EXPLICITLY
  # rather than left to errexit, because under pipefail a failed `chezmoi cat`
  # still lets shasum print the hash of an EMPTY stream, which is a well-formed
  # 64-hex string that no emptiness check would catch.
  : >"$fresh"
  for refresh_manifest_target in "${refresh_manifest_paths[@]}"; do
    if ! refresh_manifest_hash="$(chezmoi "${chezmoi_args[@]}" cat "$refresh_manifest_target" | shasum -a 256 | awk '{print $1}')"; then
      printf 'osquery known-good manifests: could not hash %s, refusing to rewrite the %s manifest\n' "$refresh_manifest_target" "$refresh_manifest_label" >&2
      return 1
    fi
    if [[ ! $refresh_manifest_hash =~ ^[0-9a-f]{64}$ ]]; then
      printf 'osquery known-good manifests: implausible hash for %s, refusing to rewrite the %s manifest\n' "$refresh_manifest_target" "$refresh_manifest_label" >&2
      return 1
    fi
    # chezmoi reports perm as a DECIMAL integer (493 for 0755). Validated as digits
    # and range-bound to the twelve permission bits BEFORE the octal conversion, so
    # a value that is not a mode aborts instead of being formatted into something
    # that looks like one. `10#` forces base ten on both uses: a leading zero would
    # otherwise make bash read the digits as octal.
    refresh_manifest_perm="${intended_perm["$refresh_manifest_target"]:-}"
    if [[ ! $refresh_manifest_perm =~ ^[0-9]{1,4}$ ]] || ((10#$refresh_manifest_perm > 4095)); then
      printf 'osquery known-good manifests: no usable intended mode for %s, refusing to rewrite the %s manifest\n' "$refresh_manifest_target" "$refresh_manifest_label" >&2
      return 1
    fi
    printf -v refresh_manifest_mode '%04o' "$((10#$refresh_manifest_perm))"
    printf '%s %s %s %s\n' "$refresh_manifest_hash" "$refresh_manifest_mode" "$owner_uid" "$refresh_manifest_target" >>"$fresh"
  done

  # Never let an empty render overwrite a good manifest.
  if [[ ! -s $fresh ]]; then
    printf 'osquery known-good manifests: refusing to install an EMPTY %s manifest (no managed files resolved)\n' "$refresh_manifest_label" >&2
    return 1
  fi

  # The deployed manifest is root-owned 0644 (world-readable), so the compare needs
  # no privilege; only a real content change warrants the sudo write. A missing
  # manifest (fresh machine) compares unequal and installs.
  if cmp -s "$fresh" "$refresh_manifest_dest"; then
    return 0
  fi

  # Fresh host: /var/osquery is created by the osquery converge tool, which
  # run_after_50 calls - AFTER this runner, by design, since the alerter judges a
  # file change exactly once and the manifests have to be current before it looks.
  # So on a first apply this arrives before the directory exists. Create the
  # manifest's parent ourselves rather than fail the apply and leave the host with
  # no manifest at all. Only when it is actually missing, so a normal apply
  # performs no extra privileged call, and idempotent either way.
  refresh_manifest_dir="$(dirname "$refresh_manifest_dest")"
  if [[ ! -d $refresh_manifest_dir ]]; then
    sudo install -d -o root -g wheel -m 0755 "$refresh_manifest_dir"
  fi

  sudo install -o root -g wheel -m 0644 "$fresh" "$refresh_manifest_dest"
}

refresh_manifest 'osquery pipeline' "$pipeline_manifest" pipeline_paths
refresh_manifest 'managed bin' "$managed_bin_manifest" managed_bin_paths
