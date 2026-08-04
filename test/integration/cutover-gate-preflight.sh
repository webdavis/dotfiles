#!/usr/bin/env bash
# cutover-gate-preflight.sh: gate 1 of the D1 cutover runner.
#
# Gate 1 is preflight: a fully-visible-clean tree, the Hermes backup, the pins
# taken LAST from freshly-fetched remote-tracking refs, the expected-delta
# ledger built from the recorded Phase A base, and the retirement manifest the
# operator must approve before any service-affecting apply.
#
# Everything runs against a SANDBOX $HOME holding a real git repository at the
# runner's absolute repo handle ($HOME/workspaces/Ivy/webdavis/dotfiles) with a
# local bare `origin`. `launchctl` is a PATH stub whose loaded set is seeded per
# case; no live launchd domain, no live checkout, and no chezmoi apply is ever
# touched.
#
# Cases:
#   A. happy path -> clean-tree pass, Hermes backup, pins from origin (not the
#      lagging local refs), delta ledger classified, retirement manifest
#      proposed, operator checkpoint (exit 10), gate1.done NOT written
#   B. an unclassified manifest hunk is `missing` and BLOCKS (exit 1)
#   C. a dirty tree refuses BEFORE any pin is recorded (pin-last ordering)
#   D. gitignored graphify-out/ residue refuses even with clean porcelain
#   E. retirement manifest: a deleted historical label found loaded becomes a
#      candidate; desired, preserve-list, and out-of-universe labels do not
#   F. approval: refuses with no proposal; approves a re-derived identical
#      manifest; refuses when the manifest changed since the operator read it
#   G. a failed fetch refuses instead of trusting stale remote-tracking refs
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR
unset XDG_CONFIG_HOME
export GIT_CONFIG_NOSYSTEM=1

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$REPO_ROOT/scripts/cutover-gate.sh"
source "$REPO_ROOT/test/integration/helpers/cutover-sandbox.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0
report() {
  local status="$1" msg="$2"
  if [[ $status == ok ]]; then
    printf '  ok   %s\n' "$msg"
  else
    printf '  FAIL %s\n' "$msg"
    failures=$((failures + 1))
  fi
}

[[ -x $RUNNER ]] || {
  printf 'FAIL: missing or non-executable runner: %s\n' "$RUNNER" >&2
  exit 1
}

uid="$(id -u)"

# ── stubs and sandbox ──────────────────────────────────────────────────────
stub_dir="$work/stubs"
export LOADED_GUI="$work/loaded-gui"
export LOADED_SYSTEM="$work/loaded-system"
export LAUNCHCTL_LOG="$work/launchctl.log"
export PRINT_DETAIL_DIR="$work/print-detail"
export CMD_LOG="$work/cmd.log"
export CHEZMOI_DATA_FILE="$work/render.json"
mkdir -p "$stub_dir" "$PRINT_DETAIL_DIR"
: >"$LOADED_GUI"
: >"$LOADED_SYSTEM"
: >"$LAUNCHCTL_LOG"
: >"$CMD_LOG"
cutover_make_launchctl_stub "$stub_dir"
cutover_make_command_stubs "$stub_dir"

command -v yq >/dev/null 2>&1 || {
  printf 'SKIP: yq is not on PATH; the render-versus-source check cannot be exercised\n'
  exit 0
}

# run_gate <home> <args...> : runs the runner in the sandbox with the launchctl
# stub on PATH. Captures stdout/stderr to files and the status in RC.
#
# Checklist item 6: every case runs the runner from a NEUTRAL cwd outside any
# git repository, so nothing it does can be resolving through the caller's
# directory instead of the absolute repo handle.
out_file="$work/stdout"
err_file="$work/stderr"
neutral="$work/neutral"
mkdir -p "$neutral"
run_gate() {
  local home="$1"
  shift
  RC=0
  (
    cd "$neutral" || exit 1
    HOME="$home" PATH="$stub_dir:$PATH" CUTOVER_PHASE_A_BASE="$SANDBOX_BASE" \
      "$RUNNER" "$@"
  ) >"$out_file" 2>"$err_file" || RC=$?
}

ledger_of() { printf '%s/.local/state/cutover' "$1"; }

seed_loaded() {
  : >"$LOADED_GUI"
  : >"$LOADED_SYSTEM"
  local arg
  for arg in "$@"; do
    case "$arg" in
      system:*) printf '%s\n' "${arg#system:}" >>"$LOADED_SYSTEM" ;;
      *) printf '%s\n' "$arg" >>"$LOADED_GUI" ;;
    esac
  done
}

printf 'cutover-gate-preflight cases:\n'

# ── Checklist item 6: the neutral cwd is real ──────────────────────────────
if git -C "$neutral" rev-parse --show-toplevel >/dev/null 2>&1; then
  report bad "item 6: the neutral cwd is inside a git repository, so cwd independence is not proven"
else
  report ok "item 6: every case runs the runner from a cwd outside any git repository"
fi

# ── Case A: happy path ─────────────────────────────────────────────────────
hA="$work/homeA"
cutover_build_sandbox "$hA"
cutover_write_classification "$hA"
seed_loaded com.webdavis.atuin-daemon com.webdavis.osquery-uptime-watchdog
ledA="$(ledger_of "$hA")"
# origin/main advances past the local ref: the pins must follow the REMOTE.
side="$work/side"
cutover_git clone --quiet "$hA/origin.git" "$side"
printf 'late\n' >"$side/late.txt"
cutover_git -C "$side" add -A
cutover_git -C "$side" commit --quiet -m 'a late commit on origin/main'
cutover_git -C "$side" push --quiet origin main
origin_main="$(git -C "$side" rev-parse HEAD)"
local_main="$(git -C "$hA/workspaces/Ivy/webdavis/dotfiles" rev-parse main)"

run_gate "$hA" 1
if [[ $RC -eq 10 ]]; then
  report ok "A: stops at the operator checkpoint (exit 10)"
else
  report bad "A: expected the checkpoint exit 10, got $RC (err: $(cat "$err_file"))"
fi
if [[ ! -f "$ledA/gate1.done" ]]; then
  report ok "A: gate 1 is NOT marked passed before approval"
else
  report bad "A: gate1.done written before the operator approved"
fi
main_pin="$(sed -n 's/^MAIN_SHA=//p' "$ledA/pins.env" 2>/dev/null || true)"
int_pin="$(sed -n 's/^INT_SHA=//p' "$ledA/pins.env" 2>/dev/null || true)"
if [[ $main_pin == "$origin_main" ]]; then
  report ok "A: MAIN_SHA pinned from origin/main, not the lagging local ref"
else
  report bad "A: MAIN_SHA=$main_pin, expected origin/main=$origin_main (local was $local_main)"
fi
if [[ $int_pin =~ ^[0-9a-f]{40}$ ]]; then
  report ok "A: INT_SHA recorded as a full 40-hex SHA"
else
  report bad "A: INT_SHA is not a full 40-hex SHA: '$int_pin'"
fi
if [[ -s "$ledA/expected-delta.diff" ]]; then
  report ok "A: expected-delta manifest regenerated from the recorded base"
else
  report bad "A: no expected-delta manifest"
fi
if grep -q '^landed-unchanged	a.txt' "$ledA/delta-ledger.tsv" 2>/dev/null; then
  report ok "A: an identical hunk classifies itself as landed-unchanged"
else
  report bad "A: a.txt was not auto-classified landed-unchanged (ledger: $(cat "$ledA/delta-ledger.tsv" 2>/dev/null))"
fi
if grep -q '^intentionally-improved	b.txt' "$ledA/delta-ledger.tsv" 2>/dev/null; then
  report ok "A: the operator's intentionally-improved classification is recorded"
else
  report bad "A: b.txt classification missing from the ledger"
fi
if grep -q '^deliberately-omitted-with-reason	c.txt' "$ledA/delta-ledger.tsv" 2>/dev/null; then
  report ok "A: the operator's deliberate omission is recorded"
else
  report bad "A: c.txt classification missing from the ledger"
fi
backup_glob=("$hA"/workspaces/backups/*.hermes*.backup*)
if [[ -e ${backup_glob[0]} ]]; then
  report ok "A: Hermes profile state backed up per the backup convention"
else
  report bad "A: no Hermes backup under $hA/workspaces/backups"
fi
if [[ -f "${backup_glob[0]}/profiles/concerned/config.yaml" ]]; then
  report ok "A: the backup carries the per-profile config"
else
  report bad "A: the Hermes backup does not contain the profile config"
fi
# desired-state triples
if grep -q "^com.webdavis.atuin-daemon	gui/$uid	persistent" "$ledA/desired-services.tsv" 2>/dev/null; then
  report ok "A: KeepAlive=true renders a persistent (label, domain, predicate)"
else
  report bad "A: atuin-daemon triple wrong (got: $(cat "$ledA/desired-services.tsv" 2>/dev/null))"
fi
if grep -q "^com.webdavis.osquery-uptime-watchdog	gui/$uid	scheduled" "$ledA/desired-services.tsv" 2>/dev/null; then
  report ok "A: StartInterval renders a scheduled predicate (RunAtLoad is not persistence)"
else
  report bad "A: uptime-watchdog triple wrong"
fi
if grep -q '^systems.nixos.nix-installer.nix-hook	system	conditional' "$ledA/desired-services.tsv" 2>/dev/null; then
  report ok "A: the script-rendered out-of-prefix system label is in the desired set"
else
  report bad "A: the script-rendered nix-hook label is missing from the desired set"
fi
# per-domain enumeration, never a bare list
if grep -qx "print gui/$uid" "$LAUNCHCTL_LOG" && grep -qx 'print system' "$LAUNCHCTL_LOG"; then
  report ok "A: enumerates the user domain AND the system domain, one at a time"
else
  report bad "A: missing a per-domain enumeration (log: $(tr '\n' '|' <"$LAUNCHCTL_LOG"))"
fi

# ── Case B: an unclassified hunk is `missing` and blocks ────────────────────
hB="$work/homeB"
cutover_build_sandbox "$hB"
cutover_write_classification "$hB"
ledB="$(ledger_of "$hB")"
repoB="$hB/workspaces/Ivy/webdavis/dotfiles"
cutover_git -C "$repoB" checkout --quiet integration/modernization
printf 'never landed\n' >"$repoB/d.txt"
cutover_git -C "$repoB" add -A
cutover_git -C "$repoB" commit --quiet -m 'a hunk that never landed'
cutover_git -C "$repoB" push --quiet origin integration/modernization
cutover_git -C "$repoB" checkout --quiet main
seed_loaded
run_gate "$hB" 1
if [[ $RC -eq 1 ]]; then
  report ok "B: a missing hunk blocks cutover (exit 1)"
else
  report bad "B: expected a refusal, got $RC"
fi
if grep -q 'd.txt' "$err_file"; then
  report ok "B: the refusal names the unclassified path"
else
  report bad "B: the refusal does not name d.txt (err: $(cat "$err_file"))"
fi
if [[ ! -f "$ledB/gate1.done" ]]; then
  report ok "B: gate 1 not marked passed"
else
  report bad "B: gate1.done written despite a missing hunk"
fi

# ── Case C: a dirty tree refuses BEFORE pinning ────────────────────────────
hC="$work/homeC"
cutover_build_sandbox "$hC"
cutover_write_classification "$hC"
ledC="$(ledger_of "$hC")"
printf 'untracked\n' >"$hC/workspaces/Ivy/webdavis/dotfiles/stray.txt"
seed_loaded
run_gate "$hC" 1
if [[ $RC -eq 1 ]]; then
  report ok "C: a dirty tree refuses"
else
  report bad "C: expected a refusal on a dirty tree, got $RC"
fi
if grep -q 'stray.txt' "$err_file"; then
  report ok "C: the refusal lists the offending entry"
else
  report bad "C: the refusal does not list stray.txt (err: $(cat "$err_file"))"
fi
if [[ ! -f "$ledC/pins.env" ]]; then
  report ok "C: pin-last ordering, nothing is pinned before the tree is clean"
else
  report bad "C: pins were recorded despite a dirty tree"
fi

# ── Case D: gitignored graphify-out residue refuses ────────────────────────
hD="$work/homeD"
cutover_build_sandbox "$hD"
cutover_write_classification "$hD"
mkdir -p "$hD/workspaces/Ivy/webdavis/dotfiles/graphify-out"
printf '{}\n' >"$hD/workspaces/Ivy/webdavis/dotfiles/graphify-out/graph.json"
seed_loaded
run_gate "$hD" 1
if [[ $RC -eq 1 ]] && grep -q 'graphify-out' "$err_file"; then
  report ok "D: gitignored graphify-out residue refuses even with clean porcelain"
else
  report bad "D: graphify-out residue did not refuse (rc=$RC, err: $(cat "$err_file"))"
fi

# ── Case E: the retirement manifest ────────────────────────────────────────
hE="$work/homeE"
cutover_build_sandbox "$hE"
cutover_write_classification "$hE"
ledE="$(ledger_of "$hE")"
seed_loaded \
  com.github.openclaw-setup.watchdog \
  com.webdavis.osquery-fim-notify \
  com.webdavis.atuin-daemon \
  com.spotify.client \
  io.osquery.agent \
  system:com.openssh.sshd
run_gate "$hE" 1
if grep -qx "com.github.openclaw-setup.watchdog	gui/$uid" "$ledE/retirement-proposed.tsv" 2>/dev/null; then
  report ok "E: a deleted historical out-of-prefix label found loaded is a retirement candidate"
else
  report bad "E: the openclaw orphan is not in the proposal (got: $(cat "$ledE/retirement-proposed.tsv" 2>/dev/null))"
fi
if grep -qx "com.webdavis.osquery-fim-notify	gui/$uid" "$ledE/retirement-proposed.tsv" 2>/dev/null; then
  report ok "E: a renamed-away pre-rename label found loaded is a retirement candidate"
else
  report bad "E: the renamed-away label is not in the proposal"
fi
for keep in com.webdavis.atuin-daemon com.spotify.client io.osquery.agent com.openssh.sshd; do
  if grep -q "^$keep	" "$ledE/retirement-proposed.tsv" 2>/dev/null; then
    report bad "E: $keep must never be a retirement candidate"
  else
    report ok "E: $keep is not a retirement candidate"
  fi
done
if grep -q 'com.github.openclaw-setup.watchdog' "$ledE/managed-label-universe.tsv" 2>/dev/null; then
  report ok "E: the universe is derived from repository history, not a prefix match"
else
  report bad "E: the managed-label universe does not carry the out-of-prefix historical label"
fi

# ── Case F: the approval checkpoint ────────────────────────────────────────
hF="$work/homeF"
cutover_build_sandbox "$hF"
cutover_write_classification "$hF"
ledF="$(ledger_of "$hF")"
seed_loaded com.github.openclaw-setup.watchdog com.webdavis.atuin-daemon
run_gate "$hF" 1 --approve-retirement
if [[ $RC -eq 1 ]]; then
  report ok "F: approving with no reviewed proposal refuses"
else
  report bad "F: approval without a proposal did not refuse (rc=$RC)"
fi
run_gate "$hF" 1
run_gate "$hF" 1 --approve-retirement
if [[ $RC -eq 0 ]]; then
  report ok "F: approving the re-derived, unchanged manifest passes gate 1"
else
  report bad "F: approval of an unchanged manifest failed (rc=$RC, err: $(cat "$err_file"))"
fi
if [[ -f "$ledF/gate1.done" ]] && [[ -f "$ledF/retirement-approved.tsv" ]]; then
  report ok "F: approval records gate1.done and the approved manifest"
else
  report bad "F: approval did not record the pass marker or the approved manifest"
fi
# the manifest changes under the operator: approval must not rubber-stamp it
rm -f "$ledF/gate1.done"
seed_loaded com.github.openclaw-setup.watchdog com.webdavis.osquery-fim-notify com.webdavis.atuin-daemon
run_gate "$hF" 1 --approve-retirement
if [[ $RC -eq 1 ]] && grep -qi 'changed' "$err_file"; then
  report ok "F: a manifest that changed since review refuses approval"
else
  report bad "F: a changed manifest was approved anyway (rc=$RC, err: $(cat "$err_file"))"
fi

# ── Case F2: re-running gate 1 restarts the procedure ──────────────────────
# Re-pinning invalidates everything downstream: a plain gate 1 run re-derives
# the manifest, so the earlier approval no longer reviewed what gate 2 would
# execute, and any later gate that already passed did so against the old pins.
seed_loaded com.github.openclaw-setup.watchdog com.webdavis.atuin-daemon
run_gate "$hF" 1
run_gate "$hF" 1 --approve-retirement
: >"$ledF/gate2.landed"
: >"$ledF/gate2.done"
: >"$ledF/gate3.done"
: >"$ledF/gate4.done"
run_gate "$hF" 1
stale=()
for marker in gate1.done gate2.landed gate2.done gate3.done gate4.done retirement-approved.tsv; do
  [[ -e "$ledF/$marker" ]] && stale+=("$marker")
done
if [[ ${#stale[@]} -eq 0 ]]; then
  report ok "F2: re-running gate 1 clears the approval and every downstream pass marker"
else
  report bad "F2: re-running gate 1 left stale state: ${stale[*]}"
fi

# ── Case F3: the rendered data view must agree with the source file ────────
# chezmoi's data-only reads walk the source directory's nested worktrees
# ignoring .chezmoiignore (twpayne/chezmoi#4940), and .chezmoidata merges
# last-one-wins, so one stale copy under a worktree silently replaces the
# repository's own declaration in every rendered view. Verified live on this
# machine: the source file declares 54 casks and `chezmoi data` reported 47.
# Those subtrees are gitignored, so the clean-tree check cannot see them.
hF3="$work/homeF3"
cutover_build_sandbox "$hF3"
cutover_write_classification "$hF3"
seed_loaded
# a stale nested copy drops a cask from the rendered view
yq -o=json '{"packages": .packages}' \
  "$hF3/workspaces/Ivy/webdavis/dotfiles/.chezmoidata/system_packages_autoinstall.yaml" |
  jq '.packages.macos.homebrew.casks -= ["lulu"]' >"$CHEZMOI_DATA_FILE"
run_gate "$hF3" 1
if [[ $RC -eq 1 ]]; then
  report ok "F3: a rendered data view that disagrees with the source file refuses"
else
  report bad "F3: a disagreeing render passed (rc=$RC)"
fi
if grep -q 'system_packages_autoinstall.yaml' "$err_file"; then
  report ok "F3: the refusal names the data file"
else
  report bad "F3: the refusal does not name the data file (err: $(cat "$err_file"))"
fi
if grep -q '4940' "$err_file"; then
  report ok "F3: the refusal cites the upstream issue so the cause is findable"
else
  report bad "F3: the refusal does not cite the upstream issue"
fi
if [[ ! -f "$(ledger_of "$hF3")/pins.env" ]]; then
  report ok "F3: nothing is pinned against a source tree that renders differently"
else
  report bad "F3: pins were recorded despite a disagreeing render"
fi
cutover_write_render_view "$hF3" "$CHEZMOI_DATA_FILE"
run_gate "$hF3" 1
if [[ $RC -eq 10 ]]; then
  report ok "F3: an agreeing render proceeds to the checkpoint"
else
  report bad "F3: an agreeing render did not proceed (rc=$RC, err: $(cat "$err_file"))"
fi

# ── Case G: a failed fetch refuses ─────────────────────────────────────────
hG="$work/homeG"
cutover_build_sandbox "$hG"
cutover_write_classification "$hG"
ledG="$(ledger_of "$hG")"
cutover_git -C "$hG/workspaces/Ivy/webdavis/dotfiles" remote set-url origin "$hG/does-not-exist.git"
seed_loaded
run_gate "$hG" 1
if [[ $RC -eq 1 ]]; then
  report ok "G: a failed fetch refuses"
else
  report bad "G: a failed fetch did not refuse (rc=$RC)"
fi
if [[ ! -f "$ledG/pins.env" ]]; then
  report ok "G: no pins recorded from stale remote-tracking refs"
else
  report bad "G: pins were recorded after a failed fetch"
fi

# ── Case H: a pristine checkout, and graphify-out RESIDUE ──────────────────
# graphify-out/graph.json is the COMMITTED map, so the directory exists in every
# clean checkout. Refusing on the directory made gate 1 unpassable. What must
# refuse is the ignored rebuild output beside the tracked map.
hH="$work/homeH"
cutover_build_sandbox "$hH"
cutover_write_classification "$hH"
seed_loaded
run_gate "$hH" 1
if [[ $RC -eq 10 ]]; then
  report ok "H: a pristine checkout carrying the tracked graphify map passes the clean-tree check"
else
  report bad "H: the tracked graphify map blocked a clean checkout (rc=$RC, err: $(cat "$err_file"))"
fi
printf '<html/>\n' >"$hH/workspaces/Ivy/webdavis/dotfiles/graphify-out/report.html"
run_gate "$hH" 1
if [[ $RC -eq 1 ]] && grep -q 'graphify-out' "$err_file"; then
  report ok "H: ignored graphify-out residue still refuses"
else
  report bad "H: ignored residue passed (rc=$RC, err: $(cat "$err_file"))"
fi
rm -f "$hH/workspaces/Ivy/webdavis/dotfiles/graphify-out/report.html"

# ── Case I: a deployable file git does not track ───────────────────────────
# git status omits ignored paths, so an ignored-but-managed source file is
# invisible to porcelain and still reaches $HOME through the staged apply.
# chezmoi managed is the authority on what deploys.
hI="$work/homeI"
cutover_build_sandbox "$hI"
cutover_write_classification "$hI"
seed_loaded
export CHEZMOI_MANAGED_FILE="$work/managed.txt"
printf 'seed.txt\npaseo.json\n' >"$CHEZMOI_MANAGED_FILE"
run_gate "$hI" 1
if [[ $RC -eq 1 ]] && grep -q 'paseo.json' "$err_file"; then
  report ok "I: a managed source file git does not track refuses, and is named"
else
  report bad "I: an untracked deployable file passed (rc=$RC, err: $(cat "$err_file"))"
fi
printf 'seed.txt\n' >"$CHEZMOI_MANAGED_FILE"
run_gate "$hI" 1
if [[ $RC -eq 10 ]]; then
  report ok "I: a fully tracked managed set proceeds"
else
  report bad "I: a tracked managed set was rejected (rc=$RC, err: $(cat "$err_file"))"
fi
FAIL_MANAGED=1 run_gate "$hI" 1
if [[ $RC -eq 1 ]]; then
  report ok "I: a failed 'chezmoi managed' refuses instead of assuming an empty deployable set"
else
  report bad "I: a failed managed enumeration was tolerated (rc=$RC)"
fi
unset CHEZMOI_MANAGED_FILE

# ── Case J: source-only trees never become desired services ────────────────
# This repository's own test helper carries plist fixtures beside a
# 'launchctl bootstrap system' line, and this runner quotes both. Scanning them
# turns fixtures into desired SYSTEM services, and two of those fixture labels
# are the historical orphans that must be RETIRED, so an unscoped scan cancels
# their retirement.
hJ="$work/homeJ"
cutover_build_sandbox "$hJ"
cutover_write_classification "$hJ"
repoJ="$hJ/workspaces/Ivy/webdavis/dotfiles"
mkdir -p "$repoJ/test/integration/helpers" "$repoJ/scripts" "$repoJ/docs"
{
  printf '#!/usr/bin/env bash\n'
  printf '\t<key>Label</key>\n'
  printf '\t<string>com.github.openclaw-setup.watchdog</string>\n'
  printf '\t<key>Label</key>\n'
  printf '\t<string>com.example.test-fixture</string>\n'
  # shellcheck disable=SC2016  # fixture text, not an expansion
  printf 'launchctl bootstrap system "$PLIST_PATH"\n'
} >"$repoJ/test/integration/helpers/fixtures.sh"
cutover_git -C "$repoJ" add -A
cutover_git -C "$repoJ" commit --quiet -m 'a test helper carrying plist fixtures'
cutover_git -C "$repoJ" push --quiet origin main
seed_loaded com.github.openclaw-setup.watchdog
run_gate "$hJ" 1
ledJ="$(ledger_of "$hJ")"
if grep -q 'com.example.test-fixture' "$ledJ/desired-services.tsv" 2>/dev/null; then
  report bad "J: a test fixture label became a desired service"
else
  report ok "J: source-only trees contribute no desired services"
fi
if grep -qx "com.github.openclaw-setup.watchdog	gui/$uid" "$ledJ/retirement-proposed.tsv" 2>/dev/null; then
  report ok "J: the historical orphan is still a retirement candidate, not shadowed by a fixture"
else
  report bad "J: a fixture cancelled a real retirement (proposal: $(cat "$ledJ/retirement-proposed.tsv" 2>/dev/null))"
fi

# ── Case K: the base override is refused in the real repository ────────────
hK="$work/homeK"
cutover_build_sandbox "$hK"
cutover_write_classification "$hK"
seed_loaded
repoK="$hK/workspaces/Ivy/webdavis/dotfiles"
# graft the recorded Phase A base into the sandbox, which is what makes a
# repository "the real one" as far as the guard is concerned
recorded=2bd973369158b49535e8e16e80c968444ab23f1d
if git -C "$REPO_ROOT" cat-file -e "$recorded^{commit}" 2>/dev/null; then
  cutover_git -C "$repoK" fetch --quiet --no-tags "$REPO_ROOT" "$recorded" 2>/dev/null || true
  if git -C "$repoK" cat-file -e "$recorded^{commit}" 2>/dev/null; then
    run_gate "$hK" 1
    if [[ $RC -eq 1 ]] && grep -q 'CUTOVER_PHASE_A_BASE' "$err_file"; then
      report ok "K: the base override is refused where the recorded Phase A base exists"
    else
      report bad "K: the override was accepted in a repository holding the recorded base (rc=$RC)"
    fi
  else
    report ok "K: SKIPPED (the recorded base could not be grafted into the sandbox)"
  fi
else
  report ok "K: SKIPPED (this checkout does not contain the recorded Phase A base)"
fi

# ── Case L: a failed gate 1 restart invalidates downstream passes ──────────
# Pins are rewritten early. If invalidation waited for success, a gate 1 that
# fails classification would leave gate4.done standing beside the NEW pins, and
# gate 5 would close the pull requests against a soak that never covered them.
hL="$work/homeL"
cutover_build_sandbox "$hL"
cutover_write_classification "$hL"
seed_loaded
ledL="$(ledger_of "$hL")"
run_gate "$hL" 1
run_gate "$hL" 1 --approve-retirement
: >"$ledL/gate2.done"
: >"$ledL/gate3.done"
: >"$ledL/gate4.done"
repoL="$hL/workspaces/Ivy/webdavis/dotfiles"
cutover_git -C "$repoL" checkout --quiet integration/modernization
printf 'never landed\n' >"$repoL/late.txt"
cutover_git -C "$repoL" add -A
cutover_git -C "$repoL" commit --quiet -m 'an unclassified hunk lands on integration'
cutover_git -C "$repoL" push --quiet origin integration/modernization
cutover_git -C "$repoL" checkout --quiet main
run_gate "$hL" 1
if [[ $RC -eq 1 ]]; then
  report ok "L: the restart fails classification, as intended"
else
  report bad "L: the unclassified hunk did not block (rc=$RC)"
fi
stale=()
for marker in gate1.done gate2.done gate3.done gate4.done gate5.done retirement-approved.tsv; do
  [[ -e "$ledL/$marker" ]] && stale+=("$marker")
done
if [[ ${#stale[@]} -eq 0 ]]; then
  report ok "L: a FAILED gate 1 leaves no downstream pass attached to the new pins"
else
  report bad "L: stale passes survived a failed gate 1 restart: ${stale[*]}"
fi

# ── Case M: classification is bound to the exact tree-entry pair ───────────
hM="$work/homeM"
cutover_build_sandbox "$hM"
cutover_write_classification "$hM"
seed_loaded
ledM="$(ledger_of "$hM")"
run_gate "$hM" 1
if [[ $RC -eq 10 ]]; then
  report ok "M: matching pairs classify"
else
  report bad "M: the baseline classification did not apply (rc=$RC, err: $(cat "$err_file"))"
fi
repoM="$hM/workspaces/Ivy/webdavis/dotfiles"
printf 'improved-on-main, again\n' >"$repoM/b.txt"
cutover_git -C "$repoM" add -A
cutover_git -C "$repoM" commit --quiet -m 'main changes b.txt again'
cutover_git -C "$repoM" push --quiet origin main
run_gate "$hM" 1
if [[ $RC -eq 1 ]] && grep -q 'b.txt' "$err_file"; then
  report ok "M: a stale classification stops applying once either side changes"
else
  report bad "M: an outdated classification still covered the file (rc=$RC, err: $(cat "$err_file"))"
fi
if grep -q 'b.txt' "$ledM/delta-unclassified.tsv" 2>/dev/null; then
  report ok "M: the runner prints a prefilled row carrying the new pair"
else
  report bad "M: no prefilled classification row for the changed pair"
fi

# ── Case N: renames, modes, and quoted pathnames ───────────────────────────
hN="$work/homeN"
cutover_build_sandbox "$hN"
seed_loaded
repoN="$hN/workspaces/Ivy/webdavis/dotfiles"
cutover_git -C "$repoN" checkout --quiet integration/modernization
# a REAL rename: the base file moves, so rename detection collapses the pair
cutover_git -C "$repoN" mv "$repoN/dot_aws/credentials.tmpl" "$repoN/dot_aws/private_credentials.tmpl"
printf 'installer\n' >"$repoN/exec-me.sh"
chmod +x "$repoN/exec-me.sh"
printf 'unicode\n' >"$repoN/café.txt"
cutover_git -C "$repoN" add -A
cutover_git -C "$repoN" commit --quiet -m 'integration renames a secret source, adds a mode and a quoted path'
cutover_git -C "$repoN" push --quiet origin integration/modernization
cutover_git -C "$repoN" checkout --quiet main
# main takes the rename DESTINATION but never makes the source-side deletion,
# copies the file without its executable bit, and never takes the quoted path
printf 'aws credentials template\n' >"$repoN/dot_aws/private_credentials.tmpl"
printf 'installer\n' >"$repoN/exec-me.sh"
chmod -x "$repoN/exec-me.sh"
cutover_git -C "$repoN" add -A
cutover_git -C "$repoN" commit --quiet -m 'main takes the destination, keeps the old secret source, drops the mode'
cutover_git -C "$repoN" push --quiet origin main
cutover_write_classification "$hN"
run_gate "$hN" 1
ledN="$(ledger_of "$hN")"
if [[ $RC -eq 1 ]]; then
  report ok "N: the mode-only and never-landed paths block"
else
  report bad "N: mode and unicode paths were waved through (rc=$RC)"
fi
if grep -q 'exec-me.sh' "$err_file"; then
  report ok "N: an executable-bit change is not landed-unchanged"
else
  report bad "N: a mode-only difference classified as landed (err: $(cat "$err_file"))"
fi
if grep -q 'caf' "$err_file"; then
  report ok "N: a non-ASCII pathname is classified, not silently landed"
else
  report bad "N: a quoted pathname became a false landed-unchanged (ledger: $(cat "$ledN/delta-ledger.tsv" 2>/dev/null))"
fi
# The rename's SOURCE side: main never deleted the old secret file, and rename
# detection would have hidden that path from the manifest entirely.
if grep -q 'dot_aws/credentials.tmpl' "$err_file"; then
  report ok "N: the source side of a rename is classified, so an un-made deletion blocks"
else
  report bad "N: rename detection hid the source-side deletion (missing: $(cut -f2 "$ledN/delta-missing.tsv" 2>/dev/null | tr '\n' ' '))"
fi

# ── Case O: membership is a (label, domain) pair ───────────────────────────
# A label that moved between domains is desired in the NEW domain while its old
# instance is still loaded in the old one. Matching on the label alone reads the
# stale copy as desired and the retirement that should catch it never fires.
hO="$work/homeO"
cutover_build_sandbox "$hO"
cutover_write_classification "$hO"
repoO="$hO/workspaces/Ivy/webdavis/dotfiles"
mkdir -p "$repoO/Library/LaunchDaemons"
printf '<plist><dict><key>Label</key><string>com.webdavis.osquery-fim-notify</string><key>KeepAlive</key><true/></dict></plist>\n' \
  >"$repoO/Library/LaunchDaemons/com.webdavis.osquery-fim-notify.plist.tmpl"
cutover_git -C "$repoO" add -A
cutover_git -C "$repoO" commit --quiet -m 'the label moves to the system domain'
cutover_git -C "$repoO" push --quiet origin main
# loaded in the OLD user domain, rendered by main in the system domain
seed_loaded com.webdavis.osquery-fim-notify system:com.webdavis.osquery-fim-notify
run_gate "$hO" 1
ledO="$(ledger_of "$hO")"
if grep -qx "com.webdavis.osquery-fim-notify	gui/$uid" "$ledO/retirement-proposed.tsv" 2>/dev/null; then
  report ok "O: the stale user-domain instance of a moved label is a retirement candidate"
else
  report bad "O: a moved label's old instance was read as desired (proposal: $(cat "$ledO/retirement-proposed.tsv" 2>/dev/null))"
fi
if grep -qx "com.webdavis.osquery-fim-notify	system" "$ledO/retirement-proposed.tsv" 2>/dev/null; then
  report bad "O: the desired system instance was proposed for retirement"
else
  report ok "O: the desired system instance is left alone"
fi

if [[ $failures -gt 0 ]]; then
  printf 'cutover-gate-preflight: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'cutover-gate-preflight: OK (clean tree, backup, pin-last from origin, delta ledger, history-derived retirement manifest, approval checkpoint)\n'
