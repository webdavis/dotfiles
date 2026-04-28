# macOS Tahoe Disk Cleanup — Quarterly Runbook

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recover 300-400+ GB of disk space on a macOS 26+ MacBook Pro every quarter, by reaping regenerable developer artifacts (simulator runtimes, Xcode caches, Docker images, app caches, retired-app data) without touching user data, active Docker volumes, or iMessage history.

**Architecture:** Phase-ordered by impact-per-effort. Each phase: inventory (read-only), decision (kill list), execute (`trash` not `rm`), verify (`df` delta). All deletions go to trash-cli's `~/.local/share/Trash` so they're recoverable until a final `trash-empty` realizes the bytes.

**Tech Stack:** bash, `xcrun simctl`, `docker`, `trash-cli` (Python pipx-installed), `du`/`df`, Python 3 + tomllib for TOML parsing if needed. macOS 26.2 Tahoe target. Apple Silicon.

**Re-run cadence:** Quarterly. Expect 50-150 GB recovered on subsequent runs (the first run was 386 GB because years of cruft had accumulated).

---

## Hard exclusions (NEVER touch)

| Exclusion | Reason |
|---|---|
| `~/Library/Messages` | Future iMessage DB import planned |
| Docker named volumes containing user state (e.g., `synapse-data`, app database volumes) | Persistent state Dockerfiles can't recreate |
| `~/Library/Developer/Xcode/DerivedData/{essential-feed*,EssentialApp*,EssentialFeed*,Maeve*,webdavis*,WebdavisIo*}` ONLY IF you've decided to preserve cached builds (default in this runbook: scorched-earth — even keeper-project DerivedData goes; first build after cleanup is slow) | User's three active iOS projects |
| Xcode the application itself (`/Applications/Xcode.app`) | Actively needed |
| `/private/var/folders/*` system caches | macOS reaps these autonomously; manual emptying can break running apps |
| Time Machine local snapshots (`tmutil listlocalsnapshots /`) | Backup integrity |
| `~/Library/Caches/com.apple.dt.Xcode` ONLY IF currently building | Module cache rebuild adds 10-30 min on next build; safe to wipe between active sessions |

---

## File structure

This runbook is the only artifact. Execution log accumulates at the bottom of this file.

- **Modify:** `docs/superpowers/plans/2026-04-27-macos-disk-cleanup-plan.md` (this file — fill the execution log table on each quarterly run)
- **Reference:** `docs/superpowers/specs/2026-04-27-macos-disk-cleanup-design.md` (original spec)

---

## Pre-flight (always run first)

### Task 0: Baseline disk usage

**Files:** None (capture stdout)

- [ ] **Step 1: Capture starting disk usage**

```bash
df -h /System/Volumes/Data | tail -1
diskutil info /System/Volumes/Data | grep -iE 'free|purgeable' | head -3
```

Expected output: line showing total/used/free GB on the Data volume.

- [ ] **Step 2: Record the baseline in this runbook's execution log**

Add a new row to the **Execution log** table at the bottom of this file with today's date and the "Used before" GB number from Step 1.

- [ ] **Step 3: Confirm trash is empty before starting**

```bash
trash-list 2>&1 | wc -l
du -sh ~/.local/share/Trash 2>/dev/null
```

Expected: `0` items, `8.0K` size. If trash has items, decide whether to empty before starting (so post-cleanup trash size accurately reflects this run's deletions).

---

## Phase 1 — Simulator runtimes (typical recovery: 200-400 GB)

### Task 1.1: Inventory installed simulator runtimes

**Files:** None (read-only)

- [ ] **Step 1: List runtimes with UUIDs (JSON output is reliable; text command can lag)**

```bash
xcrun simctl runtime list -j > /tmp/runtimes.json
python3 << 'EOF'
import json
with open("/tmp/runtimes.json") as f:
    data = json.load(f)
print(f"{len(data)} runtimes installed")
for uuid, v in data.items():
    print(f"  uuid={uuid}  build={v.get('build')}  state={v.get('state')}")
EOF
```

Expected: list of UUIDs with build numbers.

- [ ] **Step 2: Cross-reference build numbers to platform names**

```bash
xcrun simctl list runtimes
```

Expected: human-readable list mapping `iOS 26.4 (26.4.1 - 23E254a)` etc.

- [ ] **Step 3: Determine the keep set**

The user's keepers as of 2026-04: **iOS only + watchOS** (specifically: latest iOS major + minor, latest watchOS). Drop tvOS, visionOS, all older OS major versions. Re-confirm the keep list with the user before proceeding if the project mix has changed.

Write the keep-list builds to `/tmp/keep_builds.txt`, one per line. Example:

```bash
cat > /tmp/keep_builds.txt <<EOF
23E254a
23T240b
EOF
```

### Task 1.2: Build kill list (everything not in keep)

- [ ] **Step 1: Generate the delete-UUID list**

```bash
python3 << 'EOF'
import json
with open("/tmp/runtimes.json") as f:
    data = json.load(f)
with open("/tmp/keep_builds.txt") as f:
    keep = {l.strip() for l in f if l.strip()}
deletes = [uuid for uuid, v in data.items() if v.get("build") not in keep]
print(f"Will delete {len(deletes)} runtimes:")
with open("/tmp/delete_uuids.txt", "w") as out:
    for u in deletes:
        out.write(u + "\n")
        print(f"  {u}")
EOF
```

Expected: list of N UUIDs, one per line, in `/tmp/delete_uuids.txt`.

- [ ] **Step 2: Have the user review the kill list and confirm before deletion**

Show the list in a human-readable form (cross-referencing builds → platform/version). User confirms "yes" before proceeding.

### Task 1.3: Execute simulator runtime deletes

- [ ] **Step 1: Shutdown any running simulators (defensive)**

```bash
xcrun simctl shutdown all
```

Expected: silent success or "No matching..." (also fine).

- [ ] **Step 2: Delete each runtime by UUID**

```bash
while IFS= read -r uuid; do
  printf '  %s ... ' "${uuid:0:8}"
  result=$(xcrun simctl runtime delete "$uuid" 2>&1)
  echo "${result:-ok}"
done < /tmp/delete_uuids.txt
```

Expected: each line ends with `ok`. If a runtime is in use, the daemon may reject it; retry after `xcrun simctl shutdown all`.

- [ ] **Step 3: Reap orphaned simulators (devices whose runtime was just deleted)**

```bash
xcrun simctl delete unavailable
```

Expected: silent success.

- [ ] **Step 4: Verify final state via JSON (text output is cached)**

```bash
xcrun simctl runtime list -j | python3 -c '
import json, sys
data = json.load(sys.stdin)
print(f"{len(data)} runtimes remain:")
for uuid, v in data.items():
    print(f"  {v.get(\"build\")}")
'
```

Expected: only the keepers' builds remain.

- [ ] **Step 5: Verify mounted volumes shrank**

```bash
df -h | grep -cE 'CoreSimulator/(Volumes|Cryptex)'
```

Expected: count equals (number of keepers × ~3 because each runtime mounts a Volume + a Cryptex bundle).

- [ ] **Step 6: Record disk delta in execution log row**

```bash
df -h /System/Volumes/Data | tail -1 | awk '{print "after Phase 1:", $3, "used"}'
```

---

## Phase 2 — Xcode user data (typical recovery: 30-90 GB)

### Task 2.1: Inventory ~/Library/Developer/Xcode

**Files:** None (read-only)

- [ ] **Step 1: Capture per-subdir sizes**

```bash
du -sh ~/Library/Developer/Xcode/* 2>/dev/null | sort -h
```

Expected: list including DerivedData, Archives, iOS DeviceSupport, watchOS DeviceSupport, etc.

### Task 2.2: Wipe DerivedData (scorched-earth — first build after will be slow)

- [ ] **Step 1: Show what will be lost**

```bash
ls ~/Library/Developer/Xcode/DerivedData/ | wc -l
du -sh ~/Library/Developer/Xcode/DerivedData
```

Expected: folder count (often 50-200) and total size.

- [ ] **Step 2: Confirm with user (scorched-earth means even keeper-project caches go)**

"Wiping all DerivedData. Next Xcode build for essential-feed/Maeve/webdavis will rebuild from source (5-30 min depending on project). Proceed?"

- [ ] **Step 3: Trash everything inside**

```bash
trash ~/Library/Developer/Xcode/DerivedData/*
```

Expected: silent success. Folder is now empty.

### Task 2.3: Skip Archives unless they exceed 1 GB

- [ ] **Step 1: Check size**

```bash
du -sh ~/Library/Developer/Xcode/Archives 2>/dev/null
```

If under 1 GB: skip this task. If over: trash any `*.xcarchive` older than 1 year.

```bash
find ~/Library/Developer/Xcode/Archives -name "*.xcarchive" -mtime +365 -depth 1 -exec trash {} +
```

### Task 2.4: Wipe retired iOS DeviceSupport (older than current iOS major)

- [ ] **Step 1: List current device-support entries with sizes**

```bash
du -sh ~/Library/Developer/Xcode/iOS\ DeviceSupport/* 2>/dev/null | sort -h
```

Expected: one folder per `<version> (<build>) <arch>` combination.

- [ ] **Step 2: Determine the keep iOS major (matches your simulator runtime keep)**

If keeping iOS 26.x → delete every folder named `14.*`, `15.*`, `16.*`, `17.*`, `18.*`, `19.*`, `25.*`. Keep only `26.*`.

- [ ] **Step 3: Trash the retired versions**

```bash
# Example for keeping only iOS 26.x:
for d in ~/Library/Developer/Xcode/iOS\ DeviceSupport/*; do
  name=$(basename "$d")
  case "$name" in
    26.*) ;; # keep
    *) echo "trashing: $name"; trash "$d" ;;
  esac
done
```

Expected: each retired version named and trashed.

- [ ] **Step 4: Repeat for watchOS/tvOS/visionOS DeviceSupport folders if present**

```bash
ls ~/Library/Developer/Xcode/ | grep DeviceSupport
```

Apply the same iteration pattern to any matching folders.

- [ ] **Step 5: Record disk delta**

```bash
df -h /System/Volumes/Data | tail -1
```

---

## Phase 3 — Docker housekeeping (typical recovery: 5-30 GB live + named-volume audit)

### Task 3.1: Confirm Docker is running

- [ ] **Step 1: Check Docker daemon**

```bash
docker version >/dev/null 2>&1 && echo "Docker UP" || echo "Docker DOWN — start Docker Desktop first"
```

If down: start Docker Desktop, wait ~10 sec, retry.

### Task 3.2: Prune unused images, build cache, networks (preserves containers + named volumes)

- [ ] **Step 1: Capture baseline**

```bash
docker system df
```

Expected: shows Images / Containers / Local Volumes / Build Cache rows with TOTAL/ACTIVE/SIZE/RECLAIMABLE.

- [ ] **Step 2: Run prune (note: `--all` is image-only; volumes are untouched)**

```bash
docker system prune --all --force
```

Expected: list of `Deleted Images:` followed by `Total reclaimed space: <N>`.

- [ ] **Step 3: Verify**

```bash
docker system df
```

Expected: Images dropped to 0 or near 0; Local Volumes UNCHANGED (this is intentional).

### Task 3.3: Audit named volumes (manual decision — `system prune` does NOT touch these)

`docker system prune --all` is image-only. Named volumes (declared in compose files or created with `docker volume create`) survive forever unless explicitly deleted. They often contain real user state (databases, configs).

- [ ] **Step 1: List volumes with sizes and link counts**

```bash
docker system df -v 2>&1 | grep -A 100 'Local Volumes' | head -30
```

Expected: per-volume table with `LINKS` column showing how many containers reference each volume.

- [ ] **Step 2: Identify orphaned volumes (`LINKS == 0`)**

Volumes with `LINKS: 0` have no current container using them. Most often these are:
- `buildx_buildkit_*_state` — orphaned BuildKit builder cache (always safe to delete)
- Old test/staging volumes from since-removed compose stacks
- Volumes from images you deleted in Task 3.2

- [ ] **Step 3: Per-volume decision — does the volume contain data you want to keep?**

For each `LINKS: 0` volume, decide:
- **Definitely safe to delete:** any `buildx_buildkit_*_state` (build cache, regenerable on next `docker buildx`)
- **Probably safe:** volumes named after services you haven't run in 3+ months
- **Keep:** anything you can't immediately identify the purpose of (volumes are cheap to keep)

Confirm each delete with the user before running.

- [ ] **Step 4: Delete confirmed-orphaned volumes**

```bash
docker volume rm <name1> <name2> ...
```

Expected: each name echoed back.

### Task 3.4: Compact Docker Desktop VM disk

Docker on macOS runs in a VM whose disk is sparse — deletions inside the VM don't immediately return space to macOS. Compaction realizes the recovery.

- [ ] **Step 1: Open Docker Desktop → Settings → Advanced → "Clean / Purge data" → Cache**

Manual UI step. Click "Clean" and wait (can take 1-5 min). Docker may need to restart.

- [ ] **Step 2: Verify disk delta**

```bash
df -h /System/Volumes/Data | tail -1
```

Expected: macOS-side free space increased by however much was inside the VM disk pre-compact.

---

## Phase 4 — Application Support per-app drilldown (typical recovery: 5-50 GB)

This phase wasn't in the original spec but emerged as a major recovery vector. macOS does NOT auto-clean `~/Library/Application Support/` when you uninstall an app — the data lingers indefinitely. Any app you've uninstalled in the past N years has likely left a folder here.

### Task 4.1: Inventory top consumers

- [ ] **Step 1: List top 20 by size**

```bash
du -sh ~/Library/Application\ Support/* 2>/dev/null | sort -rh | head -20
```

Expected: ranked list. Focus on entries >500 MB.

### Task 4.2: Check which top consumers correspond to apps you no longer use

- [ ] **Step 1: For each top consumer, check whether the .app bundle still exists**

```bash
for name in <each-top-folder>; do
  if ls /Applications/ | grep -iq "$name"; then
    echo "  $name: app still in /Applications/"
  else
    echo "  $name: ORPHANED (app uninstalled, data remains)"
  fi
done
```

Expected: each top consumer flagged orphaned-or-installed.

- [ ] **Step 2: For installed apps you no longer USE, ask the user explicitly per-app**

Don't decide solo — ask: "I see <App> at <size>. Still using it?"

### Task 4.3: Trash retired-app data

For each confirmed-retired app, trash all four common storage locations:

- [ ] **Step 1: Trash Application Support folder**

```bash
trash ~/Library/Application\ Support/<AppName>
```

- [ ] **Step 2: Trash Containers (sandboxed apps)**

```bash
# Bundle ID, not display name
trash ~/Library/Containers/<bundle-id>
```

- [ ] **Step 3: Trash Group Containers (cross-bundle shared data)**

```bash
find ~/Library/Group\ Containers -maxdepth 1 -name "*<bundle-id>*" -type d -exec trash {} \;
```

- [ ] **Step 4: Trash Caches (if not already wiped in Phase 5)**

```bash
trash ~/Library/Caches/<bundle-id>
```

- [ ] **Step 5: If app .app bundle still exists, optionally uninstall**

Drag from `/Applications/` to trash, or `trash /Applications/<AppName>.app`.

---

## Phase 5 — `~/Library/Caches/*` wipe (typical recovery: 20-50 GB)

### Task 5.1: Inventory and confirm

- [ ] **Step 1: List top consumers**

```bash
du -sh ~/Library/Caches/* 2>/dev/null | sort -rh | head -20
```

Expected: ranked list. Common big items: `Homebrew` (often 5-15 GB), `Arc`/`Firefox`/`Google` (browser caches), `pip`/`pypoetry`/`pdm`/`deno`/`org.swift.swiftpm` (package managers), `ms-playwright`, `com.spotify.client`.

- [ ] **Step 2: Confirm with user — every cache here is regenerable; first launch of each app will be slower**

### Task 5.2: Wipe everything

- [ ] **Step 1: Trash all of Caches**

```bash
trash ~/Library/Caches/*
```

Expected: silent success.

- [ ] **Step 2: Verify**

```bash
du -sh ~/Library/Caches 2>/dev/null
```

Expected: small (<100 MB — apps may have re-created some immediately).

---

## Phase 6 — Apple Intelligence asset toggle (recovery: 14-19 GB if disabled)

This is a UI step that releases purgeable APFS space at next reboot.

### Task 6.1: Decide

- [ ] **Step 1: Inventory current asset sizes**

```bash
for d in com_apple_MobileAsset_UAF_FM_GenerativeModels com_apple_MobileAsset_UAF_FM_Visual com_apple_MobileAsset_UAF_FM_Overrides com_apple_MobileAsset_UAF_FM_CodeLM; do
  size=$(du -sh /System/Library/AssetsV2/$d 2>/dev/null | awk '{print $1}')
  echo "  $size  $d"
done
```

Expected: 4 directories with sizes. CodeLM is Predictive Code Completion (used by Xcode if PCC is enabled). Generative + Visual + Overrides are the consumer-facing Apple Intelligence features.

- [ ] **Step 2: Decide based on usage**

| Use case | Action |
|---|---|
| Use Apple Intelligence (Writing Tools, Image Playground, smart Siri) | Keep enabled — skip phase |
| Use Xcode Predictive Code Completion BUT not consumer AI | Apple Intelligence may need to stay enabled depending on Xcode version (test after disabling) |
| Don't use either | Disable for ~19 GB recovery |

### Task 6.2: Disable Apple Intelligence

- [ ] **Step 1: Open System Settings → Apple Intelligence & Siri**

Manual UI step.

- [ ] **Step 2: Toggle off Apple Intelligence**

macOS marks the asset directories purgeable but doesn't immediately delete them.

- [ ] **Step 3: After full cleanup, reboot to release purgeable space**

(Don't reboot mid-cleanup; do it after Phase 9.)

---

## Phase 7 — Misc filesystem cruft (typical recovery: 1-5 GB)

### Task 7.1: Arc browser PartialDownloads

- [ ] **Step 1: Count and trash**

```bash
ls ~/Downloads/.company.thebrowser.Browser.* 2>/dev/null | wc -l
trash ~/Downloads/.company.thebrowser.Browser.* 2>/dev/null
```

Expected: count > 0 (often 50-200 stale partial downloads), then trashed.

### Task 7.2: Homebrew cleanup

- [ ] **Step 1: Run brew cleanup**

```bash
brew cleanup --prune=all
```

Expected: list of pruned bottle archives + freed-space report.

### Task 7.3: Retired-project node_modules / .next / build/ / target/

- [ ] **Step 1: Find candidates**

```bash
find ~/workspaces -maxdepth 3 -type d \( -name node_modules -o -name .next -o -name target -o -name build \) -prune -mtime +180 2>/dev/null
```

Expected: list of build caches in projects untouched for 180+ days.

- [ ] **Step 2: Confirm each project is retired (no recent commits, no in-progress work)**

For each candidate: `cd <project> && git status -s | wc -l && git log -1 --format='%cr'`.

- [ ] **Step 3: Trash confirmed retired-project caches**

```bash
trash <each-confirmed-path>
```

### Task 7.4: System logs (only if disk pressure remains)

```bash
sudo du -sh /private/var/log /private/var/db/diagnostics 2>/dev/null
```

If `/private/var/db/diagnostics` is over 5 GB and disk is still tight, can be cleared:

```bash
sudo rm -rf /private/var/db/diagnostics/*.diag
```

Otherwise skip — system logs are useful for crash investigations.

---

## Phase 8 — iOS version refresh (when Xcode prompts)

If during the runbook execution Xcode prompts to install new components after a recent Xcode update:

- [ ] **Step 1: In Xcode's component installer, check ONLY platforms you use**

Per the user's setup as of 2026-04: macOS (forced), iOS, watchOS. Do NOT check tvOS or visionOS.

- [ ] **Step 2: After install completes, delete the now-stale older runtime**

If you previously kept iOS 26.1 and Xcode just installed iOS 26.4:

```bash
# Re-list runtimes by JSON to find the old one
xcrun simctl runtime list -j | python3 -c '
import json, sys
for uuid, v in json.load(sys.stdin).items():
    print(uuid, v.get("build"))
'
```

Match the old build number, delete by UUID:

```bash
xcrun simctl runtime delete <old-uuid>
xcrun simctl delete unavailable
```

Expected: ~16 GB recovery per old iOS major.

---

## Phase 9 — Empty trash (realize the bytes)

trash-cli stores deletions in `~/.local/share/Trash`. Until you empty it, the bytes are still on disk.

### Task 9.1: Spot-check trash contents

- [ ] **Step 1: List by source**

```bash
trash-list 2>&1 | wc -l
du -sh ~/.local/share/Trash 2>/dev/null
```

Expected: total item count and size matching what you trashed across phases.

- [ ] **Step 2: Look for unexpected items (anything outside the expected categories)**

```bash
trash-list 2>&1 | awk '{ for(i=3;i<=NF;i++) printf "%s ", $i; print "" }' | \
  grep -vE '^/(Users/[^/]+/(Library/(Caches|Developer|Application Support|Containers)|Downloads/\.company\.|workspaces/.*node_modules))' | \
  head -20
```

Expected: empty (everything is in known categories). If unexpected paths appear, investigate before emptying.

- [ ] **Step 3: Confirm large items by size**

```bash
du -sh ~/.local/share/Trash/files/* | sort -rh | head -10
```

Expected: largest items are simulator runtime volumes, retired-app Application Support folders, DerivedData folders. Anything unexpected, investigate.

### Task 9.2: Empty trash

- [ ] **Step 1: Confirm with user one last time before destructive realize**

"About to empty trash (N items, X GB). This is irreversible. Proceed?"

- [ ] **Step 2: Empty**

```bash
time trash-empty -f
```

Expected: completes in 30s-2min depending on item count. trash-cli deletes per-item; high item count is the bottleneck, not total bytes.

- [ ] **Step 3: Verify**

```bash
trash-list 2>&1 | wc -l
df -h /System/Volumes/Data | tail -1
```

Expected: 0 items remaining; disk usage dropped by ~the trash size.

---

## Phase 10 — Reboot to release purgeable space

If Phase 6 disabled Apple Intelligence, or if any other operation marked space "purgeable" in APFS terminology (visible via `diskutil info /System/Volumes/Data | grep -i purgeable`), the bytes don't actually return to free until a reboot triggers reclamation.

- [ ] **Step 1: Check purgeable space before reboot**

```bash
diskutil info /System/Volumes/Data | grep -iE 'free|purgeable'
```

If purgeable is high (>5 GB), reboot is worth doing.

- [ ] **Step 2: Reboot**

Manual: System menu → Restart, or `sudo shutdown -r now`.

- [ ] **Step 3: Re-verify after reboot**

```bash
df -h /System/Volumes/Data | tail -1
diskutil info /System/Volumes/Data | grep -iE 'free|purgeable'
```

Expected: free space increased by previously-purgeable amount; purgeable is small.

---

## Post-flight

### Task 11: Update execution log

- [ ] **Step 1: Add a complete row to the execution log table at the bottom of this file**

Columns: Date | Used before (GB) | Used after (GB) | Recovered (GB) | Notes (anything unusual or new this run).

- [ ] **Step 2: Commit the updated runbook**

```bash
cd ~/.local/share/chezmoi
git add docs/superpowers/plans/2026-04-27-macos-disk-cleanup-plan.md
git commit -m "chore(cleanup): execution log for $(date +%Y-%m-%d) quarterly disk cleanup"
```

---

## Execution log

| Date       | Used before (GB) | Used after (GB) | Recovered (GB) | Notes |
| ---------- | ---------------: | --------------: | -------------: | :---- |
| 2026-04-27 |              774 |             388 |            386 | Initial run; ad-hoc additions later codified into this runbook (Application Support drilldown, Apple Intelligence toggle, Docker named-volume audit, iOS 26.1→26.4 swap). Phase 1 alone: 213 GB. Phase 2-4 trash: 92 GB. Application Support: 41 GB. Docker volume cleanup: 5 GB. Apple Intelligence + purgeable: ~15 GB. iOS swap: 12 GB. |
