# Claude Code settings (the modify-template)

`private_dot_claude/modify_settings.json` is a chezmoi **modify-template** (no `.tmpl` extension, by
chezmoi convention) that selectively enforces a fixed set of stable fields in `~/.claude/settings.json`.
On every `chezmoi apply`, the script reads the current target file, overlays the stable fields below via
`setValueAtPath`, and writes the merged result back.

The retired `--exclude=templates` did not skip a modify-template (measured 2026-08-02), so this path runs
on every `just a`, not only on a full apply.

Fields fall into **three** categories, not two.

## 1. Chezmoi-controlled stable fields

Overwritten from the template on every apply, whatever the live file holds.

- `permissions.allow` (read-only tools: Read, Grep, Glob, WebFetch, WebSearch, plus eight read-only
  `Bash(...)` globs: `find`, `cat`, `ls`, `head`, `tail`, `wc`, `grep`, `tree`), `permissions.deny` (14
  rules, listed below), `permissions.defaultMode` = `bypassPermissions`.
  - `Read(.env)`, `Read(.env.*)`, `Read(secrets/**)`, `Read(credentials.json)`,
    `Read(~/.aws/credentials)`, `Read(~/.ssh/id_*)`, `Read(~/.ssh/*_rsa)`, `Read(~/.ssh/*_ed25519)`,
    `Read(~/.claude/.credentials.json)`, `Read(~/.codex/auth.json)`, `Read(~/.config/pns/config.toml)`,
    `Read(~/.config/osquery/webhook-secret)`, `Read(~/.hermes/.env)`, `Read(~/**/*.kdbx)`.
  - The `~/` prefixes are load-bearing and the first four rules lack one deliberately. A bare or
    `./`-prefixed pattern is CURRENT-DIRECTORY relative, which is exactly right for a project's own
    `.env`, `secrets/` and `credentials.json`, and was wrong for the home-anchored rules:
    `Read(.ssh/id_*)` in user settings matched a project's own `.ssh` directory and never `~/.ssh` (found
    2026-08-05).
- `hooks`, 12 event keys:
  - `UserPromptSubmit` runs `pns hook prompt`, which marks the turn's start.
  - `Stop` runs ONE async command, `pns hook stop`: the engine reports the turn and decides the lights in
    the same pass, where a second hook used to decide the tier on its own.
  - `PostModelSwitch` runs `pns hook model-switch` async, restricted to `source == "auto"`, and redirects
    its own stdout to `/dev/null` because this is one of the events whose exit-0 stdout reaches the
    model's next turn regardless of the `async` flag.
  - `StopFailure` runs `pns hook stop-failure`, async for the same reason `Stop` is: it fires INSTEAD of
    `Stop` when a turn dies rather than finishing, so without it a dead pane gets no card at all.
  - `Notification` carries a QUOTA-ONLY hook: one exact pipe-separated matcher naming the three
    `quota_auto_resume_*` types, async, running `pns hook quota`. The `permission_prompt` matcher used to
    run `alerter` directly, the last notification path that reached the operator without passing the
    presence engine, and it double-fired against the approval hook below; it was deleted rather than
    replaced, so the slot sat empty until the quota entry took it. The approval itself hangs off
    `PermissionRequest`, which runs `pns hook blocked` NOT async, because the harness waits for it and
    registers the card before the prompt is drawn. Its exit code is NOT the operator's answer: that comes
    back through moshi's own bridge typing into the prompt (measured 2026-08-29, `modify_settings.json`:
    approve and deny both leave the hook exiting 0 with empty stdout).
  - `ConfigChange` runs `pns hook config-change` async, one exact pipe-separated matcher naming the five
    documented config sources, carding a configuration change as an audit trail rather than a turn
    needing attention.
  - `PermissionDenied` runs `pns hook denied` async, reporting the tool call auto-mode refused without
    ever asking; async is what keeps pns out of the retry decision the harness awaits on this hook.
  - `Elicitation` runs `pns hook asked` async, carding the MCP server that stopped mid-tool-call to ask
    the operator for input; async is what keeps pns out of the answer, since this hook runs before the
    dialog is shown and exit code 2 alone would decline the request outright.
  - `PostToolBatch` runs `pns hook resolved` async with no matcher, clearing the nag record when an
    assistant tool batch resolves, whether the operator approved the call or denied it: a denied call
    still produces a tool_result, so it resolves the batch rather than skipping it. The classifier's own
    refusals are `PermissionDenied`'s to report, not this entry's.
  - `PostToolUse` carries two matchers, `AskUserQuestion` and `ExitPlanMode`, calling `pns hook asked`
    and `pns hook plan-ready`.
  - `SessionStart` is herdr's own agent-state integration, and ownership is split.
    `herdr integration install claude` creates the hook FILE at `~/.claude/hooks/herdr-agent-state.sh`,
    which is deliberately unmanaged, and writes this ENTRY the first time. The template then redeclares
    the same entry, because the `hooks` write is whole-value and would otherwise erase it.
- `skillOverrides`, one `setValueAtPath` per on-demand skill (29 today), each set to
  `user-invocable-only`, transcribed BY HAND from the tiers table in `dot_agents/custom-skill-lock.json`.
  Nothing enforces that the two agree: the roster guard was declaration-consistency checking and went
  with the 2026-08-05 test-scope ruling. Per key, so overrides the user sets for other skills drift
  freely.
- `statusLine`, `cleanupPeriodDays` (= 365, a year of session retention), `autoUpdatesChannel` (=
  `stable`, pins the release channel so updates lag `latest`), `remoteControlAtStartup` (= `true`, starts
  the Remote Control bridge every session), `spinnerTipsEnabled` (= `false`, operator 2026-08-11: no
  spinner tips anywhere), `effortLevel` (= `xhigh`, the top of the `low`/`medium`/`high`/`xhigh` enum;
  stable rather than free-drift so a `/config` write cannot quietly leave a session on a lower reasoning
  budget).
- `env`, six keys, written per key so an env var the operator sets by hand drifts freely:
  `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` = `1`, `CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY` = `1`,
  `DISABLE_TELEMETRY` = `1`, `DISABLE_ERROR_REPORTING` = `1`, `DISABLE_NON_ESSENTIAL_MODEL_CALLS` = `1`,
  `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` = `75`.
- `attribution.commit` and `attribution.pr`, both set to the empty string, which is the native switch for
  the co-author trailer and the generated-with footer the global rules already forbid by hand.
- `extraKnownMarketplaces`, six entries on darwin and five on Linux. The five GitHub ones each carry
  `autoUpdate` = `true`: `ponytail`, `openai-codex`, `worktrunk`, `last30days-skill`, `plannotator`.
  Claude Code refreshes a marketplace and its installed plugins at startup on its own, but only defaults
  that on for marketplaces Anthropic publishes (read out of the shipped 2.1.220 binary on 2026-08-03), so
  without those five entries those plugins sit at their install version forever. The sixth, `pns`, is
  darwin-only and deliberately carries NO `autoUpdate`: it is this repo's own marketplace, a directory at
  `~/.claude/pns-marketplace` that chezmoi converges on every apply, and `.chezmoiignore` drops that
  directory on Linux, where declaring it would be a startup refresh that always fails. The write is per
  marketplace key, so a marketplace added with `claude plugin marketplace add` keeps its own entry. What
  those silent startup updates changed is recorded weekly by
  `~/.local/libexec/unattended-upgrades/claude/report-plugin-updates.sh`; see the plugin update record in
  `docs/runbooks/agent-skills-store.md`.

`plannotator` is declared here rather than installed from its own `curl | bash` script on purpose. That
script writes a binary, hooks, skills and slash commands into `~/.claude/` and `~/.codex/`, which are
chezmoi targets, so the next full apply would erase part of what it wrote and leave the rest. The
marketplace route delivers the plan-review hooks, which is the part worth having, and touches nothing
chezmoi owns.

## 2. Free-drift (Claude Code owns)

`alwaysThinkingEnabled`, `useAutoModeDuringPlan`, `voiceEnabled`, `skipDangerousModePermissionPrompt`,
and any future setting `/config` adds.

## 3. `enabledPlugins`, which is neither

The **roster** is chezmoi-controlled and the per-plugin **state** is not. The template declares thirteen
plugin ids on every OS and appends `pns@pns` on darwin, so fourteen on this machine (keys are
`<name>@<marketplace>`, which is the form Claude Code writes, not the bare name the CLI prints on
success) and the write is whole-value, so a marketplace plugin enabled live but missing from the
declaration is turned OFF by the next apply. Within that roster, both members of Claude Code's own union
for this key are the machine's to set and are carried through unchanged: the JSON boolean `false` that
`claude plugin disable` writes, and the JSON array of version constraints that its schema calls the
extended format (a plugin held at a reviewed release). Every other shape renders `true`: an absent key, a
JSON null, a string and a number.

So `claude plugin disable <id>` STICKS across applies for the declared ids, and applying is not the way
to turn one back on: use `claude plugin enable <id>`. The trade was taken deliberately, because a
containment verb a scheduled apply can silently revoke is not containment.

**The promise stops at the declared roster, and an erased entry costs different things depending on where
the plugin came from.** Claude Code 2.1.220 resolves the two kinds differently (read out of the shipped
binary on 2026-08-02). A marketplace plugin is discovered THROUGH this key: the loader walks the merged
settings' `enabledPlugins` entries and skips any whose value is undefined, so an id the file does not
hold is never loaded. Erasing an undeclared marketplace plugin's `false` therefore leaves it off, by a
different mechanism, though the file stops recording why. A plugin under `~/.claude/skills/` is found by
scanning that directory instead, and its entry only adjusts state afterwards; with no entry it falls back
to the plugin manifest's `defaultEnabled`, which defaults to true. Every skill this repo symlinks into
`~/.claude/skills/` is that second kind, so a `claude plugin disable` on one writes a `false` that the
next apply erases, and the skill comes back on. Nothing is drifting today, the live file holds exactly
the declared ids, but the skills case needs one `claude plugin disable` to become real.

**Read the price before relying on this.** It is not the recovery ergonomics (though those are real:
`claude plugin disable --all` is one command, there is no `enable --all`, so undoing a mass disable is
one `claude plugin enable` per declared plugin). The price is that **this repo no longer re-asserts
plugin state at all.** The old whole-value write forced every declared plugin back to `true` on every
apply, which incidentally remediated tampering. Now a live `false` is carried forward for as long as it
sits in the file, and because the render reproduces it byte for byte, `chezmoi status` and `chezmoi diff`
print nothing for it (measured 2026-08-02), which includes `just d`. Stated plainly: any process running
as this user, which is every agent with Bash under `permissions.defaultMode = bypassPermissions`,
permanently disables `security-guidance` or `superpowers` by writing one boolean, and nothing in this
repo detects it or restores it. Auditing that key means reading `~/.claude/settings.json` or running
`claude plugin list`; drift tooling will not raise it.

**And "sticks" means "against an apply", not "cannot load".** Settings precedence runs user, project,
local, flag, policy, so a repo whose project settings enable the plugin loads it there whatever
`~/.claude/settings.json` says. Claude Code ships an `ineffective-disable` diagnostic for exactly that
case (the diagnostic id and all five source names are in the shipped 2.1.220 binary, read 2026-08-02).

## When the live file is not JSON, a run_before script quarantines it

Reading the live file is what makes preserving its state possible, and a modify-template that cannot read
it fails the entire run: every later target and every `run_after_` script is skipped, and the unreadable
file is left in place, so `permissions.deny` is not restored either. Whitespace-only and empty files are
handled (the read is trimmed first), and so is anything that parses. A file that is non-empty and is not
JSON, such as a write truncated by a crash or a full disk, is not, and no template can fix it: chezmoi's
JSON readers all fail the template on bad input and Go templates have no error recovery, so there is
nothing to fall back from. That is why the repair runs BEFORE the template rather than inside it.

**So a corrupt live file does not normally block the apply.**
`.chezmoiscripts/run_before_12-quarantine-unparseable-claude-settings.sh` moves an unreadable settings
file into `~/workspaces/backups/<timestamp>.claude-settings-quarantined.backup.json` (moved, never
deleted; the timestamp uses hyphens, not colons), warns loudly, and leaves `{}` for the template to
rebuild from. A readable file is left byte-identical, including the shapes that are not JSON objects
(empty, whitespace-only, a whole-file array), and an absent one stays absent. What the move costs is
per-plugin state, which lives only in the live file: every declared plugin comes back enabled, so a
disable has to be re-applied with `claude plugin disable <id>` after a quarantine.

Two cases the script hands back to a human, and it says both out loud. With no `jq` on PATH it exits
without a verdict rather than risk destroying a healthy file, and a move it cannot complete is reported
with a warning that the apply will fail in `modify_settings.json` until the file is repaired by hand.
Repair `~/.claude/settings.json` in either case; the apply's own error names the template.

`test/unit/claude-settings-quarantine.sh` pins the repair side of this: an unreadable file is moved and
replaced with `{}`, and every shape the template does survive is left byte-identical, including the three
that are not JSON objects. The template side is unpinned. The test that applied the template into a
throwaway destination once per live-file shape per target OS was deleted in the 2026-08-05 purge
(`d348c136`), so everything in the three sections above is held by review rather than by a gate.

## Promoting a `/config` toggle

Add a `setValueAtPath` call for that key in `private_dot_claude/modify_settings.json` and commit.

Background: `/config` writes ergonomic toggles directly into `~/.claude/settings.json` (verified
empirically), and Claude Code does not provide a user-level `~/.claude/settings.local.json` for
overrides, only project-scope `.claude/settings.local.json` exists. The modify-template approach is the
cleanest way to keep policy fields under chezmoi control while letting `/config` mutate everything else
freely. See https://www.chezmoi.io/user-guide/manage-different-types-of-file/ for the `modify_` template
and `setValueAtPath` reference.
