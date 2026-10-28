# Deployed binary cleanup audit

Status: audit, written 2026-09-17 for ledger task 21a. Nothing was deleted, moved or trashed to
produce it, and no apply ran. This repository builds no removal mechanisms (operator ruling
2026-08-02), so the outcome of an audit is a list the operator runs by hand.

Finding in one line: **all four old binaries are already gone.** `~/.local/libexec/pns/pns`,
`~/.local/libexec/uu/uu`, `~/.local/libexec/posture/posture` and `~/.local/libexec/lights` do not
exist on dresden today. One leftover remains, and it is a directory rather than a binary: the empty
`~/.local/libexec/uu/`. That is the whole of the trash list below.

## What task 21a asked, and what changed under it

The ledger entry records that the full apply of 2026-09-15 passed and that `ls` still reported all
four binaries, each dated 2026-09-09, and asks for the caller verification plus the operator's
approval of the exact files. The caller verification is below and it holds. The file list does not:
between that ledger note and this audit the four were removed by something other than an apply.

Evidence of absence, `stat` on 2026-09-17:

```
stat: /Users/stephen/.local/libexec/pns/pns: stat: No such file or directory
stat: /Users/stephen/.local/libexec/uu/uu: stat: No such file or directory
stat: /Users/stephen/.local/libexec/posture/posture: stat: No such file or directory
stat: /Users/stephen/.local/libexec/lights: stat: No such file or directory
```

Evidence of when. A directory's mtime moves when an entry is added or removed, and all four parents
carry the same stamp, which reads as one sweep rather than four events:

```
/Users/stephen/.local/libexec       drwx------  mtime=2026-09-15T05:29:50-0600
/Users/stephen/.local/libexec/pns   drwx------  mtime=2026-09-15T05:29:50-0600
/Users/stephen/.local/libexec/uu    drwxr-xr-x  mtime=2026-09-15T05:29:50-0600
/Users/stephen/.local/libexec/posture  drwx------  mtime=2026-09-15T05:29:50-0600
```

`~/.local/libexec` itself carries that stamp because `lights` sat directly in it. Two facts say the
removal was not `trash`: `~/.Trash` holds no entry named `pns`, `uu`, `posture` or `lights`, and the
apply transcripts under `~/.local/state/chezmoi-apply/` only reach back to 2026-09-17 and mention none
of the four paths. So the deletion is unattributed. It is also correct, which is why this audit
records it rather than proposing to undo it.

The current binaries are all in place at the declared destination:

```
-rwxr-xr-x  3226272  Sep 15 13:53  /Users/stephen/.cargo/bin/lights
-rwxr-xr-x  7330880  Sep 17 04:18  /Users/stephen/.cargo/bin/pns
-rwxr-xr-x  3974528  Sep 17 04:18  /Users/stephen/.cargo/bin/posture
-rwxr-xr-x  4272992  Sep 16 19:07  /Users/stephen/.cargo/bin/uu
```

## 1. The callers

Two searches, the repository on branch `docs/deployed-binary-cleanup-audit` and the deployed side
under `$HOME`. Every entry says which path the hit names. Nothing anywhere is a live caller of an
old path.

### Repository, live code and configuration

Every entry below names the CARGO path. Where the note says "via `rust_tools`" the file does not
spell the directory at all: it reads `rust_tools.install_dir` from
`.chezmoidata/rust_tools.yaml` at render time, which is the one edit the 2026-09-09 move needed.

- `.chezmoidata/rust_tools.yaml:17`, `install_dir: ".cargo/bin"`. The single declaration every
  other caller resolves through.
- `.chezmoiscripts/run_after_59-setup-osquery.sh:8`,
  `converge="${CHEZMOI_HOME_DIR:-$HOME}/.cargo/bin/posture"`. The converge call on every apply.
- `.chezmoiscripts/run_onchange_after_54-build-lights.sh.tmpl`,
  `run_onchange_after_58-build-pns-engine.sh.tmpl`, `run_onchange_after_58-build-posture.sh.tmpl`
  and `run_onchange_after_59-build-uu.sh.tmpl`, via `rust_tools`. The four builders.
- `.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh`, in `refresh_manifest`, writes
  the pns and posture manifest rows at `$home/.cargo/bin/$refresh_manifest_binary`.
- `Library/LaunchAgents/com.webdavis.pns-daemon.plist.tmpl`, via `rust_tools`. The pns clock.
- `Library/LaunchAgents/com.webdavis.uu.plist.tmpl`, via `rust_tools`. The weekly uu run.
- The six `Library/LaunchAgents/com.webdavis.osquery-*.plist.tmpl` agents, via `rust_tools`:
  `digest`, `firewall-gatekeeper-monitor`, `heartbeat`, `results-alerter`, `tailscale-monitor` and
  `uptime-watchdog`. The posture subcommands.
- `private_dot_claude/modify_settings.json`, via `rust_tools`. The twelve pns hook commands.
- `dot_aerospace.toml:198-209`, `~/.cargo/bin/lights`. Eleven function-key bindings.
- `justfile:156` and `justfile:204`, `~/.cargo/bin/uu run brew` and `~/.cargo/bin/uu run skills`.
- `dot_bashrc.tmpl`, via `rust_tools`. The long-running command notifier's begin and end callbacks.
- `dot_config/nvim/lua/plugins/pns.lua:7`, `binary = "~/.cargo/bin/pns"`. The pns.nvim pin.
- `dot_config/osquery/private_page-launchd-allowlist.txt:39-45`, seven `program` fields naming
  `~/.cargo/bin/posture <subcommand>` and `~/.cargo/bin/pns daemon run`.
- `dot_config/posture/private_config.toml.tmpl` and `dot_config/uu/private_config.toml.tmpl`, via
  `rust_tools`. The producer command and the osquery converge command.
- `scripts/cutover-gate.sh:1006`, `relay="$HOME/.cargo/bin/pns"`.
- `lights/crates/lights-adapters/src/notification.rs:27`, `home.join(".cargo/bin/pns")`. lights'
  own producer default.
- `posture/crates/posture/src/watchdog/configuration.rs:80`, `home.join(".cargo/bin/pns")`.
  posture's own producer default.
- `posture/crates/posture-domain/src/known_good.rs:160`,
  `target == format!("{home}/.cargo/bin/posture")`. The pipeline-path exact match.
- `uu/crates/uu-adapters/src/schedule.rs:16`, `Path::new(home).join(".cargo/bin/uu")`. The plist
  uu writes for itself.
- `dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh:9`,
  `agent="$HOME/.cargo/bin/pns"`. The command the Codex installer writes.

One repository file names an old path in live code, and it is not a caller:

`dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh:41` and `:43-45` build the `$owned`
array, the set of legacy commands the `migrate` filter RECOGNIZES and rewrites to the cargo path:
`.local/libexec/pns/pns hook <action>`, `.local/libexec/pns/hooks/relay-agent.sh <action>`,
`.local/libexec/pns/codex-hooks/relay-agent.sh <action>` and `.local/bin/relay-agent.sh <action>`.

That list is the migration landed by PR #534. The installer's own `agent` variable is
`$HOME/.cargo/bin/pns` on line 9 and it exits 0 when that binary is absent, so it can only ever write
the cargo path. Naming the old path is what lets it repair a hooks file that still holds one.

Everything else in the repository that names an old path is a test fixture or a document:

- `test/unit/pns-codex-hook-migration.test.sh` and `test/unit/pns-codex-install-hooks.sh` build
  legacy hook entries on purpose, to prove the migration rewrites them.
- `docs/remaining-work.md` lines 318, 319, 1769, 1957, 1961, 1979, 2292 and 2293 record the leftovers
  and this task. Left untouched here: the orchestrator records the outcome.
- `docs/superpowers/plans/2026-09-0{1,5,6}-*.md` and
  `docs/superpowers/specs/2026-09-0{5,6,8}-*.md`, plus `2026-09-14-lights-manifest-coverage-design.md`
  and `2026-09-14-june-hardening-dispositions-design.md`, are dated design records that describe the
  pre-2026-09-09 layout. The lights coverage design already carries the correction.
- `pns/docs/specs/*.md` and `pns/docs/pns-refactor.md` name `.local/libexec/pns/channels` and the
  installer's source path, not a binary.

### Deployed side under `$HOME`

Searched `~/Library/LaunchAgents`, `~/.local/bin`, `~/.local/libexec`, `~/.config`, `~/.claude` and
`~/.codex` for all four old paths.

- `~/Library/LaunchAgents`: none.
- `~/.local/bin`: none.
- `~/.local/libexec`: one, `pns/hooks/codex/install-hooks.sh:41`, the deployed copy of the legacy
  pattern list above.
- `~/.config`: none.
- `~/.claude`: none in `settings.json` or any hook. Five review transcripts under
  `~/.claude/pipeline/sol-pr-reviews/out/` quote old paths out of PR bodies they reviewed, which are
  logs rather than callers.
- `~/.codex`: none.

The two live hook tables both name the cargo path:

```
$ jq -r '.hooks | to_entries[] | .value[].hooks[]?.command' ~/.codex/hooks.json
PNS_AGENT=codex /Users/stephen/.cargo/bin/pns hook blocked
PNS_AGENT=codex /Users/stephen/.cargo/bin/pns hook stop
(plus four seshagy/herdr entries and one plannotator entry, none naming pns)
```

All twelve pns hook commands in `~/.claude/settings.json` read
`/Users/stephen/.cargo/bin/pns hook <event>`.

## 2. What must be preserved

`~/.local/libexec/pns/` must survive as a directory. Verified two ways.

`chezmoi managed --path-style=absolute` declares it, with one file inside it:

```
/Users/stephen/.local/libexec/pns
/Users/stephen/.local/libexec/pns/hooks
/Users/stephen/.local/libexec/pns/hooks/codex
/Users/stephen/.local/libexec/pns/hooks/codex/install-hooks.sh
```

That file is the only member, it is present on disk at mode 0700 and 4,893 bytes dated 2026-09-13,
and it is the bash script the libexec rule keeps there because writing another tool's config file is
what it does. Its source is `dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh`, the only
file under `dot_local/libexec/pns/` in the repository.

A second, quieter reason not to delete the directory: `pns/crates/pns/src/channel_dispatch.rs:86`
defaults the executable-channel directory to `$HOME/.local/libexec/pns/channels`. That path does not
exist today, which pns reads as no channels installed, so nothing is broken, but the directory is
still a live lookup root for the running engine and not merely the installer's parent.

`~/.local/libexec/posture/` must also survive, and everything in it is live managed data rather than
a stale binary. The deployed tree matches `chezmoi managed` file for file:

```
~/.local/libexec/posture/controls.json
~/.local/libexec/posture/converge/desired/osquery.conf
~/.local/libexec/posture/converge/desired/osquery.flags
~/.local/libexec/posture/converge/desired/packs/agent-attack-surface.conf
~/.local/libexec/posture/converge/desired/packs/installed-software-drift.conf
~/.local/libexec/posture/converge/desired/packs/intrusion-detection.conf
~/.local/libexec/posture/converge/desired/packs/security-policy-regression.conf
```

That is the staging tree `osquery-converge` installs into `/var/osquery` from, plus the flat-file
control catalog the poller reads. Nothing there is a leftover.

Two unmanaged files sit in the pns tree and are neither live nor stale binaries:
`~/.local/libexec/pns/.DS_Store` and `~/.local/libexec/pns/hooks/.DS_Store`, Finder cruft. They are
not in this audit's scope and are not in the command list.

`~/.local/libexec/uu/` has no managed member at all. There is no `dot_local/libexec/uu` in the
repository, `chezmoi managed` declares nothing under that path, and the deployed directory is empty at
mode 0755. It is purely the parent the removed binary left behind.

## 3. The integrity finding

Read from the generator and the two judges, not from the runbook:
`.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh`,
`dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh`, and posture's
`known_good.rs` plus `integrity.rs`.

**A stale binary was never in a manifest.** The manifest file SET comes from `chezmoi managed`, which
is chezmoi's source-state intent, never a listing of the protected tree. The generator's own docblock
says so and gives the reason: a file an attacker plants in a covered directory is not managed, never
enters a manifest, and is therefore either paged forever or left to the untracked-neighbor path. An
old binary is exactly that shape. Confirmed against the deployed manifests, which are root-owned 0644
and need no privilege to read:

```
$ grep -c -E "libexec/(pns/pns|uu/uu|posture/posture|lights)$" \
    /var/osquery/pipeline-known-good.sha256 /var/osquery/managed-bin-known-good.sha256
/var/osquery/pipeline-known-good.sha256:0
/var/osquery/managed-bin-known-good.sha256:0

$ grep -E "libexec/(pns|uu)" /var/osquery/managed-bin-known-good.sha256
<sha256> 0700 501 /Users/stephen/.local/libexec/pns/hooks/codex/install-hooks.sh
```

**All four were watched, though, and one of them would have paged on removal.** The watch set in
`dot_local/libexec/posture/converge/desired/osquery.conf.tmpl` has a `managed_bin` group covering
`~/.local/bin/%%` and `~/.local/libexec/%%` recursively, so every one of the four old paths was
watched, and a `pipeline_integrity` group covering `~/.local/libexec/osquery/%%` and
`~/.local/libexec/posture/%%` with hashes.

Tracking then splits on which arm the path lands in. `pipeline_verdict` and posture's
`integrity_verdict` agree exactly:

```
_pipeline_is_tracked() {
  case "$target" in
    "$HOME"/.local/libexec/osquery/* | "$HOME"/.local/libexec/posture/*) return 0 ;;
    ...
    "$HOME"/.local/bin/* | "$HOME"/.local/libexec/*) _managed_bin_is_tracked "$target" ;;
```

```
  _pipeline_is_tracked "$target" || return 1
  [[ $verb == DELETED ]] && return 0
```

So, per path, had the operator trashed them while they still existed:

1. `~/.local/libexec/posture/posture` matches the first arm by PREFIX, which is unconditional and
   owes nothing to the manifest. It is tracked whatever the manifest holds, and a `DELETED` verb on a
   tracked path returns page before any tuple check runs. **Trashing it would have paged a CRIT, and
   no apply, before or after, could have prevented that:** the DELETED arm short-circuits the manifest
   comparison entirely, so there is no manifest state that suppresses the removal of a tracked file.
1. `~/.local/libexec/pns/pns`, `~/.local/libexec/uu/uu` and `~/.local/libexec/lights` fall to
   `_managed_bin_is_tracked`, which is manifest-driven: a path is tracked only when it appears as a
   line in the managed-bin manifest. None of the three ever did, so all three were untracked, and
   `pipeline_verdict` returns 1, log-only. **Trashing those three was silent.** The one exception is
   the fail-safe hinge: a manifest that is missing, unreadable, empty or untrustworthy tracks
   EVERYTHING under those directories, so with a broken manifest their removal would page too, loudly
   and correctly.

**No apply is needed on either side of a trash of any of these paths.** The manifests derive from
intent, so removing an unmanaged file does not stale them, and the empty `~/.local/libexec/uu/`
directory carries no manifest line to refresh. The reverse direction also holds: a later apply will
not recreate any of the four, because chezmoi manages none of them.

**The remaining trash, the empty `~/.local/libexec/uu/` directory, generates no event.** osquery's
`file_events` reports files, and the directory holds none, so removing an empty directory is silent
on both the pipeline and the managed-bin arms. There is no order to observe and no page to expect.

## 4. The commands, in order

One command. `trash` rather than `rm`, per the standing rule.

1. ```bash
   trash ~/.local/libexec/uu
   ```

   Safe because the directory is empty, chezmoi manages nothing under it (there is no
   `dot_local/libexec/uu` in the repository and `chezmoi managed` declares no path below it), no
   caller in the repository or under `$HOME` names it, it carries no line in either known-good
   manifest, and an empty directory emits no `file_events` record, so its removal pages nothing and
   needs no apply after it.

Nothing else is proposed, for these reasons:

- `~/.local/libexec/pns/pns`, `~/.local/libexec/uu/uu`, `~/.local/libexec/posture/posture` and
  `~/.local/libexec/lights` are already absent. There is nothing to trash.
- `~/.local/libexec/pns/` and `~/.local/libexec/posture/` are preserved. Both hold live managed
  files, listed in section 2, and the pns directory is also the running engine's default channels
  lookup root.
- No path in this audit still has a live caller, so nothing had to be withheld on that ground.

## Open questions

1. **Does the ledger want the unattributed 2026-09-15 05:29 removal traced?** The evidence says one
   sweep at one instant, outside `trash` and outside an apply. The result is the state task 21a was
   asking for, so tracing it is forensics rather than cleanup, and this audit does not pursue it.
1. **Should the two `.DS_Store` files under `~/.local/libexec/pns/` be swept in the same sitting?**
   They are unmanaged, untracked by the manifest-driven bin arm, and silent either way. Out of this
   task's scope; a one-line yes adds them to the command above.
