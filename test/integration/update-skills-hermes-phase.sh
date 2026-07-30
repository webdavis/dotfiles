#!/usr/bin/env bash
# update-skills-hermes-phase.sh, proves the weekly hermes registry-update
# phase, offline. The real script runs unmodified in a sandbox: a scratch HOME
# and PATH stubs for `hermes` and `npx` that record their argv instead of
# touching the network (the subprocess boundary is the legitimate test double).
# The fixture lock marks skills registry-served across profiles, one held, one
# store-fallback, one default-only, plus two failure shapes. Assertions:
#   1. Per-profile invocations come from the lock: every (registry skill,
#      profile) pair gets exactly `hermes -p <profile> skills update
#      <lockKey>`, keyed by the mechanism's lockKey, NOT the skill name
#      (ClawHub slugs differ from frontmatter names).
#   2. The default profile is WALKED like any other (its un-entanglement is
#      done, no registry entry there has a store-symlinked install path
#      anymore): a registry skill mapped only to default gets exactly
#      `hermes -p default skills update <lockKey>`.
#   3. held: true is skipped (and said so), kubernetes-specialist's shape.
#   4. store-fallback and kind-none skills get no update invocations.
#   5. Failure isolation: the stub exits non-zero for one skill; the run logs
#      a WARN naming it, keeps going (a later profile's update still runs),
#      and the whole run still exits 0.
#   6. "Blocked" output on exit 0 is a loud warning too, not a success,
#      updates re-apply the install gate, and a block must reach the operator.
#   7. --install-only never reaches the phase (it is network-dependent).
#   8. --dry-run prints would-update lines and invokes hermes zero times.
#   9. A machine without hermes on PATH but a NON-EMPTY hermesRegistry records a
#      REQUIRED failure (item 4: a missing prerequisite with work to do is not a
#      silent success), warns loudly, and still exits 0.
set -euo pipefail

# When git runs a hook such as pre-commit (this test runs under one via
# `just test`), it exports GIT_DIR/GIT_INDEX_FILE, which point every later git
# command at the OUTER repository. Unset them so nothing here can reach it.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

scratch_dir="$(mktemp -d)"
trap 'rm -rf "$scratch_dir"' EXIT

# Scratch HOME: the script derives every path from $HOME.
HOME="$scratch_dir/home"
export HOME
mkdir -p "$HOME/.agents/skills"

# The fixture lock. hermesRegistry shapes mirror production (profiles per
# entry, keyed by lockKey):
#   alpha    ClawHub slug != name (lockKey alpha-slug), concerned
#   beta     nicodemus, runs AFTER elaine's failure (isolation)
#   gamma    default only, walked like any other profile (un-entangled)
#   delta    held: true, nicodemus, must be skipped, visibly
#   epsilon  store-symlinked (hermesProfiles butters, NOT in hermesRegistry),
#            never a hermes update
#   failer   elaine, the stub exits 1 for its lockKey
#   blocked  concerned, the stub prints "Blocked" and exits 0
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"anchor": "core"},
  "hermesProfiles": {
    "epsilon": ["butters"]
  },
  "hermesRegistry": {
    "alpha": {"profiles": ["concerned"], "source": "clawhub", "identifier": "clawhub/alpha-slug", "lockKey": "alpha-slug"},
    "beta": {"profiles": ["nicodemus"], "source": "skills.sh", "identifier": "skills-sh/fixture/beta", "lockKey": "beta"},
    "gamma": {"profiles": ["default"], "source": "skills.sh", "identifier": "skills-sh/fixture/gamma", "lockKey": "gamma"},
    "delta": {"profiles": ["nicodemus"], "source": "skills.sh", "identifier": "skills-sh/fixture/delta", "lockKey": "delta", "held": true},
    "failer": {"profiles": ["elaine"], "source": "clawhub", "identifier": "clawhub/failer", "lockKey": "failer-key"},
    "blocked": {"profiles": ["concerned"], "source": "clawhub", "identifier": "clawhub/blocked", "lockKey": "blocked-key"}
  },
  "npxTracked": {"anchor": {"repo": "fixture/pack"}},
  "clawhubTracked": {},
  "forks": {}
}
EOF
# anchor: an npx-tracked store real dir so the tracked union is non-empty (the
# zero-union gate refuses any full/install-only run otherwise). It migrates into
# a live generation and is orthogonal to the hermes registry phase under test.
mkdir -p "$HOME/.agents/skills/anchor"
printf -- '---\nname: anchor\ndescription: fixture\n---\n' >"$HOME/.agents/skills/anchor/SKILL.md"
printf '{"skills":{"anchor":{}}}\n' >"$HOME/.agents/.skill-lock.json"

# PATH stubs. hermes records argv (one line per invocation), fails for
# failer-key, prints a Blocked verdict for blocked-key; npx records and
# succeeds so the full run never reaches the network.
stub_dir="$scratch_dir/stubs"
mkdir -p "$stub_dir"
hermes_log="$scratch_dir/hermes-argv.log"
cat >"$stub_dir/hermes" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$hermes_log"
for arg in "\$@"; do
  if [[ \$arg == "failer-key" ]]; then
    echo "stub: update exploded" >&2
    exit 1
  fi
  if [[ \$arg == "blocked-key" ]]; then
    echo "Blocked: security scan verdict caution"
    exit 0
  fi
done
echo "Updated"
EOF
cat >"$stub_dir/npx" <<EOF
#!/usr/bin/env bash
printf 'npx %s\n' "\$*" >>"$scratch_dir/npx.log"
echo "stub: nothing to update"
EOF
chmod +x "$stub_dir/hermes" "$stub_dir/npx"
export PATH="$stub_dir:$PATH"

# ── full run: the weekly shape ──────────────────────────────────────────────
output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" || fail "full run exited non-zero despite failure isolation: $output"

# 1) Exactly the registry set, keyed by lockKey.
for expected in \
  "-p concerned skills update alpha-slug" \
  "-p nicodemus skills update beta" \
  "-p elaine skills update failer-key" \
  "-p concerned skills update blocked-key" \
  "-p default skills update gamma"; do
  grep -Fxq -- "$expected" "$hermes_log" ||
    fail "missing hermes invocation '$expected'; got: $(cat "$hermes_log")"
done
if grep -Fxq -- "-p concerned skills update alpha" "$hermes_log"; then
  fail "an invocation used the skill name (alpha) instead of the lockKey (alpha-slug)"
fi
[[ "$(wc -l <"$hermes_log" | tr -d ' ')" == "5" ]] ||
  fail "expected exactly 5 hermes invocations, got: $(cat "$hermes_log")"

# 2) default is walked like any other profile (un-entanglement done).
grep -Fxq -- "-p default skills update gamma" "$hermes_log" ||
  fail "default-profile skill gamma was not updated (default is un-entangled and must be walked)"

# 3) held skipped, visibly.
grep -q "delta" "$hermes_log" && fail "held skill delta was updated"
printf '%s\n' "$output" | grep -qi "held" || fail "held skip was not reported: $output"

# 4) store-fallback never updated.
grep -q "epsilon" "$hermes_log" && fail "store-fallback skill epsilon got a hermes update"

# 5) failure isolation: WARN names the failer, beta (a later profile) still
#    ran, and the run exited 0 (asserted at the top).
printf '%s\n' "$output" | grep -i "warn" | grep -q "failer-key" ||
  fail "no WARN naming the failed update: $output"
grep -Fxq -- "-p nicodemus skills update beta" "$hermes_log" ||
  fail "a failure aborted later profiles (beta never ran)"

# 6) Blocked output on exit 0 is warned about.
printf '%s\n' "$output" | grep -i "warn" | grep -q "blocked-key" ||
  fail "a Blocked update result was not surfaced as a warning: $output"

# 7) --install-only skips the phase.
: >"$hermes_log"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only >/dev/null 2>&1 || fail "--install-only run failed"
[[ ! -s $hermes_log ]] || fail "--install-only reached the hermes phase: $(cat "$hermes_log")"

# 8) --dry-run reports and never invokes.
: >"$hermes_log"
dry_output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --dry-run 2>&1)" || fail "--dry-run run failed"
[[ ! -s $hermes_log ]] || fail "--dry-run invoked hermes: $(cat "$hermes_log")"
printf '%s\n' "$dry_output" | grep -q "would update" || fail "--dry-run did not report would-update lines: $dry_output"

# 9) hermes off PATH with a NON-EMPTY hermesRegistry: a REQUIRED failure (item
#    4), warned loudly, run still exits 0. The stripped PATH keeps npx (stub),
#    jq, and git so only hermes is missing.
no_hermes_dir="$scratch_dir/no-hermes"
mkdir -p "$no_hermes_dir"
cp "$stub_dir/npx" "$no_hermes_dir/npx"
ln -s "$(command -v jq)" "$no_hermes_dir/jq"
ln -s "$(command -v git)" "$no_hermes_dir/git"
missing_output="$(UPDATE_SKILLS_FORCE=1 PATH="$no_hermes_dir:/usr/bin:/bin" bash "$SCRIPT" 2>&1)" ||
  fail "run without hermes on PATH exited non-zero: $missing_output"
printf '%s\n' "$missing_output" | grep -qi "REQUIRED-FAILURE.*hermes" ||
  fail "missing hermes with a non-empty hermesRegistry did not record a required failure: $missing_output"
printf '%s\n' "$missing_output" | grep -qi "WITHHOLDING" ||
  fail "missing hermes prerequisite did not withhold the weekly stamp: $missing_output"

echo "update-skills-hermes-phase: OK"
