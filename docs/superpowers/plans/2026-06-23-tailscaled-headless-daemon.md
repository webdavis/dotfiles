# Headless tailscaled system daemon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert this Mac (dresden) from the `tailscale-app` GUI cask to the open-source `tailscale` formula run as a `sudo brew services` launchd system daemon (boots before login, no re-approval click).

**Architecture:** Codify the package swap + a sudo-free status/reminder chezmoiscript in the repo, then perform a one-time interactive cutover (uninstall GUI → start daemon → `tailscale up` → Disable Key Expiry). Auth and DNS are roaming-safe and require no stored secrets.

**Tech Stack:** Homebrew (`tailscale` formula), `brew services` launchd system daemon, chezmoi `run_onchange` scripts, the repo's `just` lint/test tooling.

**Reference spec:** `docs/superpowers/specs/2026-06-23-tailscaled-headless-daemon-design.md`

## Global Constraints

- Daemon = `sudo brew services start tailscale` (system LaunchDaemon, brew binary). NOT `tailscaled install-system-daemon` (stale-on-upgrade copy).
- Auth = Option A: one-time interactive `sudo tailscale up --accept-dns=true` + admin-console **Disable Key Expiry**. No auth keys, no KeePassXC.
- DNS = `--accept-dns=true` ONLY. **Never** a static `100.100.100.100` resolver (breaks off-tailnet; dresden roams).
- Commits: **stage specific paths only** (never `git add -A` / `git commit -a`). The working tree holds unrelated uncommitted codex + moshi changes — Task 1 commits those first so the shared files (`system_packages_autoinstall.yaml`, `CLAUDE.md`) are clean for the tailscaled commit.
- Every commit passes the pre-commit hook (`just lint-check` + `just test`).
- The cutover is interactive (sudo, browser login, admin console) — the operator runs those; the chezmoiscript stays sudo-free and never authenticates.
- Don't patch tailscale/brew; supported commands only.

---

### Task 1: Prerequisite — commit the pending codex + moshi changes (un-entangle shared files)

**Why:** the working tree has two finished-but-uncommitted changes that share files the tailscaled task edits — codex lives in `system_packages_autoinstall.yaml`, moshi touches `CLAUDE.md` (+ its own files). Committing them first (each its own commit) keeps the tailscaled commit clean.

**Files:** `.chezmoidata/system_packages_autoinstall.yaml` (codex), `dot_config/moshi/private_auth.json.tmpl` + `dot_local/bin/executable_claude-moshi-notify.sh` + `CLAUDE.md` + `AGENTS.md` (moshi).

- [ ] **Step 1: Confirm with the operator** that the codex swap and the moshi rename are ready to commit (both are done + lint-clean from the background agents). If yes, proceed.

- [ ] **Step 2: Commit codex alone**

```bash
cd /Users/stephen/workspaces/Ivy/webdavis/dotfiles
git add .chezmoidata/system_packages_autoinstall.yaml
git diff --cached --stat   # confirm ONLY the codex/codex-app cask + npm @openai/codex removal
git commit -m "feat(packages): codex + codex-app casks (replace npm @openai/codex)"
```
Expected: pre-commit passes; one commit, one file.

- [ ] **Step 3: Commit moshi alone**

```bash
git add dot_config/moshi/private_auth.json.tmpl dot_local/bin/executable_claude-moshi-notify.sh CLAUDE.md AGENTS.md
git status --short   # the old private_setting.json.tmpl shows as deleted (rename) — included via the add above? if shown as ' D', also: git add -A dot_config/moshi/
git commit -m "fix(moshi): read webhook secret from auth.json; rename template + script path"
```
Expected: pre-commit passes; the template rename (`private_setting.json.tmpl` → `private_auth.json.tmpl`) is recorded. After this, `git status` shows a clean tree except for pre-existing untracked `.agents/` / `skills-lock.json`.

- [ ] **Step 4: Verify clean base**

Run: `git status --short | grep -v '^??'`
Expected: empty (no tracked-file changes left) — the tailscaled work now starts from a clean tree.

---

### Task 2: chezmoi codification (package swap + status reminder + docs)

**Files:**
- Modify: `.chezmoidata/system_packages_autoinstall.yaml` (−`tailscale-app` cask, +`tailscale` formula)
- Create: `.chezmoiscripts/run_onchange_after_66-tailscaled-status.sh.tmpl`
- Modify: `scripts/lint.sh` (`find_shell_templates`)
- Modify: `CLAUDE.md` (new "Tailscale (headless daemon)" subsection)

- [ ] **Step 1: Swap the package** — `.chezmoidata/system_packages_autoinstall.yaml`

Remove from `casks:`:
```yaml
        - tailscale-app
```
Add to `formulae:` (alphabetical, between `tart` and `tealdeer`):
```yaml
        - tailscale
```

- [ ] **Step 2: Create the status/reminder script** — `.chezmoiscripts/run_onchange_after_66-tailscaled-status.sh.tmpl`

```text
{{ if eq .chezmoi.os "darwin" -}}
#!/bin/bash

set -euo pipefail

# Tailscale headless-daemon status reminder (sudo-free). Does NOT start or
# authenticate anything -- Option A auth is a deliberate one-time manual step.
# Runs on first deploy (and whenever this script changes) to prompt the cutover;
# stays quiet once the daemon is up and authenticated. Mirrors the atuin/happy
# daemon scripts' "remind if not healthy" shape.

tailscale="${TAILSCALE_BIN:-/opt/homebrew/bin/tailscale}"
[[ -x $tailscale ]] || exit 0 # formula not installed yet; brew bundle handles that.

if "$tailscale" status >/dev/null 2>&1; then
  exit 0 # connected + authenticated -- nothing to remind.
fi

out="$("$tailscale" status 2>&1 || true)"
if grep -qiE 'logged out|needslogin|not logged in' <<<"$out"; then
  echo "tailscaled is running but NOT authenticated. One-time:" >&2
  echo "  sudo tailscale up --accept-dns=true" >&2
  echo "  then flip 'Disable Key Expiry' on this node in the admin console." >&2
else
  echo "tailscaled system daemon is not running. One-time:" >&2
  echo "  sudo brew services start tailscale" >&2
fi
{{ end -}}
```

- [ ] **Step 3: Add the script to lint** — `scripts/lint.sh`, in `find_shell_templates`, after the herdr-smart-nav line:

```bash
    -o -name "run_onchange_after_57-build-herdr-smart-nav-plugin.sh.tmpl" \
    -o -name "run_onchange_after_66-tailscaled-status.sh.tmpl" \
```

- [ ] **Step 4: Document** — `CLAUDE.md`, add a new subsection immediately after the "### Happy Daemon (Remote Agent Control)" section:

```markdown
### Tailscale (headless daemon)

Tailscale runs as the open-source `tailscale` **formula** (not the `tailscale-app` GUI cask) as a launchd
**system daemon** via `sudo brew services start tailscale` — it boots before login and uses the `utun`
interface, so there is no Network/System Extension to re-approve after updates (the GUI variants' weakness
on a headless host). State persists at `/Library/Tailscale` across reboots. Auth is a one-time manual
`sudo tailscale up --accept-dns=true` plus flipping **Disable Key Expiry** on the node in the admin
console — after that it never re-authenticates (no auth keys, no rotation, no KeePassXC).
`run_onchange_after_66-tailscaled-status.sh.tmpl` is a sudo-free reminder that prints those one-time steps
when the daemon is down or unauthenticated; it never runs sudo or authenticates.

**DNS:** always `--accept-dns=true` (dynamic, roaming-safe) — never a static `100.100.100.100` resolver
(that breaks off-tailnet). The OSS macOS DNS path is the known weak spot (`tailscale/tailscale#13461`,
`#14746`): normal DNS keeps working while roaming, but resolving *other* tailnet hostnames *from* this
machine may be flaky on a foreign network — pin the few needed tailnet hosts in `/etc/hosts` if so.

**Updates:** the weekly brew-upgrade updates the formula; the running daemon picks up the new binary on
the next reboot (`sudo brew services restart tailscale` for an immediate bounce — the unattended weekly
job can't sudo).

**Future (new home Mac, ~3-6 months out):** when an always-home Mac takes over the daemon-host role, this
machine (dresden, which is carried) cuts back to the GUI `tailscale-app` cask (better roaming DNS) and the
new Mac runs this daemon — make the chezmoi config machine-conditional then.
```

- [ ] **Step 5: Verify + commit (specific paths only)**

```bash
CI=1 chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_66-tailscaled-status.sh.tmpl | shellcheck -   # clean
just s   # shellcheck ✅
just y   # yq (YAML) ✅
just m   # mdformat (CLAUDE.md) ✅
git add .chezmoidata/system_packages_autoinstall.yaml .chezmoiscripts/run_onchange_after_66-tailscaled-status.sh.tmpl scripts/lint.sh CLAUDE.md
git diff --cached --stat   # confirm ONLY these 4 files
git commit -m "feat(tailscale): headless tailscaled system daemon (formula + status reminder)"
```
Expected: render + shellcheck clean; pre-commit passes; 4 files committed.

---

### Task 3: Live interactive cutover (operator present — sudo + browser + admin console)

**Files:** none (live system actions). The GUI app and tailscaled cannot coexist (both own the tunnel), so the GUI must stop before the daemon starts. A brief Tailscale gap is fine (operator is home / local).

- [ ] **Step 1: Quit + uninstall the GUI app**

```bash
osascript -e 'quit app "Tailscale"' 2>/dev/null || true
brew uninstall --cask tailscale-app 2>&1 | tail -2
```
Expected: app quit; cask uninstalled.

- [ ] **Step 2: Confirm the formula + start the system daemon**

```bash
brew list tailscale >/dev/null 2>&1 && echo "formula present" || brew install tailscale
sudo brew services start tailscale
sudo brew services list | grep tailscale   # expect 'started' with a root/system entry
```
Expected: `tailscale` service `started`.

- [ ] **Step 3: Authenticate (browser) + verify the daemon is up**

```bash
sudo tailscale up --accept-dns=true
```
Click the printed login URL; authenticate as the same tailnet user. Then:
```bash
tailscale status | head; tailscale ip -4
```
Expected: status shows this node + peers; an IP in `100.x.y.z`.

- [ ] **Step 4: Disable Key Expiry (admin console, manual)**

In `https://login.tailscale.com/admin/machines`, find this node → ⋯ menu → **Disable key expiry**. (Prevents the 180-day node-key expiry from ever forcing a re-auth.)

- [ ] **Step 5: Verify MagicDNS at home + clean up the old node**

```bash
tailscale status | grep -i 'magicdns\|MagicDNS' || true
# resolve a known tailnet host by name (replace with a real one):
tailscale status --json | jq -r '.Peer[].DNSName' | head -1   # pick a name, then:
# scutil --dns | grep -A2 nameserver | head   # confirm 100.100.100.100 is in the resolver set
```
Then in the admin console, delete the now-offline old GUI node entry (cosmetic).
Expected: MagicDNS resolves at home; resolver includes `100.100.100.100`.

---

### Task 4: Verification, update-handling, and deferred checks

**Files:** none (verification + one decision recorded).

- [ ] **Step 1: Confirm no extension prompt + daemon health**

```bash
sudo brew services list | grep tailscale   # 'started'
tailscale status >/dev/null && echo "authenticated + connected"
```
Expected: started + connected; note that the next OS/app update will NOT raise a Network/System Extension approval (there is no extension anymore).

- [ ] **Step 2: Update-handling decision (default: reboot-refresh)**

Record the decision: the weekly brew-upgrade updates the `tailscale` formula; the running daemon keeps the old binary until the next reboot (or a manual `sudo brew services restart tailscale`). Default to **reboot-time refresh** (Tailscale tolerates minor client/daemon version skew far better than atuin's gRPC). Only if a future upgrade visibly breaks connectivity, add a single scoped `NOPASSWD` sudoers entry for `brew services restart tailscale` so the weekly job can bounce it unattended. No code change now.

- [ ] **Step 3: Reboot-survival check (operator-scheduled)**

When a reboot is convenient: reboot, then **without logging in / without any interaction** confirm remote reachability (from your phone: `ssh`/ping the node), and after login `tailscale status` shows connected. Expected: comes up authenticated automatically (state at `/Library/Tailscale`).

- [ ] **Step 4: DEFERRED roaming DNS check (operator, when next traveling)**

Cannot be tested at home. When you next take dresden to another network: confirm normal DNS works (browse the web) and try resolving a tailnet hostname (`tailscale status` peer name). If tailnet-name resolution fails while roaming, pin the few hosts you need in `/etc/hosts` (e.g. `100.x.y.z hostname`) — static entries also work offline. Report back and we'll codify the `/etc/hosts` pins if needed.

---

## Self-Review

**Spec coverage:** package swap → T2.S1; sudo-free status reminder → T2.S2; lint wiring → T2.S3; CLAUDE.md doc → T2.S4; `sudo brew services` daemon → T3.S2; Option A auth + Disable Key Expiry → T3.S3-4; `--accept-dns=true` (no static) → T3.S3 + the doc; MagicDNS verify + `/etc/hosts` mitigation → T3.S5 + T4.S4; updates/reboot-refresh → T4.S2-3; future GUI cutback → CLAUDE.md doc; codex/moshi un-entangle → T1. No gaps.

**Placeholder scan:** none — full script, exact YAML/lint/CLAUDE.md edits, exact cutover commands. (T3.S5 uses a real `jq` to pick a tailnet name rather than a literal placeholder hostname.)

**Consistency:** the script name `run_onchange_after_66-tailscaled-status.sh.tmpl` is identical in T2.S2/S3/S5 and the CLAUDE.md doc; `--accept-dns=true` is identical in the script, T3.S3, and the doc; `sudo brew services start tailscale` identical across T2 script, T3.S2, and the doc.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-23-tailscaled-headless-daemon.md`.
