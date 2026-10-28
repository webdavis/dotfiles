# Skill reorganization, final plan (explicit, verified)

## Context
`~/.agents/skills` is the single source for **portable** skills; Claude Code + Hermes consume them via per-skill symlinks (Codex reads the store natively). Hermes's adapted/native catalog stays in Hermes. Every skill is mapped explicitly (verified via GitHub API + hub lock), installed/verified one-by-one, then a report is produced. All store + config changes are committed on **`feat/cli-agent-tracking-workflow`** (the chezmoi source / dotfiles main checkout). The `workflow/job-search/luke-morrison-smith` worktree is **kept** (revisited later). Backups before every destructive step.

## Resolved decisions
- **video-transcript-downloader:** KEEP the custom fork (your whisply fallback). It's clawhub-managed and already update-safe (absent from the npx lock; no clawhub lock on disk). Add to the updater **skip-list**; never `clawhub update --force` it or re-add it to a lock.
- **whisply:** your store copy is the clean `webdavis/dotfiles` version (symlink to Claude/Codex). Hermes has a separate **adapted** whisply (adds a Hermes STT-integration section + `references/hermes-stt-provider.md`, patched 3×), leave it in Hermes (part of its adapted catalog); do NOT sync either direction.
- **code-review:** REMOVE the existing clawhub copy; the Anthropic KW engineering `code-review` keeps the canonical name (user decision). **competitive-brief + review-contract:** DIFFERENT across categories → keep both, category-namespaced (4 renamed), kept current by the updater.
- **lobster:** npx-installable from GitHub, `npx skills add guwidoe/lobster-skill` (not on skills.sh). Reinstall fresh → store + symlink Claude/Hermes. **adhd-assistant:** flagged suspicious → **REMOVED** (deleted from `~/.hermes/skills`, backed up). Also purge its hermes hub-lock entries so `hermes skills update` can't re-fetch it; never reinstall.
- **Mid-workflow update safety (resolved):** updating a skill on disk does NOT corrupt an in-progress agent, all three harnesses snapshot frontmatter (session start) and the SKILL.md body (invocation) into immutable history; changes apply to the next load only. Sole risk = a non-atomic write racing an on-demand `references/`/`scripts/` read → fixed by the atomic-swap updater below.
- **Matt Pocock (35):** reinstall, physical copy in `~/.agents/skills`, symlinked into **Claude Code** and Hermes.

## Tracking (first + last)
Bring the **Hermes config edits** (`prune_builtins:false`, `consolidate:true`, `write_approval:true`, `guard_agent_created:true`) under chezmoi and commit on `feat/cli-agent-tracking-workflow`. Commit all store/symlink changes + the new updater script/manifest/schedule on the same branch.

---

## Action A, Anthropic knowledge-work skills (14 categories, 129)
Per category: `npx --yes skills@latest add anthropics/knowledge-work-plugins/<category> --full-depth --global --agent claude-code -y`, then symlink each into `~/.hermes/skills`. **Exclude:** customer-support, productivity, pdf-viewer, partner-built. Non-collision KW skills stay `npx skills update`-able (github source in lock).

- **bio-research (6):** instrument-data-to-allotrope, nextflow-development, scientific-problem-selection, scvi-tools, single-cell-rna-qc, start
- **cowork-plugin-management (2):** cowork-plugin-customizer, create-cowork-plugin
- **data (10):** analyze, build-dashboard, create-viz, data-context-extractor, data-visualization, explore-data, sql-queries, statistical-analysis, validate-data, write-query
- **design (7):** accessibility-review, design-critique, design-handoff, design-system, research-synthesis, user-research, ux-copy
- **engineering (10):** architecture, code-review→`engineering-code-review`✦, debug, deploy-checklist, documentation, incident-response, standup, system-design, tech-debt, testing-strategy
- **enterprise-search (5):** digest, knowledge-synthesis, search-strategy, search, source-management
- **finance (8):** audit-support, close-management, financial-statements, journal-entry-prep, journal-entry, reconciliation, sox-testing, variance-analysis
- **human-resources (9):** comp-analysis, draft-offer, interview-prep, onboarding, org-planning, people-report, performance-review, policy-lookup, recruiting-pipeline
- **legal (9):** brief, compliance-check, legal-response, legal-risk-assessment, meeting-briefing, review-contract→`legal-review-contract`✦, signature-request, triage-nda, vendor-check
- **marketing (8):** brand-review, campaign-plan, competitive-brief→`marketing-competitive-brief`✦, content-creation, draft-content, email-sequence, performance-report, seo-audit
- **operations (9):** capacity-plan, change-request, compliance-tracking, process-doc, process-optimization, risk-assessment, runbook, status-report, vendor-review
- **product-management (8):** competitive-brief→`product-management-competitive-brief`✦, metrics-review, product-brainstorming, roadmap-update, sprint-planning, stakeholder-update, synthesize-research, write-spec
- **sales (9):** account-research, call-prep, call-summary, competitive-intelligence, create-an-asset, daily-briefing, draft-outreach, forecast, pipeline-review
- **small-business (29):** business-pulse, call-list, canva-creator, cash-flow-snapshot, close-month, content-strategy, contract-review, crm-cleanup, crm-maintenance, customer-pulse-check, customer-pulse, friday-brief, handle-complaint, invoice-chase, job-post-builder, lead-triage, margin-analyzer, monday-brief, month-end-prep, month-heads-up, plan-payroll, price-check, quarterly-review, review-contract→`small-business-review-contract`✦, run-campaign, sales-brief, smb-onboard, smb-router, tax-prep, tax-season-organizer, ticket-deflector

✦ = renamed (collision). Your existing `code-review` (clawhub) stays untouched. The 5 ✦ skills are managed by the deterministic updater below (npx can't track renamed dirs).

## Unified safe skill updater (scheduled, all provenances)
One scheduled job updates **everything**, dispatched by provenance, every overwrite atomic:
- **Dispatch by source:** npx-tracked → `npx skills update --global`; clawhub → `clawhub update`; vendored-from-github (the 5 namespaced KW skills + any github-subpath skill) → shallow sparse-clone the path.
- **Atomic swap (required, wraps every overwrite):** build each skill's new content in a temp dir on the same FS, then swap via `rename`/atomic symlink (`skills/foo`→`foo-vN`; build `foo-vN+1`; `mv -Tf`). Readers see all-old or all-new; an in-flight read finishes on the old inode. Never overwrite in place or `rm`-then-recreate (clawhub `rm`+extract and npx in-place writes are *wrapped*, not run directly against the live store).
- **Skip-list (ours, authoritative):** never touch forks/manual skills, `video-transcript-downloader` + any user-edited copies. (Tool pinning is unreliable: npx has none; clawhub-pin needs a lock entry the fork can't safely get.)
- **Idle-gate + lockfile:** skip a skill while a live `claude`/`codex`/`hermes` session uses it; serialize the job with a lockfile.
- **Manifest** (chezmoi-tracked) maps vendored skills `{name, repo, ref, path}` (5 namespaced KW skills → `anthropics/knowledge-work-plugins/<cat>/skills/<skill>`, ref `main`).
- **Script** `scripts/update-skills` (dotfiles; `set -euo pipefail`; root justfile) + **launchd** weekly; tracked in dotfiles.

## Action B, Portable skills.sh skills → store + symlink (Claude + Hermes)
Verified none are Hermes-bundled. Remove any Hermes hub copy, reinstall from source, symlink Claude + Hermes:
`frontend-design` (anthropics/skills), `kubernetes-specialist` (jeffallan/claude-skills), `peekaboo` (steipete/agent-scripts), `web-design-guidelines` (vercel-labs/agent-skills), `todoist-cli`*, `senior-architect`*, `video-transcript-downloader`*, *already in store; only ensure Claude+Hermes symlinks. (v-t-d keeps your custom copy; do not reinstall it.)

## Action C, lobster (npx) + adhd-assistant (removed)
`lobster`: reinstall fresh via `npx skills add guwidoe/lobster-skill --global --agent claude-code` (replaces the clawhub copy → npx-managed), then symlink into Hermes. **adhd-assistant: REMOVED**, was in `~/.hermes/skills` (backed up + deleted); purge its 3 entries from `~/.hermes/skills/.hub/lock.json` so it can't be re-fetched.

## Action D, Matt Pocock (35) → store + symlink (Claude + Hermes)
`npx skills add mattpocock/skills --global --agent claude-code`, then symlink each into Hermes.

## Action E, moshi, herdr → store + symlink (Claude + Hermes)
Replace the chezmoi-vendored real dirs in `~/.claude/skills` with store symlinks and stop the `justfile update-agent-skills` vendoring (else re-vendored on apply). Symlink into Hermes.

## Action F, Remove all paseo skills
Remove `paseo, paseo-advisor, paseo-committee, paseo-handoff, paseo-loop, paseo-epic` from store, `~/.claude/skills`, `~/.codex/skills`, `~/.hermes/skills`, and the lock (manual; back up first). ⚠ paseo CLI may re-create.

## Action G, Audit every Hermes-native skill (one-by-one, read-only)
For each `~/.hermes/skills` skill not a store symlink: confirm tracked (`.bundled_manifest`, Hermes/official `.hub/lock.json`, or `.usage.json` `created_by:agent`). Flag orphans / stray skills.sh sources.

## Action H, Audit + refactor `~/.claude/skills`
Each entry = a store symlink or an intentional Claude-only real dir. Convert portable real dirs to store symlinks, remove dangling links, no dup names.

## Verification (one-by-one, triple-check)
For every skill touched: store dir is real; `readlink` in each target harness resolves into `../../.agents/skills/<skill>`; lock entry present (or in the vendor manifest for the 5 ✦); Hermes-native confirmed tracked; no dangling symlinks; no remaining name collisions. Dry-run the updater script. Run the audit twice against this explicit map.

## Report (end)
Per-skill table: `name | class (KW-<cat> / KW-namespaced / portable / matt / moshi-herdr / clawhub / custom-fork / hermes-native / paseo-removed) | store | claude link | hermes link | codex(native) | source | update path (npx / vendor-updater / clawhub / manual) | status`. Plus counts, the 5 renamed, orphans flagged, updater test result, and the commit on `feat/cli-agent-tracking-workflow`.

## Risks / scale
- **Scale:** ~129 KW + ~45 portable/Matt/moshi-herdr ≈ **~174 store skills, ~350 symlinks** + 6 paseo removed + ~72 native audited + a new updater script & launchd job. Large.
- chezmoi vendoring of herdr/moshi (E); paseo re-create (F); Hermes re-seed of removed hub copies (B); Matt reinstall reverses earlier removal (intended).
- KW set is domain-heavy (finance/legal/bio/HR), full set lands in every harness.
