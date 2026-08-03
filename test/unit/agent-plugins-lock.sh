#!/usr/bin/env bash
# agent-plugins-lock.sh, the agent-plugin vertical's own guard: the lock is
# well-formed, it names the same twelve plugins the settings render enables, and
# it stays decoupled from the skills vertical.
#
# WHY THE LOCK AND THE SETTINGS ROSTER MUST AGREE, IN BOTH DIRECTIONS. The
# settings modify-template writes enabledPlugins whole-value, so an id the
# updater refreshes but the template omits is switched OFF by the next apply,
# and the updater then keeps a disabled plugin current forever. The other way
# round, an id the template enables but the lock does not track is a plugin
# nothing updates: it loads, it runs third-party code, and it silently rots.
#
# WHY THE COMPARISON IS AGAINST test/unit/claude-enabled-plugins.sh's
# DECLARED_PLUGINS AND NOT AGAINST THE TEMPLATE'S SOURCE. Matching the
# template's `$declaredPlugins` text would approve a template that does not
# RENDER what it appears to declare, which is a shape that measured green there
# on 2026-07-31 (an entry moved inside a Go-template comment). That test applies
# the template for real and asserts the rendered map against its own array, so
# comparing this lock to that array closes the chain lock == array == render.
# An empty extraction is a hard failure here, because a renamed array would
# otherwise make this rule compare against nothing and pass.
#
# WHY THE DECOUPLING IS CHECKED MECHANICALLY. Plugins and skills are separate
# verticals by operator ruling, TESTS INCLUDED: neither vertical's tests may
# read the other's lock, and neither lock may carry the other's data. A ruling
# nothing enforces is a ruling that survives exactly until the first convenient
# cross-read. THIS FILE is the one exemption to the cross-reference scan, and it
# has to be: a guard that compares two verticals is the only thing that legally
# names both. It never READS the skills lock, and the scan below is the reason
# that stays true for every other file.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOCK="$REPO_ROOT/dot_agents/custom-agent-plugins-lock.json"
SETTINGS_PLUGIN_TEST="$REPO_ROOT/test/unit/claude-enabled-plugins.sh"

# The two lock basenames, for the cross-reference scan. Named here rather than
# derived so the scan says exactly which pair of files may never meet.
readonly PLUGINS_LOCK_BASENAME='custom-agent-plugins-lock.json'
readonly SKILLS_LOCK_BASENAME='custom-skill-lock.json'

# The identity a weekly entry can honestly report for one plugin. `unknowable`
# is the lane whose declared version is the literal string "unknown": its
# lastUpdated bumps on a no-op refresh, so a change signal read from it is
# noise, and the entry must say so rather than report a change.
readonly -a KNOWN_IDENTITY_LANES=('versioned' 'git-sha' 'unknowable')

# The only harness that loads these plugins today. A value outside this set is
# a claim this repo has not measured, not a feature.
readonly -a KNOWN_HARNESSES=('claude-code')

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

command -v jq >/dev/null 2>&1 || fail "jq is not on PATH, so the lock cannot be read"
[[ -f $LOCK ]] || fail "missing plugin lock: $LOCK"
[[ -f $SETTINGS_PLUGIN_TEST ]] || fail "missing settings plugin test: $SETTINGS_PLUGIN_TEST"
jq empty "$LOCK" 2>/dev/null || fail "$LOCK is not valid JSON"

# --- Rule 1: the lock is non-degenerate -------------------------------------
# A lock that parses but declares nothing would make every rule below vacuous,
# and would make the updater a weekly no-op that reports success.
plugin_ids="$(jq -r '.plugins // {} | keys[]' "$LOCK" | sort)"
[[ -n $plugin_ids ]] || fail "the lock declares no plugins, so the updater would refresh nothing and say so to nobody"
marketplace_names="$(jq -r '.marketplaces // {} | keys[]' "$LOCK" | sort)"
[[ -n $marketplace_names ]] || fail "the lock declares no marketplaces, so a fresh machine could not obtain any plugin"

# --- Rule 2: every plugin entry is well-formed ------------------------------
# The id carries its own marketplace after the @, which is the form Claude Code
# writes into settings.json and the form the CLI accepts. Recording the
# marketplace in the entry as well is what lets the updater add an absent
# marketplace before installing, so the two spellings must agree: a mismatch
# would have the updater add one marketplace and install from another.
bad_shape="$(jq -r '.plugins // {} | to_entries[]
  | select((.key | test("^[^@]+@[^@]+$") | not)
      or ((.value.marketplace // "") == "")
      or ((.key | split("@")[1]) != .value.marketplace))
  | .key' "$LOCK")"
[[ -z $bad_shape ]] ||
  fail "plugin ids must read <name>@<marketplace> and carry the SAME marketplace in the entry: $bad_shape"

bad_lane="$(jq -r --argjson lanes "$(printf '%s\n' "${KNOWN_IDENTITY_LANES[@]}" | jq -R . | jq -s .)" \
  '.plugins // {} | to_entries[] | select((.value.identityLane // "") as $l | ($lanes | index($l)) == null)
   | "\(.key)=\(.value.identityLane // "<absent>")"' "$LOCK")"
[[ -z $bad_lane ]] ||
  fail "identityLane must be one of ${KNOWN_IDENTITY_LANES[*]}: $bad_lane"

bad_harnesses="$(jq -r --argjson known "$(printf '%s\n' "${KNOWN_HARNESSES[@]}" | jq -R . | jq -s .)" \
  '.plugins // {} | to_entries[]
   | select(((.value.harnesses | type) != "array")
       or ((.value.harnesses | length) == 0)
       or ((.value.harnesses - $known) | length > 0))
   | .key' "$LOCK")"
[[ -z $bad_harnesses ]] ||
  fail "harnesses must be a non-empty array drawn from ${KNOWN_HARNESSES[*]}: $bad_harnesses"

# --- Rule 3: marketplaces are well-formed, used, and complete ---------------
# A plugin naming a marketplace the lock does not describe cannot be installed
# on a fresh machine, and a marketplace nothing installs from is a dead entry
# whose repo nobody would notice going stale.
bad_marketplace="$(jq -r '.marketplaces // {} | to_entries[]
  | select(((.value.source // "") == "") or ((.value.repo // "") == ""))
  | .key' "$LOCK")"
[[ -z $bad_marketplace ]] || fail "marketplace entries need a non-empty source and repo: $bad_marketplace"

used_marketplaces="$(jq -r '.plugins // {} | [.[].marketplace] | unique | .[]' "$LOCK" | sort)"
undescribed="$(comm -23 <(printf '%s\n' "$used_marketplaces") <(printf '%s\n' "$marketplace_names"))"
[[ -z $undescribed ]] ||
  fail "a plugin names a marketplace the lock does not describe, so a fresh machine cannot obtain it: $undescribed"
unused="$(comm -13 <(printf '%s\n' "$used_marketplaces") <(printf '%s\n' "$marketplace_names"))"
[[ -z $unused ]] ||
  fail "the lock describes a marketplace no tracked plugin installs from; delete it or track the plugin: $unused"

# --- Rule 4: the lock and the settings roster are the same set --------------
declared_plugins="$(sed -n '/^readonly -a DECLARED_PLUGINS=($/,/^)$/p' "$SETTINGS_PLUGIN_TEST" |
  sed -n "s/^  '\(.*\)'\$/\1/p" | sort)"
[[ -n $declared_plugins ]] ||
  fail "found no DECLARED_PLUGINS entries in $SETTINGS_PLUGIN_TEST; that array was renamed or reshaped, so this rule would compare the lock against nothing"
if [[ $plugin_ids != "$declared_plugins" ]]; then
  printf 'FAIL: the plugin lock and the settings roster (DECLARED_PLUGINS) are different sets:\n' >&2
  diff <(printf '%s\n' "$declared_plugins") <(printf '%s\n' "$plugin_ids") >&2 || true
  exit 1
fi

# --- Rule 5: the two verticals stay decoupled ------------------------------
# Neither lock may carry the other's data, and no test but this one may read
# both. Checked as file references rather than as prose, so a comment that
# mentions the other vertical is fine and a cross-read is not.
if grep -qF -- "$SKILLS_LOCK_BASENAME" "$LOCK"; then
  fail "the plugin lock references $SKILLS_LOCK_BASENAME; the two verticals are decoupled by ruling"
fi
SKILLS_LOCK="$REPO_ROOT/dot_agents/$SKILLS_LOCK_BASENAME"
[[ -f $SKILLS_LOCK ]] || fail "the skills lock is missing at $SKILLS_LOCK, so the decoupling check would pass vacuously"
if grep -qF -- "$PLUGINS_LOCK_BASENAME" "$SKILLS_LOCK"; then
  fail "the skills lock references $PLUGINS_LOCK_BASENAME; the two verticals are decoupled by ruling"
fi

this_file="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
crossing_tests=""
while IFS= read -r candidate; do
  [[ $candidate == "$this_file" ]] && continue
  grep -qF -- "$PLUGINS_LOCK_BASENAME" "$candidate" || continue
  grep -qF -- "$SKILLS_LOCK_BASENAME" "$candidate" || continue
  crossing_tests+="$candidate"$'\n'
done < <(find "$REPO_ROOT/test" -type f \( -name '*.sh' -o -name '*.bats' \) | sort)
[[ -z $crossing_tests ]] ||
  fail "these tests read BOTH vertical locks, which the operator ruling forbids: $(tr '\n' ' ' <<<"$crossing_tests")"

plugin_count="$(printf '%s\n' "$plugin_ids" | wc -l | tr -d ' ')"
marketplace_count="$(printf '%s\n' "$marketplace_names" | wc -l | tr -d ' ')"
unknowable_count="$(jq -r '[.plugins // {} | .[] | select(.identityLane == "unknowable")] | length' "$LOCK")"
printf 'agent-plugins-lock: OK (%s plugins from %s marketplaces, %s in the unknowable identity lane; the lock and the settings roster are the same set; no test reads both vertical locks)\n' \
  "$plugin_count" "$marketplace_count" "$unknowable_count"
