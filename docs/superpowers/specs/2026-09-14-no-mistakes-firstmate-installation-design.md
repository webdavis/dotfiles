# no-mistakes and firstmate installation

Status: design, written 2026-09-14 with the operator asleep. Nothing is installed. Every choice made
in the operator's place is listed under Assumptions with its alternative, and the choices that are
genuinely theirs are listed under Open questions. Open Question 5 in
`docs/remaining-work.md` (the default harness and profile audience) gates adoption.

## Why this exists

The ledger entry asks for a written plan covering harnesses, project roots, validation commands,
review and merge authority, firstmate's Herdr backend and its treehouse dependency alongside the
existing worktree workflow, and each tool's update lane through uu. It also asks that existing hooks
and the per-invocation destructive-action approvals survive no-mistakes' Git proxy and repair
behavior. Its done-means is a plan naming every configuration choice and update lane, with nothing
installed until the audience is selected.

These two tools are unusual for this repository because neither is a library and neither is a
passive CLI. no-mistakes puts a background daemon and a local Git remote in front of every push.
firstmate spawns autonomous agents into the terminal multiplexer this machine already runs, gives
each one a worktree, and merges their work. Both therefore land on top of six systems this
repository already owns: the chezmoi-managed LaunchAgent set, the osquery file-integrity and
launchd-persistence monitors, the managed skills store, the user-wide Git hook dispatchers, the
worktrunk worktree layout, and the reviewed pull-request body contract. Most of the design below is
about those six intersections rather than about either tool in isolation.

### Constraints this plan is written against

From `CLAUDE.md` and the operator rulings in memory:

- The operator runs applies; agents propose. Never recommend an apply without first proving the
  scripts it touches render and run (ruling 2026-09-08).
- Every scheduled or supervised job on this machine is a chezmoi-tracked plist under
  `Library/LaunchAgents/` with a matching `run_onchange_after_*` loader.
- `~/.local/bin` holds only what the operator types; anything a hook, launchd, a keybinding or a
  `just` recipe invokes lives under `~/.local/libexec`.
- This repository builds no removal mechanisms (ruling 2026-08-02): no `.chezmoiremove`, no
  `remove_` entries, no retirement scripts.
- Destructive actions need per-invocation confirmation, `git push --force` included, and never to
  `main`.
- Tests cover the behavior of tools this repository wrote and nothing else (ruling 2026-08-05).
  Third-party install and configuration is therefore reviewed, not gated by a test.
- No worktree may live inside the chezmoi source tree, because chezmoi merges every `.chezmoidata`
  directory at any depth and a nested copy wins.
- Optimal over cheap (ruling 2026-09-05): do not pick a design because it is less work.

## What is on this machine today, measured

| Fact | Evidence |
| --- | --- |
| No checkout, binary or declaration for any of the three tools | `command -v no-mistakes firstmate treehouse` all miss; a repository-wide grep finds `no-mistakes`, `firstmate` and `treehouse` only in `docs/remaining-work.md` |
| `kunchenguid` is already a trusted upstream here | `dot_agents/custom-skill-lock.json` tracks `kunchenguid/chrome-devtools-axi`, `kunchenguid/gh-axi` and `kunchenguid/lavish-axi` |
| herdr 0.9.0, protocol 22, server running | `herdr status --json` reports client and server protocol 22 and `compatible: true` |
| tmux is absent | `command -v tmux` misses; the herdr migration removed it |
| `jq`, `python3`, `go` 1.x, `git` 2.55.0, `gh` authenticated as `webdavis` | `command -v`, `git --version`, `gh auth status` |
| node exists only on the fnm lane | `command -v node` resolves inside `~/.local/state/fnm_multishells/...`; there is no `/opt/homebrew/bin/node` |
| `acpx` 0.15.1 is installed globally on that lane | `npm ls -g --depth=0` |
| treehouse 2.3.0 is a bottled formula in homebrew-core | `brew info treehouse` reports `From: https://github.com/Homebrew/homebrew-core/...`, stable 2.3.0, MIT |
| `kunchenguid/homebrew-tap` exists but carries only four casks | tree listing shows `Casks/baby-menu.rb`, `hi-bit.rb`, `pi-launcher.rb`, `short-pipe.rb`; no formula for no-mistakes |
| Latest upstream tags | no-mistakes `v1.72.0`, treehouse `v2.3.0`, firstmate has no releases at all |
| `core.hooksPath` is set user-wide to `~/.config/git/hooks`, holding four hooks | `git config --global --get core.hooksPath`, directory listing |
| `~/.claude/settings.json` has `defaultMode: bypassPermissions`, a `deny` list of fourteen read patterns, and no `PreToolUse` hook | parsed live |

Three of firstmate's required global tools are missing: `tasks-axi`, `quota-axi` and
`lavish-axi` are not in `npm ls -g`, and `chrome-devtools-axi` and `gh-axi` exist here only as
skills that shell out through `npx`, not as installed binaries on `PATH`.

## What each tool actually is

Verified against upstream documentation fetched on 2026-09-14.

### no-mistakes

A Go binary that installs a local bare Git repository as a second remote named `no-mistakes`. You
push a committed branch to it; a per-user background daemon creates a disposable worktree, runs an
agent-driven pipeline over it (intent, rebase, review, test, document, lint, push, pull request,
continuous integration), applies mechanical fixes itself, parks anything needing judgement at an
approval gate, and forwards the branch to the real push target and opens a pull request only after
every gate is green. Three entry points: `git push no-mistakes <branch>`, the `no-mistakes` terminal
interface, and a `/no-mistakes` agent skill driving the non-interactive `no-mistakes axi` surface.

Facts that shape this design:

- The installer places the real binary at `~/.no-mistakes/bin/no-mistakes` and symlinks
  `~/.local/bin/no-mistakes` to it, because `~/.local/bin` is on `PATH`. Read from
  `docs/install.sh`.
- It then runs `no-mistakes daemon restart`, which prefers a managed service. On macOS that is
  `~/Library/LaunchAgents/com.kunchenguid.no-mistakes.daemon.<suffix>.plist`, where `<suffix>` is a
  short stable hash of `NM_HOME`. If the managed path is unavailable it falls back to a detached
  process.
- Global configuration is hand-maintained at `~/.no-mistakes/config.yaml`. `init` never rewrites it;
  when it computes a `worktree_roots` entry it prints the line for you to paste.
- Per-repository configuration is `.no-mistakes.yaml` at the repository root. `commands.*`, `agent`,
  `gates`, `protected_paths`, `pr.template`, `pr.base_branch`, `ci.*`, `test.instructions`,
  `document.instructions`, `review.path_instructions` and `disable_project_settings` are read only
  from the default-branch copy, at the commit a fresh fetch resolved, because those fields execute
  shell or choose which process runs with the maintainer's credentials.
  `allow_repo_commands: true` on the default branch re-enables per-branch `commands` and `agent`;
  `gates` never follow that opt-in.
- `init` writes the agent skill to `~/.claude/skills/no-mistakes/SKILL.md` and
  `~/.agents/skills/no-mistakes/SKILL.md`, at user level, for every repository. It writes no skill
  files into the repository.
- Official release binaries carry an embedded telemetry host (`https://a.kunchenguid.com`) and
  website identifier, so telemetry is on by default. `NO_MISTAKES_TELEMETRY=0` turns it off.
  `go install` produces a binary with no embedded website identifier, so telemetry stays off there
  unless a runtime value is set. The payload is documented as low-cardinality only: command names,
  statuses, durations, counts, flag booleans, agent names, step categories. Upstream states that
  run identifiers, repository paths, branch names, prompts, model output and diffs are never sent.
- A background update check runs on every invocation except `update` and the version queries.
  `NO_MISTAKES_NO_UPDATE_CHECK=1` suppresses it.
- `no-mistakes update` downloads the latest release, verifies its SHA-256 (secure hash algorithm,
  256-bit) checksum, replaces the binary atomically and resets the daemon. It refuses to restart the
  daemon while runs are pending or active and prints each one; `-y` does not bypass that guard,
  `--force` does. A process descended from an active pipeline agent cannot stop, restart or update
  the daemon at all, with no override.
- Continuous-integration repairs are force-pushed through a guarded path, but only when no-mistakes
  can prove the repair builds on the head already reviewed. When it cannot, and always for
  merge-conflict repairs, the repair goes back through Review first.
  `ci.revalidate_repairs: true` sends every repair through Review at the cost of a full extra pass.

### treehouse

A Go binary, in homebrew-core, that manages a per-repository pool of git worktrees under
`~/.treehouse/`. Worktrees are detached-HEAD and reset to the furthest-ahead default branch, so
branch names never collide. No daemon: every operation is an inline command writing a locked
on-disk pool file. `treehouse get --lease` reserves one as a durable home that no later `get` hands
out and no `prune` removes until `treehouse return`. `prune` is a dry run unless `--yes`, and
removes only idle managed worktrees already merged into the default branch with a clean tree.
`--root .` keeps the pool inside the project instead.

### firstmate

Not a binary and not installable. It is a cloned repository that acts as an agent distribution: an
`AGENTS.md` contract, bundled skills, and a `bin/` toolbelt of shell scripts. Launching a supported
harness with the clone as the working directory turns that session into the first mate. It spawns
crewmates into a session backend, gives each a treehouse worktree, supervises them with an
event-driven bash watcher, and returns finished pull requests, approved local merges, or standalone
scout reports.

Facts that shape this design:

- The universal toolchain is node, git, `gh` with `gh auth login`, no-mistakes 1.46.0 or newer,
  `gh-axi`, `chrome-devtools-axi`, `tasks-axi` and `quota-axi`. `lavish-axi` is presentation-only
  and degrades to plain text. The backend delta for herdr adds the `herdr` client, `jq` and
  `treehouse`. Optional protocol-16 ordering wants `python3`.
- Backend selection order is a per-task `--backend` flag, then `FM_BACKEND`, then the first
  non-empty line of the gitignored `config/backend` file, then runtime detection from `$TMUX`,
  `HERDR_ENV=1` or cmux markers, then the hard default `tmux`. Because tmux is absent here, leaving
  the default in place would fail every spawn, so the backend must be declared rather than inferred.
- The herdr backend requires protocol 14 or newer. This machine reports 22.
- `FM_HOME` selects the operational home. Unset, the clone root is the home, and `data/`, `state/`,
  `config/` and `projects/` live inside the clone (all gitignored).
- The herdr home label is `firstmate` for the primary home and `2ndmate-<id>` for a secondmate.
  Upstream warns against naming a personal workspace `firstmate`, because the adapter cannot tell
  that collision from its own container.
- Presentation spaces (one disposable workspace per task) are default-on at herdr 0.8.0 and above.
  `config/herdr-presentation-spaces` holding `off` opts out.
- `config/claude-permission-mode` selects the flag every Claude worker carries. Absent or `bypass`
  means `claude --dangerously-skip-permissions`; `auto` means `--permission-mode auto`. Any other
  value refuses every spawn.
- Delivery mode is per project and per task: `no-mistakes`, `direct-PR` or `local-only`, with an
  optional `+yolo` merge-autonomy flag. `data/projects.md` records each project's standing posture.
  Merges run through `bin/fm-pr-merge.sh`, which requires one live read proving the pull request is
  open, not a draft, mergeable, conflict-free and green at the current head, then binds
  `gh pr merge --match-head-commit`. `--auto`, `--admin` and branch deletion are refused without an
  explicit `--attended-override`.
- Self-update is `/updatefirstmate`: a fast-forward of the clone and of registered secondmate homes,
  then a persist-gated restart of every live mate. Dirty, diverged, offline and off-default targets
  are reported and left untouched. There are no releases, so the clone's default branch is the only
  version there is.
- The firstmate primary session under Claude Code relies on a tracked project-scope Stop hook for
  tokenless watcher re-arm.

## The seven places these tools touch systems this repository owns

This is the substance of the plan. Each row is a measured interaction, not a worry.

### 1. An unmanaged LaunchAgent appears, and the persistence monitor default-denies it

`no-mistakes daemon restart` writes
`~/Library/LaunchAgents/com.kunchenguid.no-mistakes.daemon.<suffix>.plist`. Two monitors see it.

`dot_local/libexec/posture/converge/desired/osquery.conf.tmpl` watches
`$HOME/Library/LaunchAgents/%%` in both `file_paths.launch_agents` and
`file_paths_hashes.launch_agents`. In `dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh`,
`_pipeline_is_tracked` matches `$HOME/Library/LaunchAgents/com.webdavis.osquery-*.plist` only, so a
`com.kunchenguid.*` plist is an untracked neighbor at the file-integrity arm and is logged rather
than paged. It then reaches the launchd persistence detector, which default-denies an unknown user
agent, so it pages until its tuple is allowlisted.

The only writer is `posture allowlist add <label>`, which edits the chezmoi source
`dot_config/osquery/private_page-launchd-allowlist.txt`, applies that one target and refreshes the
pipeline-integrity manifest, in that order. It captures a pinned plist SHA-256 and refuses to write
an unpinned tuple (`posture/crates/posture/src/allowlist/output.rs`:
`refused: sha256 hash capture failed for {path}; not writing an unpinned tuple`).

The consequence is an ongoing cost, not a one-time step: any no-mistakes update or daemon refresh
that rewrites the plist bytes invalidates the pin and pages once, and the fix is to re-run
`posture allowlist add com.kunchenguid.no-mistakes.daemon.<suffix>`. That is correct behavior for a
security boundary, and it is a real recurring interruption the operator should agree to before the
tool is installed.

This is also the clearest violation of the stated shape of this machine: a scheduled job that is not
a chezmoi-tracked plist with a loader. The honest reading is that the rule describes jobs this
repository authors, and a third-party tool's own service is a different category, in the same way
the osquery cask's root daemon is converged rather than authored. Recording that distinction in the
runbook is part of the work.

### 2. An unmanaged symlink appears in `~/.local/bin`, and that one is benign

The installer links `~/.local/bin/no-mistakes`. `osquery.conf.tmpl` watches
`$HOME/.local/bin/%%` under `managed_bin`, but `_managed_bin_is_tracked` takes its tracked set from
`/var/osquery/managed-bin-known-good.sha256`, which is derived from `chezmoi managed`. An unmanaged
symlink is therefore not in the manifest, is an untracked neighbor, and comes back silent. The
comment in `pipeline-verdict.sh` names exactly this case, listing herdr, mise, bob, hermes and
yt-dlp as the existing precedent. No action needed.

It is also consistent with the `~/.local/bin` rule: `no-mistakes` is a command the operator types.

### 3. `no-mistakes init` drops an undeclared directory into both skill stores

`init` writes `~/.claude/skills/no-mistakes/SKILL.md` and `~/.agents/skills/no-mistakes/SKILL.md`.
Both are managed surfaces here: `private_dot_claude/skills/` holds 73 `symlink_*` declarations, and
`~/.agents/skills` is the canonical store whose provenance lives in
`dot_agents/custom-skill-lock.json`.

What happens, read from uu's source rather than assumed:

- `uu/crates/uu-adapters/src/lanes/skills/fanout.rs` builds its `surviving` set from every directory
  or symlink in `~/.agents/skills`, with no roster filter, so `no-mistakes` joins the desired set
  for Claude delivery.
- `fanout/converge.rs::directory` then tries to create `~/.claude/skills/no-mistakes` as a symlink.
  The path already exists as a real directory, so neither the `is_symlink` branch nor the
  `!link.exists()` branch fires: it is left exactly as `init` wrote it, silently.
- `validate.rs` walks roster names only, so a non-roster store entry is neither validated nor
  pruned, and the generation exchange only rebuilds `~/.agents/.skills-current`, which a real
  directory in `~/.agents/skills` sits beside rather than inside.

So the outcome is benign and stable: two real copies, one per store, both refreshed by
`no-mistakes init` after an upgrade, neither touched by uu, neither declared anywhere. The costs are
that a fresh machine gets them only when `init` next runs, and that the lock's provenance tables
stop being a complete description of the store. This is the same defect the Backpass ledger entry
already names for accepted skill extractions, so it should get the same answer rather than a second
one. The recommendation is to leave the two directories undeclared and record them in
`docs/runbooks/agent-skills-store.md` as a third app-owned case beside `cua-driver` and `graphify`,
with the refresh mechanism being `no-mistakes init`. Vendoring a copy under `dot_agents/skills/`
would be worse: it would be overwritten by the next `init` and would then drift from the installed
binary's expected protocol.

### 4. The gate's bare repository inherits the user-wide `core.hooksPath`

`dot_gitconfig.tmpl` sets `core.hooksPath = ~/.config/git/hooks` globally. The gate is a bare
repository at `~/.no-mistakes/repos/<id>.git` whose `pre-receive` admission and `post-receive`
notification hooks are what make a push reach the daemon. A bare repository inherits the global
setting, and `~/.config/git/hooks` holds only `prepare-commit-msg`, `pre-commit`, `pre-push` and
`post-commit`, so without isolation neither receive hook would ever run and the gate would look
installed while doing nothing.

Upstream isolates the gate's hook path with `git config --worktree core.hooksPath` on a best-effort
basis, "when Git supports `config --worktree`", and its troubleshooting page names shared-config
`core.hookspath` writes as the exact hazard.

Measured here rather than assumed: with git 2.55.0, `git init --bare gate.git`,
`git config extensions.worktreeConfig true`, then `git config --worktree core.hooksPath <dir>`
succeeds and reads back the override, while a second bare repository with no override reports the
inherited `~/.config/git/hooks`. The mechanism works on this machine and the failure mode is real if
it does not run. It becomes the first acceptance check after `init`.

### 5. Every commit a crewmate makes fires a nested `claude -p`

`~/.config/git/hooks/prepare-commit-msg` prepopulates a Conventional Commits message through
`claude -p --model=sonnet`, bypassed with `SKIP_AI_COMMIT=1`. A firstmate fleet is several Claude
Code crewmates committing in parallel inside treehouse worktrees, and each commit would spawn
another Claude invocation for its message. That is quota and latency the operator never asked for,
and the message it writes is discarded anyway when no-mistakes rewrites fix commits with
`commit.fix_message`.

`config/launch-env-allowlist` in the firstmate home limits the ambient environment passed to
workers, so it is the right place to pin `SKIP_AI_COMMIT=1` for crewmates while leaving the
operator's own interactive commits on the hook.

The `pre-commit` and `pre-push` dispatchers act only when a repository tracks its own executable
`.githooks/<name>`, so a crewmate in a dotfiles worktree runs `just test-unit` plus gitleaks at
commit and `just lint-check` at push. That is wanted, not a conflict: it means a crewmate cannot
push past the gates the operator's own pushes face. It does mean `just lint-check` runs twice per
change, once at the crewmate's push and once as the gate's Lint step, and that `lint-check` writes
its fixes into the working tree before failing, which inside a disposable worktree is contained.

### 6. treehouse's pool and worktrunk's layout are disjoint, and one setting would break that

worktrunk puts branch worktrees at `~/.herdr/worktrees/<repo>/<branch>` so herdr's sidebar shows
them, and `wt up` rebases every one of them. treehouse puts anonymous detached-HEAD pool worktrees
under `~/.treehouse/`, with its own lock file and its own `prune --all` scoped to its own root. The
two roots, naming schemes and lifecycles do not overlap, so they coexist without either tool seeing
the other's worktrees. herdr never scans for worktrees anyway, so treehouse worktrees are invisible
in the sidebar, which for a machine-managed pool is the right outcome.

The one setting that would break this repository is treehouse's in-project storage, `--root .` or
`root = "."`. That would put worktrees inside the chezmoi source tree, and chezmoi merges every
`.chezmoidata` directory at any depth with the nested copy winning, which is the measured cause of
both the stale posture ceiling on 2026-09-14 and a 40-second template render. In-project storage
must be forbidden for the dotfiles repository, in the plan and in the runbook.

The same reasoning forbids putting the firstmate clone inside the dotfiles checkout: firstmate
clones each project into `projects/<name>`, so a firstmate home under the source tree would place a
full second copy of this repository, `.chezmoidata` included, inside the source tree.

### 7. Automatic pull requests collide with the reviewed body contract

`~/.claude/commands/pr.md` is the single source for the body contract: five `##` sections in a fixed
order, plus a mandatory anti-pattern review of the drafted body before any GitHub call. no-mistakes
opens the pull request itself and writes the body, and `pr.template` cannot close the gap: upstream
states plainly that its protected evidence appendix is appended in addition to the template and that
this "does not satisfy a policy requiring only the template's headings or bytes". Structural
requirements apply only to top-level `#` headings, so a five-`##` template would not even be
enforced, and `pr.publish_intent: false` removes only the generated Intent section.

`pr` and `ci` are both valid `--skip` names, and the gate advertises Git push options, so
`git push -o no-mistakes.skip=pr,ci no-mistakes <branch>` runs the full local validation, forwards
the branch, and leaves authorship to `/pr`. That is the resolution this plan recommends. Note the
related refusal: skipping Review leaves no approval binding and Push then fails closed unless Push
is skipped too, so `review` must never be in a skip list intended to keep Push working.

## Approaches considered

### Approach A: adopt both, dotfiles only, minimum blast radius

Install treehouse from homebrew-core and no-mistakes from its installer. Clone firstmate outside the
source tree. Gate exactly one repository, `webdavis/dotfiles`, through no-mistakes with
`pr` and `ci` skipped so the existing `/pr` flow keeps body authorship. Register exactly that one
project with firstmate in `no-mistakes` mode with `+yolo` off, so every merge waits on the captain.
Declare treehouse in the Homebrew data file and add one `command` lane for no-mistakes; leave
firstmate on `/updatefirstmate`.

Trade-offs. Smallest surface that still exercises everything: one daemon, one gate, one project
registry line, one new Homebrew formula, one new uu lane. The osquery allowlist entry and the skills
store drift are paid once. It does not answer whether the tools are useful across the operator's
other repositories, and it spends a fleet supervisor on a single project, which is close to the
worst case for firstmate's own value proposition.

### Approach B: adopt no-mistakes alone, defer firstmate

Install treehouse and no-mistakes, gate one or more repositories, and do not clone firstmate at all
until no-mistakes has run for a while.

Trade-offs. This is the decomposition the dependency already implies: firstmate requires no-mistakes
1.46.0 or newer as an essential toolchain entry, and its default project mode is `no-mistakes`, so
firstmate without no-mistakes is a degraded install while no-mistakes without firstmate is complete.
It defers the five most invasive items (four new global npm packages, the
`--dangerously-skip-permissions` worker posture, the `firstmate` workspace label, the Stop-hook
interaction, and autonomous merges) behind one decision instead of bundling them. Against it: the
ledger entry asks for a plan covering
both, and a second planning round costs another cycle. The plan can cover both while sequencing the
installs, which is what this document does.

### Approach C: adopt both across every active repository at once

Gate the dotfiles repository, the Obsidian vault, and the operator's other clones; register the same
set with firstmate.

Trade-offs. Rejected on evidence. The vault is auto-committed on a timer by Obsidian Git and its own
`CLAUDE.md` forbids manual staging, so a gate that expects you to push a committed branch fights the
timer directly. Each gated repository needs `.no-mistakes.yaml` committed to its own default branch
before its gate-control fields take effect, so this is N review cycles rather than one. And the
`worktree_roots` refusals mean placement has to be resolved per repository. The cost is real and the
benefit is unmeasured.

## Recommended design

Approach B's sequencing with Approach A's scope: plan both tools now, install no-mistakes and
treehouse first, and hold firstmate behind its own gate. Every item below is stated so it can be
checked before it is trusted.

### Phase 1: treehouse

Declaration, not a producer. `treehouse` is a bottled homebrew-core formula, so it follows the
documented Homebrew workflow exactly: `brew install treehouse`, then a line in
`.chezmoidata/system_packages_autoinstall.yaml` under `packages.macos.homebrew.formulae` in
alphabetical order. uu's existing `brew` lane upgrades it weekly with no new lane and no new script.

Configuration: none. The default root `~/.treehouse/` is correct and is what firstmate expects. Do
not write a `treehouse.toml` in this repository, and never pass `--root .` or set `root = "."` for
it, for the `.chezmoidata` reason in interaction 6.

Acceptance: `treehouse --version` reports 2.3.0 or newer; `brew bundle check` after the declaration
lands reports no missing formula.

### Phase 2: no-mistakes

**Install.** The curl installer, accepting `~/.no-mistakes/bin/no-mistakes` plus the
`~/.local/bin/no-mistakes` symlink. Set `NO_MISTAKES_TELEMETRY=0` in the managed shell before the
first run, not after (see the environment item below). The `go install` alternative is discussed
under Assumptions.

**Telemetry and self-update, declaratively.** Add to the interactive block of `dot_bashrc.tmpl`:

```
export NO_MISTAKES_TELEMETRY=0
export NO_MISTAKES_NO_UPDATE_CHECK=1
```

Both reach the daemon, which resolves its environment once at startup by running
`$SHELL -l -i -c 'env -0'` and preserves the shell's `PATH` order. `~/.bash_profile` sources
`~/.bashrc`, so an export there is visible to that probe. Both need a `no-mistakes daemon restart`
to take effect, and the second one matters because uu owns update lanes on this machine and a tool
that also checks for itself on every invocation reports the same news twice.

**One measured wrinkle in that probe.** Running the exact documented probe shape against this
machine's deployed `~/.bashrc` produces 104 environment records whose `PATH` is intact and carries
five fnm entries, but whose *first* record is corrupted: stdout opens with 28 bytes of terminal
mouse-mode reset escapes (`ESC[?1006l` through `ESC[?1000l`) before `SHELL=`, so the first
NUL-delimited record parses as a variable named
`\e[?1006l...\e[?1000lSHELL`. The escapes come from the unconditional `herdr` call at the bottom of
`dot_bashrc.tmpl`, which runs in that probe because the shell is interactive, `HERDR_ENV` is unset,
`SSH_ORIGINAL_COMMAND` is unset and `herdr` is on `PATH`.

Today this is benign: only `SHELL` is damaged and `PATH` survives with fnm on it. It is worth fixing
anyway, because the documented fallback when login-shell resolution fails is a process environment
that "may omit version-manager directories such as nvm, fnm, or volta", and node exists on this
machine only on the fnm lane. The fix is one guard line before that `herdr` call:

```
[[ -t 1 ]] || return   # a non-terminal interactive shell must never attach
```

That is the root-cause fix rather than a no-mistakes-specific one: it covers every tool that probes
the login shell this way, and it also stops the daemon from invoking a herdr client at each startup,
which on a machine where the herdr server is not already running would make the server a child of a
launchd-managed daemon. Treat it as a separate one-line change with its own commit, not as part of
the tool install, and verify it by re-running the probe and confirming stdout starts at `SHELL=`.

**Global configuration** at `~/.no-mistakes/config.yaml`, hand-maintained by the operator, not
tracked by chezmoi (see Assumptions for why):

```yaml
agent: [claude, codex]
agent_config:
  codex:
    model: gpt-5.4
    effort: low
log_level: info
session_reuse: true
ci:
  revalidate_repairs: true
```

`agent: auto` would resolve to `claude` first anyway, but the explicit ordered list documents the
intent and gives a fallback to `codex`, both of which are installed and authenticated here. Every
other field stays at its default so the file says only what differs, matching this repository's own
config-hygiene ruling.

`ci.revalidate_repairs: true` is the answer to the destructive-approval constraint. It is the only
setting that makes every continuous-integration repair pass through Review before it is published,
which is the closest available equivalent to a per-invocation approval for an automated force-push.
The cost is a full extra pipeline pass per repair, and it is worth naming as a cost rather than
hiding it.

**Per-repository configuration**, `.no-mistakes.yaml`, committed to `main` in the dotfiles
repository, because gate-control fields are read from the default branch:

```yaml
commands:
  lint: "just lint-check"
  test: "just test-unit"
  format: "just S"
protected_paths:
  - ".chezmoidata/**"
  - ".github/**"
  - "dot_config/osquery/private_page-launchd-allowlist.txt"
  - "dot_config/pns/private_config.toml.tmpl"
  - "dot_agents/custom-skill-lock.json"
  - "Library/LaunchAgents/**"
  - "**/Cargo.lock"
gates:
  - name: actions-security
    after: lint
    command: "just lint-actions-security"
  - name: rust
    after: test
    command: "just test-rust"
```

Reasoning per entry:

- `commands.lint` is `just lint-check`, the same drift gate the pre-push hook and continuous
  integration run. It writes fixes before failing, which is safe in a disposable worktree and turns
  formatting drift into an auto-fix commit rather than a finding.
- `commands.test` is `just test-unit`, not `just test`. Upstream calls `commands.test` targeted
  local validation rather than continuous-integration parity, and `test-unit` is this repository's
  own commit gate. The heavier suites are declared as gates instead, so a pass through the pipeline
  means strictly more than a pass through `test-unit` and no core step is weakened.
- `commands.format` is `just S` (shfmt alone) rather than `just l`, because `just l` runs all twelve
  formatters and overlaps `lint-check` entirely.
- Two gates carry the remaining continuous-integration gates. `just test-rust` is separate because
  `cargo test` alone misses `cargo fmt --all --check`, which is what continuous integration fails
  on. Gates park for a decision rather than auto-fixing, which is the right shape for both.
- `protected_paths` names the files an automatic commit must never stage. Four of them are load
  bearing: `.chezmoidata/**` feeds every render, the pns config template is a generated file held to
  byte equality by `just test-rust`, the launchd allowlist is a declared security boundary, and the
  skill lock is the store's provenance. `**/Cargo.lock` is there because a fixer resolving a build
  failure by moving a dependency is a decision, not a mechanical fix.
- `allow_repo_commands` stays absent, so it stays `false`. Leaving it off means a pushed branch
  cannot change what the gate runs, which matters here because Dependabot opens branches in this
  repository weekly. The single-developer argument for turning it on does not survive that.
- No `pr.template`. Body authorship stays with `/pr`, per interaction 7.

**How a change reaches the gate.** Standard invocation for this repository:

```
git push -o no-mistakes.skip=pr,ci no-mistakes <branch>
```

then the existing `/pr` flow once the branch is on `origin`. `review` is never skipped, because
skipping it leaves no approval binding and Push fails closed.

**Update lane.** One `command` lane in `dot_config/uu/private_config.toml.tmpl`, following the
existing `composio`, `codegraph` and `herdr-binary` rows:

```
[lanes.no-mistakes]
type = "command"
run = ["{{ joinPath .chezmoi.homeDir ".no-mistakes/bin/no-mistakes" }}", "update", "-y"]
deadline_secs = 21600
```

The real binary path is used rather than the `~/.local/bin` symlink so the lane does not depend on a
link the installer manages. `-y` answers the daemon-executable-mismatch prompt without bypassing the
active-run guard, so a week with a parked run fails the lane loudly instead of killing the run,
which is the behavior wanted from an unattended updater. `--force` must never appear here.

The lane's known follow-on is interaction 1: an update that rewrites the daemon plist invalidates
the pinned allowlist hash and pages once. Record that in the runbook next to the lane so the page is
recognized rather than investigated.

**Acceptance checks for phase 2**, in order, each one a command with an expected answer:

1. `no-mistakes --version` reports 1.72.0 or newer.
1. `no-mistakes doctor` reports a runnable pipeline agent and the provider tools it found.
1. `no-mistakes daemon status` reports a ready daemon, and
   `launchctl print gui/$(id -u)/com.kunchenguid.no-mistakes.daemon.<suffix>` resolves.
1. `git -C ~/.no-mistakes/repos/<id>.git config --get core.hooksPath` does **not** answer
   `~/.config/git/hooks`, and
   `ls -la ~/.no-mistakes/repos/<id>.git/hooks/pre-receive ...post-receive` shows both executable.
   This is interaction 4 and it is the check most likely to fail.
1. `posture allowlist list` includes the daemon label, and one osquery tick later the alerter has
   not paged for it.
1. A throwaway branch with one trivial commit, pushed with `-o no-mistakes.skip=pr,ci`, reaches
   `origin` with the pipeline green, and `no-mistakes runs` records it.
1. `~/.agents/skills/no-mistakes/SKILL.md` and `~/.claude/skills/no-mistakes/SKILL.md` both exist,
   and a following `uu run skills` leaves both in place and reports no new warning.
1. `uu run no-mistakes` records an `ok` line on a week with nothing to upgrade.

### Phase 3: firstmate, behind its own gate

Do not start this until phase 2 has run for at least one real change and Open Question 5 is
answered.

**Prerequisites to declare first.** Four npm packages join
`.chezmoidata/system_packages_autoinstall.yaml` under the existing `fnm` node block, alphabetically:
`chrome-devtools-axi`, `gh-axi`, `quota-axi`, `tasks-axi`. `lavish-axi` is optional and
presentation-only. This has a side effect worth stating: `gh-axi` and `chrome-devtools-axi` are used
here today through `npx -y`, and installing them globally means the skills and firstmate resolve
different copies unless the skills are left alone. Leave the skills as they are; firstmate needs the
binaries on `PATH` and the skills keep working through `npx`. The existing weekly npm lane upgrades
all four, so no new lane is needed for them.

**Clone location.** `~/workspaces/firstmate`, a plain clone of `main`, outside every existing
repository and outside the chezmoi source tree. `FM_HOME` stays unset, so the clone root is the
home and `data/`, `state/`, `config/` and `projects/` live inside it, all gitignored upstream. This
is deliberately not under `~/workspaces/Ivy/webdavis/`, which holds the operator's own repositories,
and deliberately not inside the dotfiles checkout, per interaction 6.

**Local configuration**, all gitignored inside the home, all written once by hand:

| File | Value | Why |
| --- | --- | --- |
| `config/backend` | `herdr` | tmux is absent, so the hard default would fail every spawn. Declaring it also avoids relying on `HERDR_ENV` auto-detection and its opt-out notice |
| `config/claude-permission-mode` | `bypass` or `auto`, operator's call | `bypass` matches this machine's existing `defaultMode: bypassPermissions`; `auto` is stricter. See Open questions |
| `config/herdr-presentation-spaces` | `off` | herdr 0.9.0 would otherwise project each task into its own disposable workspace, on a machine whose nine workspace chords and sidebar are hand-tuned |
| `config/launch-env-allowlist` | include `SKIP_AI_COMMIT=1` | interaction 5 |
| `config/watched-tools.json` | absent initially | uu already owns update reporting; a second reporter is duplicate signal |
| `data/projects.md` | one line for `dotfiles`, mode `no-mistakes`, no `+yolo` | review and merge authority stays with the operator |

**The workspace label.** firstmate claims the herdr workspace label `firstmate` for its primary
home and refuses rather than guessing when two workspaces share it. None of the eight project
workspaces in `dot_config/herdr/config.toml` uses that name today, and none should be added.

**Review and merge authority.** `+yolo` off for every project. That means firstmate brings a pull
request and waits, which is what the review-pipeline rulings in memory require, and it keeps
`bin/fm-pr-merge.sh` in its attended posture where `--auto`, `--admin` and branch deletion are
refused without an explicit override. The merge itself stays with the operator's `/pr-merge` flow
and its `--merge` convention.

**Update lane.** None. firstmate has no releases, and `/updatefirstmate` does more than a
fast-forward: it persists and restarts every live mate, which an unattended weekly job must not do
while a fleet is working. A `git -C ~/workspaces/firstmate fetch` lane that only reports would be
possible later if the operator wants a nudge, but it is not needed for correctness and is not
recommended now.

**The Stop-hook interaction to verify, not assume.** A firstmate primary session under Claude Code
relies on a tracked project-scope Stop hook for watcher re-arm, and `~/.claude/settings.json`
already declares a user-scope Stop hook that routes through pns. Both should fire, but that means
every firstmate turn end raises a pns event as well. Verify both hooks run, and decide whether the
firstmate session should sit behind `pns quiet`. Do not change `modify_settings.json` for this
without measuring first.

**Acceptance checks for phase 3.** `bin/fm-bootstrap.sh` reports no `MISSING:` line; a single scout
task spawns a herdr tab in the `firstmate` workspace, leaves a report under `data/<id>/report.md`,
and tears down cleanly; a single ship task on a throwaway branch produces a pull request and stops;
`treehouse` shows the pool worktree returned after teardown; and `wt up` still rebases only the
worktrunk worktrees.

## Failure modes and what each one looks like

| Failure | Symptom | First check |
| --- | --- | --- |
| Gate hook path not isolated | `git push no-mistakes` succeeds and nothing happens; no run appears | `git -C <gate> config --get core.hooksPath` |
| Daemon not running | push rejected before any gate ref changes (pre-receive fails closed) | `no-mistakes daemon status`, then `<gate>/notify-push.log` |
| Daemon environment resolution fell back | pipeline agent cannot find `node` or a fnm-installed tool | the daemon's own log; re-run the login-shell probe by hand |
| Unallowlisted daemon plist | a critical page naming an unknown user LaunchAgent, every tick | `posture allowlist list` |
| Allowlist pin stale after an update | the same page returning right after a no-mistakes upgrade | compare the plist hash to the allowlist tuple |
| `worktree_roots` misconfigured | daemon refuses to start | `no-mistakes doctor`; `init` refuses the bad directories up front |
| Update lane blocked by a parked run | uu reports the lane failed, naming the active runs | `no-mistakes runs`; resolve the gate, do not use `--force` |
| firstmate spawn refused | a terminal blocker naming the missing tool or version floor | `bin/fm-bootstrap.sh`, `herdr status --json` |
| Two workspaces labelled `firstmate` | spawn refuses rather than choosing | `herdr` workspace list |
| treehouse pool inside the source tree | slow renders, stale `.chezmoidata` values | `treehouse` root setting; never `.` |

## Security notes

- The gate-control trust model is the reason `allow_repo_commands` stays off. `commands.*` and
  `gates[].command` run arbitrary shell on this machine with the operator's credentials, and
  Dependabot pushes branches here weekly.
- `protected_paths` is a staging guard, not a sandbox. It stops an automatic commit from including a
  listed path; it does not stop an agent from editing one. Upstream says so directly. The listed
  paths are chosen because a silent automatic commit to them is the damaging case.
- The one automated force-push in the system is the continuous-integration repair path, guarded by a
  proof that the repair builds on the reviewed head. `ci.revalidate_repairs: true` removes the
  unreviewed variant entirely. Nothing in either tool force-pushes to `main`.
- Telemetry is on by default in release builds and off with `NO_MISTAKES_TELEMETRY=0`. The
  documented payload is low-cardinality counters and enums; upstream states repository paths, branch
  names, prompts and diffs are never sent. That claim is not independently verified here, which is
  the argument for turning it off rather than auditing it.
- `--dangerously-skip-permissions` for crewmates is the upstream default and matches this machine's
  existing global `bypassPermissions` mode, so it changes no posture. What it does change is the
  number of unattended agents running under it at once. `permissions.deny` still covers the fourteen
  sensitive read patterns, and hooks still fire regardless of permission mode.
- The firstmate Relay feature (answering public mentions on X and Discord) is opt-in through a local
  `.env` pairing token. Do not enable it. It is out of scope below.

## Out of scope

- Gating any repository other than `webdavis/dotfiles`. The Obsidian vault is excluded on evidence:
  Obsidian Git auto-commits it on a timer and its own `CLAUDE.md` forbids manual staging.
- firstmate secondmates, local or remote over SSH.
- firstmate Relay, the spoken interface, and the mail plane.
- Any backend other than herdr: tmux, zellij, cmux and Orca.
- A `run_onchange` producer or a chezmoi-tracked plist for the no-mistakes daemon. This repository
  does not author that service, and it builds no removal mechanisms.
- Declaring the `no-mistakes` skill in `dot_agents/custom-skill-lock.json` or
  `private_dot_claude/skills/`. Interaction 3 explains why the app-owned pattern is the answer.
- Pinning treehouse or no-mistakes to a version. Neither is a byte rewriter of tracked files, so the
  mdformat-style pin argument does not apply.
- Any change to `modify_settings.json`, `~/.claude/CLAUDE.md`, or the shared agent-rules partial.
- Writing code. This entry is a plan.

## Assumptions made in the operator's place

Each one is a choice this document made so it could be concrete. Each names its alternative.

1. **Install no-mistakes from the curl installer, not `go install`.** Chosen because the installer is
   the supported path, sets up the managed service, and is what `no-mistakes update` expects.
   *Alternative:* `go install github.com/kunchenguid/no-mistakes/cmd/no-mistakes@latest`, which
   produces a binary with no embedded telemetry identifier so telemetry is off without an
   environment variable, and lands in `~/go/bin` instead of `~/.no-mistakes/bin`. Against it: the
   update path then replaces a locally built binary with an official release build, reintroducing
   the embedded identifier, and the version is whatever the module proxy serves rather than the
   release the installer resolves.
1. **Leave `~/.no-mistakes/config.yaml` untracked and hand-maintained.** Chosen because upstream
   states the global config is hand-maintained and `init` deliberately never rewrites it, and
   because a chezmoi-managed copy would fight `init`'s printed `worktree_roots` guidance and would
   abort an apply if the file were ever unparseable. *Alternative:* track it as
   `dot_no-mistakes/config.yaml.tmpl`, gaining a fresh-machine story and drift detection, at the
   cost of a template that must stay ahead of upstream's schema.
1. **`agent: [claude, codex]` rather than `auto`.** Chosen so the choice is readable and has a
   fallback. *Alternative:* `auto`, which resolves to the same first entry today and needs no edit
   when a new harness is installed.
1. **`ci.revalidate_repairs: true`.** Chosen because it is the only available approximation of the
   per-invocation approval the destructive-action rule requires for a force-push. *Alternative:*
   leave it `false` and accept the proof-bound repair path, which upstream argues is safe and which
   avoids a full extra pipeline pass per repair.
1. **`commands.test` is `just test-unit`, with the heavy suites as gates.** Chosen because upstream
   frames `commands.test` as targeted validation and because gates park for a decision instead of
   auto-fixing, which is right for a Rust format check. *Alternative:* `commands.test: just test`
   and no gates, one setting instead of three, at the cost of a fixer trying to auto-fix a
   `cargo fmt` failure and a much longer inner loop.
1. **Skip `pr` and `ci`, keep `/pr`.** Chosen because the reviewed body contract cannot be expressed
   through `pr.template`. *Alternative:* let no-mistakes open pull requests with `pr.template`
   pointing at a committed five-section template and `pr.publish_intent: false`, accepting the
   appended evidence and losing the anti-pattern review. That is a change to the body contract and
   therefore an operator decision, not an agent's.
1. **`config/herdr-presentation-spaces` set to `off`.** Chosen because this machine's workspace set
   and nine quick-jump chords are hand-tuned and a disposable workspace per task would churn them.
   *Alternative:* leave it unset and take the default-on projection, which is the shape upstream has
   verified most recently.
1. **Clone firstmate to `~/workspaces/firstmate` with `FM_HOME` unset.** Chosen because it is
   outside every repository and every source tree, and because an unset `FM_HOME` is the simplest
   correct layout. *Alternative:* `~/.firstmate` as a hidden tool-owned home, which reads as
   tool-managed state rather than a repository you pull, at the cost of being invisible in
   `~/workspaces`.
1. **Install the four missing axi-family npm packages globally rather than declining firstmate.**
   Chosen because they are hard toolchain requirements. *Alternative:* do not adopt firstmate, which
   is exactly what Open Question 5 may decide, in which case none of them is needed.
1. **Add the `[[ -t 1 ]] || return` guard to `dot_bashrc.tmpl` as a separate change.** Chosen
   because the corruption is measured, the fix is one line, and it benefits every login-shell probe
   rather than one tool. *Alternative:* leave bashrc alone, since `PATH` currently survives the
   probe intact, and accept the corrupted first record plus a herdr client invocation at each daemon
   start.
1. **No uu lane for firstmate.** Chosen because `/updatefirstmate` restarts live mates and an
   unattended weekly job must not. *Alternative:* a fetch-and-report `command` lane that never
   merges, giving a weekly nudge with no restart risk.

## Open questions

1. **Open Question 5, the one already on record.** Which harnesses and Hermes profiles are the
   audience for no-mistakes and firstmate? The `/no-mistakes` skill reaches Claude Code and every
   harness reading `~/.agents/skills` automatically, with no per-harness choice available, so the
   real question is whether firstmate is adopted at all and which harness runs the primary session.
   Claude Code, Grok and Pi are upstream co-primaries; only Claude Code and Codex are installed
   here.
1. **Is firstmate adopted, or is phase 3 declined?** Adopting it costs four new global npm packages,
   a supervised fleet of agents running under `--dangerously-skip-permissions`, a reserved herdr
   workspace label, and a second Stop hook on the primary session. Declining it costs nothing built
   so far, because phases 1 and 2 stand alone.
1. **`bypass` or `auto` for `config/claude-permission-mode`?** `bypass` matches this machine's
   current global mode. `auto` is the stricter posture upstream added for a captain who refuses
   unattended bypass. This is a safety preference, not a technical one.
1. **Is the recurring allowlist page acceptable?** Every no-mistakes update that rewrites the daemon
   plist invalidates the pinned hash and pages once until
   `posture allowlist add com.kunchenguid.no-mistakes.daemon.<suffix>` is re-run. The alternative is
   to not install a tool that manages its own LaunchAgent.
1. **Which repositories, beyond dotfiles, should the gate cover eventually?** Each one needs its own
   `.no-mistakes.yaml` on its own default branch, its own `worktree_roots` placement decision, and
   its own validation commands. This plan gates one.
1. **Does the reviewed pull-request body contract stay with `/pr`?** If the operator would rather
   let no-mistakes write bodies, `~/.claude/commands/pr.md` needs amending and assumption 6 flips.
1. **Should the no-mistakes skill be recorded in the skills-store runbook as a third app-owned
   case?** It needs no lock table row to work, but leaving it entirely unrecorded means the store's
   provenance stops describing the store. This is the same decision the Backpass entry already
   carries and the two should be answered together.

## Verification plan

No code is written by this entry, so there is nothing to test first. What is verifiable is each
install step, and every acceptance check above is stated as a command with an expected answer. The
order matters: phase 1 stands alone; phase 2's check 4 (gate hook-path isolation) is the one most
likely to fail and must pass before any real branch is pushed through the gate; phase 3 must not
begin until phase 2 has carried one real change end to end.

Before any apply that touches this repository's own scripts, render and run them the way the
2026-09-08 ruling requires, and report exit codes. The only source changes this plan proposes are a
formula line, four npm package lines, one uu lane block, and the one-line bashrc guard, so that
sweep is small: render `dot_bashrc.tmpl` and `dot_config/uu/private_config.toml.tmpl`, shellcheck
the rendered bashrc, and confirm `just lint-check` and `just test` stay green.
