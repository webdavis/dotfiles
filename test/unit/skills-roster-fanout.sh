#!/usr/bin/env bash
# skills-roster-fanout.sh — the committed skills roster, the lock's tier /
# hermes-profile / hermes-registry / npx-provenance tables, and the per-harness
# declarations must agree, forever.
#
# Roster = vendored store entries (dot_agents/skills/* dirs and symlink_*
# declarations) + npx-tracked skills (the lock's npxTracked table; their store
# copies are installed by the npx `skills` CLI, not vendored) + clawhub-tracked
# skills (the lock's clawhubTracked table; their store copies are installed by
# the `clawhub` CLI, not vendored). Rules:
#   1. Claude (private_dot_claude/skills) declares exactly one store symlink
#      per roster skill — the full roster reaches Claude regardless of
#      provenance.
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
#      dot_hermes/profiles/<name>/skills) with the correct relative target for
#      that dir's depth, no stray declarations.
#   8. Collision-named skills (humanizer, hyperframes — hermes's catalog wins
#      those names) are never declared in any hermes skills dir and never
#      carry a non-empty hermesProfiles mapping, regardless of what the other
#      tables say. summarize-pro and todoist-cli left this set: their only
#      hermes copies were hub installs (since retired), so no catalog copy
#      wins those names and the store symlink is the wanted delivery.
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
  local entry base
  for entry in "$REPO_ROOT/private_dot_claude/skills"/*; do
    base="$(basename "$entry")"
    case "$base" in
      symlink_*) target_name "$base" ;;
      *) fail "non-symlink entry '$base' in private_dot_claude/skills (harness skill dirs hold only store symlinks)" ;;
    esac
  done
}

claude_declared="$(claude_declarations | sort)"
if [[ $claude_declared != "$roster_sorted" ]]; then
  printf 'FAIL: private_dot_claude/skills symlink declarations do not match the skills roster:\n' >&2
  diff <(printf '%s\n' "$roster_sorted") <(printf '%s\n' "$claude_declared") >&2 || true
  exit 1
fi

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
  fail "a skill is BOTH hermes-owned (hermesRegistry) and store-symlinked (hermesProfiles) — reconcile: $both"

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
# `grep -vx default` exclusion is retired and must not creep back — default is
# walked exactly like any other profile. (The derivation's correctness and
# per-profile failure isolation are exercised against a fixture lock in
# test/update-skills-hermes-phase.sh.)
updater="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
if ! grep -q 'hermesRegistry.*profiles' "$updater"; then
  fail "update-skills.sh must derive the hermes-update profiles from the lock (hermesRegistry)"
fi
if grep -q 'grep -vx default' "$updater"; then
  fail "update-skills.sh still excludes the default profile — its un-entanglement is done; walk it like any other"
fi
if grep -q 'HERMES_UPDATE_PROFILES=(' "$updater"; then
  fail "update-skills.sh still hardcodes HERMES_UPDATE_PROFILES — derive it from the lock instead"
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
      printf '%s/%s\n' "${dir#"$REPO_ROOT"/}" "$skill"
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
    fail "collision-named skill '$collision' is declared in a hermes skills dir (catalog wins — never declare it)"
  fi
  collision_profiles="$(jq -r --arg s "$collision" '.hermesProfiles[$s] // [] | length' "$LOCK")"
  [[ $collision_profiles == "0" ]] ||
    fail "collision-named skill '$collision' has a non-empty hermesProfiles mapping (catalog wins — must be [])"
done

roster_count="$(printf '%s\n' "$roster_sorted" | wc -l | tr -d ' ')"
npx_count="$(printf '%s\n' "$npx_keys" | wc -l | tr -d ' ')"
clawhub_count=0
[[ -n $clawhub_keys ]] && clawhub_count="$(printf '%s\n' "$clawhub_keys" | wc -l | tr -d ' ')"
registry_count=0
[[ -n $registry_keys ]] && registry_count="$(printf '%s\n' "$registry_keys" | wc -l | tr -d ' ')"
hermes_count=0
[[ -n $actual_hermes ]] && hermes_count="$(printf '%s\n' "$actual_hermes" | wc -l | tr -d ' ')"
printf 'skills-roster-fanout: OK (%s skills; %s npx-tracked; %s clawhub-tracked; %s hermes-owned; %s store->hermes symlinks)\n' \
  "$roster_count" "$npx_count" "$clawhub_count" "$registry_count" "$hermes_count"
