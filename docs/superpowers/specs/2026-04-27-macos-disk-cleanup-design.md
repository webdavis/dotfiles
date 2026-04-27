---
title: "macOS Tahoe Disk Cleanup — 4-phase reaper-safe recovery (~365-520 GB)"
date: 2026-04-27
host: dresden (MacBook Pro M1, macOS 26.2 Tahoe)
audience: stephen
status: approved (scope + recommendation), execution pending
---

# macOS Tahoe Disk Cleanup Design

## Problem statement

`dresden` is at **88% disk utilization** (774 GB of 926 GB Data volume used; 112 GB free). Investigation
reveals the dominant consumer is Apple developer tooling, not personal files. The user has chosen a
**scorched-earth** stance on cached/derived state: anything regenerable from project source or
Dockerfiles is fair game.

Hard constraints:

1. **Keep Xcode installed and functional** (not its cached state) for three iOS-only projects:
   `essential-feed-case-study`, `Maeve`, `webdavis`. DerivedData / module caches / build products
   are NOT preserved — first build after cleanup will be slow as Xcode rebuilds.
1. **Keep Docker fully operational** — actively used. Active containers and named volumes survive
   (`system prune` without `--volumes`); unused images, build cache, and dangling networks go.
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
|         1 | Simulator runtimes — keep iOS 26.1 only      |         365-415 GB | None (regenerable)            |
|        2a | DerivedData — wipe all 133 folders           |           50-70 GB | First Xcode rebuild slow      |
|        2b | Archives older than 1y                       |     skip (~28 MB)  | n/a                           |
|        2c | iOS DeviceSupport — wipe all                 |             ~31 GB | None (regenerable)            |
|        3a | Docker prune (no --volumes)                  |           10-20 GB | None (regenerable)            |
|        3b | `~/Library/Caches/*` — wipe entirely         |             ~33 GB | UI slowdowns on first relaunch |
|         4 | Misc filesystem cruft                        |            5-15 GB | None (regenerable or expired) |
| **Total** |                                              |    **~494-584 GB** |                               |

After execution, expected utilization: **~35-45%** (down from 88%).

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

**Keep** (latest iOS only, per scorched-earth stance):

- `iOS 26.1` (com.apple.CoreSimulator.SimRuntime.iOS-26-1) — single keeper

**Delete** (25 runtimes — all others):

- iOS 18.3, 18.4, 18.5, 26.0 (4 older/superseded iOS)
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

## Phase 2 — Xcode user data (~80-100 GB)

### 2a. DerivedData (wipe all)

`~/Library/Developer/Xcode/DerivedData/` contains 133 per-project build-cache folders accumulated over
years. Per scorched-earth direction: wipe entirely. The 3 keeper projects rebuild from source on next
Xcode launch (slow first build, normal after that).

```bash
# Read first to log what's being removed:
ls ~/Library/Developer/Xcode/DerivedData/ | wc -l   # expect ~133
du -sh ~/Library/Developer/Xcode/DerivedData

# Wipe:
trash ~/Library/Developer/Xcode/DerivedData/*
```

Estimated recovery: 50-70 GB.

### 2b. Archives (skip — already lean)

Actual scan: ~28 MB total in 4 dated folders (2021-2022). Not worth a deletion pass.

### 2c. iOS DeviceSupport (delete all — retired iOS only)

`~/Library/Developer/Xcode/iOS DeviceSupport/<version> (build)/` accumulates one folder per
iPhone/iPad you've ever connected for debugging. Scan reveals every entry is iOS 14.x or 15.x —
none matter for current iOS targets.

| Folder | Size |
|---|---:|
| iOS 14.6 (18F72) arm64e | 4.1 GB |
| iOS 14.7.1 (18G82) arm64e | 4.1 GB |
| iOS 15.0.2 (19A404) arm64e | 5.0 GB |
| iOS 15.2.1 (19C63) arm64e | 5.2 GB |
| iOS 15.3.1 (19D52) arm64e | 5.2 GB |
| iOS 15.4.1 (19E258) arm64e | 5.3 GB |
| iOS 15.5 (19F77) arm64e | 2.2 GB |
| **Total** | **~31 GB** |

```bash
trash ~/Library/Developer/Xcode/iOS\ DeviceSupport/*
```

Estimated recovery: ~31 GB.

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

### 3b. Wipe `~/Library/Caches/*` entirely (~33 GB)

Per scorched-earth direction: every app in `~/Library/Caches` regenerates its cache on next launch.
First open of Slack/browsers/IDEs after the wipe will be slower than usual (re-fetch + re-index);
no actual data is lost.

```bash
du -sh ~/Library/Caches/* 2>/dev/null | sort -h | tail -10   # log before
trash ~/Library/Caches/*
```

Estimated recovery: ~33 GB.

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
