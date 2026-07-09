#!/usr/bin/env bash
# update-skills-refresh-scope.sh — the store refresh is npx-native: it runs
# `npx skills update` and NEVER git-clones a skill to install it. The git-pin
# supply-chain machinery (clone -> checkout pin -> tree-hash -> swap) is gone,
# so a full run's only legitimate git use is the fork/vendored upstream
# drift-check — and with no forks entries there is nothing to clone at all.
#
# Setup: a scratch HOME with a lock whose one npx-tracked skill is already
# present in the store (so the install pass has nothing to add) and no forks,
# plus PATH shims for git and npx that log every invocation instead of touching
# the network. Assertions: a full (non --install-only) run still calls
# `npx skills update`, and never invokes git — proving no residual git-pin
# install path and no clone of any retired upstream.
#
# The clawhub update pass rides the same full run: two clawhub-tracked skills
# are already present in the store, so the pass must per-skill `clawhub update`
# them in the store workdir (--workdir $HOME/.agents --dir skills, bare store
# name — never install, never --force), scrub Finder .DS_Store litter first
# (it breaks the CLI's fingerprint match), and when the CLI refuses with
# "local changes" because of the repo-asserted Codex overlay (the one local
# file this repo writes into tracked skill dirs), set exactly that file aside
# and retry once — the overlay assert re-creates it later in the same run.
set -euo pipefail

# When git runs a hook such as pre-commit (this test runs under one via
# `just test`), it exports GIT_DIR/GIT_INDEX_FILE, which point every later git
# command at the OUTER repository. Unset them so nothing here can reach it.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

# One npx-tracked skill, already present in the store: the install pass must
# skip it (present), and the refresh is the npx update pass's job. Two
# clawhub-tracked skills, also present: tracked-claw updates cleanly (its
# .DS_Store must be scrubbed first); refused-claw carries the repo-asserted
# Codex overlay, which makes the real CLI refuse with "local changes" — the
# updater must set exactly that file aside and retry once.
mkdir -p "$HOME/.agents/skills/tracked-skill"
printf -- '---\nname: tracked-skill\ndescription: present\n---\n' >"$HOME/.agents/skills/tracked-skill/SKILL.md"
mkdir -p "$HOME/.agents/skills/tracked-claw"
printf -- '---\nname: tracked-claw\ndescription: present\n---\n' >"$HOME/.agents/skills/tracked-claw/SKILL.md"
touch "$HOME/.agents/skills/tracked-claw/.DS_Store"
mkdir -p "$HOME/.agents/skills/refused-claw/agents"
printf -- '---\nname: refused-claw\ndescription: present\n---\n' >"$HOME/.agents/skills/refused-claw/SKILL.md"
printf 'policy:\n  allow_implicit_invocation: false' >"$HOME/.agents/skills/refused-claw/agents/openai.yaml"
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"tracked-skill": "core", "tracked-claw": "core", "refused-claw": "on-demand"},
  "hermesProfiles": {"tracked-skill": [], "tracked-claw": [], "refused-claw": []},
  "hermesRegistry": {},
  "npxTracked": {"tracked-skill": {"repo": "fixture/tracked-skill"}},
  "clawhubTracked": {
    "tracked-claw": {"slug": "@fixture/tracked-claw", "registry": "https://clawhub.example"},
    "refused-claw": {"slug": "@fixture/refused-claw", "registry": "https://clawhub.example"}
  },
  "forks": {}
}
EOF

# PATH shims: log every git/npx invocation, never touch the network.
shim_dir="$scratch_dir/shims"
mkdir -p "$shim_dir"
cat >"$shim_dir/git" <<EOF
#!/usr/bin/env bash
printf 'git %s\n' "\$*" >>"$scratch_dir/git.log"
exit 1
EOF
cat >"$shim_dir/npx" <<EOF
#!/usr/bin/env bash
printf 'npx %s\n' "\$*" >>"$scratch_dir/npx.log"
echo "shim: nothing to update"
EOF
chmod +x "$shim_dir/git" "$shim_dir/npx"

# The clawhub shim: logs every invocation; `update refused-claw` refuses with
# the real CLI's "local changes" line for as long as the overlay file exists
# (mirroring the real fingerprint mismatch — verified live on v0.23.1), and
# succeeds once the updater has set the overlay aside. Refusal exits 0, like
# the real CLI.
cat >"$shim_dir/clawhub" <<EOF
#!/usr/bin/env bash
printf 'clawhub %s\n' "\$*" >>"$scratch_dir/clawhub.log"
args=("\$@")
skill="\${args[\${#args[@]} - 1]}"
if [[ \$skill == "refused-claw" && -e "\$HOME/.agents/skills/refused-claw/agents/openai.yaml" ]]; then
  echo "refused-claw: local changes (no match). Use --force to overwrite."
else
  echo "shim: \$skill up to date"
fi
EOF
chmod +x "$shim_dir/clawhub"
export PATH="$shim_dir:$PATH"

# Full run (NOT --install-only): exercises every refresh pass.
output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" || fail "full run exited non-zero: $output"

# 1) The npx-native refresh is alive.
grep -q 'skills@latest update' "$scratch_dir/npx.log" 2>/dev/null ||
  fail "full run never invoked 'npx skills update' (npx-tracked refresh lost)"

# 2) The install pass never git-clones (the git-pin machinery is gone), and
#    with no forks entries the drift-check clones nothing either — so git is
#    never invoked at all.
if [[ -s "$scratch_dir/git.log" ]]; then
  fail "full run invoked git despite the npx-native model and no forks: $(cat "$scratch_dir/git.log")"
fi

# 3) None of the retired git-pin upstreams is ever cloned (belt-and-suspenders
#    on assertion 2: no roster skill is git-installed anymore).
for retired_repo in knowledge-work-plugins anthropics/skills vercel-labs/agent-skills heygen-com/hyperframes; do
  if grep -q "$retired_repo" "$scratch_dir/git.log" 2>/dev/null; then
    fail "full run git-cloned $retired_repo (installs are npx-native now, not git-pin)"
  fi
done

# 4) The clawhub update pass refreshes each tracked skill IN the store: bare
#    store name, --workdir $HOME/.agents --dir skills, per-skill (never --all),
#    and — both skills being present — never an install.
grep -q -- "--workdir $HOME/.agents --dir skills update tracked-claw" "$scratch_dir/clawhub.log" 2>/dev/null ||
  fail "full run never invoked 'clawhub update tracked-claw' against the store workdir: $(cat "$scratch_dir/clawhub.log" 2>/dev/null)"
if grep -qE '(^| )install( |$)' "$scratch_dir/clawhub.log"; then
  fail "the clawhub pass ran an install for a present skill: $(cat "$scratch_dir/clawhub.log")"
fi

# 5) Finder litter is scrubbed before the update: .DS_Store breaks the real
#    CLI's fingerprint match, so the pass removes it first.
[[ ! -e "$HOME/.agents/skills/tracked-claw/.DS_Store" ]] ||
  fail "tracked-claw's .DS_Store survived the update pass (it breaks clawhub's fingerprint match)"

# 6) The refusal ladder: the repo-asserted overlay makes the CLI refuse with
#    "local changes"; the pass sets exactly that file aside and retries once —
#    two update invocations for refused-claw, never --force — and the overlay
#    is back afterwards (re-asserted from the tiers table later in the run).
refused_updates="$(grep -c 'update refused-claw' "$scratch_dir/clawhub.log" || true)"
[[ $refused_updates -eq 2 ]] ||
  fail "expected exactly 2 'update refused-claw' invocations (refusal, then retry with the overlay set aside); got $refused_updates: $(cat "$scratch_dir/clawhub.log")"
if grep -q -- '--force' "$scratch_dir/clawhub.log"; then
  fail "the clawhub pass passed a --force flag (automation must never force): $(cat "$scratch_dir/clawhub.log")"
fi
overlay="$HOME/.agents/skills/refused-claw/agents/openai.yaml"
[[ -f $overlay ]] || fail "refused-claw's Codex overlay was not re-asserted after the update pass set it aside"
grep -q 'allow_implicit_invocation: false' "$overlay" ||
  fail "refused-claw's re-asserted overlay has wrong content: $(<"$overlay")"

echo "update-skills-refresh-scope: OK"
