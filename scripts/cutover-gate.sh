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
# and no stored file count is normative. The env override exists for the test
# suite, which builds sandbox repositories that cannot contain this commit.
PHASE_A_BASE="${CUTOVER_PHASE_A_BASE:-2bd973369158b49535e8e16e80c968444ab23f1d}"

LEDGER="$HOME/.local/state/cutover"
INT_BRANCH="integration/modernization"

# Checklist item 17: gh's resolver precedence is --repo > GH_REPO > cwd remote,
# so the repository is named explicitly, host included, in the documented
# [HOST/]OWNER/REPO form. (`gh pr close` has no --hostname flag; the host
# travels in --repo, verified against `gh pr close --help`.)
GH_TARGET="github.com/webdavis/dotfiles"
REFERENCE_PRS=(25 31 32)

# Gate 4's soak window. Gate 5 "runs days after Gate 1", so three days is the
# default; --window-hours overrides it.
SOAK_HOURS_DEFAULT=72

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
  printf '  4 [--window-hours <n>]    soak the final topology (default %s hours)\n' "$SOAK_HOURS_DEFAULT" >&2
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

# Checklist item 8: a failed fetch must never let stale remote-tracking refs
# satisfy a later comparison.
fetch_guarded() {
  git -C "$repo" fetch origin ||
    die "git fetch origin failed; stale remote-tracking refs must never satisfy a pin check"
}

valid_sha() { [[ $1 =~ ^[0-9a-f]{40}$ ]]; }

# Checklist item 4: gitignored paths escape porcelain, so graphify-out/ is
# checked separately. Callers re-run this immediately before the apply.
require_clean_tree() {
  local dirty
  dirty="$(git -C "$repo" status --porcelain --untracked-files=all)"
  [[ -z $dirty ]] ||
    die "the tree is not clean; classify keep/discard/back-up and move kept files OUT of the source tree first:"$'\n'"$dirty"
  [[ ! -e "$repo/graphify-out" ]] ||
    die "$repo/graphify-out is present; it is gitignored, so it escapes a porcelain listing and would deploy unclassified content"
  ok "tree is clean, fully visible (no dirty, no untracked, no graphify-out residue)"
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
# <rev> installs. Markdown is excluded (documentation quotes plist keys), and a
# file only counts when it actually bootstraps a job, which is also where the
# domain comes from.
installer_labels_at() {
  local rev="$1" file blob flat domain
  blob="$scratch/installer"
  flat="$scratch/installer.flat"
  while IFS= read -r file; do
    [[ -n $file ]] || continue
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
  done < <(git -C "$repo" grep -l -F '<key>Label</key>' "$rev" -- ':!Library' ':!*.md' 2>/dev/null |
    sed 's|^[^:]*:||' || true)
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
# A hunk whose blob is identical at the pinned main classifies itself. Every
# other hunk needs an operator line in delta-classification.tsv, three
# tab-separated fields:
#
#   intentionally-improved<TAB><path><TAB><reason>
#   deliberately-omitted-with-reason<TAB><path><TAB><reason>
build_delta_ledger() {
  valid_sha "$PHASE_A_BASE" || die "the Phase A base '$PHASE_A_BASE' is not a full 40-hex SHA"
  git -C "$repo" cat-file -e "$PHASE_A_BASE^{commit}" 2>/dev/null ||
    die "the recorded Phase A base $PHASE_A_BASE is not a commit in $repo"
  git -C "$repo" diff "$PHASE_A_BASE" "$INT_SHA" >"$LEDGER/expected-delta.diff" ||
    die "could not regenerate the expected-delta manifest"

  local classification="$LEDGER/delta-classification.tsv"
  local ledger="$LEDGER/delta-ledger.tsv" missing="$LEDGER/delta-missing.tsv"
  local path int_blob main_blob recorded kind reason
  : >"$ledger"
  : >"$missing"
  while IFS= read -r path; do
    [[ -n $path ]] || continue
    int_blob="$(git -C "$repo" rev-parse --verify --quiet "$INT_SHA:$path" || true)"
    main_blob="$(git -C "$repo" rev-parse --verify --quiet "$MAIN_SHA:$path" || true)"
    if [[ $int_blob == "$main_blob" ]]; then
      printf 'landed-unchanged\t%s\tidentical blob at the pinned main\n' "$path" >>"$ledger"
      continue
    fi
    recorded=''
    if [[ -f $classification ]]; then
      recorded="$(awk -F'\t' -v p="$path" '$2 == p {print; exit}' "$classification")"
    fi
    kind="$(printf '%s' "$recorded" | cut -f1)"
    reason="$(printf '%s' "$recorded" | cut -f3)"
    case "$kind" in
      intentionally-improved | deliberately-omitted-with-reason)
        if [[ -z $reason ]]; then
          printf 'missing\t%s\tclassified %s with no reason given\n' "$path" "$kind" >>"$missing"
        else
          printf '%s\t%s\t%s\n' "$kind" "$path" "$reason" >>"$ledger"
        fi
        ;;
      *)
        printf 'missing\t%s\tdiffers from the pinned main and is unclassified\n' "$path" >>"$missing"
        ;;
    esac
  done < <(git -C "$repo" diff --name-only "$PHASE_A_BASE" "$INT_SHA")

  if [[ -s $missing ]]; then
    cat "$missing" >>"$ledger"
    die "$(wc -l <"$missing" | tr -d ' ') manifest hunk(s) classify as MISSING and block the cutover. Land them, or classify each in $classification as a tab-separated 'intentionally-improved|deliberately-omitted-with-reason<TAB>path<TAB>reason' line:"$'\n'"$(cut -f2 "$missing")"
  fi
  ok "expected-delta ledger: $(wc -l <"$ledger" | tr -d ' ') hunk(s) classified, none missing"
}

# Gate 1, step 4a: the desired-state set, every launchd job the pinned main
# renders, each a (label, domain, steady-state predicate) triple.
derive_desired_services() {
  local out="$LEDGER/desired-services.tsv" path flat label domain predicate
  : >"$out"
  flat="$scratch/plist.flat"
  while IFS= read -r path; do
    [[ -n $path ]] || continue
    git -C "$repo" show "$MAIN_SHA:$path" | flatten_xml >"$flat"
    label="$(label_from_path "$path")"
    domain="$(domain_from_path "$path")"
    predicate="$(predicate_of_plist "$flat")"
    printf '%s\t%s\t%s\n' "$label" "$domain" "$predicate" >>"$out"
  done < <(git -C "$repo" ls-tree -r --name-only "$MAIN_SHA" -- \
    Library/LaunchAgents Library/LaunchDaemons)
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
  : >"$out"
  while IFS= read -r path; do
    [[ -n $path ]] || continue
    printf '%s\t%s\n' "$(label_from_path "$path")" "$(domain_from_path "$path")" >>"$out"
  done < <({
    git -C "$repo" log --all --root --diff-filter=AD --name-status \
      -- 'Library/LaunchAgents/*' 'Library/LaunchDaemons/*'
    git -C "$repo" log --all --root --diff-filter=R -M --name-status \
      -- 'Library/LaunchAgents/*' 'Library/LaunchDaemons/*'
  } | awk -F'\t' '/^[ADR]/ { for (i = 2; i <= NF; i++) if ($i != "") print $i }')
  while IFS= read -r commit; do
    [[ -n $commit ]] || continue
    installer_labels_at "$commit" >>"$out"
  done < <(git -C "$repo" log --all --root --format=%H -S '<key>Label</key>' -- ':!Library' ':!*.md')
  # Currently-rendered labels are, by definition, part of "ever rendered".
  cut -f1,2 "$LEDGER/desired-services.tsv" >>"$out"
  sort -u -o "$out" "$out"
  ok "managed-label universe: $(wc -l <"$out" | tr -d ' ') label(s) derived from repository history"
}

label_listed() { cut -f1 "$2" | grep -qxF -- "$1"; }

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
  local out="$LEDGER/retirement-derived.tsv" label domain
  : >"$out"
  while IFS=$'\t' read -r label domain; do
    [[ -n $label ]] || continue
    label_listed "$label" "$LEDGER/desired-services.tsv" && continue
    is_preserved "$label" && continue
    label_listed "$label" "$LEDGER/managed-label-universe.tsv" || continue
    printf '%s\t%s\n' "$label" "$domain" >>"$out"
  done <"$LEDGER/loaded-services.tsv"
  sort -u -o "$out" "$out"
}

gate1() {
  say "gate 1, preflight. This will:"
  say "  - require a fully-visible-clean tree (no dirty, no untracked, no graphify-out)"
  say "  - back up Hermes profile state under the backup convention"
  say "  - fetch, then pin origin/main and origin/$INT_BRANCH LAST"
  say "  - rebuild the expected-delta manifest from $PHASE_A_BASE and classify every hunk"
  say "  - build the retirement manifest and stop for your approval"
  say ''

  require_clean_tree
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
    cp "$proposed" "$approved"
    : >"$LEDGER/gate1.done"
    ok "retirement manifest approved: $(wc -l <"$approved" | tr -d ' ') label(s)"
    say ''
    say "GATE 1 PASSED. Next: cutover-gate.sh 2 --second-session-open"
    return 0
  fi

  # Re-running gate 1 RESTARTS the procedure. The pins were just re-taken and
  # the manifest re-derived, so an earlier approval no longer reviewed what
  # gate 2 would execute, and any later gate that already passed did so against
  # the old pins.
  rm -f "$LEDGER/retirement-approved.tsv" "$LEDGER/gate1.done" \
    "$LEDGER/gate2.landed" "$LEDGER/gate2.done" "$LEDGER/gate3.done" \
    "$LEDGER/gate4.done" "$LEDGER/gate5.done"
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
# Checklist item 15: every probe is domain-qualified, one label at a time.
service_loaded() { launchctl print "$1/$2" >/dev/null 2>&1; }

# Gate 2, stage 2: retire exactly the approved manifest, nothing discovered
# mid-apply. A label already absent is a no-op, not a failure.
execute_approved_retirement() {
  local approved="$LEDGER/retirement-approved.tsv" label domain retired=0
  [[ -f $approved ]] ||
    die "no approved retirement manifest at $approved; gate 1's checkpoint has not been completed"
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
    say "  - re-verify the checkout is still attached to main at the pin"
    say "  - boot out exactly the retirement manifest approved at gate 1"
    say "  - verify Tailscale and sshd reachability before you end this session"
    say ''
    [[ -f "$LEDGER/gate2.landed" ]] ||
      die "the activation landing stage has not run; start with 'cutover-gate.sh 2 --second-session-open'"
    load_pins
    require_attached_at_pin
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
  load_pins
  fetch_guarded
  require_pins_unmoved
  require_clean_tree

  git -C "$repo" checkout main ||
    die "could not check out main"
  git -C "$repo" merge --ff-only "$MAIN_SHA" ||
    die "main could not be fast-forwarded to $MAIN_SHA"
  require_attached_at_pin
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
  "$tool" --dry-run ||
    die "the reconcile dry run failed; nothing is reconciled live until the dry run is clean"
  ok "reconcile dry run clean"
  "$tool" ||
    die "the live reconcile failed"
  ok "live reconcile converged"
}

# Checklist items 14 and 15: probe every manifest entry domain-qualified and
# hold it to ITS recorded predicate. A blanket "running" check would wrongly
# fail the one-shots and the conditional-KeepAlive nix-hook, and a before/after
# diff can never prove a retirement, because a loaded job outlives its plist.
verify_against_manifest() {
  local approved="$LEDGER/retirement-approved.tsv"
  local desired="$LEDGER/desired-services.tsv"
  local label domain predicate body="$scratch/print"
  while IFS=$'\t' read -r label domain; do
    [[ -n $label ]] || continue
    ! service_loaded "$domain" "$label" ||
      die "$domain/$label is still loaded; it was approved for retirement and a deleted plist does not unload a running job"
  done <"$approved"
  ok "every approved-retired label is absent"

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

# The live smoke set: notifications, the hermes gateway, the osquery heartbeat,
# and source-to-target convergence. Gate 4 re-runs it at the end of the soak.
run_smoke_checks() {
  local note="$1" relay="$HOME/.local/bin/relay.sh"
  local heartbeat="$HOME/.local/libexec/osquery/heartbeat.sh" drift

  [[ -x $relay ]] || die "$relay is missing; notifications cannot be proven to work"
  "$relay" --agent cutover-gate --state 'done' --project cutover --detail "$note" ||
    die "relay could not fire a test notification"
  ok "relay fired a test notification"

  command -v hermes >/dev/null 2>&1 || die "hermes is not on PATH"
  hermes gateway status >/dev/null 2>&1 || die "the hermes gateway is not healthy"
  ok "hermes gateway healthy"

  [[ -x $heartbeat ]] || die "$heartbeat is missing; the osquery heartbeat cannot be proven"
  "$heartbeat" || die "the osquery heartbeat failed"
  ok "osquery heartbeat sent"

  drift="$(
    cd "$repo" || exit 1
    chezmoi status --exclude=templates
  )" || die "chezmoi status failed"
  [[ -z $drift ]] ||
    die "chezmoi reports source-to-target drift (KeePassXC-gated templates excluded):"$'\n'"$drift"
  ok "no source-to-target drift outside the KeePassXC-gated templates"
}

gate3() {
  say "gate 3, reconciliation and verification. This will:"
  say "  - run $repo/scripts/live-reconcile.sh, dry-run then live"
  say "  - probe every manifest entry domain-qualified against its predicate"
  say "  - run the repository test suite and the live smoke checks"
  say ''

  require_gate_passed 2
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

# mtime <file> : seconds since the epoch, BSD stat first (macOS), GNU second.
mtime() { stat -f %m "$1" 2>/dev/null || stat -c %Y "$1"; }

gate4() {
  say "gate 4, soak. This will:"
  say "  - measure the soak window from the gate 3 pass"
  say "  - re-probe the daily-critical paths once the window has elapsed"
  say ''

  require_gate_passed 3
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
          if ! [[ $WINDOW_HOURS =~ ^[0-9]+$ ]] || [[ $((10#$WINDOW_HOURS)) -le 0 ]]; then
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
"gate$gate"
