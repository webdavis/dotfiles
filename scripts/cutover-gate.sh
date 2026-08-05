#!/usr/bin/env bash
# cutover-gate.sh: the D1 cutover gate runner.
#
# The SP2 plan (docs/superpowers/plans/2026-07-03-sp2-combine-and-split.md,
# "Cutover tooling PR" + Phase D) states the cutover as invariants only and
# gives this script every command: the operator runs the cutover through
# `cutover-gate.sh <gate>` and never ad hoc. Gates run in order, each in its
# own shell, and each must pass before the next begins.
#
#   1  preflight        clean tree, Hermes backup, pins LAST, expected-delta
#                       ledger, retirement manifest + operator approval
#   2  activation       pins re-verified, attached landing at the pin, the
#                       operator's staged apply, approved retirement executed
#   3  reconciliation   live-reconcile (dry-run then live), post-retirement
#                       verification, test suite, live smoke checks
#   4  soak             the final retired topology runs for the soak window
#   5  closure          pins re-verified, the reference PRs closed
#
# Exit codes: 0 pass, 1 refusal (a check failed), 2 usage, 10 operator
# checkpoint pending (the gate stopped for a human step and has not passed).
#
# Ledger artifacts live OUTSIDE the checkout ($HOME/.local/state/cutover),
# because gate 1 requires a fully-visible-clean tree.
set -euo pipefail

# Checklist item 5: the repo handle is absolute and validated at the top of
# EVERY invocation; each gate runs in its own shell and nothing carries over.
repo="$HOME/workspaces/Ivy/webdavis/dotfiles"

# Checklist item 2: the expected-delta manifest is regenerated from the
# RECORDED Phase A base SHA to the pinned integration tip, so no mutable ref
# and no stored file count is normative.
RECORDED_PHASE_A_BASE=2bd973369158b49535e8e16e80c968444ab23f1d
PHASE_A_BASE="${CUTOVER_PHASE_A_BASE:-$RECORDED_PHASE_A_BASE}"

LEDGER="$HOME/.local/state/cutover"
INT_BRANCH="integration/modernization"

# Checklist item 17: gh's resolver precedence is --repo > GH_REPO > cwd remote,
# so the repository is named explicitly, host included, in the documented
# [HOST/]OWNER/REPO form. (`gh pr close` has no --hostname flag; the host
# travels in --repo, verified against `gh pr close --help`.)
GH_TARGET="github.com/webdavis/dotfiles"
REFERENCE_PRS=(25 31 32)

# Gate 4's soak window. Operator ruling 2026-08-05: no soak wait; gate 4's value
# is the topology re-verify and smoke re-run, not the clock, so the default is
# zero and --window-hours can restore a window on a machine that wants one.
SOAK_HOURS_DEFAULT=0

# The PRESERVE list is orthogonal to the managed-label universe (checklist item
# 13): these are package/OS-owned services that are never retirement
# candidates, no matter what the enumeration turns up.
PRESERVE_LABELS=(
  io.osquery.agent
  com.tailscale.tailscaled
  com.openssh.sshd
)

usage() {
  printf 'usage: cutover-gate.sh <1|2|3|4|5> [options]\n' >&2
  printf '\n' >&2
  printf '  1 [--approve-retirement]  preflight; re-run with the flag to approve the manifest\n' >&2
  printf '  2 --second-session-open   activation preflight and attached landing at the pin\n' >&2
  printf '  2 --post-apply            execute the approved retirement after the staged apply\n' >&2
  printf '  3                         reconciliation, verification, test suite, smoke checks\n' >&2
  printf '  4 [--window-hours <n>]    re-verify the final topology (default %s hours of soak)\n' "$SOAK_HOURS_DEFAULT" >&2
  printf '  5                         re-verify the pins and close the reference PRs\n' >&2
  exit 2
}

die() {
  printf 'REFUSED: %s\n' "$*" >&2
  exit 1
}

checkpoint() {
  printf '\nCHECKPOINT: %s\n' "$*"
  exit 10
}

say() { printf '%s\n' "$*"; }
ok() { printf '  ok   %s\n' "$*"; }

# ---------------------------------------------------------------------------
# Shared checks
# ---------------------------------------------------------------------------

# Checklist item 5.
require_repo() {
  [[ -d "$repo/.git" ]] ||
    die "the repo handle $repo is not a git checkout; every gate runs against that absolute path"
}

# The base override exists only so the test suite can build sandbox repositories,
# which cannot contain the recorded Phase A commit. In the REAL repository that
# commit is reachable, and an override there would be catastrophic rather than
# convenient: pointing it at INT_SHA makes the manifest empty, so the ledger
# classifies nothing and gate 1 passes having proved nothing. The repository
# itself decides which case this is, so no honour-system marker is involved.
require_base_override_is_sandbox_only() {
  [[ -n ${CUTOVER_PHASE_A_BASE:-} ]] || return 0
  git -C "$repo" cat-file -e "$RECORDED_PHASE_A_BASE^{commit}" 2>/dev/null || return 0
  die "CUTOVER_PHASE_A_BASE is set, but $repo contains the recorded Phase A base $RECORDED_PHASE_A_BASE, so this is the real repository and the override would replace the expected-delta manifest with one nobody reviewed. Unset it."
}

# Checklist item 8: a failed fetch must never let stale remote-tracking refs
# satisfy a later comparison.
fetch_guarded() {
  git -C "$repo" fetch origin ||
    die "git fetch origin failed; stale remote-tracking refs must never satisfy a pin check"
}

valid_sha() { [[ $1 =~ ^[0-9a-f]{40}$ ]]; }

# tree_entry <rev> <path> : "<mode> <object>" for that path, or empty when the
# path does not exist there. ls-tree carries the MODE, which rev-parse does not:
# a file that should have become executable, or a blob that should have become a
# symlink, has the same content and a different entry.
tree_entry() {
  git -C "$repo" ls-tree "$1" -- "$2" 2>/dev/null |
    awk 'NR == 1 { printf "%s %s", $1, $3 }'
}

# Checklist item 4, and the reason it needs more than porcelain: git omits
# ignored paths from `status`, so anything ignored yet still deployable would
# reach $HOME through the staged apply without the ledger ever classifying it.
#
# Three separate reads, because no single one sees all of it:
#   - porcelain --untracked-files=all: dirty and untracked tracked-space entries
#   - graphify-out RESIDUE: graph.json is TRACKED (the committed map), so the
#     directory legitimately exists in a pristine checkout. What must be absent
#     is the ignored rebuild output beside it, which is what the live post-commit
#     hook regenerates.
#   - managed-but-untracked: `chezmoi managed` is the authority on what deploys;
#     any source path it names that git does not track is unpinned, unclassified
#     content that the apply would install anyway (paseo.json reaches $HOME this
#     way today, ignored through .git/info/exclude).
#
# Callers re-run this immediately before the apply.
require_clean_tree() {
  local dirty residue managed_untracked
  dirty="$(git -C "$repo" status --porcelain --untracked-files=all)"
  [[ -z $dirty ]] ||
    die "the tree is not clean; classify keep/discard/back-up and move kept files OUT of the source tree first:"$'\n'"$dirty"

  residue="$(git -C "$repo" status --porcelain --untracked-files=all --ignored=matching -- graphify-out || true)"
  [[ -z $residue ]] ||
    die "$repo/graphify-out carries ignored rebuild residue; it escapes a porcelain listing and would deploy unclassified content:"$'\n'"$residue"

  managed_untracked="$(managed_but_untracked)"
  [[ -z $managed_untracked ]] ||
    die "chezmoi would deploy source files that git does not track, so the ledger can never classify them and the pinned commit does not describe them:"$'\n'"$managed_untracked"

  ok "tree is clean, fully visible (no dirty, no untracked, no graphify-out residue, nothing deployable untracked)"
}

# Every source FILE `chezmoi managed` names that `git ls-files` does not.
# chezmoi managed honours .chezmoiignore (it is not one of the data-only reads
# twpayne/chezmoi#4940 breaks), so it is the honest authority on what deploys.
#
# TWO THINGS THIS COMPARISON MUST GET RIGHT, both measured on dresden 2026-08-04
# when the first version reported 281 offenders against a tree that had none:
#
#   --exclude=dirs. `chezmoi managed` names managed DIRECTORIES as well as
#   files; `git ls-files` never names a directory. Without the exclusion every
#   managed directory is an offender that no commit can ever clear, so the gate
#   is unpassable by construction. 73 of the 281 were directories.
#
#   LC_ALL=C on comm, not only on sort. comm assumes its inputs are sorted in
#   ITS OWN collation. Pinning the sorts to C while comm ran under the login
#   locale (en_US.UTF-8 here) made comm treat correctly-sorted input as
#   unsorted and emit nonsense: 208 of the 281 were files that ARE tracked and
#   appear verbatim in both lists. A gate that names tracked files as
#   deployable-but-unclassified teaches the operator to disbelieve it, which is
#   worse than no gate.
managed_but_untracked() {
  local managed tracked
  managed="$scratch/managed"
  tracked="$scratch/tracked"
  chezmoi managed --source "$repo" --path-style source-relative --exclude=dirs >"$managed" ||
    die "chezmoi managed failed against $repo; the deployable set cannot be established"
  git -C "$repo" ls-files >"$tracked" ||
    die "git ls-files failed in $repo"
  LC_ALL=C sort -o "$managed" "$managed"
  LC_ALL=C sort -o "$tracked" "$tracked"
  LC_ALL=C comm -23 "$managed" "$tracked"
}

# chezmoi's DATA-ONLY reads (`chezmoi data`, `chezmoi execute-template`) walk
# the source directory's nested worktree and .claude subtrees IGNORING
# .chezmoiignore: in sourcestate.go's walkFunc the `case s.templateDataOnly:
# return nil` arm precedes the ignore-prefix SkipDir (twpayne/chezmoi#4940).
# Because .chezmoidata merges last-one-wins, ONE stale copy of a data file under
# a nested worktree silently replaces this repository's own declaration in every
# rendered view. Measured on dresden 2026-08-04: the source file declares 54
# casks, `chezmoi data` reported 47, and the seven it dropped (codex, codex-app,
# fluidvoice, lulu, minutes, oversight, paseo) are packages a cutover apply
# would then be free to uninstall. apply, status, managed and cat honour the
# ignore and stay correct.
#
# Two consequences, and both matter here. Nothing in this runner asserts against
# a rendered view: the data assertion below reads .chezmoidata/*.yaml directly
# with yq, and gate 3's drift smoke check uses `chezmoi status`, which is
# immune. And the disagreement itself is a finding, so this refuses on it: the
# nested worktrees are gitignored, so they escape the porcelain listing exactly
# as graphify-out does, and the clean-tree check alone would let a source tree
# that renders a different package set than it declares through the gate.
require_render_matches_source() {
  local data_file="$repo/.chezmoidata/system_packages_autoinstall.yaml"
  local source_view render_view stale
  command -v yq >/dev/null 2>&1 || die "yq is required to read $data_file directly"
  command -v jq >/dev/null 2>&1 || die "jq is required to compare the rendered data view"
  [[ -f $data_file ]] || die "missing $data_file"
  source_view="$(yq -o=json '{"packages": .packages}' "$data_file" | jq -S .)" ||
    die "could not read the package declaration out of $data_file"
  render_view="$(chezmoi --source "$repo" data | jq -S '{packages}')" ||
    die "chezmoi data failed against $repo"
  if [[ $source_view != "$render_view" ]]; then
    # a missing worktrees dir makes find exit non-zero; the refusal below is
    # the point, so the listing is best-effort
    stale="$(find "$repo/.worktrees" "$repo/.claude" -type f \
      -path '*/.chezmoidata/*' 2>/dev/null | head -5 || true)"
    die "the rendered data view disagrees with $data_file, so this source directory renders a package set it does not declare (twpayne/chezmoi#4940: data-only reads walk nested worktrees ignoring .chezmoiignore, and .chezmoidata merges last-one-wins). Remove the finished worktrees under the source directory and re-run. Stale copies found:"$'\n'"${stale:-  (none under .worktrees or .claude; look for other nested source copies)}"
  fi
  ok "the rendered data view agrees with $data_file"
}

# Checklist item 7: pins are reloaded from the ledger and each is validated as
# a full 40-hex SHA before use; missing, short, or empty pins abort.
load_pins() {
  local pins="$LEDGER/pins.env"
  [[ -f $pins ]] || die "no pins recorded at $pins; gate 1 has not run"
  MAIN_SHA="$(sed -n 's/^MAIN_SHA=//p' "$pins" | head -n1)"
  INT_SHA="$(sed -n 's/^INT_SHA=//p' "$pins" | head -n1)"
  valid_sha "${MAIN_SHA:-}" || die "MAIN_SHA in $pins is not a full 40-hex SHA: '${MAIN_SHA:-}'"
  valid_sha "${INT_SHA:-}" || die "INT_SHA in $pins is not a full 40-hex SHA: '${INT_SHA:-}'"
  ok "pins reloaded: MAIN_SHA=$MAIN_SHA INT_SHA=$INT_SHA"
}

# Checklist items 9 and 10: both pins re-verified fail-closed. On either
# mismatch the procedure restarts at gate 1.
require_pins_unmoved() {
  local live_main live_int
  live_main="$(git -C "$repo" rev-parse "origin/main")"
  live_int="$(git -C "$repo" rev-parse "origin/$INT_BRANCH")"
  [[ $live_main == "$MAIN_SHA" ]] ||
    die "origin/main moved ($live_main != $MAIN_SHA); the soaked state is not the closing state, restart at gate 1"
  [[ $live_int == "$INT_SHA" ]] ||
    die "origin/$INT_BRANCH moved ($live_int != $INT_SHA); restart at gate 1"
  ok "both pins still match origin/main and origin/$INT_BRANCH"
}

# Checklist item 11: the live checkout is attached to main at the pin, never
# detached.
require_attached_at_pin() {
  local branch head
  branch="$(git -C "$repo" symbolic-ref --quiet --short HEAD || true)"
  head="$(git -C "$repo" rev-parse HEAD)"
  [[ $branch == "main" ]] ||
    die "the checkout is not attached to main (HEAD is '${branch:-detached}'); a detached checkout leaves the live source floating off-branch"
  [[ $head == "$MAIN_SHA" ]] ||
    die "HEAD ($head) is not the pinned MAIN_SHA ($MAIN_SHA)"
  ok "checkout attached to main at the pin"
}

require_gate_passed() {
  [[ -f "$LEDGER/gate$1.done" ]] ||
    die "gate $1 has not passed; gates run in order and each must pass before the next begins"
}

# Pass markers are existence-only, so they must be INVALIDATED AT ENTRY, before
# the gate does anything that could change what an earlier pass meant. Two ways
# that bites otherwise: a failed retry leaves the previous run's marker standing
# (gate 4 then soaks against a gate 3 that just failed), and a gate 1 re-run
# rewrites the pins before clearing anything (gate 5 then closes the PRs against
# a gate4.done earned under the OLD pins). Clearing first makes every invocation
# a restart of everything downstream of it, which is the plan's own semantics.
#
#   begin_gate <stage>   stage: 1, 2-landing, 2-post-apply, 3, 4, 5
begin_gate() {
  local stage="$1"
  case "$stage" in
    1) rm -f "$LEDGER"/gate{1,3,4,5}.done "$LEDGER"/gate2.{landed,done} \
      "$LEDGER/retirement-approved.tsv" ;;
    2-landing) rm -f "$LEDGER"/gate{3,4,5}.done "$LEDGER"/gate2.{landed,done} ;;
    2-post-apply) rm -f "$LEDGER"/gate{2,3,4,5}.done ;;
    3) rm -f "$LEDGER"/gate{3,4,5}.done ;;
    4) rm -f "$LEDGER"/gate{4,5}.done ;;
    5) rm -f "$LEDGER/gate5.done" ;;
    *) die "internal: begin_gate called with an unknown stage '$stage'" ;;
  esac
}

# ---------------------------------------------------------------------------
# Gates
# ---------------------------------------------------------------------------

# Chezmoi attribute prefixes stack (private_encrypted_foo.plist.tmpl); strip
# them all, then the suffixes, to recover the launchd label from a source path.
label_from_path() {
  local base="${1##*/}"
  while :; do
    case "$base" in
      private_*) base="${base#private_}" ;;
      encrypted_*) base="${base#encrypted_}" ;;
      executable_*) base="${base#executable_}" ;;
      readonly_*) base="${base#readonly_}" ;;
      *) break ;;
    esac
  done
  base="${base%.tmpl}"
  printf '%s' "${base%.plist}"
}

domain_from_path() {
  case "$1" in
    */LaunchDaemons/*) printf 'system' ;;
    *) printf 'gui/%s' "$(id -u)" ;;
  esac
}

# One XML tag per line, whitespace squeezed, so key/value pairs can be read with
# a fixed-string grep. awk does the split because BSD sed cannot emit a newline
# from a replacement.
flatten_xml() { awk '{gsub(/></, ">\n<"); print}' | tr -d '[:blank:]'; }

# Checklist item 14: the steady-state predicate comes from the plist semantics.
# RunAtLoad launches once and is NOT persistence, so it never makes a job
# persistent; a conditional KeepAlive dictionary is its own class.
predicate_of_plist() {
  local flat="$1" next
  next="$(grep -A1 -Fx '<key>KeepAlive</key>' "$flat" | sed -n '2p' || true)"
  case "$next" in
    '<true/>')
      printf 'persistent'
      return
      ;;
    '<dict>')
      printf 'conditional'
      return
      ;;
  esac
  if grep -qFx -e '<key>StartInterval</key>' -e '<key>StartCalendarInterval</key>' \
    -e '<key>WatchPaths</key>' "$flat"; then
    printf 'scheduled'
  else
    printf 'demand'
  fi
}

# Emits "<label>\t<domain>" for every launchd job a NON-plist source file in
# <rev> installs, with the domain taken from the bootstrap call itself.
#
# The scan is limited to paths chezmoi DEPLOYS. The source-only trees carry
# plist fixtures and bootstrap strings that install nothing: this repository's
# own test helper contains four fixture plists next to a `launchctl bootstrap
# system` line, and this runner quotes both while implementing them. Scanning
# them turns test fixtures into desired SYSTEM services, and two of those
# fixture labels are precisely the historical orphans that must be RETIRED, so
# an unscoped scan would move them into the desired set and quietly cancel their
# retirement. Anything under test/, scripts/ or docs/ is source-only by
# .chezmoiignore and can never install a job on this machine.
installer_labels_at() {
  local rev="$1" file blob flat domain files
  blob="$scratch/installer"
  flat="$scratch/installer.flat"
  files="$scratch/installer-files"
  git -C "$repo" grep -l -F '<key>Label</key>' "$rev" \
    -- ':!Library' ':!*.md' ':!test' ':!scripts' ':!docs' >"$files" 2>/dev/null ||
    : >"$files" # no match is not a failure
  while IFS= read -r file; do
    [[ -n $file ]] || continue
    file="${file#*:}"
    git -C "$repo" show "$rev:$file" >"$blob" 2>/dev/null || continue
    grep -q 'launchctl bootstrap' "$blob" || continue
    if grep -q 'launchctl bootstrap system' "$blob"; then
      domain='system'
    else
      domain="gui/$(id -u)"
    fi
    flatten_xml <"$blob" >"$flat"
    awk -v domain="$domain" '
      /^<key>Label<\/key>$/ {
        if ((getline value) > 0 && value ~ /^<string>.*<\/string>$/) {
          gsub(/^<string>|<\/string>$/, "", value)
          print value "\t" domain
        }
      }' "$flat"
  done <"$files"
}

# Gate 1, step 2. The per-profile config.yaml enablement/platform_toolsets and
# the codegraph MCP state are otherwise-untracked encrypted files, so they are
# what the backup captures, under the backup convention.
hermes_backup() {
  local src="$HOME/.hermes" stamp dest rel found=0
  [[ -d $src ]] ||
    die "no $src to back up; gate 1 captures Hermes profile state before anything moves"
  stamp="$(date -u +%Y-%m-%dT%H-%M-%S)"
  dest="$HOME/workspaces/backups/$stamp.hermes-profiles.backup"
  mkdir -p "$dest"
  while IFS= read -r file; do
    [[ -n $file ]] || continue
    rel="${file#"$src"/}"
    mkdir -p "$dest/$(dirname "$rel")"
    cp -p "$file" "$dest/$rel" || die "could not back up $file"
    found=$((found + 1))
  done < <(find "$src" -maxdepth 3 -type f \( -name 'config.yaml' -o -name '*.age' \) 2>/dev/null)
  [[ $found -gt 0 ]] || die "found no Hermes profile state under $src to back up"
  printf '%s\n' "$dest" >"$LEDGER/hermes-backup.path"
  ok "Hermes profile state backed up: $dest ($found file(s))"
}

# Gate 1, step 3. Pins come from the freshly-fetched REMOTE-tracking refs;
# local branch refs lag the remote and would describe a different revision from
# the one gate 2 activates.
record_pins() {
  local main_sha int_sha
  main_sha="$(git -C "$repo" rev-parse "origin/main")" ||
    die "cannot resolve origin/main"
  int_sha="$(git -C "$repo" rev-parse "origin/$INT_BRANCH")" ||
    die "cannot resolve origin/$INT_BRANCH"
  valid_sha "$main_sha" || die "origin/main did not resolve to a full 40-hex SHA: '$main_sha'"
  valid_sha "$int_sha" || die "origin/$INT_BRANCH did not resolve to a full 40-hex SHA: '$int_sha'"
  MAIN_SHA="$main_sha"
  INT_SHA="$int_sha"
  printf 'MAIN_SHA=%s\nINT_SHA=%s\n' "$MAIN_SHA" "$INT_SHA" >"$LEDGER/pins.env"
  ok "pins recorded LAST, from origin: MAIN_SHA=$MAIN_SHA INT_SHA=$INT_SHA"
}

# Gate 1, step 5. Checklist items 2 and 3: the manifest is the IMMUTABLE
# recorded-base -> pinned-integration delta, and every hunk in it is classified
# against the pinned main. Only `missing` blocks.
#
# A path whose tree ENTRY (mode and object, not just content) is identical at
# the pinned main classifies itself. Every other path needs an operator row in
# delta-classification.tsv, four tab-separated fields:
#
#   <kind><TAB><path><TAB><entry-pair><TAB><reason>
#
# where kind is intentionally-improved or deliberately-omitted-with-reason and
# entry-pair is the exact `<int-entry>|<main-entry>` string the runner prints in
# delta-unclassified.tsv. Binding the row to that pair is what stops one
# classification from covering a file forever: change either side and the row no
# longer matches, so the path returns to `missing` and must be re-reviewed.
# Comparing tree ENTRIES rather than blob ids also catches a mode or type change
# (a 100644 that should have become 100755, a file that should have become a
# symlink) that content equality alone reads as landed.
build_delta_ledger() {
  valid_sha "$PHASE_A_BASE" || die "the Phase A base '$PHASE_A_BASE' is not a full 40-hex SHA"
  git -C "$repo" cat-file -e "$PHASE_A_BASE^{commit}" 2>/dev/null ||
    die "the recorded Phase A base $PHASE_A_BASE is not a commit in $repo"
  # --no-renames keeps BOTH sides of a rename in the manifest. With rename
  # detection on, only the destination is listed, so a source-side deletion main
  # failed to make would never be classified at all.
  git -C "$repo" diff --no-renames "$PHASE_A_BASE" "$INT_SHA" >"$LEDGER/expected-delta.diff" ||
    die "could not regenerate the expected-delta manifest"

  # The path list is materialized through a CHECKED command. Reading it straight
  # from a process substitution discards git's exit status, so a transient
  # failure would present as a manifest with zero paths and a clean ledger.
  # -z keeps non-ASCII and control-character pathnames verbatim; git C-quotes
  # them otherwise and the quoted string resolves to nothing on both sides,
  # which compares equal and reads as landed-unchanged.
  local paths="$scratch/delta-paths"
  git -C "$repo" diff --no-renames --name-only -z "$PHASE_A_BASE" "$INT_SHA" >"$paths" ||
    die "could not enumerate the expected-delta manifest paths"

  local classification="$LEDGER/delta-classification.tsv"
  local ledger="$LEDGER/delta-ledger.tsv" missing="$LEDGER/delta-missing.tsv"
  local unclassified="$LEDGER/delta-unclassified.tsv"
  local path int_entry main_entry pair recorded kind reason
  : >"$ledger"
  : >"$missing"
  : >"$unclassified"
  while IFS= read -r -d '' path; do
    [[ -n $path ]] || continue
    int_entry="$(tree_entry "$INT_SHA" "$path")"
    main_entry="$(tree_entry "$MAIN_SHA" "$path")"
    if [[ -z $int_entry && -z $main_entry ]]; then
      # BOTH SIDES EMPTY IS TWO DIFFERENT SITUATIONS, and the first version of
      # this branch conflated them. The manifest is `git diff PHASE_A_BASE
      # INT_SHA`, so a path integration DELETED is in the manifest yet absent at
      # INT_SHA. When main deleted it too, both lookups come back empty and the
      # two branches AGREE in the strongest way available: the file is gone from
      # both. Measured on dresden 2026-08-05, 92 of the 201 remaining blockers
      # were exactly this (the tmux helpers, the retired Claude LaunchAgent, the
      # sesh configs, the retired skills), and demanding a written reason for
      # each would have meant asserting a decision for 92 non-events.
      #
      # The discriminator is the base: a path that existed at PHASE_A_BASE and
      # is gone from both pins was deleted deliberately, twice. A path absent
      # from the base as well has no business being in a diff against the base,
      # so that one is still a real anomaly and still refuses.
      if git -C "$repo" cat-file -e "$PHASE_A_BASE:$path" 2>/dev/null; then
        printf 'landed-unchanged\t%s\tdeleted at the pinned integration and at the pinned main, so both sides agree it is gone\n' \
          "$path" >>"$ledger"
      else
        printf 'missing\t%s\tabsent from the base and from both pins, so the manifest entry itself is unexplained\n' \
          "$path" >>"$missing"
      fi
      continue
    fi
    if [[ $int_entry == "$main_entry" ]]; then
      printf 'landed-unchanged\t%s\tidentical tree entry at the pinned main\n' "$path" >>"$ledger"
      continue
    fi
    pair="$int_entry|$main_entry"
    recorded=''
    if [[ -f $classification ]]; then
      recorded="$(awk -F'\t' -v p="$path" -v pair="$pair" \
        '$2 == p && $3 == pair {print; exit}' "$classification")"
    fi
    kind="$(printf '%s' "$recorded" | cut -f1)"
    reason="$(printf '%s' "$recorded" | cut -f4)"
    case "$kind" in
      intentionally-improved | deliberately-omitted-with-reason)
        if [[ -z $reason ]]; then
          printf 'missing\t%s\tclassified %s with no reason given\n' "$path" "$kind" >>"$missing"
        else
          printf '%s\t%s\t%s\n' "$kind" "$path" "$reason" >>"$ledger"
        fi
        ;;
      *)
        printf '<kind>\t%s\t%s\t<reason>\n' "$path" "$pair" >>"$unclassified"
        printf 'missing\t%s\tdiffers from the pinned main and is unclassified for this exact pair (%s)\n' \
          "$path" "$pair" >>"$missing"
        ;;
    esac
  done <"$paths"

  if [[ -s $missing ]]; then
    cat "$missing" >>"$ledger"
    die "$(wc -l <"$missing" | tr -d ' ') manifest hunk(s) classify as MISSING and block the cutover. Land them, or classify each in $classification, one tab-separated row per path, using the prefilled rows in $unclassified (kind is intentionally-improved or deliberately-omitted-with-reason; the third field must stay the exact entry pair):"$'\n'"$(cut -f2 "$missing")"
  fi
  ok "expected-delta ledger: $(wc -l <"$ledger" | tr -d ' ') hunk(s) classified, none missing"
}

# Gate 1, step 4a: the desired-state set, every launchd job the pinned main
# renders, each a (label, domain, steady-state predicate) triple.
derive_desired_services() {
  local out="$LEDGER/desired-services.tsv" path flat label domain predicate
  : >"$out"
  flat="$scratch/plist.flat"
  local plists="$scratch/desired-plists"
  git -C "$repo" ls-tree -r --name-only "$MAIN_SHA" -- \
    Library/LaunchAgents Library/LaunchDaemons >"$plists" ||
    die "could not list the tracked launchd plists at $MAIN_SHA"
  while IFS= read -r path; do
    [[ -n $path ]] || continue
    git -C "$repo" show "$MAIN_SHA:$path" | flatten_xml >"$flat"
    label="$(label_from_path "$path")"
    domain="$(domain_from_path "$path")"
    predicate="$(predicate_of_plist "$flat")"
    printf '%s\t%s\t%s\n' "$label" "$domain" "$predicate" >>"$out"
  done <"$plists"
  # Script-rendered jobs (the nix-hook heredoc) carry no tracked plist. Their
  # KeepAlive dictionary makes them conditional.
  while IFS=$'\t' read -r label domain; do
    [[ -n $label ]] || continue
    printf '%s\t%s\tconditional\n' "$label" "$domain" >>"$out"
  done < <(installer_labels_at "$MAIN_SHA")
  sort -u -o "$out" "$out"
  ok "desired-state set: $(wc -l <"$out" | tr -d ' ') (label, domain, predicate) triple(s)"
}

# Gate 1, step 4b. Checklist item 12: enumerate PER DOMAIN, user and system;
# `launchctl list` reads only the caller's bootstrap context.
enumerate_loaded_services() {
  local out="$LEDGER/loaded-services.tsv" domain
  : >"$out"
  for domain in "gui/$(id -u)" system; do
    launchctl print "$domain" 2>/dev/null |
      awk -v domain="$domain" '
        /^\tservices = \{$/ { inblock = 1; next }
        inblock && /^\t\}$/ { inblock = 0; next }
        inblock && NF > 0 { print $NF "\t" domain }' >>"$out" ||
      die "could not enumerate launchd domain $domain"
  done
  sort -u -o "$out" "$out"
  ok "live loaded set: $(wc -l <"$out" | tr -d ' ') service(s) across gui/$(id -u) and system"
}

# Gate 1, step 4c. Checklist item 13: the retirement candidate universe is the
# EXACT, history-derived inventory of every label this repo has ever rendered.
# A "com.webdavis.*" prefix match is wrong: history holds out-of-prefix labels.
# --root is included so labels added in the root commit are not invisible.
derive_label_universe() {
  local out="$LEDGER/managed-label-universe.tsv" path commit label domain
  local history="$scratch/universe-paths" commits="$scratch/universe-commits"
  : >"$out"
  {
    git -C "$repo" log --all --root --diff-filter=AD --name-status \
      -- 'Library/LaunchAgents/*' 'Library/LaunchDaemons/*' ||
      die "could not walk the add/delete history of the launchd sources"
    git -C "$repo" log --all --root --diff-filter=R -M --name-status \
      -- 'Library/LaunchAgents/*' 'Library/LaunchDaemons/*' ||
      die "could not walk the rename history of the launchd sources"
  } | awk -F'\t' '/^[ADR]/ { for (i = 2; i <= NF; i++) if ($i != "") print $i }' >"$history"
  while IFS= read -r path; do
    [[ -n $path ]] || continue
    printf '%s\t%s\n' "$(label_from_path "$path")" "$(domain_from_path "$path")" >>"$out"
  done <"$history"
  git -C "$repo" log --all --root --format=%H -S '<key>Label</key>' \
    -- ':!Library' ':!*.md' ':!test' ':!scripts' ':!docs' >"$commits" ||
    die "could not walk the history of script-rendered launchd labels"
  while IFS= read -r commit; do
    [[ -n $commit ]] || continue
    installer_labels_at "$commit" >>"$out"
  done <"$commits"
  # Currently-rendered labels are, by definition, part of "ever rendered".
  cut -f1,2 "$LEDGER/desired-services.tsv" >>"$out"
  sort -u -o "$out" "$out"
  ok "managed-label universe: $(wc -l <"$out" | tr -d ' ') label(s) derived from repository history"
}

# Membership is by (label, domain) PAIR, not label alone. A label that moved
# between domains appears in both inventories, and matching on the label alone
# would let the stale copy in the old domain pass as desired: it is loaded in
# gui/<uid>, the source now renders it in system, and the retirement that should
# have caught the old instance never fires.
pair_listed() {
  local label="$1" domain="$2" file="$3"
  cut -f1,2 "$file" | grep -qxF -- "$label	$domain"
}

is_preserved() {
  local label="$1" keep
  case "$label" in
    com.apple.*) return 0 ;;
  esac
  for keep in "${PRESERVE_LABELS[@]}"; do
    [[ $label == "$keep" ]] && return 0
  done
  return 1
}

# Gate 1, step 4d: the retirement list is live jobs absent from the desired set,
# computed ONLY within the managed-label universe, never touching the preserve
# list of package/OS-owned services.
compute_retirement() {
  local out="${1:-$LEDGER/retirement-derived.tsv}" label domain
  : >"$out"
  while IFS=$'\t' read -r label domain; do
    [[ -n $label ]] || continue
    pair_listed "$label" "$domain" "$LEDGER/desired-services.tsv" && continue
    is_preserved "$label" && continue
    pair_listed "$label" "$domain" "$LEDGER/managed-label-universe.tsv" || continue
    printf '%s\t%s\n' "$label" "$domain" >>"$out"
  done <"$LEDGER/loaded-services.tsv"
  sort -u -o "$out" "$out"
}

# Checklist item 7's sibling for the manifest: an approved file is only as good
# as its content at execution time. Approval copies the just-derived list, and
# both gate 2 and gate 3 re-check every approved row against the ledger's own
# desired set, universe and preserve list before acting on it, so an edit made
# after the operator read it cannot smuggle a live service into the retirement.
validate_approved_manifest() {
  local approved="$LEDGER/retirement-approved.tsv" label domain rows=0
  [[ -f $approved ]] ||
    die "no approved retirement manifest at $approved; gate 1's checkpoint has not been completed"
  while IFS=$'\t' read -r label domain; do
    [[ -n $label ]] || continue
    rows=$((rows + 1))
    [[ -n $domain ]] ||
      die "approved retirement row '$label' has no launchd domain; every entry is a (label, domain) pair"
    ! pair_listed "$label" "$domain" "$LEDGER/desired-services.tsv" ||
      die "approved retirement row $domain/$label is in the DESIRED set; the pinned source renders it, so retiring it would tear down live state"
    ! is_preserved "$label" ||
      die "approved retirement row $domain/$label is on the preserve list of package or OS-owned services"
    pair_listed "$label" "$domain" "$LEDGER/managed-label-universe.tsv" ||
      die "approved retirement row $domain/$label is outside the managed-label universe; this repository has never rendered it"
  done <"$approved"
  ok "approved retirement manifest re-validated against the ledger ($rows row(s))"
}

gate1() {
  say "gate 1, preflight. This will:"
  say "  - require a fully-visible-clean tree (no dirty, no untracked, no graphify-out)"
  say "  - require the rendered data view to agree with the tracked data file"
  say "  - back up Hermes profile state under the backup convention"
  say "  - fetch, then pin origin/main and origin/$INT_BRANCH LAST"
  say "  - rebuild the expected-delta manifest from $PHASE_A_BASE and classify every hunk"
  say "  - build the retirement manifest and stop for your approval"
  say ''

  begin_gate 1
  require_clean_tree
  require_render_matches_source
  hermes_backup
  fetch_guarded
  record_pins
  build_delta_ledger
  derive_desired_services
  enumerate_loaded_services
  derive_label_universe
  compute_retirement

  local derived="$LEDGER/retirement-derived.tsv"
  local proposed="$LEDGER/retirement-proposed.tsv"
  local approved="$LEDGER/retirement-approved.tsv"

  if [[ $APPROVE_RETIREMENT -eq 1 ]]; then
    [[ -f $proposed ]] ||
      die "there is no reviewed retirement manifest; run 'cutover-gate.sh 1' first and read $proposed"
    cmp -s "$derived" "$proposed" ||
      die "the retirement manifest CHANGED since you reviewed it; approval reviews a correct manifest, it does not repair a wrong one. Re-run 'cutover-gate.sh 1' and read it again"
    # Copy the JUST-DERIVED list, not the proposal. They are equal by the check
    # above, and copying the derived one closes the window in which a concurrent
    # writer replaces the proposal between the compare and the copy.
    cp "$derived" "$approved"
    validate_approved_manifest
    : >"$LEDGER/gate1.done"
    ok "retirement manifest approved: $(wc -l <"$approved" | tr -d ' ') label(s)"
    say ''
    say "GATE 1 PASSED. Next: cutover-gate.sh 2 --second-session-open"
    return 0
  fi

  cp "$derived" "$proposed"
  say ''
  say "Retirement manifest (live jobs inside the managed-label universe that the"
  say "pinned main does not render). Every line will be booted out at gate 2:"
  if [[ -s $proposed ]]; then
    while IFS=$'\t' read -r label domain; do
      say "    $domain/$label"
    done <"$proposed"
  else
    say "    (none)"
  fi
  say ''
  say "Read $proposed. It is correct or it is not; approval is a review"
  say "checkpoint, not a repair mechanism."
  checkpoint "approve with: cutover-gate.sh 1 --approve-retirement"
}
# Checklist item 15: every probe is domain-qualified, one label at a time, and
# TRI-STATE. launchctl answers "no such service" with 113; every other non-zero
# status is an operational error (64 for a malformed domain, 112 for a GUI
# domain it cannot reach, all verified on this host). Collapsing those into
# "absent" is how a still-loaded service reads as retired, so an unknown status
# refuses instead of guessing.
#
#   service_loaded <domain> <label>   0 = loaded, 1 = confirmed absent, dies otherwise
service_loaded() {
  local status=0
  launchctl print "$1/$2" >/dev/null 2>&1 || status=$?
  case "$status" in
    0) return 0 ;;
    113) return 1 ;;
    *) die "launchctl print $1/$2 failed with status $status, which is not the not-found status (113); the load state of $1/$2 is UNKNOWN and must not be read as absence" ;;
  esac
}

# Gate 2, stage 2: retire exactly the approved manifest, nothing discovered
# mid-apply. A label already absent is a no-op, not a failure.
execute_approved_retirement() {
  local approved="$LEDGER/retirement-approved.tsv" label domain retired=0
  validate_approved_manifest
  while IFS=$'\t' read -r label domain; do
    [[ -n $label ]] || continue
    if ! service_loaded "$domain" "$label"; then
      ok "$domain/$label already absent"
      continue
    fi
    launchctl bootout "$domain/$label" ||
      die "could not bootout $domain/$label; the approved retirement did not complete"
    ! service_loaded "$domain" "$label" ||
      die "$domain/$label is still loaded after bootout; a deleted plist does not unload a running job"
    ok "retired $domain/$label"
    retired=$((retired + 1))
  done <"$approved"
  ok "approved retirement executed: $retired label(s) booted out"
}

# Gate 2, stage 2: a broken apply must not be discovered after the original
# session is gone.
verify_remote_reachability() {
  command -v tailscale >/dev/null 2>&1 ||
    die "tailscale is not on PATH; remote reachability cannot be verified before ending this session"
  tailscale status >/dev/null 2>&1 ||
    die "tailscale status failed; do not end the original session until remote reachability is restored"
  ok "Tailscale reachable"
  service_loaded system com.openssh.sshd ||
    die "system/com.openssh.sshd is not loaded; the SSH fallback into this machine is gone"
  ok "sshd loaded in the system domain"
}

gate2() {
  if [[ $POST_APPLY -eq 1 ]]; then
    [[ $SECOND_SESSION_OPEN -eq 0 ]] ||
      die "--post-apply is the stage AFTER the staged apply; run it on its own"
    say "gate 2, post-apply. This will:"
    say "  - re-verify both pins and the attached checkout"
    say "  - require evidence that the staged apply actually converged"
    say "  - boot out exactly the retirement manifest approved at gate 1"
    say "  - verify Tailscale and sshd reachability before you end this session"
    say ''
    [[ -f "$LEDGER/gate2.landed" ]] ||
      die "the activation landing stage has not run; start with 'cutover-gate.sh 2 --second-session-open'"
    begin_gate 2-post-apply
    load_pins
    # The pins were last checked BEFORE a manual step of unbounded duration.
    # Dependabot auto-merges into main during exactly that window, and retiring
    # services against a pin that has moved retires them for a revision this
    # machine is not running.
    fetch_guarded
    require_pins_unmoved
    require_attached_at_pin
    require_apply_converged
    execute_approved_retirement
    verify_remote_reachability
    : >"$LEDGER/gate2.done"
    say ''
    say "GATE 2 PASSED. Next: cutover-gate.sh 3"
    return 0
  fi

  say "gate 2, staged activation. This will:"
  say "  - re-verify both pins fail-closed against the freshly fetched remote"
  say "  - re-check the clean tree immediately before the apply"
  say "  - land the checkout ATTACHED to main at the pinned SHA"
  say "  - stop so you can run the staged interactive chezmoi apply yourself"
  say ''

  [[ $SECOND_SESSION_OPEN -eq 1 ]] ||
    die "open a second remote session first, so a broken apply cannot lock you out, then re-run with --second-session-open"
  require_gate_passed 1
  begin_gate 2-landing
  load_pins
  fetch_guarded
  require_pins_unmoved
  require_clean_tree

  git -C "$repo" checkout main ||
    die "could not check out main"
  git -C "$repo" merge --ff-only "$MAIN_SHA" ||
    die "main could not be fast-forwarded to $MAIN_SHA"
  require_attached_at_pin
  # The working tree just changed under chezmoi, and the data-only render is a
  # property of the tree, not of the branch: gate 1 proved agreement for the
  # PREVIOUS checkout. Re-prove it here, while the operator can still stop,
  # rather than let a nested stale copy decide what the staged apply installs.
  require_clean_tree
  require_render_matches_source
  : >"$LEDGER/gate2.landed"

  say ''
  say "The live source is now main at $MAIN_SHA."
  say "Run the full interactive chezmoi apply yourself, in stages, with KeePassXC"
  say "unlocked, keeping the integration branch and the previously deployed files"
  say "available for rollback. This runner never performs that apply."
  checkpoint "when the staged apply is done, run: cutover-gate.sh 2 --post-apply"
}
# Checklist item 18: the reconcile tool is the already-merged, pinned one in the
# checkout, run by absolute path, dry-run before live. It is never authored ad
# hoc during the cutover.
run_live_reconcile() {
  local tool="$repo/scripts/live-reconcile.sh"
  [[ -x $tool ]] ||
    die "$tool is missing or not executable; gate 3 runs the merged reconcile tool, it does not author one"
  # Checking HEAD proves nothing about a working-tree file: the dry run and the
  # live run are two invocations of a path, and an edit between them means the
  # code that was proven clean is not the code that mutates the machine. Both
  # invocations are gated on the file still being byte-identical to the blob at
  # the pinned commit, which is the reviewed one.
  require_pinned_tool "$tool" 'scripts/live-reconcile.sh'
  "$tool" --dry-run ||
    die "the reconcile dry run failed; nothing is reconciled live until the dry run is clean"
  ok "reconcile dry run clean"
  require_pinned_tool "$tool" 'scripts/live-reconcile.sh'
  "$tool" ||
    die "the live reconcile failed"
  ok "live reconcile converged"
}

# require_pinned_tool <working-tree path> <path in the pinned tree>
require_pinned_tool() {
  local file="$1" tracked="$2" pinned="$scratch/pinned-tool"
  git -C "$repo" show "$MAIN_SHA:$tracked" >"$pinned" ||
    die "$tracked does not exist at the pinned commit $MAIN_SHA"
  cmp -s "$file" "$pinned" ||
    die "$file differs from its content at the pinned commit $MAIN_SHA; gate 3 runs the reviewed, pinned tool, never a working-tree edit"
}

# Checklist items 14 and 15: probe every manifest entry domain-qualified and
# hold it to ITS recorded predicate. A blanket "running" check would wrongly
# fail the one-shots and the conditional-KeepAlive nix-hook, and a before/after
# diff can never prove a retirement, because a loaded job outlives its plist.
verify_against_manifest() {
  local approved="$LEDGER/retirement-approved.tsv"
  local desired="$LEDGER/desired-services.tsv"
  local label domain predicate body="$scratch/print"
  validate_approved_manifest
  while IFS=$'\t' read -r label domain; do
    [[ -n $label ]] || continue
    ! service_loaded "$domain" "$label" ||
      die "$domain/$label is still loaded; it was approved for retirement and a deleted plist does not unload a running job"
  done <"$approved"
  ok "every approved-retired label is absent"

  # The approved list is a SNAPSHOT taken at gate 1. A historical plist that
  # loaded after approval (a reboot, a login item, a stray bootstrap) is not in
  # it and would otherwise ride through closure untouched, which is the exact
  # loaded-orphan class this whole manifest exists to catch. Recompute the
  # extras from the live domains and refuse on any.
  local extras="$scratch/retirement-extras"
  enumerate_loaded_services
  compute_retirement "$extras"
  [[ ! -s $extras ]] ||
    die "managed labels are loaded that the pinned source does not render and nobody approved for retirement; they appeared after gate 1's snapshot and must be reviewed:"$'\n'"$(sed 's|^\([^	]*\)	\(.*\)$|  \2/\1|' "$extras")"
  ok "no unapproved managed labels are loaded"

  while IFS=$'\t' read -r label domain predicate; do
    [[ -n $label ]] || continue
    launchctl print "$domain/$label" >"$body" 2>/dev/null ||
      die "$domain/$label is not loaded, but the pinned source renders it"
    case "$predicate" in
      persistent)
        if ! grep -q 'state = running' "$body" || ! grep -q 'pid = ' "$body"; then
          die "$domain/$label is a persistent (KeepAlive=true) job but is not running"
        fi
        ;;
      conditional)
        grep -q 'last exit code = 0' "$body" ||
          die "$domain/$label is a conditional-KeepAlive job whose last exit was not clean"
        ;;
      scheduled)
        grep -qE 'run interval = |event triggers = \{' "$body" ||
          die "$domain/$label is a scheduled job with no registered trigger"
        ;;
    esac
  done <"$desired"
  ok "every desired label satisfies its recorded steady-state predicate"
}

# Checklist item 6: non-git commands run under a guarded cd, because `git -C`
# does not change the cwd for anything else.
run_repo_test_suite() {
  (
    cd "$repo" || exit 1
    just test
  ) || die "the repository test suite is red on the activated source"
  ok "repository test suite green"
}

# Source-to-target convergence, over EVERY managed entry.
#
# `--exclude=templates` was the obvious reading of the plan's "excluding
# KeePassXC-gated templates", and it is the wrong one: the flag excludes entry
# TYPES, so it drops every templated target, secret or not. The Homebrew
# weekly-upgrade plist is a template; a deleted one would go unreported while
# launchctl still shows the already-loaded job as healthy, and the job would
# vanish at the next logout. The KeePassXC templates are not a problem to
# exclude at this point in the procedure either: the operator has just run the
# staged apply with the database unlocked, so a full status can render them.
chezmoi_drift() {
  local drift
  drift="$(
    cd "$repo" || exit 1
    chezmoi status
  )" || die "chezmoi status failed; if it could not render the KeePassXC-gated templates, unlock the database and re-run this gate rather than narrowing the check"
  printf '%s' "$drift"
}

# Gate 2, stage 2. The staged apply is a manual step this runner deliberately
# does not perform, which also means it has no idea whether it happened. Without
# evidence, --post-apply retires services and records a pass for an apply that
# failed halfway (a brew bundle that died on an unavailable formula) or never
# ran at all. Convergence over every managed entry IS that evidence.
require_apply_converged() {
  local drift
  drift="$(chezmoi_drift)"
  [[ -z $drift ]] ||
    die "chezmoi still reports source-to-target drift, so the staged apply did not converge. Finish or repair the apply before anything is retired:"$'\n'"$drift"
  ok "the staged apply converged: no source-to-target drift over any managed entry"
}

# The live smoke set: notifications, the hermes gateway, the osquery pipeline,
# and source-to-target convergence. Gate 4 re-runs it at the end of the soak.
run_smoke_checks() {
  local note="$1" relay="$HOME/.local/bin/relay.sh"
  local heartbeat="$HOME/.local/libexec/osquery/heartbeat.sh"
  local freshness="$HOME/.local/libexec/osquery/canary-freshness.sh"
  local drift now canary age max_age

  [[ -x $relay ]] || die "$relay is missing; notifications cannot be proven to work"
  "$relay" --agent cutover-gate --state 'done' --project cutover --detail "$note" ||
    die "relay could not fire a test notification"
  ok "relay fired a test notification"

  command -v hermes >/dev/null 2>&1 || die "hermes is not on PATH"
  hermes gateway status >/dev/null 2>&1 || die "the hermes gateway is not healthy"
  ok "hermes gateway healthy"

  # The heartbeat is fired because the plan's smoke set says to fire it, but its
  # exit status proves nothing: every branch ends in `send_alert ... || true`,
  # so it exits 0 for healthy, missing, stale, future-dated and clock-error
  # alike. The verdict it would have sent is the canary's freshness, so that is
  # what gets asserted, through the same canary-freshness.sh seam the heartbeat
  # and the uptime watchdog already share.
  [[ -x $heartbeat ]] || die "$heartbeat is missing; the osquery heartbeat cannot be fired"
  "$heartbeat" || die "the osquery heartbeat failed to run"
  [[ -r $freshness ]] ||
    die "$freshness is missing, so the heartbeat's own verdict cannot be checked and a silent unhealthy heartbeat would read as a pass"
  # shellcheck source=/dev/null
  source "$freshness"
  max_age="${OSQUERY_CANARY_MAX_AGE:-1800}"
  [[ $max_age =~ ^[0-9]+$ ]] || max_age=1800
  canary="$(newest_canary_timestamp || true)"
  [[ $canary =~ ^[0-9]+$ ]] ||
    die "osqueryd has produced no scheduled heartbeat canary, so the heartbeat just sent an unhealthy record; the pipeline is not proven alive"
  now="$(date +%s)"
  age=$((now - canary))
  [[ $age -le $max_age && $age -ge -$max_age ]] ||
    die "the osqueryd heartbeat canary is ${age}s old against a ${max_age}s bound, so the daemon is not producing scheduled results"
  ok "osquery heartbeat sent and its canary is fresh (${age}s)"

  drift="$(chezmoi_drift)"
  [[ -z $drift ]] ||
    die "chezmoi reports source-to-target drift:"$'\n'"$drift"
  ok "no source-to-target drift over any managed entry"
}

gate3() {
  say "gate 3, reconciliation and verification. This will:"
  say "  - run $repo/scripts/live-reconcile.sh, dry-run then live"
  say "  - probe every manifest entry domain-qualified against its predicate"
  say "  - run the repository test suite and the live smoke checks"
  say ''

  require_gate_passed 2
  begin_gate 3
  load_pins
  require_attached_at_pin
  run_live_reconcile
  verify_against_manifest
  run_repo_test_suite
  run_smoke_checks 'gate 3 live smoke check'
  : >"$LEDGER/gate3.done"
  say ''
  say "GATE 3 PASSED. The soak starts now. Next: cutover-gate.sh 4"
}

# mtime <file> : seconds since the epoch.
#
# GNU coreutils and BSD stat take mutually invalid flags, and GNU's failure mode
# is not a clean one: its -f is --file-system, so `stat -f %m FILE` fails on the
# format operand while STILL printing a human-readable block for FILE. Taking
# either tool's output on faith is how a soak clock reads "  File: ..." as an
# epoch. GNU's -c is tried first, BSD's -f second, and the result must be
# digits or this refuses rather than guessing.
mtime() {
  local file="$1" value
  value="$(stat -c %Y "$file" 2>/dev/null || true)"
  if [[ ! $value =~ ^[0-9]+$ ]]; then
    value="$(stat -f %m "$file" 2>/dev/null || true)"
  fi
  [[ $value =~ ^[0-9]+$ ]] ||
    die "could not read a modification time for $file; stat returned '${value:-}'"
  printf '%s' "$value"
}

gate4() {
  say "gate 4, soak. This will:"
  say "  - measure the soak window from the gate 3 pass"
  say "  - re-verify the launchd topology and re-probe the daily-critical paths"
  say "    once the window has elapsed"
  say ''

  require_gate_passed 3
  begin_gate 4
  load_pins
  require_attached_at_pin

  local started now elapsed remaining
  started="$(mtime "$LEDGER/gate3.done")"
  now="$(date +%s)"
  elapsed=$(((now - started) / 3600))
  if [[ $elapsed -lt $((10#$WINDOW_HOURS)) ]]; then
    remaining=$((10#$WINDOW_HOURS - elapsed))
    say "soaked ${elapsed}h of ${WINDOW_HOURS}h. Watch notifications, hermes, osquery"
    say "and shell startup for regressions, and close no reference PR yet."
    checkpoint "${remaining}h remaining; re-run cutover-gate.sh 4 after that"
  fi
  ok "soak window elapsed: ${elapsed}h of ${WINDOW_HOURS}h"
  # The soak is where a service quietly dies. The smoke set does not look at
  # launchd at all, so a job that was unloaded during the window (its plist
  # untouched, so chezmoi stays clean) would ride through gate 5 unnoticed and
  # the cutover would close with it disabled. The topology soaked has to be the
  # topology verified, so this is the same check gate 3 ran, at the other end of
  # the window.
  verify_against_manifest
  run_smoke_checks 'gate 4 end-of-soak check'
  : >"$LEDGER/gate4.done"
  say ''
  say "GATE 4 PASSED. Next: cutover-gate.sh 5"
}

gate5() {
  say "gate 5, closure. This will:"
  say "  - re-verify both pins and the attached checkout in this fresh shell"
  say "  - close PRs ${REFERENCE_PRS[*]} against $GH_TARGET explicitly"
  say "  - mutate nothing in the repository"
  say ''

  require_gate_passed 4
  begin_gate 5
  load_pins
  fetch_guarded
  require_pins_unmoved
  require_attached_at_pin

  command -v gh >/dev/null 2>&1 || die "gh is not on PATH; the reference PRs cannot be closed"
  local pr comment
  comment='Closed by the D1 cutover: main now carries this work through the merged slice PRs, and the converged topology soaked before closure.'
  for pr in "${REFERENCE_PRS[@]}"; do
    # GH_REPO/GH_HOST are cleared as well as overridden, so an inherited
    # environment cannot reach the resolver at all.
    env -u GH_REPO -u GH_HOST gh pr close "$pr" --repo="$GH_TARGET" --comment "$comment" ||
      die "could not close PR #$pr against $GH_TARGET"
    ok "closed PR #$pr"
  done
  : >"$LEDGER/gate5.done"
  say ''
  say "GATE 5 PASSED. The cutover is complete."
}

# ---------------------------------------------------------------------------
# Entry
# ---------------------------------------------------------------------------

[[ $# -ge 1 ]] || usage
gate="$1"
shift

APPROVE_RETIREMENT=0
SECOND_SESSION_OPEN=0
POST_APPLY=0
WINDOW_HOURS="$SOAK_HOURS_DEFAULT"

# Per-gate options are parsed BEFORE any repository work: an unknown argument is
# usage to stderr and a non-zero exit, never a silent fallthrough.
case "$gate" in
  1)
    for arg in "$@"; do
      case "$arg" in
        --approve-retirement) APPROVE_RETIREMENT=1 ;;
        *) usage ;;
      esac
    done
    ;;
  2)
    for arg in "$@"; do
      case "$arg" in
        --second-session-open) SECOND_SESSION_OPEN=1 ;;
        --post-apply) POST_APPLY=1 ;;
        *) usage ;;
      esac
    done
    ;;
  3 | 5)
    [[ $# -eq 0 ]] || usage
    ;;
  4)
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --window-hours)
          [[ $# -ge 2 ]] || usage
          WINDOW_HOURS="$2"
          # the regex is checked FIRST, so the arithmetic never sees a
          # non-numeric argument
          # zero is legal: it means "re-verify now, no wait" (the default)
          if ! [[ $WINDOW_HOURS =~ ^[0-9]+$ ]]; then
            usage
          fi
          shift 2
          ;;
        *) usage ;;
      esac
    done
    ;;
  *) usage ;;
esac

require_repo
mkdir -p "$LEDGER"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
require_base_override_is_sandbox_only
"gate$gate"
