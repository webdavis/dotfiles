# Local agents: gnhf

[gnhf](https://github.com/kunchenguid/gnhf) is an overnight orchestrator: it calls a coding agent in a
loop, and each iteration makes one small, committed, documented change towards an objective. It is an npm
CLI on the fnm lane, pinned, and it is driven by hand today. Nothing schedules it.

## The three pieces

| Piece         | Source                                                                | Target               |
| ------------- | --------------------------------------------------------------------- | -------------------- |
| the CLI       | `.chezmoidata/system_packages_autoinstall.yaml`, `packages.macos.fnm` | fnm's default node   |
| the config    | `dot_gnhf/config.yml`                                                 | `~/.gnhf/config.yml` |
| telemetry off | `dot_bashrc.tmpl`                                                     | `GNHF_TELEMETRY=0`   |

The config is a plain file rather than a template because nothing gnhf reads is a secret: the agent it
drives is the already-signed-in `claude` CLI, and gnhf's own credentials do not exist. `GNHF_TELEMETRY`
is the only opt-out gnhf offers, so it lives in the shell rather than in that file.

Gotcha: gnhf writes `~/.gnhf/config.yml` itself, with upstream defaults, the first time it runs. A config
change that has not been applied therefore does not fail loudly, it just runs under the old file, or
under upstream's. `chezmoi diff` before a run is the check.

## Agent wiring

`agent: claude` in the config. gnhf builds the argv itself and runs the CLI non-interactively:

```text
claude [--model <model>] -p <prompt> --verbose --output-format stream-json \
  --json-schema <schema> --dangerously-skip-permissions
```

The permission flag is gnhf's default and is added only when no permission flag of your own is present;
it matches this machine's `permissions.defaultMode = bypassPermissions`, so the `settings.json` deny list
stays what guards the six sensitive paths. Pick a model with `--model <model>` for one run or
`agentModel.claude` in the config. gnhf rejects `-p`, `--print`, `--verbose`, `--output-format` and
`--json-schema` in `agentArgsOverride.claude` at config load, because those are the flags it manages.

## Worktree wiring

gnhf ships its own isolation: `--worktree` puts a checkout at
`<repo-parent>/dotfiles-gnhf-worktrees/<run-slug>/`. That is a third worktree layout, invisible to
herdr's sidebar and outside worktrunk's sweep, so runs here do not use it. Create the worktree the way
every other agent does, then run gnhf with `--current-branch` inside it, which commits on the branch that
already exists instead of cutting a `gnhf/` branch of its own:

```bash
herdr worktree create --cwd ~/workspaces/Ivy/webdavis/dotfiles \
  --branch gnhf/<slug> --no-focus
cd ~/.herdr/worktrees/dotfiles/gnhf-<slug>
gnhf --current-branch --max-iterations <n> --stop-when "<observable condition>" \
  < docs/gnhf-objective.md
```

`--current-branch` and `--worktree` together are an error in gnhf, which is the guardrail that keeps the
two layouts from both appearing. `--push` stays off: pushing and opening the pull request are the
operator's, after reading the branch. Resuming is the same command line again on a clean tree in the same
worktree, which continues the saved run history and the iteration numbering.

## First target and objective

The first target repository is this one, and the objective is `docs/gnhf-objective.md`, piped on stdin so
the whole file is the prompt. `docs/` is in `.chezmoiignore`, so the file is source only and never
reaches `$HOME`; it travels with every worktree, which is why the launch line above reads it as a
relative path. Edit it before a run. It carries the repository's standing constraints, so only the first
line, the objective itself, changes night to night.

## Facts that bite

- gnhf commits with `git -c commit.gpgsign=false -c tag.gpgsign=false commit -m <message>`, so hooks RUN.
  Here that means `just test-unit` plus gitleaks gate every iteration, which is what you want, and a red
  gate is a failed iteration that gets rolled back. Three consecutive failures abort the run. The
  user-wide `prepare-commit-msg` hook does not fire, because `-m` supplies the message.
- `commitMessage.preset: conventional` is set for that reason: with the hook out of the path, it is the
  only thing that makes an iteration commit read as `type(scope): summary` rather than gnhf's own
  `gnhf <iteration>: <summary>`.
- Run metadata lands in `.gnhf/runs/<runId>/` in the worktree (prompt, `notes.md`, the JSONL debug log,
  `end-state.json`). gnhf keeps it out of the commits by appending `.gnhf/runs/` to
  `git rev-parse --git-path info/exclude`, which in a linked worktree resolves to the MAIN checkout's
  `.git/info/exclude`. The entry shows up there once, for every worktree, and is not a stray edit.
- A usage-limit rejection is a wait, not a failure: gnhf sleeps until the window resets and retries the
  same iteration, under a 24h total leash you can narrow with `--max-rate-limit-wait`.
- No LaunchAgent, by decision: the Discord progress recap producer lands before the first unattended
  night, so until then a run is watched.
