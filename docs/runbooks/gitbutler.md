# GitButler

[GitButler](https://gitbutler.com/) is a Git client that keeps several branches applied at once. Its
`but` CLI is the part that matters here: it is what the AI-agent integration drives, and it is the reason
the desktop app is installed at all.

## What is installed

One cask, `gitbutler`, declared in `.chezmoidata/system_packages_autoinstall.yaml` under
`packages.macos.homebrew.casks`. It delivers both halves:

| Artifact    | Path                                                   |
| ----------- | ------------------------------------------------------ |
| Desktop app | `/Applications/GitButler.app`                          |
| `but` CLI   | `/opt/homebrew/bin/but`, a symlink into the app bundle |

The CLI is a symlink to `GitButler.app/Contents/MacOS/gitbutler-tauri`, so it is not a separate download
and there is nothing to install after the cask. Upstream's
[installation page](https://docs.gitbutler.com/cli-guides/installation) offers only the app's own
"Install CLI" button and a `curl | sh` script; the cask covers both and is what this repository declares,
so neither of those is used here.

Verified on 2026-09-14: `but --version` reports `0.22.3` from the cask alone.

## The skill: generated into the store on every apply

`but skill install` writes a real directory. Run non-interactively with `--global` it lands in
`~/.claude/skills/gitbutler`, which is exactly the undeclared stray that
`docs/runbooks/agent-skills-store.md` exists to prevent. So the skill is generated into the store
instead, by `~/.config/uu/scripts/generate-tool-skills.sh`:

```bash
but skill install --path ~/.agents/skills/gitbutler
```

The apply runs that script (`run_after_63`), and the uu `tool-skills` lane runs it again after the `brew`
lane, so a cask upgrade regenerates the skill without waiting for an apply. The script prints
`gitbutler: <old> to <new>` when the generated `version` changed and nothing otherwise.

The entry in `.chezmoidata/agent-skills.yaml` is `but: - skill: gitbutler` with `on_demand: true` and
`hermes: [nicodemus]`: Claude Code gets a link into the store and the skill marked user-invocable only,
Codex reads the store and gets the `agents/openai.yaml` on-demand overlay, and the nicodemus Hermes
profile gets a link. Nothing is committed: `but skill check --global` compares the generated copy with
`but --version`, and `but skill check --update` rewrites the deployed copy, which the next apply or lane
run regenerates anyway.

## What the agent wizard configured

`but agent setup` is an interactive four-step wizard. It refuses to run without a terminal and points at
`but agent setup --print`, which is the supported preview. Two of its steps were performed by hand
through the managed mechanisms above and below, and the third was deliberately skipped.

**Step: the skill, for Claude Code and Codex.** Generated into the store by the apply, not the wizard.
See above.

**Step: global workflow instructions.** The wizard writes its steering block into `~/.claude/CLAUDE.md`
and `~/.codex/AGENTS.md`. Both are chezmoi targets rendered from
`.chezmoitemplates/global-agent-rules.md`, so a write there survives until the next apply and no longer.
The steering therefore lives in that partial as a `## GitButler` section, condensed and with one
deliberate change: upstream's text tells the agent to use `but` unconditionally, and the section here
gates that on `but status` succeeding, because no repository on this machine is a GitButler project yet
and `but` write commands do not work in a plain git checkout. The `<!-- gitbutler-agent-setup:start -->`
and `:end` marker comments are left out on purpose, so the wizard never treats the rendered file as a
block it owns.

The policy knobs from [tuning agent behavior](https://docs.gitbutler.com/ai-agents/tuning-agent-behavior)
that the section keeps are the wizard's own defaults: amend local fixes into the commit they belong to,
and split unrelated changes within one file by hunk. The rest (checkpoint commits, stacked pull requests,
branch naming, "ship it" publish authorization, direct landing, draft pull requests, recovery snapshots)
are left off, since several of them conflict with this repository's own pull-request flow.

**Step: workspace mode (`but setup`).** Not run. See the next section.

The wizard writes no Claude Code hooks, so nothing here needs a `private_dot_claude/modify_settings.json`
hook declaration. Its only two outputs are skill files and steering text.

## The workspace-mode decision

`but setup` is what turns a git repository into a GitButler project, and it is the one step that was not
taken. Measured on 2026-09-14 by running it inside a throwaway clone of this repository under a throwaway
`HOME`, it makes four changes:

1. Switches `HEAD` from `main` to a new `gitbutler/workspace` branch.
1. Writes five local config keys: `gitbutler.project.targetref` (`refs/remotes/origin/main`),
   `gitbutler.project.targetcommitid`, `gitbutler.project.pushremote` (`origin`),
   `gitbutler.project.portedmeta`, and `log.excludedecoration` (`refs/gitbutler`).
1. Installs `.git/hooks/pre-commit` and `.git/hooks/post-checkout`, both marked
   `GITBUTLER_MANAGED_HOOK_V1`, preserving any existing hook as `<name>-user`.
1. Registers the repository in `~/Library/Application Support/com.gitbutler.app/projects.json`.

Three reasons it stayed undone:

- **The branch switch reaches every worktree.** A dozen agent lanes run out of
  `~/.herdr/worktrees/dotfiles/*`, and moving the main checkout off `main` while they work is disruptive
  in a way that is hard to undo mid-flight.
- **Its hooks are inert on this machine.** `dot_gitconfig.tmpl` sets `core.hooksPath` to
  `~/.config/git/hooks` user-wide, and git ignores `.git/hooks` entirely once that is set, so GitButler's
  workspace-branch guard would be installed and never run. A per-repository `core.hooksPath` override to
  fix that is the exact thing `docs/runbooks/git-hooks.md` forbids, because it would shadow the four
  user-wide hooks.
- **It is reversible but not free.** `but teardown`, or just `git checkout main`, returns the repository
  to normal git mode.

This is the operator's call, not an agent's, and the global rules section says so.

One side effect to know about: merely RUNNING `but` inside a repository writes a single local config key,
`gitbutler.project.portedMeta = true`, a one-time metadata-migration marker. It appeared in this
repository's `.git/config` from the read-only commands used to verify the install. Git ignores the
unknown section, nothing else was written (no target ref, no push remote, no workspace branch, no hooks,
no entry in the project registry), and `git config --local --unset gitbutler.project.portedMeta` removes
it. Since linked worktrees share `.git/config`, a `but` invocation from any agent worktree lands the key
in the shared config.

## How it stays current

The `gitbutler` cask is `auto_updates true`, which means **`brew upgrade` skips it** and the desktop app
updates itself in place. uu's weekly `brew` lane runs a plain `brew upgrade`, no `--greedy`, so it will
not bump the cask either; what it does guarantee is that `brew bundle` keeps the cask installed and that
the guarded `brew bundle cleanup` does not remove it, now that it is declared.

The practical consequence: `but --version` follows the app's own self-update immediately, because the CLI
is a symlink into the bundle, while Homebrew's recorded cask version drifts behind. So the version to
trust is `but --version`, and the skill-staleness signal is `but skill check --global`, not anything
Homebrew reports.
