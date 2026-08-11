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
  `Bash(...)` globs), `permissions.deny` (`Read(.env)`, `Read(.env.*)`, `Read(secrets/**)`,
  `Read(credentials.json)`, `Read(.aws/credentials)`, `Read(.ssh/id_*)`), `permissions.defaultMode` =
  `bypassPermissions`.
- `hooks`, five event keys:
  - `UserPromptSubmit` marks session start.
  - `Stop` runs two commands, `claude-stop-pulse.sh` (Hue lights) and an async `relay-agent.sh done`.
  - `Notification` (`permission_prompt` matcher) runs two, `alerter --timeout 30` and an async
    `relay-agent.sh blocked`.
  - `PostToolUse` carries two matchers, `AskUserQuestion` and `ExitPlanMode`, both calling
    `relay-agent.sh`.
- `skillOverrides`, one `setValueAtPath` per on-demand skill (27 today), each set to
  `user-invocable-only`, sourced from `dot_agents/custom-skill-lock.json` and gated by
  `test/unit/skills-roster-fanout.sh`. Per key, so overrides the user sets for other skills drift freely.
- `statusLine`, `cleanupPeriodDays` (= 365, a year of session retention), `autoUpdatesChannel` (=
  `stable`, pins the release channel so updates lag `latest`), `remoteControlAtStartup` (= `true`, starts
  the Remote Control bridge every session), `effortLevel` (= `xhigh`, the top of the
  `low`/`medium`/`high`/`xhigh` enum; stable rather than free-drift so a `/config` write cannot quietly
  leave a session on a lower reasoning budget).
- Four `extraKnownMarketplaces` entries, each with `autoUpdate` = `true`: `ponytail`, `openai-codex`,
  `worktrunk`, `last30days-skill`. Claude Code refreshes a marketplace and its installed plugins at
  startup on its own, but only defaults that on for marketplaces Anthropic publishes (read out of the
  shipped 2.1.220 binary on 2026-08-03), so without these four entries those plugins sit at their install
  version forever. The write is per marketplace key, so a marketplace added with
  `claude plugin marketplace add` keeps its own entry. What those silent startup updates changed is
  recorded weekly by `~/.local/libexec/unattended-upgrades/claude/report-plugin-updates.sh`; see the
  plugin update record in `docs/runbooks/agent-skills-store.md`.

## 2. Free-drift (Claude Code owns)

`alwaysThinkingEnabled`, `useAutoModeDuringPlan`, `voiceEnabled`, `skipDangerousModePermissionPrompt`,
and any future setting `/config` adds.

## 3. `enabledPlugins`, which is neither

The **roster** is chezmoi-controlled and the per-plugin **state** is not. The template declares twelve
plugin ids (keys are `<name>@<marketplace>`, which is the form Claude Code writes, not the bare name the
CLI prints on success) and the write is whole-value, so a marketplace plugin enabled live but missing
from the declaration is turned OFF by the next apply. Within that roster, both members of Claude Code's
own union for this key are the machine's to set and are carried through unchanged: the JSON boolean
`false` that `claude plugin disable` writes, and the JSON array of version constraints that its schema
calls the extended format (a plugin held at a reviewed release). Every other shape renders `true`: an
absent key, a JSON null, a string and a number.

So `claude plugin disable <id>` STICKS across applies for the twelve declared ids, and applying is not
the way to turn one back on: use `claude plugin enable <id>`. The trade was taken deliberately, because a
containment verb a scheduled apply can silently revoke is not containment.

**The promise stops at the twelve, and an erased entry costs different things depending on where the
plugin came from.** Claude Code 2.1.220 resolves the two kinds differently (read out of the shipped
binary on 2026-08-02). A marketplace plugin is discovered THROUGH this key: the loader walks the merged
settings' `enabledPlugins` entries and skips any whose value is undefined, so an id the file does not
hold is never loaded. Erasing an undeclared marketplace plugin's `false` therefore leaves it off, by a
different mechanism, though the file stops recording why. A plugin under `~/.claude/skills/` is found by
scanning that directory instead, and its entry only adjusts state afterwards; with no entry it falls back
to the plugin manifest's `defaultEnabled`, which defaults to true. Every skill this repo symlinks into
`~/.claude/skills/` is that second kind, so a `claude plugin disable` on one writes a `false` that the
next apply erases, and the skill comes back on. Nothing is drifting today, the live file holds exactly
the twelve ids, but the skills case needs one `claude plugin disable` to become real.

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

## When the live file is not JSON, the whole apply dies

Reading the live file is what makes preserving its state possible, and a modify-template that cannot read
it fails the entire run: every later target and every `run_after_` script is skipped, and the unreadable
file is left in place, so `permissions.deny` is not restored either. Whitespace-only and empty files are
handled (the read is trimmed first), and so is anything that parses. A file that is non-empty and is not
JSON, such as a write truncated by a crash or a full disk, is not, and no template can fix it: chezmoi's
JSON readers all fail the template on bad input and Go templates have no error recovery, so there is
nothing to fall back from.

**Recovery:** the apply's own error names `modify_settings.json`; repair `~/.claude/settings.json` if you
can, and reach for deletion knowing what it costs. **Deleting re-enables every disabled plugin and drops
every version pin.** Plugin state is the one managed thing the template reads out of the live file rather
than declaring, so with nothing to read all twelve ids render `true`. Everything else this repo manages
does come back from the template, and free-drift keys are lost as before. Nothing reports the plugin loss
afterwards, either: a rendered `false` is byte-identical to the live one, so `chezmoi status`,
`chezmoi diff` and `just d` say nothing about this key in any case. Note the disabled set before
deleting, by eye if the file no longer parses, and put it back with one `claude plugin disable <id>` per
plugin. Repairing this automatically needs something that runs before the template, which does not exist
yet.

**A corrupt live file cannot block the apply**, because the repair runs first.
`.chezmoiscripts/run_before_12-quarantine-unparseable-claude-settings.sh` moves an unreadable settings
file into `~/workspaces/backups/<timestamp>.claude-settings-quarantined.backup.json` (moved, never
deleted; the timestamp uses hyphens, not colons), warns loudly, and leaves `{}` for the template to
rebuild from. A readable file is left byte-identical, including the shapes that are not JSON objects
(empty, whitespace-only, a whole-file array), and an absent one stays absent. What the move costs is
per-plugin state, which lives only in the live file: every declared plugin comes back enabled, so a
disable has to be re-applied with `claude plugin disable <id>` after a quarantine.

`test/unit/claude-enabled-plugins.sh` applies the template into a throwaway destination once per
live-file shape per target OS and pins all of the above, including the three unparseable shapes, which it
requires to fail, to name the template in the error, and to leave the live file byte-identical.

## Promoting a `/config` toggle

Add a `setValueAtPath` call for that key in `private_dot_claude/modify_settings.json` and commit.

Background: `/config` writes ergonomic toggles directly into `~/.claude/settings.json` (verified
empirically), and Claude Code does not provide a user-level `~/.claude/settings.local.json` for
overrides, only project-scope `.claude/settings.local.json` exists. The modify-template approach is the
cleanest way to keep policy fields under chezmoi control while letting `/config` mutate everything else
freely. See https://www.chezmoi.io/user-guide/manage-different-types-of-file/ for the `modify_` template
and `setValueAtPath` reference.
