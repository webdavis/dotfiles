#!/usr/bin/env bash
#
# The Tier-2 runner (run_onchange_after_41-macos-system-setup) sudo-executes a
# DEPLOYED helper, ~/.local/libexec/tailnet/reconcile-hosts-pin.sh, instead of
# inlining a ~1KB `sudo sh -c` body. That only works because of one property of
# chezmoi's apply order:
#
#   every managed FILE is written before ANY after-phase script runs.
#
# The whole extraction rests on it, and it is a property of chezmoi rather than
# of this repo, so it is pinned here instead of assumed. run_after_05 already
# depends on the same thing (it reads the freshly deployed pipeline files), so a
# regression would break more than the pins.
#
# What this pins, precisely: a minimal chezmoi source tree carrying the REAL
# reconciler at its real source path plus a probe script in the same after-phase
# slot the runner occupies (41) is applied into an empty destination. The probe
# must find the reconciler present AND executable, on a FIRST apply, which is
# the case with the least deployed state and therefore the one most likely to
# fail. A before-phase probe is included as the control: it must NOT see the
# file, so a pass here cannot come from a destination that was already
# populated.
set -euo pipefail

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
destination="$work/home"
mkdir -p "$src/.chezmoiscripts" "$src/$(dirname "$RECONCILER_SOURCE_PATH")" "$destination"
cp "$RECONCILER" "$src/$RECONCILER_SOURCE_PATH"

# The probe stands in for the runner: same phase, same order prefix. It cannot
# be the runner itself, which needs sudo and would edit /etc/hosts.
cat >"$src/.chezmoiscripts/run_after_41-probe.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
helper="\$HOME/$RECONCILER_TARGET_SUFFIX"
if [[ -x \$helper ]]; then
  echo "AFTER-PHASE: helper present and executable"
else
  echo "AFTER-PHASE: helper ABSENT or not executable"
fi
EOF

# The control. Nothing may be deployed yet when a before-phase script runs, so
# an "absent" here is what proves the after-phase result is about ORDERING and
# not about a destination that already held the file.
cat >"$src/.chezmoiscripts/run_before_09-probe.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
helper="\$HOME/$RECONCILER_TARGET_SUFFIX"
if [[ -x \$helper ]]; then
  echo "BEFORE-PHASE: helper present"
else
  echo "BEFORE-PHASE: helper absent"
fi
EOF
chmod +x "$src/.chezmoiscripts/run_after_41-probe.sh" "$src/.chezmoiscripts/run_before_09-probe.sh"

apply_output="$work/apply.out"
HOME="$destination" chezmoi --source "$src" --destination "$destination" \
  apply --force --no-tty >"$apply_output" 2>&1 ||
  fail "chezmoi apply failed: $(cat "$apply_output")"

grep -qxF 'BEFORE-PHASE: helper absent' "$apply_output" ||
  fail "the before-phase control saw the helper already deployed, so this run proves nothing about ordering: $(cat "$apply_output")"
grep -qxF 'AFTER-PHASE: helper present and executable' "$apply_output" ||
  fail "the helper was NOT deployed before the after-phase slot the pin runner occupies; the runner would sudo-execute a missing file: $(cat "$apply_output")"

[[ -x "$destination/$RECONCILER_TARGET_SUFFIX" ]] ||
  fail "the reconciler was not deployed executable to $RECONCILER_TARGET_SUFFIX"

echo "tailnet-pin-helper-deploy-order: OK (chezmoi writes every managed file before any after-phase script runs, on a first apply)"
