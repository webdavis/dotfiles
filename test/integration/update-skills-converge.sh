#!/usr/bin/env bash
# update-skills-converge.sh, the symlink fan-out must CONVERGE each managed dir
# to the lock's desired set, not just add missing links. The additive
# `[[ -e ]] || ln -s` left stale links behind, never fixed a wrong target, and
# crashed on a DANGLING link (`[[ -e ]]` is false for it, so `ln -s` then failed
# on the existing name). The audit found 29 store links vs 13 declared in the
# hermes default profile.
#
# Desired set (from the lock): Claude = the full store roster; each hermes
# profile = exactly its hermesProfiles entries, minus the catalog-collision
# names (humanizer, hyperframes) which hermes serves from its own catalog and
# which must NEVER be symlinked from the store. Convergence per managed dir:
#   * create a missing desired link;
#   * REPLACE an updater-owned link whose target differs (wrong-target, incl.
#     dangling);
#   * REMOVE an updater-owned link no longer desired (stale);
#   * NEVER touch a real directory (hub-owned registry dir, catalog), a
#     non-store symlink, or anything in a profile the lock does not map.
# "updater-owned" = a symlink whose literal target points under ~/.agents/skills
# (works for dangling links too, the string still points there).
#
# The real script runs unmodified in a sandbox: FORCE bypasses the weekly stamp and
# the weekly stamp, offline stubs neutralize the network passes, and the FULL run
# exercises destructive convergence (replace/remove), which the additive
# --install-only bootstrap deliberately never does.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
STORE="$HOME/.agents/skills"
CLAUDE="$HOME/.claude/skills"
HERMES="$HOME/.hermes/skills"
mkdir -p "$STORE" "$CLAUDE" "$HERMES"

# Fixture store: six real skill dirs the convergence assertions target, plus a
# single npx-tracked `anchor` so the roster's tracked UNION is non-empty (the
# zero-union roster gate refuses any full run otherwise; there is no legitimate
# empty roster). anchor migrates into a live generation and is not asserted on;
# the six vendored real dirs are what this test exercises.
for s in keeper mover revived demoted humanizer dualname nonclaude anchor; do
  mkdir -p "$STORE/$s"
  printf -- '---\nname: %s\ndescription: fixture\n---\n' "$s" >"$STORE/$s/SKILL.md"
done

# Fixture lock. humanizer is a catalog-collision name mapped to default ON
# PURPOSE, convergence must still refuse to create it hermes-side and must
# remove a stale one. dualname is hermes-OWNED (hermesProfiles [] + a
# hermesRegistry entry): hermes keeps a real hub dir of that name, untouchable.
# nonclaude carries claudeDelivery "none": this vertical deliberately does not
# serve Claude Code for that store entry, so the Claude fan-out must neither
# create its link nor keep one, while its hermes delivery is unaffected. The
# fan-out linked EVERY store entry before, so a Claude link removed by hand came
# straight back on the next weekly run.
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {
    "keeper": "core", "mover": "core", "revived": "core",
    "demoted": "core", "humanizer": "core", "dualname": "on-demand",
    "nonclaude": "on-demand", "anchor": "core"
  },
  "claudeDelivery": {
    "nonclaude": "none"
  },
  "hermesProfiles": {
    "keeper": ["default"],
    "mover": ["default"],
    "revived": ["default"],
    "demoted": [],
    "humanizer": ["default"],
    "dualname": [],
    "nonclaude": ["default"]
  },
  "hermesRegistry": {
    "dualname": {"profiles": ["default"], "source": "clawhub", "identifier": "clawhub/dualname", "lockKey": "dualname"}
  },
  "npxTracked": {"anchor": {"repo": "fixture/pack"}},
  "clawhubTracked": {},
  "forks": {}
}
EOF
# Seed the flat npx lock so the anchor migrates cleanly into a live generation.
printf '{"skills":{"anchor":{}}}\n' >"$HOME/.agents/.skill-lock.json"

# ── Pre-existing drift ─────────────────────────────────────────────────────
# Claude: one correct link (kept), one stale updater-owned link to a skill that
# left the store (removed). Every other store skill is missing (created).
ln -s "../../.agents/skills/keeper" "$CLAUDE/keeper"
ln -s "../../.agents/skills/gone" "$CLAUDE/gone" # stale: gone not in store
# A correct-looking link for a store skill this vertical does not deliver to
# Claude. It is planted here rather than left absent so the case measures a
# LINK THAT EXISTS being reconciled away, not merely one that was never made:
# the live machine has exactly this shape today.
ln -s "../../.agents/skills/nonclaude" "$CLAUDE/nonclaude"

# Hermes default drift:
#   keeper, absent            → created (missing)
#   mover, wrong target      → replaced
#   revived, DANGLING target   → replaced (the old ln -s crashed here)
#   demoted, correct target but hermesProfiles [] → removed (stale)
#   humanizer, collision name    → removed, never re-created
#   dualname, REAL hub dir      → untouched
#   external, non-store symlink  → untouched
#   hermes-superpowers, real dir → untouched
ln -s "../../.agents/skills/WRONGTARGET" "$HERMES/mover"
ln -s "../../.agents/skills/revived-old" "$HERMES/revived" # dangling (revived-old absent)
ln -s "../../.agents/skills/demoted" "$HERMES/demoted"
ln -s "../../.agents/skills/humanizer" "$HERMES/humanizer"
mkdir -p "$HERMES/dualname"
printf -- '---\nname: dualname\ndescription: hub-owned\n---\n' >"$HERMES/dualname/SKILL.md"
ln -s "/tmp/external-target" "$HERMES/external"
mkdir -p "$HERMES/hermes-superpowers"
printf 'mirror\n' >"$HERMES/hermes-superpowers/marker"

# Destructive reconciliation (replace wrong-target links, remove stale ones)
# runs only on the FULL weekly path, never under the additive --install-only
# bootstrap, so this test exercises a FULL run. Offline stubs stand in for the
# network passes a full run would otherwise make (npx update; the hermes
# registry phase for the dualname hub entry). FORCE bypasses the weekly stamp and
# the weekly stamp, so the second (idempotence) run reconverges instead of
# early-exiting on the stamp.
stub_dir="$tmp/stubs"
mkdir -p "$stub_dir"
# npx stub: installs each --skill and maintains the CLI global lock, so the
# anchor's generation build validates (like the other integration stubs).
cat >"$stub_dir/npx" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
prev=""; skills=()
for a in "$@"; do [[ $prev == --skill ]] && skills+=("$a"); prev="$a"; done
cli_lock="${XDG_STATE_HOME:-$HOME/.local/state}/skills/.skill-lock.json"
mkdir -p "$(dirname "$cli_lock")"
[[ -f $cli_lock ]] || printf '{"version":3,"skills":{}}\n' >"$cli_lock"
for s in "${skills[@]}"; do
  mkdir -p "$HOME/.agents/skills/$s"
  printf -- '---\nname: %s\ndescription: fixture\n---\n' "$s" >"$HOME/.agents/skills/$s/SKILL.md"
  jq --arg s "$s" '.skills[$s] = {source: "github:fixture/pack", agents: ["claude-code","codex"]}' \
    "$cli_lock" >"$cli_lock.tmp" && mv "$cli_lock.tmp" "$cli_lock"
done
EOF
printf '#!/usr/bin/env bash\necho stub\n' >"$stub_dir/hermes"
printf '#!/usr/bin/env bash\nexit 0\n' >"$stub_dir/alerter"
chmod +x "$stub_dir"/*
export PATH="$stub_dir:$PATH"

# ── RED gate: capture whether the current script even survives the dangling
#    link. It is informational; the assertions below are the contract.
run() { UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1; }
output="$(run)" || fail "update-skills full run exited non-zero (a dangling link must not crash the fan-out): $output"

# ── Claude convergence ─────────────────────────────────────────────────────
for s in keeper mover revived demoted humanizer dualname; do
  [[ -L "$CLAUDE/$s" ]] || fail "Claude link missing for store skill: $s"
  [[ "$(readlink "$CLAUDE/$s")" == "../../.agents/skills/$s" ]] ||
    fail "Claude link for $s has wrong target: $(readlink "$CLAUDE/$s")"
done
[[ ! -e "$CLAUDE/gone" && ! -L "$CLAUDE/gone" ]] ||
  fail "stale updater-owned Claude link 'gone' was not removed"

# claudeDelivery "none": present in the store, delivered to hermes, and NOT to
# Claude. Both halves are asserted, because a fan-out that simply stopped
# linking that name everywhere would pass the first assertion alone.
[[ ! -e "$CLAUDE/nonclaude" && ! -L "$CLAUDE/nonclaude" ]] ||
  fail "the Claude link for a claudeDelivery 'none' skill was kept or re-created: $(readlink "$CLAUDE/nonclaude" 2>/dev/null)"
[[ -L "$HERMES/nonclaude" && "$(readlink "$HERMES/nonclaude")" == "../../.agents/skills/nonclaude" ]] ||
  fail "claudeDelivery 'none' suppressed the HERMES link too; the field is about Claude delivery alone"

# ── Hermes default convergence ─────────────────────────────────────────────
# created (was missing)
[[ -L "$HERMES/keeper" && "$(readlink "$HERMES/keeper")" == "../../.agents/skills/keeper" ]] ||
  fail "hermes 'keeper' link was not created with the right target"
# replaced (wrong target)
[[ -L "$HERMES/mover" && "$(readlink "$HERMES/mover")" == "../../.agents/skills/mover" ]] ||
  fail "hermes 'mover' wrong-target link was not replaced: $(readlink "$HERMES/mover" 2>/dev/null)"
# replaced (dangling) and now resolves
[[ -L "$HERMES/revived" && "$(readlink "$HERMES/revived")" == "../../.agents/skills/revived" ]] ||
  fail "hermes 'revived' dangling link was not replaced: $(readlink "$HERMES/revived" 2>/dev/null)"
[[ -e "$HERMES/revived/SKILL.md" ]] || fail "hermes 'revived' link does not resolve after convergence"
# removed (stale updater-owned, no longer desired)
[[ ! -e "$HERMES/demoted" && ! -L "$HERMES/demoted" ]] ||
  fail "stale updater-owned hermes link 'demoted' (hermesProfiles []) was not removed"
# removed (collision name), never re-created
[[ ! -e "$HERMES/humanizer" && ! -L "$HERMES/humanizer" ]] ||
  fail "collision-name hermes link 'humanizer' was not removed / was re-created"
# untouched: hub-owned real dir
[[ -d "$HERMES/dualname" && ! -L "$HERMES/dualname" ]] ||
  fail "hub-owned real dir 'dualname' was altered by convergence"
[[ -e "$HERMES/dualname/SKILL.md" ]] || fail "hub-owned 'dualname' content was disturbed"
# untouched: non-store symlink
[[ -L "$HERMES/external" && "$(readlink "$HERMES/external")" == "/tmp/external-target" ]] ||
  fail "non-store symlink 'external' was altered by convergence"
# untouched: real mirror dir
[[ -d "$HERMES/hermes-superpowers" && -e "$HERMES/hermes-superpowers/marker" ]] ||
  fail "the hermes-superpowers mirror dir was disturbed by convergence"

# ── Idempotence: a second run changes nothing and stays quiet about convergence.
second="$(run)" || fail "second --install-only run exited non-zero: $second"
if printf '%s\n' "$second" | grep -qiE 'converge: (created|replaced|removed)'; then
  fail "a no-op convergence run still logged create/replace/remove actions: $second"
fi

# ── A MULTI-DOCUMENT roster lock must not resurrect a de-delivered link.
# `jq -e '<filter>' file` reads a STREAM and evaluates the filter once per
# document, so its exit status is the LAST document's while every extractor still
# reads them all. A roster with claudeDelivery emptied and a second top-level
# `{}` appended therefore satisfied the schema gate on that trailing object, the
# undelivered-name reader came back empty, and the full run RECREATED
# nonclaude's deliberately absent ~/.claude link and stamped the week a success.
# Both readers are pinned here: the FULL run must refuse at the roster gate, and
# the --dry-run preview (which skips that gate entirely) must refuse at the
# fan-out's own claudeDelivery check.
LOCK_FILE="$HOME/.agents/custom-skill-lock.json"
GOOD_LOCK="$tmp/good-lock.json"
cp "$LOCK_FILE" "$GOOD_LOCK"
STAMP="$HOME/.local/state/update-skills/last-success"
rm -f "$STAMP"
jq '.claudeDelivery = []' "$GOOD_LOCK" >"$LOCK_FILE"
printf '{}' >>"$LOCK_FILE"
set +e
stream_out="$(run)"
stream_rc=$?
set -e
[[ $stream_rc -ne 0 ]] ||
  fail "a multi-document roster lock ran to completion instead of failing closed: $stream_out"
[[ ! -e "$CLAUDE/nonclaude" && ! -L "$CLAUDE/nonclaude" ]] ||
  fail "a multi-document roster lock recreated the de-delivered nonclaude Claude link: $(readlink "$CLAUDE/nonclaude" 2>/dev/null)"
[[ ! -f $STAMP ]] ||
  fail "a multi-document roster lock still stamped the week: $(cat "$STAMP")"
stream_dry="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --dry-run 2>&1 || true)"
if grep -qE 'would create .*/\.claude/skills/nonclaude' <<<"$stream_dry"; then
  printf '%s\n' "$stream_dry" >&2
  fail "the --dry-run fan-out offered to restore nonclaude's link from a multi-document lock"
fi
grep -qiE 'claudeDelivery.*(malformed|refus)' <<<"$stream_dry" ||
  fail "the --dry-run fan-out did not report the multi-document lock's claudeDelivery table: $stream_dry"
cp "$GOOD_LOCK" "$LOCK_FILE"

# ── A claudeDelivery KEY holding a newline forges a second exemption.
# The undelivered-name reader is line-oriented (one name per line), so the single
# key "keeper\nmover" is one entry to the schema and TWO names to the reader, and
# converge_dir reaps every updater-owned link outside the desired set: one forged
# key removed BOTH skills' ~/.claude links. Both must survive, and the run must
# say why.
jq '.claudeDelivery = {"keeper\nmover": "none"}' "$GOOD_LOCK" >"$LOCK_FILE"
set +e
newline_out="$(run)"
newline_rc=$?
set -e
for s in keeper mover; do
  [[ -L "$CLAUDE/$s" && "$(readlink "$CLAUDE/$s")" == "../../.agents/skills/$s" ]] ||
    fail "a newline inside a claudeDelivery key removed the Claude link for $s: $newline_out"
done
[[ $newline_rc -ne 0 ]] ||
  fail "a claudeDelivery key holding a newline ran to completion instead of failing closed: $newline_out"
# The preview reaches the fan-out without the roster gate in front of it, so the
# fan-out's own check has to refuse the forged key too, or the preview announces
# a reap that the gate is the only thing preventing.
newline_dry="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --dry-run 2>&1 || true)"
if grep -qE 'would remove stale .*/\.claude/skills/(keeper|mover)' <<<"$newline_dry"; then
  printf '%s\n' "$newline_dry" >&2
  fail "the --dry-run fan-out previewed reaping a link forged by a newline inside a claudeDelivery key"
fi
grep -qiE 'claudeDelivery.*(malformed|refus)' <<<"$newline_dry" ||
  fail "the --dry-run fan-out did not report the forged claudeDelivery key: $newline_dry"
cp "$GOOD_LOCK" "$LOCK_FILE"
# ...and the legitimate single-key table still de-delivers exactly its own name:
# a validator that refused every key would leave nonclaude linked instead.
run >/dev/null || fail "the run refused a legitimate single-key claudeDelivery table"
[[ ! -e "$CLAUDE/nonclaude" && ! -L "$CLAUDE/nonclaude" ]] ||
  fail "the legitimate claudeDelivery 'none' entry stopped taking effect"
for s in keeper mover; do
  [[ -L "$CLAUDE/$s" ]] || fail "the legitimate run removed the Claude link for $s"
done

# ── A MALFORMED claudeDelivery must not fail OPEN in the Claude fan-out.
# __update_skills_claude_undelivered fails open (an empty undelivered set) on a
# wrong-shaped table, and the fan-out would then RESTORE nonclaude's de-delivered
# ~/.claude link, the exact de-delivery the "none" table exists to keep. The
# weekly and install-only modes refuse a malformed roster upstream at the snapshot
# gate, but --dry-run skips that gate and reaches the fan-out directly, so the
# CONSUMER itself must refuse. nonclaude has no Claude link now (convergence
# removed it above). Rewrite claudeDelivery to an array and preview: the fan-out
# must NOT offer to create nonclaude's link, and must report the malformed table.
jq '.claudeDelivery = ["nonclaude"]' "$LOCK_FILE" >"$LOCK_FILE.tmp" && mv "$LOCK_FILE.tmp" "$LOCK_FILE"
set +e
dry_out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --dry-run 2>&1)"
dry_rc=$?
set -e
# A preview that refused part of its work must SAY SO in its exit status:
# automation that reads exit 0 otherwise accepts an incomplete preview as the
# whole picture.
[[ $dry_rc -ne 0 ]] ||
  fail "a --dry-run that refused the Claude fan-out still exited 0: $dry_out"
if grep -qE 'would create .*/\.claude/skills/nonclaude' <<<"$dry_out"; then
  printf '%s\n' "$dry_out" >&2
  fail "a malformed claudeDelivery failed OPEN; the Claude fan-out offered to restore the de-delivered nonclaude link"
fi
grep -qiE 'claudeDelivery.*(malformed|refus)' <<<"$dry_out" ||
  fail "the fan-out did not report the malformed claudeDelivery table, so a wrong-shaped table would silently fail open: $dry_out"

echo "update-skills-converge: OK"
