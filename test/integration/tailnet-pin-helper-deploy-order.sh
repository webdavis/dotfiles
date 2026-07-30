#!/usr/bin/env bash
#
# The Tier-2 runner (run_onchange_after_41-macos-system-setup) sudo-executes a
# DEPLOYED helper, <dest-dir>/.local/libexec/tailnet/reconcile-hosts-pin.sh,
# instead of inlining a ~1KB `sudo sh -c` body. That rests on three properties
# of chezmoi rather than of this repo, so all three are EXECUTED here against
# the installed chezmoi instead of asserted from memory:
#
#   1. every managed FILE is written before ANY after-phase script runs;
#   2. CHEZMOI_DEST_DIR names the directory those files were written into, even
#      when it is not $HOME;
#   3. the deployed copy is byte-identical to the source file, so the sha256
#      the runner embeds (via `include ... | sha256sum`) is the sha256 the
#      runner's own gate computes from the deployed helper.
#
# Property 1 is what makes the extraction work at all; run_after_05 already
# depends on it too, so a regression would break more than the pins. Property 2
# is why the runner derives the helper path from CHEZMOI_DEST_DIR: under
# `chezmoi apply --destination X` with an unchanged $HOME, a $HOME-derived path
# points at a directory nothing was deployed into and the runner's guard would
# abort every such apply. Property 3 is what lets the embedded hash be a GATE on
# what runs as root rather than only a re-trigger token.
#
# HOME and the destination are deliberately DIFFERENT directories here, and a
# $HOME-derived control probe must NOT find the helper. Setting them to the same
# value (which this test used to do) cannot tell the two apart, so it would pass
# whether the runner read CHEZMOI_DEST_DIR or $HOME.
#
# A before-phase probe is the ordering control: it must NOT see the file, so a
# pass cannot come from a destination that was already populated.
set -euo pipefail

# Scrubbed at SCRIPT scope, before any chezmoi or git call: git exports GIT_DIR
# to every hook it runs and this suite can run from one.
unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RECONCILER_SOURCE_PATH="dot_local/libexec/tailnet/executable_reconcile-hosts-pin.sh"
RECONCILER="$REPO_ROOT/$RECONCILER_SOURCE_PATH"
RECONCILER_TARGET_SUFFIX=".local/libexec/tailnet/reconcile-hosts-pin.sh"

fail() {
  printf 'tailnet-pin-helper-deploy-order: FAIL -- %s\n' "$*" >&2
  exit 1
}

if ! command -v chezmoi >/dev/null 2>&1; then
  printf 'SKIP: chezmoi not on PATH; cannot exercise the apply ordering\n'
  exit 0
fi
[[ -f $RECONCILER ]] || fail "missing reconciler source: $RECONCILER"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

src="$work/src"
destination="$work/dest"
# NOT the destination: the runner must find the helper through CHEZMOI_DEST_DIR.
elsewhere_home="$work/home"
mkdir -p "$src/.chezmoiscripts" "$src/$(dirname "$RECONCILER_SOURCE_PATH")" \
  "$destination" "$elsewhere_home"
cp "$RECONCILER" "$src/$RECONCILER_SOURCE_PATH"

# The probe stands in for the runner: same phase, same order prefix, and the
# same two path derivations so the difference between them is observable. It
# cannot be the runner itself, which needs sudo and would edit /etc/hosts.
cat >"$src/.chezmoiscripts/run_after_41-probe.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
helper="\${CHEZMOI_DEST_DIR:?chezmoi exports this to every script it runs}/$RECONCILER_TARGET_SUFFIX"
if [[ -x \$helper ]]; then
  echo "AFTER-PHASE: helper present and executable"
  echo "AFTER-PHASE: deployed sha256 \$(shasum -a 256 "\$helper" | cut -d ' ' -f 1)"
else
  echo "AFTER-PHASE: helper ABSENT or not executable"
fi
home_helper="\$HOME/$RECONCILER_TARGET_SUFFIX"
if [[ -e \$home_helper ]]; then
  echo "AFTER-PHASE: HOME-derived path also resolves"
else
  echo "AFTER-PHASE: HOME-derived path does not resolve"
fi
EOF

# The control. Nothing may be deployed yet when a before-phase script runs, so
# an "absent" here is what proves the after-phase result is about ORDERING and
# not about a destination that already held the file.
cat >"$src/.chezmoiscripts/run_before_09-probe.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
helper="\${CHEZMOI_DEST_DIR:?chezmoi exports this to every script it runs}/$RECONCILER_TARGET_SUFFIX"
if [[ -x \$helper ]]; then
  echo "BEFORE-PHASE: helper present"
else
  echo "BEFORE-PHASE: helper absent"
fi
EOF
chmod +x "$src/.chezmoiscripts/run_after_41-probe.sh" "$src/.chezmoiscripts/run_before_09-probe.sh"

apply_output="$work/apply.out"
HOME="$elsewhere_home" chezmoi --source "$src" --destination "$destination" \
  apply --force --no-tty >"$apply_output" 2>&1 ||
  fail "chezmoi apply failed: $(cat "$apply_output")"

grep -qxF 'BEFORE-PHASE: helper absent' "$apply_output" ||
  fail "the before-phase control saw the helper already deployed, so this run proves nothing about ordering: $(cat "$apply_output")"
grep -qxF 'AFTER-PHASE: helper present and executable' "$apply_output" ||
  fail "the helper was NOT deployed before the after-phase slot the pin runner occupies; the runner would sudo-execute a missing file: $(cat "$apply_output")"

# The Q2 control: with HOME pointed somewhere else, a $HOME-derived path must
# come up empty. If this ever resolves, the fixture stopped separating the two
# and the CHEZMOI_DEST_DIR assertion above proves nothing.
grep -qxF 'AFTER-PHASE: HOME-derived path does not resolve' "$apply_output" ||
  fail "the fixture's HOME and destination are not actually distinguishable, so this run cannot tell CHEZMOI_DEST_DIR from \$HOME: $(cat "$apply_output")"

[[ -x "$destination/$RECONCILER_TARGET_SUFFIX" ]] ||
  fail "the reconciler was not deployed executable to $RECONCILER_TARGET_SUFFIX"

# The deployed copy must hash to exactly what `include ... | sha256sum` computes
# over the source, or the runner's own gate would refuse every apply.
source_sha256="$(shasum -a 256 "$RECONCILER" | cut -d ' ' -f 1)"
deployed_sha256="$(shasum -a 256 "$destination/$RECONCILER_TARGET_SUFFIX" | cut -d ' ' -f 1)"
[[ $source_sha256 == "$deployed_sha256" ]] ||
  fail "chezmoi did not deploy the reconciler byte-identically (source $source_sha256, deployed $deployed_sha256), so the sha256 the runner embeds could never match the helper it gates on"
grep -qxF "AFTER-PHASE: deployed sha256 $source_sha256" "$apply_output" ||
  fail "the after-phase probe computed a different sha256 for the deployed helper than this test did: $(cat "$apply_output")"

echo "tailnet-pin-helper-deploy-order: OK (chezmoi writes every managed file before any after-phase script runs, CHEZMOI_DEST_DIR names the destination even when it is not the home directory, and the deployed reconciler hashes to its source)"
