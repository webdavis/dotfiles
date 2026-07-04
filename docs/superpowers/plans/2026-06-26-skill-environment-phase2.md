# Skill Environment Phase 2 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]`.

**Goal:** Make the whole skill environment **manifest-driven and reproducible without committing vendored files**: one source manifest → updater installs/refreshes → chezmoi creates all symlinks. Plus the new installs, MCP servers, binaries, and two quality comparisons.

**Architecture:**
- **One committed source manifest** (`dot_agents/private_skills-manifest.json` → `~/.agents/.skills-manifest.json`): every vendored skill → `{name, installer, repo|slug, ref, path, namespace?}`. Installers: `github` (sparse-clone subpath), `skills` (`npx skills add`), `clawhub`, `npm-bin`.
- **Updater (`update-skills.sh`) becomes install+refresh, manifest-driven.** For each manifest entry: fetch from source → atomic-swap into `~/.agents/skills/<name>`. Idempotent (installs if missing, refreshes if present). Skip-list = forks/user-authored. Loaded weekly (already) + run on first `chezmoi apply`.
- **chezmoi creates the symlinks** via a `run_after_` hook that calls the updater's symlink step (NOT ~400 template files). It commits ONLY: the manifest, the updater, fork/user-authored skill *files*, the Hermes hub-lock, and Hermes agent-created skill files. **Vendored skill files are never committed.**
- **Dresden reproduction:** `chezmoi apply` (manifest + updater + forks + hermes hub-lock/agent-skills + MCP config) → updater runs → full set rebuilt from upstreams.

## Global Constraints
- Store: `~/.agents/skills/<skill>` real dirs; harness views are relative symlinks `../../.agents/skills/<skill>`; Codex reads the store natively.
- Atomic swap for every overwrite; back up before deletes; skip-list never auto-updated: `video-transcript-downloader` (fork) + any user-edited.
- Commit on `feat/cli-agent-tracking-workflow`, conventional commits, **no AI co-author trailer**, `SKIP_AI_COMMIT=1`.
- 4 namespaced KW collisions preserved: `marketing-/product-management-competitive-brief`, `legal-/small-business-review-contract`.
- Decisions (resolved): Hermes catalog → hub-lock + commit agent-created; lobster → keep guwidoe; deep-research → **keep local** (8-phase version beats parags/deep-research-pro); Hermes Google Workspace → **add `workspace-mcp` + keep the native skill**.

## Carried-over context & loose ends (from the originating session — READ FIRST)

**Starting state (verified 2026-06-26):** store = **203** real skills + `cua-driver` (external symlink); feat HEAD `416ee94`. Phase-1 commits on feat: `3cb45cb` (code-review decision), `ba84768` (updater infra + stop herdr/moshi vendoring), `ae0126b` (moshi/herdr/whisply/vtd tracked), `416ee94` (this plan). All destructive steps backed up to `~/workspaces/backups/2026-06-26T*`. Progress ledger (gitignored): `.superpowers/sdd/progress.md`.

**Must-fix loose ends — fold into the tasks:**
1. **Uncommitted deletion:** `private_dot_claude/skills/web-research-task/` is staged-deleted but not committed — commit it (web-research-task was trashed).
2. **Wrongly-committed vendored files:** `dot_agents/skills/moshi` + `dot_agents/skills/herdr` were committed (`ae0126b`) before we learned they're vendored (moshi=`rjyo/moshi-skill` path `skills/moshi-best-practices`; herdr=`ogulcancelik/herdr`). **Un-commit them** (Task 4) — they belong in the manifest, not the repo.
3. **Hermes config:** already chezmoi-managed via `dot_hermes/create_private_config.yaml.tmpl` (a `create_` template — only writes if the file is absent, so source↔live can drift) + secrets via `~/.hermes/.env` + KeePassXC (commit `bef3953`). The 4 settings set live this session — `prune_builtins:false, consolidate:true, write_approval:true, guard_agent_created:true` — must be reflected in that source template. Add `workspace-mcp` to its `mcp_servers` (alongside `qmd`, `cua-driver`) using the `.env`/KeePassXC mechanism for the OAuth secret — never plaintext.
4. **paseo leftovers:** `~/.paseo` (daemon state) and an empty untracked `dotfiles/paseo.json` still exist — delete both (Task 7). Skills/daemon/cask already removed this session.
5. **Hermes hub-duplicates:** `senior-architect, todoist-cli, video-transcript-downloader, whisply` exist as **real dirs in `~/.hermes/skills`** (hub-installed/adapted) AND in the store. whisply = Hermes-adapted → **leave**. For the other 3: leave as Hermes-hub-managed (the hub-lock reproduces them); they harmlessly duplicate the store. Do NOT delete the adapted whisply.
6. **cua-driver = external:** `~/.agents/skills/cua-driver` → `~/.cua-driver/skills/cua-driver` (source `trycua/cua`) AND a Hermes MCP server (`cua-driver` in `mcp_servers`). In the manifest mark it **external** (managed by trycua/cua) — do not vendor it into the store.
7. **Idle-gate decision (open):** the user questioned why the updater idle-gates. The atomic swap already makes overwrites safe, so the idle-gate is belt-and-suspenders and can cause skipped runs if a harness is always up. Decide: keep or drop.
8. **n8n-mcp creds:** `czlonkowski/n8n-mcp` likely needs an n8n instance URL + API key the user hasn't supplied — **ask before configuring** (Task 6).
9. **Codex secret hygiene:** `~/.codex/config.toml` stores `GOOGLE_OAUTH_CLIENT_SECRET` in plaintext — consider moving it to a secret mechanism like Hermes's `.env`/KeePassXC (optional; flag to user).

**Key install nuance (Tasks 2–3):** `npx skills add <src> --global --all` installs into the **store** and npx-lock-tracks it (good); `npx skills add <src> --agent claude-code` (single agent) installs into `~/.claude/skills` as a real dir and must be **relocated** to the store. Prefer `--all`; otherwise install→relocate (the updater already has a defensive relocate step).

**Out of scope here (separate, time-sensitive project):** the originating goal — **Luke Morrison-Smith's first-job résumé** (`career-campaign/luke-morrison-smith/profile.md`; worktree `workflow/job-search/luke-morrison-smith`) — was never returned to (move ~2026-07-11, last day 7/9). Handle separately, not in this skills plan.

---

### Task 1: Build the source manifest
**Files:** Create `dot_agents/private_skills-manifest.json` (chezmoi → `~/.agents/.skills-manifest.json`).
- [ ] Enumerate every vendored skill with its source. Groups:
  - **knowledge-work (131):** installer `github`, repo `anthropics/knowledge-work-plugins`, ref `main`, path `<category>/skills/<skill>`; 4 namespaced.
  - **mattpocock (35):** installer `skills`, `mattpocock/skills` (`npx add --all` → store).
  - **portables (4):** frontend-design=`anthropics/skills`, kubernetes-specialist=`jeffallan/claude-skills`, peekaboo=`steipete/agent-scripts`, web-design-guidelines=`vercel-labs/agent-skills` (github, find SKILL.md by name).
  - **hyperframes family (8):** hyperframes, hyperframes-cli, hyperframes-media, hyperframes-registry, gsap, lottie, tailwind, three, website-to-hyperframes — `heygen-com/hyperframes`.
  - **find-skills:** `vercel-labs/skills`. **lobster:** `guwidoe/lobster-skill`.
  - **clawhub set (~16):** agent-browser-clawdbot, conventional-commits, elevenlabs, home-assistant, humanizer, last30days-official, market-research, playwright-mcp, playwright-scraper-skill, readwise-official, senior-architect, sql-toolkit, summarize-pro, tiktok-crawling, web-search-exa, whisply — installer `clawhub`.
  - **reclassified (user-supplied):** moshi=`rjyo/moshi-skill` (path `skills/moshi-best-practices`), herdr=`ogulcancelik/herdr`, cua-driver=`trycua/cua`, todoist-cli=`Doist/todoist-cli`, composio-cli=composio CLI (`docs.composio.dev` — verify exact skill source).
  - **deep-research:** source per comparison verdict.
- [ ] **Fork/user-authored (committed, NOT in manifest, skip-list):** `video-transcript-downloader`. Verify nothing else is a true fork.
- [ ] Verify each entry resolves (the updater dry-run in Task 2 validates).

### Task 2: Updater → manifest-driven install+refresh
**Files:** Modify `dot_local/bin/executable_update-skills.sh`.
- [ ] Read the manifest; for each entry dispatch by `installer` (github sparse-clone / `npx skills add` / clawhub) → temp → atomic swap into store (install if missing, refresh if present). Remove the current `[ -d "$STORE/$n" ] || continue` guards (those made it refresh-only).
- [ ] Keep: mkdir-lock, idle-gate (dry-run-aware), skip-list, symlink fan-out step.
- [ ] shellcheck clean; `--dry-run` lists install+refresh actions for all manifest entries with no errors.

### Task 3: New installs (add to manifest, then updater installs)
- [ ] Add to manifest + install: n8n-skills (`czlonkowski/n8n-skills`), claude-code-owasp (`agamm/claude-code-owasp`), linkedin-skills (`Linked-API/linkedin-skills`), pypict (`omkamal/pypict-claude-skill`), playwright-cli (`microsoft/playwright-cli` via skills.sh), playwright-best-practices (`currents-dev/...`), playwright-generate-test (`github/awesome-copilot/...`), just-scrape (`scrapegraphai/just-scrape`), firecrawl-scrape (`firecrawl/cli`), ffuf_claude_skill (`jthack/ffuf_claude_skill`), karpathy-llm-wiki (`astro-han/karpathy-llm-wiki`).
- [ ] **Namespace collisions:** the 3 playwright skills + `playwright-mcp`/`playwright-scraper-skill` — check for name clashes; namespace if any (e.g. keep distinct dir names).
- [ ] Symlink each into Claude + Hermes.

### Task 4: chezmoi restructure — symlinks-for-all + un-commit vendored
**Files:** `run_after_` hook (chezmoi) calling the updater's symlink step; remove committed vendored skill files.
- [ ] Add `run_onchange_after_link-agent-skills.sh.tmpl` (or `run_after_`) that ensures every store skill is symlinked into Claude + Hermes (idempotent).
- [ ] **Un-commit vendored files** wrongly committed: `dot_agents/skills/moshi`, `dot_agents/skills/herdr` (now manifest-vendored). Keep `private_video-transcript-downloader` (fork). Audit the other ~22 `dot_agents/skills/*` source dirs — remove any that are vendored (in the manifest); keep only true forks/user-authored.
- [ ] Remove the now-redundant per-skill `symlink_*.tmpl` for vendored skills if the run-hook supersedes them (or keep — decide for consistency).

### Task 5: Hermes-native catalog tracking
- [ ] Commit Hermes hub-lock (`~/.hermes/skills/.hub/lock.json`) so hub skills reinstall on a fresh Hermes.
- [ ] Detect agent-created native skills (`.usage.json` `created_by:agent`) and commit those skill files (they're unique). Do NOT commit bundled ones (ship with Hermes).
- [ ] Document the fresh-Hermes bootstrap (hub reinstall + apply agent-created).

### Task 6: MCP servers + the two comparisons
- [ ] **n8n-mcp** (`czlonkowski/n8n-mcp`): configure for Claude Code (`~/.claude.json`), Codex (`~/.codex/config.toml`), Hermes (`~/.hermes/config.yaml`).
- [ ] **deep-research:** VERDICT = **keep current** — the local 8-phase version is decisively superior to `parags/deep-research-pro` (clawhub=skills.sh, same upstream). No action.
- [ ] **Google Workspace on Hermes:** VERDICT = **add `workspace-mcp` + keep native skill.** Hermes already has `productivity/google-workspace` (gws CLI). Add to `~/.hermes/config.yaml` under `mcp_servers` (alongside `qmd`, `cua-driver`): `command: uvx`, `args: [workspace-mcp, --tool-tier, complete]`, env `GOOGLE_OAUTH_CLIENT_ID`/`GOOGLE_OAUTH_CLIENT_SECRET` (reuse the values from Codex/Claude config — store via the repo's secret mechanism, not plaintext) + `OAUTHLIB_INSECURE_TRANSPORT: "1"`. Restart Hermes; OAuth re-auth on first use. Keep the native skill (reference + cred mgmt).

### Task 7: Binaries / CLIs
- [ ] **ffuf:** install `github.com/ffuf/ffuf` (brew `ffuf` or `go install`); verify `ffuf -V`. (Pairs with the ffuf_claude_skill.)
- [ ] **composio CLI:** install per `docs.composio.dev/docs/cli`; verify.

### Task 8: Verify + report
- [ ] Updater `--dry-run` clean; store reconciles to manifest + forks; 0 dangling symlinks; chezmoi `re-add`/`status` reviewed; MCP servers present per harness.
- [ ] Commit everything on `feat`. Produce a report (per-skill class/source/update-path table, MCP status, binaries, the two comparison verdicts).
