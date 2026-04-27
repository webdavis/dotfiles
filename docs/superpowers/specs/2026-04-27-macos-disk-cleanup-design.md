______________________________________________________________________

## title: macOS Tahoe Disk Cleanup — 4-phase reaper-safe recovery (~365-520 GB) date: 2026-04-27 host: dresden (MacBook Pro M1, macOS 26.2 Tahoe) audience: stephen status: approved (scope + recommendation), execution pending

# macOS Tahoe Disk Cleanup Design

## Problem statement

`dresden` is at **88% disk utilization** (774 GB of 926 GB Data volume used; 112 GB free). Investigation
reveals the dominant consumer is Apple developer tooling, not personal files. The user wants the cruft
removed, with three explicit constraints:

1. **Keep Xcode functional** for three iOS-only projects: `essential-feed-case-study`, `Maeve`,
   `webdavis`.
1. **Keep Docker fully operational** — actively used; image/container loss is acceptable only via
   `system prune` since Dockerfiles can rebuild.
1. **Do not touch iMessage data** — planned future DB import.

## Disk inventory

```
Data volume:                                  774 GB / 926 GB used (88%)
─────────────────────────────────────────────────────────────────────
/Library/Developer/CoreSimulator/             456 GB  ← ★ dominant ★
  ├─ Volumes/                                 306 GB    runtime OS images
  ├─ Caches/                                   77 GB    runtime caches
  └─ Cryptex/                                  73 GB    cryptex bundles
~/Library/Developer/                          108 GB
  ├─ CoreSimulator (user-side simulators)      41 GB
  └─ Xcode (DerivedData, Archives, support)    67 GB
~/Library/Containers/                          82 GB
  └─ com.docker.docker                         40 GB    (KEEP — active use)
~/Library/Application Support/                 70 GB
~/Library/Caches/                              33 GB
~/Library/Messages/                            22 GB    (KEEP — future DB import)
~/Pictures/                                    33 GB
```

## Recovery target by phase

|     Phase | Target                                       | Estimated recovery | Risk                          |
| --------: | :------------------------------------------- | -----------------: | :---------------------------- |
|         1 | Simulator runtimes (24 of 27)                |         300-400 GB | None (regenerable)            |
|         2 | Xcode DerivedData / Archives / DeviceSupport |           40-70 GB | None (regenerable)            |
|         3 | Docker prune + selective caches              |           20-35 GB | None (regenerable)            |
|         4 | Misc filesystem cruft                        |            5-15 GB | None (regenerable or expired) |
| **Total** |                                              |    **~365-520 GB** |                               |

After execution, expected utilization: **~30-45%**.

______________________________________________________________________

## Phase 1 — Simulator runtime cleanup (~300-400 GB)

**Inventory** (full list from `xcrun simctl list runtimes`):

```
iOS 18.3, 18.4, 18.5, 26.0, 26.1                          ← 5 runtimes
tvOS 18.2, 18.4, 18.5, 26.0, 26.1, 26.2, 26.4              ← 7 runtimes
watchOS 11.2, 11.4, 11.5, 26.0, 26.1, 26.2, 26.4           ← 7 runtimes
visionOS 2.3, 2.4, 2.5, 26.0, 26.1, 26.2, 26.4             ← 7 runtimes
                                                  Total: 27
```

**Keep** (per "iOS only" + recent two iOS versions):

- `iOS 26.0` (com.apple.CoreSimulator.SimRuntime.iOS-26-0)
- `iOS 26.1` (com.apple.CoreSimulator.SimRuntime.iOS-26-1)

**Delete** (24 runtimes):

- iOS 18.3, 18.4, 18.5 (older iOS than your projects' deployment target needs)
- All 7 tvOS runtimes
- All 7 watchOS runtimes
- All 7 visionOS runtimes

**Procedure** (non-destructive read-then-delete; user reviews kill list before delete):

```bash
# 1. Read-only: list runtimes with bundle IDs and sizes (already done above).
xcrun simctl list runtimes

# 2. Delete one at a time (lower-blast-radius than batch delete):
xcrun simctl runtime delete com.apple.CoreSimulator.SimRuntime.iOS-18-3
xcrun simctl runtime delete com.apple.CoreSimulator.SimRuntime.iOS-18-4
xcrun simctl runtime delete com.apple.CoreSimulator.SimRuntime.iOS-18-5
# ... (full list in execution log)

# 3. Reap any simulator devices left orphaned without their runtime:
xcrun simctl delete unavailable

# 4. Verify remaining state:
xcrun simctl list runtimes
df -h /System/Volumes/Data
```

**Why this is reaper-safe:** Deleted runtimes can be reinstalled at any time via Xcode → Settings →
Components → Platforms (or `xcodebuild -downloadPlatform iOS`). They are not licensed data; they are
Apple-distributed binaries.

**Exclusion guarantees:**

- iOS 26.0 + 26.1 retained — covers current macOS 26 era device targets.
- Existing simulators (devices) using iOS 26.0 / 26.1 runtimes survive.
- No DerivedData touched in this phase.

______________________________________________________________________

## Phase 2 — Xcode user data (~40-70 GB)

Three sub-targets, each independently reviewed.

### 2a. DerivedData (filter to keep only the 3 projects)

`~/Library/Developer/Xcode/DerivedData/<ProjectName>-<hashedID>/` is per-project build cache. Anything
not matching the three keepers is orphaned.

```bash
# Read: list every DerivedData folder
ls ~/Library/Developer/Xcode/DerivedData/

# Identify keepers (folders matching essential-feed*, Maeve*, webdavis*)
# Delete the rest individually, keeping a printed audit trail.
```

**Keep:** any folder matching `essential-feed*`, `Maeve*`, `webdavis*` (case-insensitive). **Delete:**
every other DerivedData folder.

Estimated recovery: 30-50 GB depending on how many old projects accumulated.

### 2b. Archives (prune older than 1 year)

`~/Library/Developer/Xcode/Archives/` holds shipped builds. They're useful for symbolicating crash logs
from past releases, less so once a year+ has passed.

```bash
find ~/Library/Developer/Xcode/Archives -name "*.xcarchive" -mtime +365 -depth 1
# Review the list, then delete:
find ~/Library/Developer/Xcode/Archives -name "*.xcarchive" -mtime +365 -depth 1 -exec trash {} +
```

Estimated recovery: 5-15 GB.

### 2c. iOS DeviceSupport (prune retired devices)

`~/Library/Developer/Xcode/iOS DeviceSupport/<version> (build)/` accumulates one folder per iPhone/iPad
you've ever connected for debugging. Devices retired from your roster contribute dead weight.

```bash
ls ~/Library/Developer/Xcode/iOS\ DeviceSupport/ | sort
# Review by version; keep latest 2-3 iOS versions matching your project targets.
```

Estimated recovery: 5-15 GB.

______________________________________________________________________

## Phase 3 — Docker housekeeping + caches (~20-35 GB)

### 3a. Docker prune (preserves containers + volumes you actively use)

```bash
# Inventory: see what's currently used vs reclaimable
docker system df

# Reaper: removes ONLY images not referenced by any container, build cache,
# and unused networks. Containers and named volumes survive.
docker system prune --all --force

# Then compact the Docker Desktop VM disk to actually return space to macOS:
# Docker Desktop → Settings → Advanced → "Clean / Purge data" → Cache
# (alternatively the `~/.docker/desktop-script.sh` if it exists)
```

**Safety:** `--all` prunes images not used by any container. Your active containers and Dockerfiles
regenerate any image you delete. Volumes (named persistent storage) are NEVER touched by `system prune`
without `--volumes` flag, which is **NOT** included.

Estimated recovery: 10-20 GB.

### 3b. Selective cache cleanup (`~/Library/Caches/`)

Largest cache hogs to identify:

```bash
du -sh ~/Library/Caches/* 2>/dev/null | sort -h | tail -15
```

Delete top consumers for apps that regenerate caches gracefully (Homebrew, Spotify, Slack, Chrome, etc.).
Skip caches for apps with known cache-loss issues (proprietary editors, IDE indexes that take hours to
rebuild).

Estimated recovery: 10-15 GB.

______________________________________________________________________

## Phase 4 — Misc filesystem cruft (~5-15 GB)

### 4a. Arc browser PartialDownloads in `~/Downloads`

165 dotfiles named `.company.thebrowser.Browser.<random>` accumulated since at least 2026-04-02. Arc's
stale partial-download cache.

```bash
# Read list:
ls -la ~/Downloads/.company.thebrowser.Browser.* 2>/dev/null | wc -l
# Reap:
trash ~/Downloads/.company.thebrowser.Browser.*
```

### 4b. Homebrew cached install dmgs

```bash
brew cleanup --prune=all
# Or specifically:
rm ~/Library/Caches/Homebrew/downloads/*.dmg
```

### 4c. Old test ISOs / dmgs in workspaces (ansible test fixtures)

Test ISOs ship with
`ansible_collections/community/general/tests/integration/targets/iso_extract/files/test.iso` — small
individually but multiple copies. Leave alone if any project references them; otherwise `git clean` the
relevant working trees.

### 4d. Retired-project node_modules

```bash
find ~/workspaces -name node_modules -type d -prune
# Identify projects you no longer touch; delete their node_modules.
# `package-lock.json` survives; `npm install` rebuilds on demand.
```

Currently visible: ~220 MB across `headroom`, `chronicler`, `uriel/.../memory-lancedb-pro`. Probably
small on its own; included for completeness.

______________________________________________________________________

## What this design will NOT do

- **Touch `~/Library/Messages`** — explicit user exclusion.
- **Touch Docker volumes or running containers** — only unused images/build cache via
  `system prune --all` (without `--volumes`).
- **Touch DerivedData/Archives matching the 3 keeper projects** — explicit allowlist.
- **Uninstall Xcode itself** — actively needed.
- **Empty `/private/var/folders` system caches** — macOS reaps these on its own; manual emptying can
  break running apps.
- **Modify Time Machine local snapshots** — `tmutil` shows none currently, no action needed.
- **Run any single multi-target rm/trash command** — every deletion is per-target and reviewed.

## Execution policy

1. **Phase 1 first** — biggest payoff, lowest risk.
1. **User confirms each phase before execution.** "Yes proceed" requires showing the kill list, not just
   running it.
1. **Per-phase rollback:** none required (all deletions are regenerable Apple/Docker artifacts).
1. **Per-phase verification:** `df -h /System/Volumes/Data` before and after each phase, recorded in this
   doc's execution log.

## Execution log

(To be filled in as each phase completes.)

| Phase | Started | Completed | Used before (GB) | Used after (GB) | Recovered (GB) | Notes |
| ----- | ------- | --------- | ---------------- | --------------- | -------------- | ----- |
| 1     | —       | —         | 774              | —               | —              | —     |
| 2a    | —       | —         | —                | —               | —              | —     |
| 2b    | —       | —         | —                | —               | —              | —     |
| 2c    | —       | —         | —                | —               | —              | —     |
| 3a    | —       | —         | —                | —               | —              | —     |
| 3b    | —       | —         | —                | —               | —              | —     |
| 4a    | —       | —         | —                | —               | —              | —     |
| 4b    | —       | —         | —                | —               | —              | —     |
| 4c    | —       | —         | —                | —               | —              | —     |
| 4d    | —       | —         | —                | —               | —              | —     |
