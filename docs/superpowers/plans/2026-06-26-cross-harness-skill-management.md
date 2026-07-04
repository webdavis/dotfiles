# Cross-Harness Skill Management — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `~/.agents/skills` the single physical store for portable skills, symlinked into Claude Code + Hermes (Codex reads it natively); install the Anthropic knowledge-work set + portable/Matt/moshi-herdr skills; remove paseo + adhd-assistant; leave Hermes's adapted catalog intact; add a safe scheduled updater. All on `feat/cli-agent-tracking-workflow`.

**Architecture:** One canonical store; per-skill symlinks fan out to `~/.claude/skills` and `~/.hermes/skills`. Portable skills are npx/clawhub/github-sourced; Hermes-adapted skills stay Hermes-only. A single launchd-scheduled updater refreshes everything per-provenance with atomic swaps + a skip-list.

**Tech Stack:** bash (`set -euo pipefail`), `npx skills`, `clawhub`, `git`, `jq`, `rsync`, `launchd`, chezmoi.

**Spec:** `docs/superpowers/specs/2026-06-26-cross-harness-skill-management-design.md` (read it; it holds the explicit 129-skill knowledge-work map by category).

## Global Constraints
- Canonical store: `~/.agents/skills/<skill>/` (real dirs). Harness views: `~/.claude/skills/<skill>` and `~/.hermes/skills/<skill>` are **relative symlinks** `../../.agents/skills/<skill>`; Codex reads the store natively (no symlink).
- **Back up** (tar to `~/workspaces/backups/<UTC>.<name>.backup.tar.gz`) before any deletion/overwrite.
- **Atomic swap** for any in-place content overwrite (temp dir on same FS → `mv -Tf`). Never `rm`-then-recreate a live skill dir.
- **Skip-list** (never updated/overwritten by tooling): `video-transcript-downloader` + any user-edited copy.
- **code-review:** REMOVE the existing clawhub copy; the Anthropic KW engineering `code-review` keeps the canonical `code-review` name (not namespaced) — per user decision.
- **Collisions** kept both, category-namespaced (4): `marketing-competitive-brief`, `product-management-competitive-brief`, `legal-review-contract`, `small-business-review-contract`.
- Commits on `feat/cli-agent-tracking-workflow`, conventional-commits style, **no Co-Authored-By / no AI trailer**, `SKIP_AI_COMMIT=1` to bypass the haiku hook.
- "Tests" here = state-verification commands (`ls`/`readlink`/`jq`/`find`), not unit tests.
- Vars used below: `A=$HOME/.agents`, `S=$A/skills`, `C=$HOME/.claude/skills`, `H=$HOME/.hermes/skills`, `DF=$HOME/workspaces/Ivy/webdavis/dotfiles`.

---

### Task 1: Backups + paseo removal + adhd-assistant hub-lock purge

**Files:** `~/.agents/skills/*`, `~/.agents/.skill-lock.json`, `~/.claude/skills/*`, `~/.codex/skills/*`, `~/.hermes/skills/*` (+ `.hub/lock.json`), `~/.hermes/config.yaml`.

- [ ] **Step 1: Full backup**
```bash
set -euo pipefail
ts=$(date -u +"%Y-%m-%dT%H-%M-%S"); b=$HOME/workspaces/backups; mkdir -p "$b"
tar -czf "$b/$ts.agents-skills.backup.tar.gz" -C "$HOME/.agents" skills .skill-lock.json
tar -czhf "$b/$ts.hermes-skills.backup.tar.gz" -C "$HOME/.hermes" skills
cp "$HOME/.hermes/config.yaml" "$b/$ts.hermes-config.backup.yaml"
echo "backed up to $b ($ts)"
```

- [ ] **Step 2: Remove paseo skills everywhere + from the lock**
```bash
set -uo pipefail
A=$HOME/.agents; S=$A/skills
for p in paseo paseo-advisor paseo-committee paseo-handoff paseo-loop paseo-epic; do
  for d in "$S" "$HOME/.claude/skills" "$HOME/.codex/skills" "$HOME/.hermes/skills"; do
    t="$d/$p"; { [ -e "$t" ] || [ -L "$t" ]; } && { rm -rf "$t"; echo "removed $t"; }
  done
done
tmp=$(mktemp); jq 'if .skills then .skills |= with_entries(select(.key|test("^paseo")|not)) else . end' "$A/.skill-lock.json" > "$tmp" && mv "$tmp" "$A/.skill-lock.json"
```

- [ ] **Step 3: Purge adhd-assistant from Hermes hub lock** (skill dir already deleted)
```bash
HL=$HOME/.hermes/skills/.hub/lock.json
cp "$HL" "$HOME/workspaces/backups/$(date -u +%Y-%m-%dT%H-%M-%S).hermes-hub-lock.backup.json"
# inspect first; remove the adhd-assistant entry by its actual shape:
jq '(.skills? // .) ' "$HL" >/dev/null 2>&1   # confirm parseable
tmp=$(mktemp)
jq 'walk(if type=="object" then with_entries(select((.key|test("adhd-assistant"))|not)) elif type=="array" then map(select((tostring|test("adhd-assistant"))|not)) else . end)' "$HL" > "$tmp" && mv "$tmp" "$HL"
```

- [ ] **Step 4: Verify**
```bash
echo "paseo left: $(find "$HOME/.agents/skills" "$HOME/.claude/skills" "$HOME/.codex/skills" "$HOME/.hermes/skills" -maxdepth 1 -name 'paseo*' 2>/dev/null | wc -l | tr -d ' ')"  # expect 0
echo "adhd in hub lock: $(grep -c adhd-assistant "$HOME/.hermes/skills/.hub/lock.json")"  # expect 0
```
Expected: `paseo left: 0`, `adhd in hub lock: 0`.

- [ ] **Step 5: Commit** (after Task 8 chezmoi-add; lock/store changes are chezmoi targets — see Task 8). Mark this task's state verified.

---

### Task 2: Install Anthropic knowledge-work skills + namespace collisions

**Files:** `~/.agents/skills/<kw-skill>/`, `~/.agents/.skill-lock.json`, symlinks in `$C` and `$H`.

**Interfaces — Produces:** 129 KW skills in the store (14 categories per spec), 5 renamed (namespaced), each symlinked into Claude + Hermes.

- [ ] **Step 1: Install each included category** (excludes customer-support, productivity, pdf-viewer, partner-built)
```bash
set -uo pipefail
for cat in bio-research cowork-plugin-management data design engineering enterprise-search finance human-resources legal marketing operations product-management sales small-business; do
  echo "== $cat =="
  npx --yes skills@latest add "anthropics/knowledge-work-plugins/$cat" --full-depth --global --agent claude-code -y 2>&1 | tr -d '\r' | tail -3
done
```

- [ ] **Step 2: Namespace the 5 collisions** (rename store dir; the originals come from KW so re-key the lock if present)
```bash
set -uo pipefail; S=$HOME/.agents/skills
ren() { src="$S/$1"; dst="$S/$2"; [ -d "$src" ] && [ ! -e "$dst" ] && { mv "$src" "$dst"; echo "renamed $1 -> $2"; }; }
# code-review collides with existing clawhub code-review: rename ONLY the KW copy. The KW add may have refused/overwritten; verify which code-review is present, restore clawhub one from backup if overwritten, then place KW one as engineering-code-review.
ren competitive-brief marketing-competitive-brief     # if both marketing+pm installed, the 2nd add kept one; reinstall the other explicitly into a temp + place (see Step 2b)
ren review-contract  legal-review-contract
```

- [ ] **Step 2b: Resolve the same-name-across-category installs** (npx flat-installs by skill name, so marketing & PM competitive-brief, and legal & SB review-contract, collide during Step 1 — only one survived each). Reinstall each missing twin into a temp store and place it namespaced:
```bash
set -uo pipefail; S=$HOME/.agents/skills; tmp=$(mktemp -d)
fetch() { # $1=category/skills/skill  $2=dest-name
  git clone --depth 1 --filter=blob:none --sparse https://github.com/anthropics/knowledge-work-plugins "$tmp/r" 2>/dev/null || true
  git -C "$tmp/r" sparse-checkout set "$1" 2>/dev/null
  rsync -a "$tmp/r/$1/" "$S/$2/"; echo "placed $2"
}
fetch marketing/skills/competitive-brief        marketing-competitive-brief
fetch product-management/skills/competitive-brief product-management-competitive-brief
fetch legal/skills/review-contract              legal-review-contract
fetch small-business/skills/review-contract     small-business-review-contract
fetch engineering/skills/code-review            engineering-code-review
# remove any unprefixed competitive-brief / review-contract that Step 1 left (they're now the namespaced copies)
for x in competitive-brief review-contract; do [ -d "$S/$x" ] && rm -rf "$S/$x"; done
rm -rf "$tmp"
```

- [ ] **Step 3: Symlink every KW skill into Hermes** (Claude got symlinks via `--agent claude-code`; Codex native)
```bash
set -uo pipefail; S=$HOME/.agents/skills; H=$HOME/.hermes/skills
# KW skills = those just added; symlink any store skill missing from Hermes:
for d in "$S"/*/; do n=$(basename "$d"); [ -e "$H/$n" ] || ln -s "../../.agents/skills/$n" "$H/$n"; done
```

- [ ] **Step 4: Verify**
```bash
S=$HOME/.agents/skills
echo "store skills: $(find "$S" -maxdepth 1 -mindepth 1 -type d | wc -l | tr -d ' ')"
for x in engineering-code-review marketing-competitive-brief product-management-competitive-brief legal-review-contract small-business-review-contract code-review; do
  [ -d "$S/$x" ] && echo "OK $x" || echo "MISSING $x"
done
readlink "$HOME/.claude/skills/analyze"; readlink "$HOME/.hermes/skills/analyze"
```
Expected: all 5 namespaced + `code-review` present; `analyze` symlinks resolve to `../../.agents/skills/analyze`.

---

### Task 3: Portable skills.sh set + lobster

**Files:** store + `$C`/`$H` symlinks; remove Hermes hub copies of these.

- [ ] **Step 1: Install/refresh the skills.sh portables** (skip vtd — keep your fork)
```bash
set -uo pipefail
for src in anthropics/skills/frontend-design jeffallan/claude-skills/kubernetes-specialist steipete/agent-scripts/peekaboo vercel-labs/agent-skills/web-design-guidelines doist/todoist-cli/todoist-cli sickn33/antigravity-awesome-skills/senior-architect; do
  npx --yes skills@latest add "$src" --global --agent claude-code -y 2>&1 | tr -d '\r' | tail -2
done
npx --yes skills@latest add guwidoe/lobster-skill --global --agent claude-code -y 2>&1 | tr -d '\r' | tail -2
```

- [ ] **Step 2: Symlink these into Hermes; remove stale Hermes hub copies if non-symlink**
```bash
set -uo pipefail; S=$HOME/.agents/skills; H=$HOME/.hermes/skills
for n in frontend-design kubernetes-specialist peekaboo web-design-guidelines todoist-cli senior-architect lobster video-transcript-downloader; do
  [ -e "$H/$n" ] && [ ! -L "$H/$n" ] && { rm -rf "$H/$n"; }   # drop Hermes real copy
  [ -e "$H/$n" ] || ln -s "../../.agents/skills/$n" "$H/$n"
done
```

- [ ] **Step 3: Verify** — each present in store, symlinked in Claude + Hermes, vtd unchanged
```bash
S=$HOME/.agents/skills
for n in frontend-design kubernetes-specialist peekaboo web-design-guidelines lobster; do [ -d "$S/$n" ] && echo "OK $n" || echo "MISS $n"; done
grep -q whisply "$S/video-transcript-downloader/SKILL.md" && echo "vtd fork intact (whisply fallback present)"
```
Expected: all OK; vtd fork intact.

---

### Task 4: Matt Pocock skills (reinstall 35)

- [ ] **Step 1: Install + Claude symlinks**
```bash
npx --yes skills@latest add mattpocock/skills --global --all 2>&1 | tr -d '\r' | tail -5
```
- [ ] **Step 2: Symlink all 35 into Hermes**
```bash
set -uo pipefail; S=$HOME/.agents/skills; H=$HOME/.hermes/skills; lock=$HOME/.agents/.skill-lock.json
for n in $(jq -r '.skills|to_entries[]|select((.value.source//"")|test("mattpocock"))|.key' "$lock"); do
  [ -e "$H/$n" ] || ln -s "../../.agents/skills/$n" "$H/$n"
done
```
- [ ] **Step 3: Verify** — `jq '[.skills|to_entries[]|select((.value.source//"")|test("mattpocock"))]|length' $lock` → 35; spot-check `readlink ~/.hermes/skills/grill-me`.

---

### Task 5: moshi + herdr → store symlinks; stop chezmoi vendoring

**Files:** `$C/moshi`, `$C/herdr` (convert real→symlink), `$DF/justfile` (remove `update-agent-skills` curl of these), `$DF/private_dot_claude/skills/{herdr,moshi}` (remove from chezmoi source).

- [ ] **Step 1: Replace Claude real dirs with store symlinks; add Hermes symlinks**
```bash
set -uo pipefail; S=$HOME/.agents/skills; C=$HOME/.claude/skills; H=$HOME/.hermes/skills
for n in moshi herdr; do
  [ -d "$S/$n" ] || { echo "WARN $n not in store"; continue; }
  [ -L "$C/$n" ] || { rm -rf "$C/$n"; ln -s "../../.agents/skills/$n" "$C/$n"; }
  [ -e "$H/$n" ] || ln -s "../../.agents/skills/$n" "$H/$n"
done
```
- [ ] **Step 2: Stop the justfile vendoring** — edit `$DF/justfile`, delete the `update-agent-skills` recipe's `curl ... private_dot_claude/skills/{herdr,moshi}/private_SKILL.md` lines; `git -C "$DF" rm -r --cached private_dot_claude/skills/herdr private_dot_claude/skills/moshi` and delete those source dirs.
- [ ] **Step 3: Verify** — `readlink $C/herdr $C/moshi` resolve to store; `grep -c update-agent-skills $DF/justfile` reflects the edit.

---

### Task 6: Unified safe skill updater (script + manifest + launchd)

**Files:**
- Create: `$DF/scripts/update-skills` (bash)
- Create: `$DF/dot_agents/private_skills-vendor.json` (chezmoi → `~/.agents/.skills-vendor.json`)
- Create: `$DF/private_Library/private_LaunchAgents/io.webdavis.update-skills.plist` (or your launchd convention)
- Modify: `$DF/justfile` (add `update-skills` recipe)

- [ ] **Step 1: Write the manifest** (vendored = the 5 namespaced; skip = forks)
```json
{ "vendored": [
  {"name":"engineering-code-review","repo":"anthropics/knowledge-work-plugins","ref":"main","path":"engineering/skills/code-review"},
  {"name":"marketing-competitive-brief","repo":"anthropics/knowledge-work-plugins","ref":"main","path":"marketing/skills/competitive-brief"},
  {"name":"product-management-competitive-brief","repo":"anthropics/knowledge-work-plugins","ref":"main","path":"product-management/skills/competitive-brief"},
  {"name":"legal-review-contract","repo":"anthropics/knowledge-work-plugins","ref":"main","path":"legal/skills/review-contract"},
  {"name":"small-business-review-contract","repo":"anthropics/knowledge-work-plugins","ref":"main","path":"small-business/skills/review-contract"}
], "skip": ["video-transcript-downloader"] }
```

- [ ] **Step 2: Write `scripts/update-skills`** — `set -euo pipefail`; flock lockfile; if any `claude|codex|hermes` process running → exit 0 (idle-gate); then: (a) `npx skills update --global -y`; (b) `clawhub update` per clawhub-managed skill NOT in skip-list, each into a temp dir then atomic `mv -Tf` swap into `$S/<name>`; (c) for each manifest `vendored` entry: sparse-clone `repo@ref` path → temp → atomic swap into `$S/<name>`; never touch `skip` entries. (Full script body: see Step 2 code block — write it complete, no placeholders.)
- [ ] **Step 3: Dry-run** — `bash $DF/scripts/update-skills --dry-run` prints planned swaps, makes no changes. Verify skip-list honored (vtd not listed).
- [ ] **Step 4: launchd plist** — weekly schedule calling the script; `launchctl load` (or document the chezmoi-applied path). Verify `launchctl list | grep update-skills`.
- [ ] **Step 5: Commit** the script + manifest + plist + justfile recipe on feat.

---

### Task 7: Audits (Hermes-native + Claude)

- [ ] **Step 1: Hermes-native audit** — for each `$H` entry that is NOT a store symlink, confirm tracked in `.bundled_manifest`, hub `lock.json`, or `.usage.json` `created_by:agent`. Output a list of any **orphans** (untracked) and any still carrying a `skills.sh` source.
```bash
set -uo pipefail; H=$HOME/.hermes/skills
for d in "$H"/*/; do n=$(basename "$d"); [ -L "${d%/}" ] && continue
  grep -qE "^$n:" "$H/.bundled_manifest" && continue
  grep -q "\"$n\"" "$H/.hub/lock.json" 2>/dev/null && continue
  jq -e --arg k "$n" '.[$k].created_by=="agent"' "$H/.usage.json" >/dev/null 2>&1 && continue
  echo "ORPHAN (untracked): $n"
done
```
- [ ] **Step 2: Claude-skills audit** — every `$C` entry is a store symlink or an intentional real dir; report dangling links + non-store real dirs.
```bash
C=$HOME/.claude/skills
find "$C" -maxdepth 1 -type l ! -exec test -e {} \; -print | sed 's/^/DANGLING: /'
find "$C" -maxdepth 1 -mindepth 1 -type d | sed 's/^/REAL-DIR: /'
```
- [ ] **Step 3:** Resolve findings (relink dangling, convert portable real dirs to store symlinks). Commit fixes.

---

### Task 8: Track on feat + final verification + report

- [ ] **Step 1: chezmoi-add the store + lock + hermes config + symlink state into the chezmoi source**
```bash
DF=$HOME/workspaces/Ivy/webdavis/dotfiles
chezmoi --source "$DF" re-add   # capture changed managed files (review the diff!)
chezmoi --source "$DF" add ~/.agents/skills ~/.agents/.skill-lock.json ~/.agents/.skills-vendor.json ~/.hermes/config.yaml
git -C "$DF" status -s
```
- [ ] **Step 2: Commit on feat** (conventional, no AI trailer)
```bash
SKIP_AI_COMMIT=1 git -C "$DF" add -A && SKIP_AI_COMMIT=1 git -C "$DF" commit -m "feat(skills): consolidate portable skills into ~/.agents/skills, add safe updater, remove paseo/adhd"
```
- [ ] **Step 3: Final verification (triple-check vs spec)** — run the audit twice:
```bash
S=$HOME/.agents/skills
echo "store skills: $(find "$S" -maxdepth 1 -mindepth 1 -type d|wc -l|tr -d ' ')"
echo "dangling symlinks (claude/hermes): $(for d in "$HOME/.claude/skills" "$HOME/.hermes/skills"; do find "$d" -maxdepth 1 -type l ! -exec test -e {} \; -print; done|wc -l|tr -d ' ')"  # expect 0
echo "name collisions resolved: $(ls "$S" | grep -cE '^(competitive-brief|review-contract)$')"  # expect 0
echo "vtd in skip-list + intact: $(jq -r '.skip[]' "$A/.skills-vendor.json" 2>/dev/null | grep -c video-transcript-downloader)"  # expect 1
```
- [ ] **Step 4: Produce the report** — per-skill table (name | class | store | claude link | hermes link | codex-native | source | update-path | status), counts, the 5 renamed, orphans flagged, updater dry-run result, and the commit SHA on `feat/cli-agent-tracking-workflow`.

---

## Self-review notes
- Spec coverage: Actions A–H all mapped (Task2=A, Task3=B+C, Task4=D, Task5=E, Task1=F, Task6=updater, Task7=G+H, Task8=tracking/verify/report). adhd-assistant already removed (Task1 step3 purges hub lock).
- Known sharp edges flagged inline: same-name-across-category KW installs (Task 2b), code-review-vs-existing collision (Task 2b), Hermes re-seed of removed hub copies (re-verify in Task 7), chezmoi `re-add` diff review (Task 8 Step 1 — review before commit).
