#!/usr/bin/env bash
# update-skills-install.sh, proves update-skills.sh's npx install pass and
# fan-out, offline.
#
# The real script runs unmodified in a sandbox: a scratch HOME and a PATH stub
# for `npx` that stands in for the network, on `skills ... add ... --skill
# <name> ...` it writes ~/.agents/skills/<name>/SKILL.md (the real dir the
# multi-agent add lands in the store), the legitimate subprocess double.
# Assertions:
#   1. An absent npx-tracked skill ("goodskill") is installed into the store.
#   2. Its fan-out follows the lock: a Claude symlink always, plus one hermes
#      symlink per hermesProfiles mapping, the default profile's skills dir
#      and a specialist profile's, the latter created by the run itself.
#   3. An on-demand skill (per the lock's tiers table) gets the Codex policy
#      overlay agents/openai.yaml written into its store folder, and a deleted
#      overlay is re-asserted by the next run, the property that survives npx
#      refreshes replacing a skill's folder wholesale. When the upstream skill
#      already ships agents/openai.yaml with its own content, the policy block
#      is APPENDED idempotently, never an overwrite.
#   4. A core skill mapped to no hermes profile ("plainskill", hermesProfiles
#      []) still installs and reaches Claude, but gets NO hermes symlink and
#      NO Codex overlay, unmapped means deliberately absent, not defaulted.
#   5. A hermes-OWNED skill ("hubskill", hermesProfiles [] + a hermesRegistry
#      entry) installs into the store and reaches Claude, but gets NO hermes
#      symlink: hermes maintains its own hub-owned copy via `hermes skills
#      update`, and a store symlink would shadow it. Only skills with a
#      non-empty hermesProfiles mapping fan out to hermes.
#   6. A skill already in the store is never reinstalled (a planted marker file
#      survives a second run), routine runs cannot clobber local edits.
#   7. A store entry that is a SYMLINK (app-owned content, like cua-driver ->
#      ~/.cua-driver) never receives a Codex overlay.
#   8. A clawhub-tracked skill ("clawskill") absent from the store is installed
#      by the clawhub pass: the CLI is invoked with an explicit --workdir/--dir
#      pair and the lock's --registry, the nested <dir>/@owner/<name> layout it
#      produces is flattened into the store as <store>/<name>, no @owner dir
#      ever reaches the store, fan-out matches the lock, the on-demand overlay
#      lands, a present skill is never reinstalled, and --force is never passed.
set -euo pipefail

# This test runs inside the pre-commit hook, and git makes hooks inherit
# GIT_DIR/GIT_INDEX_FILE, every child git command then targets the OUTER repo.
# Unset so nothing here can reach it.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# The script derives every path (~/.agents, ~/.claude, ~/.hermes) from $HOME,
# so swapping this one variable redirects its entire blast radius into the
# sandbox.
HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"

# The npx stub: on `skills ... add ... --skill <name> ...` it materialises the
# store dir the real multi-agent add would land; on anything else (update) it
# just succeeds. Never touches the network.
stub_dir="$tmp/stubs"
mkdir -p "$stub_dir"
cat >"$stub_dir/npx" <<EOF
#!/usr/bin/env bash
set -euo pipefail
mode=""
skill=""
prev=""
for arg in "\$@"; do
  case "\$arg" in
    add) mode="add" ;;
    update) mode="update" ;;
  esac
  [[ \$prev == "--skill" || \$prev == "-s" ]] && skill="\$arg"
  prev="\$arg"
done
if [[ \$mode == "add" && -n \$skill ]]; then
  mkdir -p "\$HOME/.agents/skills/\$skill"
  printf -- '---\nname: %s\ndescription: fixture skill\n---\n# %s\n' "\$skill" "\$skill" \
    >"\$HOME/.agents/skills/\$skill/SKILL.md"
  echo "stub: installed \$skill"
else
  echo "stub: nothing to update"
fi
EOF
chmod +x "$stub_dir/npx"

# The clawhub stub: logs every invocation, and on `install <slug>` materialises
# the nested <workdir>/<dir>/@owner/<name> layout the real CLI produces (v0.23.1
# always nests when installing by @owner/name, verified live). The updater is
# responsible for flattening that into the store.
cat >"$stub_dir/clawhub" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf 'clawhub %s\n' "\$*" >>"$tmp/clawhub.log"
workdir=""
dir="skills"
mode=""
prev=""
for arg in "\$@"; do
  case "\$prev" in
    --workdir) workdir="\$arg" ;;
    --dir) dir="\$arg" ;;
  esac
  case "\$arg" in
    install | update) mode="\$arg" ;;
  esac
  prev="\$arg"
done
args=("\$@")
slug="\${args[\${#args[@]} - 1]}"
if [[ \$mode == "install" ]]; then
  dest="\$workdir/\$dir/\$slug"
  mkdir -p "\$dest/.clawhub"
  printf -- '---\nname: %s\ndescription: fixture skill\n---\n' "\$(basename "\$slug")" >"\$dest/SKILL.md"
  printf '{"version":1,"slug":"%s"}\n' "\$(basename "\$slug")" >"\$dest/.clawhub/origin.json"
  echo "stub: installed \$slug"
else
  echo "stub: \$slug up to date"
fi
EOF
chmod +x "$stub_dir/clawhub"
export PATH="$stub_dir:$PATH"

# The fixture lock. goodskill is on-demand, mapped to the default profile plus a
# specialist (fans to hermes); plainskill is core and deliberately mapped
# nowhere; hubskill is hermes-OWNED (hermesProfiles [] + a hermesRegistry
# entry), so the store must not fan it out; clawskill is clawhub-tracked
# (installed by the clawhub pass, fans to the default profile).
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {
    "goodskill": "on-demand",
    "plainskill": "core",
    "hubskill": "on-demand",
    "clawskill": "on-demand"
  },
  "hermesProfiles": {
    "goodskill": ["default", "specialist"],
    "plainskill": [],
    "hubskill": [],
    "clawskill": ["default"]
  },
  "hermesRegistry": {
    "hubskill": {"profiles": ["default"], "source": "clawhub", "identifier": "clawhub/hubskill-slug", "lockKey": "hubskill-slug"}
  },
  "npxTracked": {
    "goodskill": {"repo": "fixture/goodskill"},
    "plainskill": {"repo": "fixture/plainskill"},
    "hubskill": {"repo": "fixture/hubskill"}
  },
  "clawhubTracked": {
    "clawskill": {"slug": "@fixture/clawskill", "registry": "https://clawhub.example"}
  },
  "forks": {}
}
EOF

# The real script, unmodified. FORCE bypasses the idle-gate, which would
# otherwise refuse to run while a harness (the one executing this test) is up.
output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only 2>&1)" || fail "update-skills.sh --install-only exited non-zero: $output"

# 1) Absent npx-tracked skills get installed into the store.
[[ -f "$HOME/.agents/skills/goodskill/SKILL.md" ]] || fail "goodskill was not installed into the store"
[[ -f "$HOME/.agents/skills/plainskill/SKILL.md" ]] || fail "plainskill was not installed into the store"

# 2) Fan-out follows the lock (Codex needs no symlinks: it scans
#    ~/.agents/skills natively). goodskill maps to the default hermes profile
#    plus the "specialist" profile, whose skills dir the run must create.
for link in "$HOME/.claude/skills/goodskill" "$HOME/.hermes/skills/goodskill" \
  "$HOME/.hermes/profiles/specialist/skills/goodskill"; do
  [[ -L $link ]] || fail "missing fan-out symlink: $link"
  [[ -f "$link/SKILL.md" ]] || fail "fan-out symlink does not resolve: $link"
done

# 3) The lock marks goodskill on-demand, so the run must write the Codex policy
#    overlay into its store folder.
overlay="$HOME/.agents/skills/goodskill/agents/openai.yaml"
expected_policy=$'policy:\n  allow_implicit_invocation: false'
[[ -f $overlay ]] || fail "no Codex overlay was written for on-demand skill goodskill"
[[ "$(<"$overlay")" == "$expected_policy" ]] ||
  fail "Codex overlay for goodskill has wrong content: $(<"$overlay")"

# 4) Core + unmapped stays out of hermes and carries no overlay: reaching
#    Claude is the only fan-out an empty hermesProfiles list permits.
[[ -L "$HOME/.claude/skills/plainskill" ]] || fail "missing Claude fan-out symlink for plainskill"
[[ ! -e "$HOME/.hermes/skills/plainskill" ]] ||
  fail "plainskill (hermesProfiles []) was symlinked into the default hermes profile"
[[ ! -e "$HOME/.hermes/profiles/specialist/skills/plainskill" ]] ||
  fail "plainskill (hermesProfiles []) was symlinked into a specialist hermes profile"
[[ ! -e "$HOME/.agents/skills/plainskill/agents" ]] ||
  fail "plainskill (core tier) got a Codex overlay"

# 5) hermes-owned stays out of hermes: hermes maintains its own hub-owned copy
#    (`hermes skills update` keys off the mechanism's lockKey); a store symlink
#    at that path would shadow it. hubskill has hermesProfiles [], so the
#    fan-out plants no hermes link.
[[ -f "$HOME/.agents/skills/hubskill/SKILL.md" ]] || fail "hubskill was not installed into the store"
[[ -L "$HOME/.claude/skills/hubskill" ]] || fail "missing Claude fan-out symlink for hubskill"
[[ ! -e "$HOME/.hermes/skills/hubskill" ]] ||
  fail "hubskill (hermes-owned) was symlinked into the default hermes profile, store fan-out is hermesProfiles-only"

# 8) The clawhub pass installs an absent clawhub-tracked skill: the nested
#    @owner layout the CLI produces is flattened into the store under the
#    roster name, the @owner dir never reaches the store, fan-out and the
#    on-demand overlay follow the lock, and the invocation targets an explicit
#    --workdir/--dir pair with the lock's registry, never --force.
[[ -f "$HOME/.agents/skills/clawskill/SKILL.md" ]] || fail "clawskill was not installed into the store"
[[ -f "$HOME/.agents/skills/clawskill/.clawhub/origin.json" ]] ||
  fail "clawskill's .clawhub/origin.json did not survive the flatten into the store"
[[ ! -e "$HOME/.agents/skills/@fixture" ]] ||
  fail "the clawhub install's @owner dir leaked into the store (must be flattened to <store>/<name>)"
[[ -L "$HOME/.claude/skills/clawskill" ]] || fail "missing Claude fan-out symlink for clawskill"
[[ -L "$HOME/.hermes/skills/clawskill" ]] ||
  fail "clawskill (hermesProfiles [default]) was not symlinked into the default hermes profile"
[[ -f "$HOME/.agents/skills/clawskill/agents/openai.yaml" ]] ||
  fail "no Codex overlay was written for on-demand clawhub-tracked skill clawskill"
grep -q -- 'install @fixture/clawskill' "$tmp/clawhub.log" ||
  fail "clawhub install was not invoked with the lock's slug: $(cat "$tmp/clawhub.log")"
grep -q -- '--dir skills' "$tmp/clawhub.log" ||
  fail "clawhub install was not invoked with an explicit --dir: $(cat "$tmp/clawhub.log")"
grep -q -- '--workdir' "$tmp/clawhub.log" ||
  fail "clawhub install was not invoked with an explicit --workdir: $(cat "$tmp/clawhub.log")"
grep -q -- '--registry https://clawhub.example' "$tmp/clawhub.log" ||
  fail "clawhub install did not pass the lock's registry: $(cat "$tmp/clawhub.log")"
if grep -q -- '--force' "$tmp/clawhub.log"; then
  fail "clawhub was invoked with a --force flag (automation must never force): $(cat "$tmp/clawhub.log")"
fi

# 6) A present skill is never reinstalled: plant a marker inside it, re-run,
#    assert the marker survived. This is the property that protects local
#    edits from being clobbered by a routine run.
# 3b) In the same re-run, a deleted overlay must come back: npx refreshes
#     replace a skill's folder wholesale, so the overlay's durability depends
#     on a build re-asserting it from the lock's tiers table.
# install-only only builds/publishes when at least one roster skill is ABSENT
# (an additive install that never exchanges nothing), so trigger a genuine
# rebuild by adding a fresh absent skill `extraskill` to the roster. The rebuild
# then clones the present skills forward (markers survive), re-asserts the
# deleted overlay, and installs the new absent skill.
touch "$HOME/.agents/skills/goodskill/local-edit.marker"
touch "$HOME/.agents/skills/clawskill/local-edit.marker"
rm "$overlay"
lock_with_extraskill="$(jq '.tiers.extraskill = "core"
  | .npxTracked.extraskill = {"repo": "fixture/extraskill"}' \
  "$HOME/.agents/custom-skill-lock.json")"
printf '%s\n' "$lock_with_extraskill" >"$HOME/.agents/custom-skill-lock.json"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only >/dev/null 2>&1 || fail "second --install-only run failed"
[[ -f "$HOME/.agents/skills/extraskill/SKILL.md" ]] || fail "the fresh absent skill was not installed by the rebuild"
[[ -f "$HOME/.agents/skills/goodskill/local-edit.marker" ]] || fail "install pass reinstalled a skill that was already present"
[[ -f "$HOME/.agents/skills/clawskill/local-edit.marker" ]] ||
  fail "clawhub install pass reinstalled a skill that was already present"
[[ -f $overlay ]] || fail "a deleted Codex overlay was not re-asserted by the next build"
[[ "$(<"$overlay")" == "$expected_policy" ]] ||
  fail "re-asserted Codex overlay for goodskill has wrong content: $(<"$overlay")"

# 3c) An on-demand skill whose upstream ALREADY ships agents/openai.yaml with
#     its own content (the official hyperframes-keyframes carries an
#     interface: block there) must keep that content: the policy block is
#     APPENDED, never a whole-file overwrite. Upstream metadata survives.
meta_dir="$HOME/.agents/skills/metaskill"
mkdir -p "$meta_dir/agents"
printf -- '---\nname: metaskill\ndescription: fixture skill\n---\n# Meta\n' >"$meta_dir/SKILL.md"
printf 'interface:\n  display_name: "Meta Skill"\n' >"$meta_dir/agents/openai.yaml"
lock_with_metaskill="$(jq '.tiers.metaskill = "on-demand" | .hermesProfiles.metaskill = []
  | .npxTracked.metaskill = {"repo": "fixture/metaskill"}' \
  "$HOME/.agents/custom-skill-lock.json")"
printf '%s\n' "$lock_with_metaskill" >"$HOME/.agents/custom-skill-lock.json"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only >/dev/null 2>&1 || fail "metaskill overlay run failed"
meta_overlay="$meta_dir/agents/openai.yaml"
grep -q 'display_name: "Meta Skill"' "$meta_overlay" ||
  fail "upstream openai.yaml content was destroyed by the overlay assert: $(<"$meta_overlay")"
grep -q 'allow_implicit_invocation: false' "$meta_overlay" ||
  fail "policy block was not added to an upstream-shipped openai.yaml: $(<"$meta_overlay")"
# The append must be idempotent: a second run adds nothing.
before_bytes="$(<"$meta_overlay")"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only >/dev/null 2>&1 || fail "metaskill idempotence run failed"
[[ "$(<"$meta_overlay")" == "$before_bytes" ]] ||
  fail "overlay append is not idempotent: $(<"$meta_overlay")"

# 7) An on-demand skill whose store entry is a SYMLINK (app-owned content, like
#    cua-driver -> ~/.cua-driver) must never receive an overlay: writing through
#    the link would modify content this repo does not own.
app_owned="$tmp/app/skills/appskill"
mkdir -p "$app_owned"
printf -- '---\nname: appskill\ndescription: fixture skill\n---\n# App\n' >"$app_owned/SKILL.md"
ln -s "$app_owned" "$HOME/.agents/skills/appskill"
lock_with_appskill="$(jq '.tiers.appskill = "on-demand" | .hermesProfiles.appskill = []
  | .npxTracked.appskill = {"repo": "fixture/appskill"}' \
  "$HOME/.agents/custom-skill-lock.json")"
printf '%s\n' "$lock_with_appskill" >"$HOME/.agents/custom-skill-lock.json"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only >/dev/null 2>&1 || fail "third --install-only run failed"
[[ ! -e "$app_owned/agents" ]] ||
  fail "overlay was written through the appskill symlink into app-owned content"

echo "update-skills-install: OK"
