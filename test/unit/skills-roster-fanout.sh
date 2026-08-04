#!/usr/bin/env bash
# skills-roster-fanout.sh, the committed skills roster, the lock's tier /
# hermes-profile / hermes-registry / npx-provenance tables, and the per-harness
# declarations must agree, forever.
#
# Roster = vendored store entries (dot_agents/skills/* dirs and symlink_*
# declarations) + npx-tracked skills (the lock's npxTracked table; their store
# copies are installed by the npx `skills` CLI, not vendored) + clawhub-tracked
# skills (the lock's clawhubTracked table; their store copies are installed by
# the `clawhub` CLI, not vendored). Rules:
#   1. Claude (private_dot_claude/skills) declares exactly one store symlink
#      per roster skill MINUS the lock's claudeDelivery "none" set, regardless
#      of provenance, and each declaration's TARGET is the store path for its
#      own name (same shape rule 7 pins hermes-side; a typo there plants a
#      dangling ~/.claude/skills link and the skill silently never loads).
#      claudeDelivery keys must be roster skills and their only legal value is
#      "none", meaning THIS vertical deliberately does not serve Claude Code
#      for that store entry. An absent key is the default, a declared symlink.
#      The field says nothing about who else might serve that capability;
#      nothing here reads any lock but the skills lock.
#   2. The lock's tiers table covers exactly the roster; every value is
#      "core" or "on-demand".
#   3. The Claude settings modify-template demotes exactly the on-demand set:
#      one `setValueAtPath "skillOverrides.<name>" "user-invocable-only"` per
#      on-demand skill, and no skillOverrides entry for any core skill.
#   4. The lock's hermesProfiles table covers exactly the roster; every value
#      is an array. It IS the store->hermes symlink map: [] means the store
#      copy is deliberately not symlinked into any hermes profile.
#   5. Provenance partitions the roster THREE ways: npxTracked keys each carry
#      a non-empty "repo" and NO git-pin remnants (pin/treeHash/sourceUrl);
#      clawhubTracked keys each carry a non-empty "slug" and "registry"; the
#      three sets (vendored dirs, npxTracked, clawhubTracked) are pairwise
#      disjoint and their union equals the roster exactly (every roster skill
#      has exactly one provenance).
#   6. The lock's hermesRegistry table (skills hermes OWNS from a registry and
#      the weekly phase updates) is a subset of the roster, each entry is
#      well-formed (non-empty profiles array, source skills.sh|clawhub|
#      official, non-empty identifier + lockKey), and it is DISJOINT from the
#      store-symlinked set: no skill is both hermes-owned and store-symlinked
#      (a store-fed skill must never be `hermes skills update`d).
#   7. The hermes symlink declarations equal the non-empty hermesProfiles map
#      exactly: each store-symlinked skill is declared in exactly its mapped
#      skills dirs ("default" = dot_hermes/skills, any other profile =
#      dot_hermes/profiles/<name>/skills, where the source dir may carry a
#      private_ prefix) with the correct relative target for that dir's depth,
#      no stray declarations.
#   8. Collision-named skills (humanizer, hyperframes, hermes's catalog wins
#      those names) are never declared in any hermes skills dir and never
#      carry a non-empty hermesProfiles mapping, regardless of what the other
#      tables say. summarize-pro and todoist-cli left this set: their only
#      hermes copies were hub installs (since retired), so no catalog copy
#      wins those names and the store symlink is the wanted delivery.
#   9. The lock's forks drift-watch table covers exactly the vendored dirs
#      (minus the documented exemptions) and every entry is well-formed.
#  10. Every roster size CLAUDE.md states equals the computed one, so the
#      prose a maintainer reads cannot drift away from the tables.
# Codex has no declarations to check: it scans ~/.agents/skills natively; its
# on-demand policy files (agents/openai.yaml) are committed alongside vendored
# skills and re-asserted at run time by update-skills.sh for the npx ones.
set -euo pipefail
# The roster/declaration loops below glob directory contents; nullglob makes an
# empty dir expand to nothing instead of the literal '*' pattern.
shopt -s nullglob

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOCK="$REPO_ROOT/dot_agents/custom-skill-lock.json"
MODIFY_TEMPLATE="$REPO_ROOT/private_dot_claude/modify_settings.json"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $LOCK ]] || fail "missing lock file: $LOCK"
[[ -f $MODIFY_TEMPLATE ]] || fail "missing modify-template: $MODIFY_TEMPLATE"

# A chezmoi source name -> target skill name: strip private_/symlink_
# attribute prefixes and the .tmpl suffix.
target_name() {
  local n="$1"
  n="${n#private_}"
  n="${n#symlink_}"
  n="${n%.tmpl}"
  printf '%s\n' "$n"
}

vendored_dirs() {
  local entry
  for entry in "$REPO_ROOT/dot_agents/skills"/*; do
    target_name "$(basename "$entry")"
  done
}

roster() {
  vendored_dirs
  jq -r '.npxTracked | keys[]' "$LOCK"
  jq -r '.clawhubTracked // {} | keys[]' "$LOCK"
}

roster_sorted="$(roster | sort -u)"
[[ -n $roster_sorted ]] || fail "empty roster"

# --- Rule 1: Claude declares the full roster -------------------------------
claude_declarations() {
  local entry base skill target
  # The target chezmoi writes and update-skills.sh converges to, relative to
  # ~/.claude/skills. Kept literal here rather than derived: this rule exists to
  # catch a hand-typed target, so it must compare against a constant.
  local claude_prefix="../../.agents/skills"
  for entry in "$REPO_ROOT/private_dot_claude/skills"/*; do
    base="$(basename "$entry")"
    case "$base" in
      symlink_*) ;;
      *) fail "non-symlink entry '$base' in private_dot_claude/skills (harness skill dirs hold only store symlinks)" ;;
    esac
    skill="$(target_name "$base")"
    # $(<file) strips trailing NEWLINES, so both committed spellings (with and
    # without a final newline) compare equal. chezmoi strips MORE than that:
    # measured 2026-08-02 against both the flake's 2.62.3 and the host's 2.71.1,
    # it drops all leading and trailing whitespace, spaces, tabs and CR
    # included, and preserves whitespace only in the middle. So this comparison
    # is the stricter of the two, and the asymmetry runs the safe way: it can
    # only reject a target chezmoi would have accepted, never accept one chezmoi
    # would plant wrong. Do not "simplify" it toward chezmoi's rule; the point of
    # the rule is to refuse a hand-typed target, not to bless every spelling.
    target="$(<"$entry")"
    [[ $target == "$claude_prefix/$skill" ]] ||
      fail "declaration private_dot_claude/skills/$base points at '$target' (expected '$claude_prefix/$skill')"
    printf '%s\n' "$skill"
  done
}

# The store entries this vertical does not deliver to Claude Code. The value is
# pinned to the single word "none" so the table cannot grow a second spelling
# that means the same thing; a key naming a non-roster skill is a stale
# exemption quietly widening the hole, exactly like a stale forks exemption.
claude_undelivered="$(jq -r '.claudeDelivery // {} | keys[]' "$LOCK" | sort)"
bad_delivery="$(jq -r '.claudeDelivery // {} | to_entries[] | select(.value != "none") | "\(.key)=\(.value)"' "$LOCK")"
[[ -z $bad_delivery ]] ||
  fail "claudeDelivery values must be \"none\" (absent means the default, a declared store symlink): $bad_delivery"
stray_delivery="$(comm -23 <(printf '%s\n' "$claude_undelivered") <(printf '%s\n' "$roster_sorted"))"
[[ -z $stray_delivery ]] ||
  fail "claudeDelivery names a non-roster skill, so the exemption covers nothing: $stray_delivery"

claude_expected="$(comm -23 <(printf '%s\n' "$roster_sorted") <(printf '%s\n' "$claude_undelivered") | sed '/^$/d')"
[[ -n $claude_expected ]] || fail "claudeDelivery exempts the WHOLE roster; Claude would be served nothing"
claude_declared="$(claude_declarations | sort)"
if [[ $claude_declared != "$claude_expected" ]]; then
  printf 'FAIL: private_dot_claude/skills symlink declarations do not match the roster minus the claudeDelivery "none" set:\n' >&2
  diff <(printf '%s\n' "$claude_expected") <(printf '%s\n' "$claude_declared") >&2 || true
  exit 1
fi

# The RUNTIME half of the same decision. chezmoi only declares what a fresh
# machine gets; update-skills.sh's weekly Claude fan-out is what re-links an
# existing one, and it linked EVERY store entry unconditionally until this
# field existed, so a hand-removed link came straight back on the next Monday.
# The BEHAVIOURAL guard is test/integration/update-skills-converge.sh, which
# runs the fan-out against a fixture lock; this is the cheap tripwire that fires
# in the commit gate. It matches the READ, not the word: measured 2026-08-03,
# repointing the jq filter at another table while leaving the comments alone
# kept a bare `grep -q claudeDelivery` green, so the guard has to name the
# expression the value actually comes from.
UPDATER_SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
CLAUDE_DELIVERY_LOCK_READ='.claudeDelivery // {}'
grep -qF -- "$CLAUDE_DELIVERY_LOCK_READ" "$UPDATER_SCRIPT" ||
  fail "update-skills.sh no longer reads '$CLAUDE_DELIVERY_LOCK_READ' out of the lock, so its weekly Claude fan-out would re-create every exempted link. If the read was deliberately respelled, update this constant and confirm test/integration/update-skills-converge.sh still passes"

# --- Rule 2: tiers covers exactly the roster -------------------------------
tier_keys="$(jq -r '.tiers // {} | keys[]' "$LOCK" | sort)"
if [[ $tier_keys != "$roster_sorted" ]]; then
  printf "FAIL: the lock's tiers table does not cover exactly the roster:\n" >&2
  diff <(printf '%s\n' "$roster_sorted") <(printf '%s\n' "$tier_keys") >&2 || true
  exit 1
fi
bad_tiers="$(jq -r '.tiers | to_entries[] | select(.value != "core" and .value != "on-demand") | "\(.key)=\(.value)"' "$LOCK")"
[[ -z $bad_tiers ]] || fail "tiers values must be \"core\" or \"on-demand\": $bad_tiers"

# --- Rule 3: modify-template skillOverrides == on-demand tier set ----------
on_demand="$(jq -r '.tiers | to_entries[] | select(.value == "on-demand") | .key' "$LOCK" | sort)"
overrides="$(sed -n 's/.*setValueAtPath "skillOverrides\.\([^"]*\)" "user-invocable-only".*/\1/p' "$MODIFY_TEMPLATE" | sort)"
override_lines="$(grep -c 'skillOverrides\.' "$MODIFY_TEMPLATE" || true)"
override_count=0
[[ -n $overrides ]] && override_count="$(printf '%s\n' "$overrides" | wc -l | tr -d ' ')"
[[ $override_lines -eq $override_count ]] ||
  fail "modify_settings.json has skillOverrides lines that are not user-invocable-only setValueAtPath calls"
if [[ $overrides != "$on_demand" ]]; then
  printf 'FAIL: modify_settings.json skillOverrides do not match the on-demand tier set:\n' >&2
  diff <(printf '%s\n' "$on_demand") <(printf '%s\n' "$overrides") >&2 || true
  exit 1
fi

# --- Rule 4: hermesProfiles covers exactly the roster ----------------------
profile_keys="$(jq -r '.hermesProfiles // {} | keys[]' "$LOCK" | sort)"
if [[ $profile_keys != "$roster_sorted" ]]; then
  printf "FAIL: the lock's hermesProfiles table does not cover exactly the roster:\n" >&2
  diff <(printf '%s\n' "$roster_sorted") <(printf '%s\n' "$profile_keys") >&2 || true
  exit 1
fi
bad_profiles="$(jq -r '.hermesProfiles | to_entries[] | select((.value | type) != "array") | .key' "$LOCK")"
[[ -z $bad_profiles ]] || fail "hermesProfiles values must be arrays of profile names: $bad_profiles"

# --- Rule 5: provenance (vendored / npx / clawhub) partitions the roster ----
bad_npx="$(jq -r '.npxTracked // {} | to_entries[]
  | select(((.value.repo // "") == "")
      or (.value | has("pin")) or (.value | has("treeHash")) or (.value | has("sourceUrl")))
  | .key' "$LOCK")"
[[ -z $bad_npx ]] ||
  fail "npxTracked entries need a non-empty repo and no git-pin fields (pin/treeHash/sourceUrl): $bad_npx"
bad_clawhub="$(jq -r '.clawhubTracked // {} | to_entries[]
  | select(((.value.slug // "") == "") or ((.value.registry // "") == ""))
  | .key' "$LOCK")"
[[ -z $bad_clawhub ]] ||
  fail "clawhubTracked entries need a non-empty slug and registry: $bad_clawhub"
npx_keys="$(jq -r '.npxTracked // {} | keys[]' "$LOCK" | sort)"
clawhub_keys="$(jq -r '.clawhubTracked // {} | keys[]' "$LOCK" | sort)"
vendored_sorted="$(vendored_dirs | sort -u)"
overlap="$(comm -12 <(printf '%s\n' "$npx_keys") <(printf '%s\n' "$vendored_sorted"))"
[[ -z $overlap ]] || fail "a skill is BOTH vendored and npx-tracked (pick one): $overlap"
overlap="$(comm -12 <(printf '%s\n' "$clawhub_keys") <(printf '%s\n' "$vendored_sorted"))"
[[ -z $overlap ]] || fail "a skill is BOTH vendored and clawhub-tracked (pick one): $overlap"
overlap="$(comm -12 <(printf '%s\n' "$clawhub_keys") <(printf '%s\n' "$npx_keys"))"
[[ -z $overlap ]] || fail "a skill is BOTH npx-tracked and clawhub-tracked (pick one): $overlap"
union_sorted="$(printf '%s\n%s\n%s\n' "$npx_keys" "$clawhub_keys" "$vendored_sorted" | sort -u | sed '/^$/d')"
if [[ $union_sorted != "$roster_sorted" ]]; then
  printf 'FAIL: vendored dirs + npxTracked + clawhubTracked keys do not partition the roster:\n' >&2
  diff <(printf '%s\n' "$roster_sorted") <(printf '%s\n' "$union_sorted") >&2 || true
  exit 1
fi

# --- Rule 6: hermesRegistry is a well-formed, roster-scoped, disjoint set ---
registry_keys="$(jq -r '.hermesRegistry // {} | keys[]' "$LOCK" | sort)"
stray_registry="$(comm -23 <(printf '%s\n' "$registry_keys") <(printf '%s\n' "$roster_sorted"))"
[[ -z $stray_registry ]] || fail "hermesRegistry names a non-roster skill: $stray_registry"
bad_registry="$(jq -r '.hermesRegistry // {} | to_entries[]
  | select(
      ((.value.profiles | type) != "array") or ((.value.profiles | length) == 0)
      or ((.value.source == "skills.sh" or .value.source == "clawhub" or .value.source == "official") | not)
      or ((.value.identifier // "") == "")
      or ((.value.lockKey // "") == ""))
  | .key' "$LOCK")"
[[ -z $bad_registry ]] ||
  fail "hermesRegistry entries need a non-empty profiles array, source (skills.sh|clawhub|official), identifier, lockKey: $bad_registry"
# Disjoint: a hermes-owned registry skill must not also be store-symlinked.
store_symlinked="$(jq -r '.hermesProfiles | to_entries[] | select((.value | length) > 0) | .key' "$LOCK" | sort)"
both="$(comm -12 <(printf '%s\n' "$registry_keys") <(printf '%s\n' "$store_symlinked"))"
[[ -z $both ]] ||
  fail "a skill is BOTH hermes-owned (hermesRegistry) and store-symlinked (hermesProfiles), reconcile: $both"

# --- Rule 6b: every profile the lock names is a real hermes profile ----------
# A typo like "nicodemas" passes the non-empty check but is then silently never
# walked by the updater's HERMES_UPDATE_PROFILES. Pin profile names to the known
# five so a misspelling fails here, not silently in production.
known_profiles=$'default\nbutters\nconcerned\nelaine\nnicodemus'
lock_profiles="$(jq -r '[(.hermesRegistry // {} | .[].profiles[]?),
  (.hermesProfiles // {} | .[][]?)] | unique | .[]' "$LOCK" | sort -u)"
stray_profile="$(comm -23 <(printf '%s\n' "$lock_profiles") <(printf '%s\n' "$known_profiles" | sort))"
[[ -z $stray_profile ]] || fail "lock names an unknown hermes profile: $stray_profile"

# --- Rule 6c: the updater DERIVES walked profiles from the lock, not hardcodes -
# A hardcoded profile array would silently diverge from hermesRegistry; require
# the updater to compute the set from the lock so a new specialist is walked
# automatically. Default's un-entanglement is DONE (2026-07-09): no registry
# entry has a store-symlinked install path anymore, so the old
# `grep -vx default` exclusion is retired and must not creep back, default is
# walked exactly like any other profile. (The derivation's correctness and
# per-profile failure isolation are exercised against a fixture lock in
# test/update-skills-hermes-phase.sh.)
updater="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
if ! grep -q 'hermesRegistry.*profiles' "$updater"; then
  fail "update-skills.sh must derive the hermes-update profiles from the lock (hermesRegistry)"
fi
if grep -q 'grep -vx default' "$updater"; then
  fail "update-skills.sh still excludes the default profile, its un-entanglement is done; walk it like any other"
fi
if grep -q 'HERMES_UPDATE_PROFILES=(' "$updater"; then
  fail "update-skills.sh still hardcodes HERMES_UPDATE_PROFILES, derive it from the lock instead"
fi

# --- Rule 7: hermes declarations == the non-empty hermesProfiles map --------
expected_hermes="$(
  jq -r '.hermesProfiles | to_entries[]
    | select((.value | length) > 0)
    | .key as $skill | .value[] | "\(.)\t\($skill)"' "$LOCK" |
    while IFS=$'\t' read -r profile skill; do
      if [[ $profile == "default" ]]; then
        printf 'dot_hermes/skills/%s\n' "$skill"
      else
        printf 'dot_hermes/profiles/%s/skills/%s\n' "$profile" "$skill"
      fi
    done | sort
)"

hermes_declaration_dirs() {
  printf '%s\n' "$REPO_ROOT/dot_hermes/skills"
  local profile_dir
  for profile_dir in "$REPO_ROOT/dot_hermes/profiles"/*/; do
    printf '%s\n' "${profile_dir%/}/skills"
  done
}

actual_hermes="$(
  while IFS= read -r dir; do
    [[ -d $dir ]] || continue
    if [[ $dir == "$REPO_ROOT/dot_hermes/skills" ]]; then
      expected_prefix="../../.agents/skills/"
    else
      expected_prefix="../../../../.agents/skills/"
    fi
    for entry in "$dir"/*; do
      base="$(basename "$entry")"
      case "$base" in
        symlink_*) ;;
        *) fail "non-symlink entry '$base' in ${dir#"$REPO_ROOT"/} (hermes skills dirs hold only store symlinks)" ;;
      esac
      skill="$(target_name "$base")"
      target="$(<"$entry")"
      [[ $target == "${expected_prefix}${skill}" ]] ||
        fail "declaration ${dir#"$REPO_ROOT"/}/$base points at '$target' (expected '${expected_prefix}${skill}')"
      # A profile source dir may carry a private_ prefix, which is a chezmoi mode
      # attribute and not part of the target identity this rule pins. Compare on
      # the target spelling so the rule stays about which skill reaches which
      # profile. Failure messages above keep the source spelling, so they still
      # name a file that exists.
      compared_dir="${dir#"$REPO_ROOT"/}"
      if [[ $compared_dir == dot_hermes/profiles/*/skills ]]; then
        profile_source="${compared_dir#dot_hermes/profiles/}"
        compared_dir="dot_hermes/profiles/$(target_name "${profile_source%/skills}")/skills"
      fi
      printf '%s/%s\n' "$compared_dir" "$skill"
    done
  done < <(hermes_declaration_dirs) | sort
)"

if [[ $actual_hermes != "$expected_hermes" ]]; then
  printf "FAIL: hermes symlink declarations do not match the non-empty hermesProfiles (skill, profile) set:\n" >&2
  diff <(printf '%s\n' "$expected_hermes") <(printf '%s\n' "$actual_hermes") >&2 || true
  exit 1
fi

# --- Rule 8: collision names never reach hermes from the store -------------
# hermes's catalog wins these names (operator ruling); the store copies serve
# Claude/Codex only. Independent of the tables above, so a future lock edit
# cannot quietly re-route a collision name through the store.
collision_names=(humanizer hyperframes)
for collision in "${collision_names[@]}"; do
  if [[ -n $actual_hermes ]] && printf '%s\n' "$actual_hermes" | grep -q "/${collision}$"; then
    fail "collision-named skill '$collision' is declared in a hermes skills dir (catalog wins, never declare it)"
  fi
  collision_profiles="$(jq -r --arg s "$collision" '.hermesProfiles[$s] // [] | length' "$LOCK")"
  [[ $collision_profiles == "0" ]] ||
    fail "collision-named skill '$collision' has a non-empty hermesProfiles mapping (catalog wins, must be [])"
done

# --- Rule 9: the forks drift-watch table covers exactly the vendored dirs ---
# The forks table is the weekly drift-watch's whole input: an upstream absent
# from it is an upstream nobody is watching, silently. It is keyed by VENDORED
# CONTENT, not by the roster, because only a vendored copy can drift from an
# upstream (npx- and clawhub-tracked store copies are replaced wholesale each
# run, and the app-owned cua-driver symlink is not content this repo holds).
# Note the table is not only content FORKS: elevenlabs is vendored-not-forked
# (npx cannot install it full-tree) and is watched for exactly the same reason,
# which is why the coverage rule is "vendored", not "carries fork: true".
FORKS_UNWATCHED_VENDORED=(tiktok-crawling) # documented at CLAUDE.md, Agent Skills

# Real directories only: dot_agents/skills also holds symlink_*.tmpl chezmoi
# declarations for app-owned packs, which hold no content of ours to drift.
vendored_content_dirs() {
  local entry
  for entry in "$REPO_ROOT/dot_agents/skills"/*; do
    [[ -d $entry ]] || continue
    target_name "$(basename "$entry")"
  done
}

forks_keys="$(jq -r '.forks // {} | keys[]' "$LOCK" | sort)"
vendored_content_sorted="$(vendored_content_dirs | sort -u)"
watched_expected="$(comm -23 <(printf '%s\n' "$vendored_content_sorted") \
  <(printf '%s\n' "${FORKS_UNWATCHED_VENDORED[@]}" | sort))"
if [[ $forks_keys != "$watched_expected" ]]; then
  printf "FAIL: the lock's forks table does not cover exactly the watched vendored skills (left: expected, right: forks keys):\n" >&2
  diff <(printf '%s\n' "$watched_expected") <(printf '%s\n' "$forks_keys") >&2 || true
  exit 1
fi
# Every exemption must name a real vendored dir, so a renamed or deleted skill
# cannot leave a dead exemption quietly widening the hole.
for exempt_skill in "${FORKS_UNWATCHED_VENDORED[@]}"; do
  printf '%s\n' "$vendored_content_sorted" | grep -qx "$exempt_skill" ||
    fail "FORKS_UNWATCHED_VENDORED names '$exempt_skill', which is not a vendored skill dir (stale exemption)"
done
# Entry schema: the three fields the drift-watch walks, each a non-empty STRING
# carrying no control characters. The type half is what a `// ""` test cannot
# do: `12345 // ""` is 12345, so an unquoted lastComparedTreeHash (the one field
# a maintainer hand-edits after comparing a fork) passed the old check, and at
# run time `jq -r` read it back as "12345", matched no hash, and cried FORK
# DRIFT every week forever. The control-character half is the invisible one: a
# NUL or a trailing newline in any of the three is dropped or stripped by the
# shell that reads it, so the recorded value silently becomes a different, valid
# one and the watch compares THAT and reports it unchanged. The roster gate no
# longer refuses a run over this table, so this build-time check is where a
# committed typo has to be caught.
bad_forks="$(jq -r '
  def unusable($v): ($v | type) != "string"
    or $v == ""
    or ($v | explode | any(. < 32 or . == 127));
  .forks // {} | to_entries[]
  | select(((.value | type) != "object")
      or unusable(.value.sourceUrl)
      or unusable(.value.skillPath)
      or unusable(.value.lastComparedTreeHash))
  | .key' "$LOCK")"
[[ -z $bad_forks ]] ||
  fail "forks entries need a non-empty, control-character-free string sourceUrl, skillPath and lastComparedTreeHash: $bad_forks"

# The drift-watch relays a fork's NAME as its --project, except for the two
# advisories about the lock itself, which use reserved labels. A fork claiming
# one of those labels would be indistinguishable from the lock file downstream.
# The labels are READ OUT of the updater rather than restated here: a copy would
# drift the moment either side is renamed, and the guard would then reserve a
# label nothing uses while the live one collided freely.
UPDATER="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
reserved_relay_projects="$(sed -n 's/^FORK_RELAY_PROJECT_[A-Z_]*="\(.*\)"$/\1/p' "$UPDATER")"
[[ -n $reserved_relay_projects ]] ||
  fail "found no FORK_RELAY_PROJECT_* labels in $UPDATER; this guard would silently reserve nothing"
# An explicit if, not `grep && fail`: as the last command of a loop body, a
# grep that finds nothing makes the whole while loop exit non-zero, and under
# `set -e` a clean roster would then kill this test with no message at all.
while IFS= read -r reserved_project; do
  if printf '%s\n' "$forks_keys" | grep -qx -- "$reserved_project"; then
    fail "the forks table has a key '$reserved_project', which is a reserved lock-level relay --project label; downstream cannot tell that fork's advisory from the lock file's"
  fi
done <<<"$reserved_relay_projects"

roster_count="$(printf '%s\n' "$roster_sorted" | wc -l | tr -d ' ')"
npx_count="$(printf '%s\n' "$npx_keys" | wc -l | tr -d ' ')"
clawhub_count=0
[[ -n $clawhub_keys ]] && clawhub_count="$(printf '%s\n' "$clawhub_keys" | wc -l | tr -d ' ')"
registry_count=0
[[ -n $registry_keys ]] && registry_count="$(printf '%s\n' "$registry_keys" | wc -l | tr -d ' ')"
hermes_count=0
[[ -n $actual_hermes ]] && hermes_count="$(printf '%s\n' "$actual_hermes" | wc -l | tr -d ' ')"
forks_count=0
[[ -n $forks_keys ]] && forks_count="$(printf '%s\n' "$forks_keys" | wc -l | tr -d ' ')"

# --- Rule 10: the roster sizes CLAUDE.md states equal the computed ones -----
# CLAUDE.md is where a maintainer reads how big the roster is, and every one of
# these numbers is retyped by hand by whichever commit adds or removes a skill.
# Nothing compared them to the tables, so a mistyped or forgotten one read as
# fact forever: measured 2026-08-02, reverting the documented numbers to their
# pre-change values left this suite green, and no other test reads them. The
# figures are the ones already computed above, so the rule costs one sed pass
# per sentence.
#
# Each pattern must MATCH SOMETHING as well as agree. A reworded sentence that
# stops matching would otherwise retire its own check in silence, which is the
# same drift with an extra step, so a miss fails loudly and says which side to
# fix.
#
# Matching runs against the PARAGRAPH-UNWRAPPED text, not the file's lines.
# mdformat re-wraps CLAUDE.md at 105 columns, so a sentence's number and its
# surrounding words routinely straddle a line break (the HyperFrames one does
# today), and a line-based pattern would then have to be re-tuned every time an
# unrelated word shifts the wrap. Unwrapping makes the patterns depend on the
# prose, which is the thing being pinned, and not on the column width.
DOCUMENTATION="$REPO_ROOT/CLAUDE.md"
[[ -f $DOCUMENTATION ]] || fail "missing documentation file: $DOCUMENTATION"
documentation_unwrapped="$(awk '
  /^[[:space:]]*$/ { if (block != "") { print block; block = "" } ; next }
  { line = $0; sub(/^[[:space:]]+/, "", line)
    block = (block == "" ? line : block " " line) }
  END { if (block != "") print block }
' "$DOCUMENTATION")"
[[ -n $documentation_unwrapped ]] || fail "unwrapping $DOCUMENTATION produced no text"

core_count="$(jq -r '[.tiers | to_entries[] | select(.value == "core")] | length' "$LOCK")"
on_demand_count="$(jq -r '[.tiers | to_entries[] | select(.value == "on-demand")] | length' "$LOCK")"
# The HyperFrames group is a repo group, exactly how the updater's npx lane
# batches its `skills add` calls (`[.[].repo] | unique`), so the documented
# figure has a single derivation and no second source of truth.
hyperframes_count="$(jq -r '[.npxTracked | to_entries[]
  | select(.value.repo == "heygen-com/hyperframes")] | length' "$LOCK")"

# documented_count_is <expected> <label> <basic-regular-expression>
# The expression matches a whole unwrapped block and captures the stated number
# in its one \(...\) group. A basic regular expression, not an extended one:
# BSD and GNU sed read it the same way with no flag.
documented_count_is() {
  local expected="$1" label="$2" expression="$3"
  local stated distinct
  stated="$(printf '%s\n' "$documentation_unwrapped" | sed -n "s/$expression/\\1/p" | sort -u)"
  [[ -n $stated ]] ||
    fail "no CLAUDE.md sentence states the $label any more, so this rule now pins nothing; restore the wording, or update the pattern in test/unit/skills-roster-fanout.sh in the same commit"
  distinct="$(printf '%s\n' "$stated" | wc -l | tr -d ' ')"
  [[ $distinct -eq 1 ]] ||
    fail "CLAUDE.md states $distinct different values for the $label ($(printf '%s' "$stated" | tr '\n' ' ')); they cannot all be right"
  [[ $stated == "$expected" ]] ||
    fail "CLAUDE.md says the $label is $stated; the lock and the declarations say $expected"
}

# The backticks in these patterns are CLAUDE.md's own markdown around a table
# name, matched literally; nothing here is a command substitution.
# shellcheck disable=SC2016
{
  documented_count_is "$roster_count" 'roster size' \
    '.*single canonical skills store (\([0-9][0-9]*\) roster skills).*'
  documented_count_is "$npx_count" 'npx-tracked count' \
    '.*`npxTracked` table, \([0-9][0-9]*\) skills.*'
  documented_count_is "$clawhub_count" 'clawhub-tracked count' \
    '.*`clawhubTracked` table, \([0-9][0-9]*\) skills.*'
  documented_count_is "$core_count" 'core tier size' \
    '.*`core` (\([0-9][0-9]*\)) or `on-demand`.*'
  documented_count_is "$on_demand_count" 'on-demand tier size' \
    '.*or `on-demand` (\([0-9][0-9]*\)).*'
  documented_count_is "$hyperframes_count" 'HeyGen HyperFrames repo-group size' \
    '.*the \([0-9][0-9]*\) curated HeyGen HyperFrames skills.*'
}

printf 'skills-roster-fanout: OK (%s skills; %s npx-tracked; %s clawhub-tracked; %s hermes-owned; %s store->hermes symlinks; %s drift-watched upstreams)\n' \
  "$roster_count" "$npx_count" "$clawhub_count" "$registry_count" "$hermes_count" "$forks_count"
