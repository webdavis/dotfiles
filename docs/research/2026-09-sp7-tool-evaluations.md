# SP7 tool evaluations: strix, apple/container, minutes and gnhf, 2026-09-14

This answers the ledger line at `docs/remaining-work.md:2003`, under "SP7 scope recovered from the
roadmap and Todoist": *"Reconcile the remaining tool evaluations for strix, apple/container, minutes and
gnhf. Check prior removals and rejections before proposing adoption. Herdr remains the selected
multiplexer. The rejected git-absorb and deferred gh-dash/companion tools are not additions to #530."*

Nothing was installed, configured or applied for this record, and no repository file was changed. The one
network artifact fetched was the strix install script, downloaded into the session scratchpad and read,
never executed.

## Verdicts

| Tool              | Verdict                                                                | The prior decision it respects                           |
| ----------------- | ---------------------------------------------------------------------- | -------------------------------------------------------- |
| `minutes`         | **Adopt**, already adopted; the evaluation item is superseded          | Declared 2026-07-27 in `ad7124f3`                        |
| `gnhf`            | **Adopt**, already decided; the work stays in its own install task     | 2026-08-30 dossier "gnhf fnm lane"; ledger line 2161     |
| `apple/container` | **Decline** for now, revisit only as a Docker Desktop replacement      | Docker Desktop declared and running; `act` declared      |
| `strix`           | **Defer** on three operator inputs, with the adoption path pre-cleared | 2026-08-30 dossier, one half of which is corrected below |

## The question

Four tools were parked in an evaluation lane on 2026-08-30 and never resolved. For each one: is it
adopted or declined, does a prior removal or rejection stand in the way, and what does the decision cost
in managed declarations. The coupling recorded in the old note ("strix via uv+container as one decision")
is itself part of the question, because if it holds then strix and `apple/container` are one verdict
rather than two.

## What was checked and how

**The prior-decision trail.** Todoist `6hPV483GJgGHX95M` ("SP7: configure Backpass, no-mistakes and
firstmate; reconcile remaining tool evaluations") carries the sentence "Remaining strix, apple/container,
minutes and gnhf evaluations retain their old scope", so the old scope had to be recovered from
elsewhere. It survives in this project's agent memory index, in the "SIDE STREAM (2026-08-30, operator):
the SP7+ tool list" block of
`~/.claude/projects/-Users-stephen-workspaces-Ivy-webdavis-dotfiles/memory/pns-part2-scope.md`, which
records the dossier's verdicts: the eval lane was "recorded with install commands (apple/container brew
1.3.0, strix via uv+container as one decision, gnhf fnm lane)", and `minutes` was already marked "ALREADY
INSTALLED at filing (~/.local/bin)". The dossier file that block cites
(`scratchpad/sp7-tools-dossier.md`, 2026-08-30) is gone: session scratchpads do not persist, and a `find`
over `/private/tmp/claude-501` and `$HOME` returns nothing. **The memory block is therefore the surviving
record of those verdicts, not a second source that confirms them.**

`git log --oneline --all -S"strix"` and the same search for `apple/container` return exactly one commit,
`5893b4e2` ("docs: publish the approved modernization plan"), which is the commit that wrote the ledger
line being answered here. So neither tool has ever been declared, removed or rejected in tracked source.

**Repository sources read.** `.chezmoidata/system_packages_autoinstall.yaml` (taps, trusted taps,
formulae, casks, the `uv` list and the `fnm` node group),
`.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` (the install semantics of each of those
lanes), `dot_config/uu/private_config.toml.tmpl` (the weekly lanes),
`uu/crates/uu-adapters/src/lanes/npm.rs` and `.../config/lanes/npm.rs` (what the npm lane actually runs),
`dot_bashrc.tmpl` (PATH order), `docs/research/2026-04-12-act-runner-isolation.md` (what needs Docker
here), and `docs/superpowers/plans/2026-04-19-dotfiles-improvements-v2.md:3963` (the gh-dash deferral).

**Machine state measured.** `sw_vers` reports macOS 26.2, build 25C56. `command -v` finds `minutes` and
`gnhf` and finds neither `strix` nor `container`. `brew info container` and `brew info socktainer`
describe available-but-not-installed formulae; `brew info --cask minutes` reports the installed cask and
its upgrade. `docker --version` and `docker info` report 29.7.2 with a live daemon, and `pgrep` finds
Docker Desktop running. `npm ls -g --depth=0` on the fnm default node lists the global packages.
`minutes --help`, `minutes paths`, `gnhf --version` and `gnhf --help` were run.

**Upstream sources.** The strix and gnhf README files at `raw.githubusercontent.com`, the Apple
`container` README, the GitHub application programming interface (API) repository records for both
`usestrix/strix` and `kunchenguid/gnhf`, the Python Package Index (PyPI) JSON record for `strix-agent`,
the npm registry record for `gnhf`, and the strix install script from `https://strix.ai/install`
(downloaded, read, not run). One claim below is labelled as coming from the Homebrew formula caveat text
rather than from upstream.

### Versions measured, 2026-09-14

| Thing               | State                                                                                                       |
| ------------------- | ----------------------------------------------------------------------------------------------------------- |
| macOS               | 26.2 (25C56)                                                                                                |
| `minutes`           | cask 0.26.1 installed 2026-09-08, 0.26.2 available; command-line interface reports `minutes 0.26.1`         |
| `gnhf`              | 0.1.49 installed globally on the fnm node 24.21.0; npm registry latest 0.1.49                               |
| `container` (Apple) | not installed; Homebrew core formula 1.4.1, bottled, Apache-2.0                                             |
| `socktainer`        | not installed; Homebrew core formula 1.2.1, Apache-2.0, requires `container`                                |
| `strix`             | not installed; PyPI `strix-agent` 1.6.2 uploaded 2026-09-05, GitHub release v1.6.2 the same day, Apache-2.0 |
| Docker Desktop      | 29.7.2, daemon live, cask declared                                                                          |
| `ollama`            | 0.33.3 declared and installed, one local model present (`qwen3.5:4b`)                                       |

## Findings

### minutes: adopted on 2026-07-27, with one fresh-machine gap

`minutes` is not an open evaluation. It was declared on 2026-07-27 in commit `ad7124f3`, "chore(brew):
declare fluidvoice, minutes, ollama and silverstein/tap", and today it sits in three places in tracked
source: the cask list (`.chezmoidata/system_packages_autoinstall.yaml:212`), the `taps` list and the
`trusted_taps` list (`silverstein/tap`, lines 16 and 41). The cask installed `/Applications/Minutes.app`
on 2026-09-08 at 0.26.1 and Homebrew reports 0.26.2 available, which uu's weekly `brew` lane is already
the producer for. No action.

It is also in real use, and its integration reaches outside this repository. The Obsidian vault's root
`CLAUDE.md` documents `agent-processing-pipeline/minutes` as "a symlink to the `minutes` tool's output
directory, managed by `minutes vault setup --subdir`", and that symlink exists:
`/Users/stephen/workspaces/Ivy/agent-processing-pipeline/minutes -> /Users/stephen/meetings`, created
2026-07-22. `~/meetings` holds one meeting note (2026-07-28) plus `archive/`, `dictations/` and `memos/`,
so usage is real but light.

Two management facts matter, and only one of them is a gap.

1. **There is no configuration to manage.** `minutes paths` reports
   `config_path: /Users/stephen/.config/minutes/config.toml`, and that file does not exist. The tool runs
   on its defaults, `~/.minutes` is empty, and the output directory is the default `~/meetings`.
   Declaring a config template today would be declaring the defaults.

1. **The command-line interface is not reproducible on a fresh machine.** `~/.local/bin/minutes` is a
   symlink to `/Applications/Minutes.app/Contents/MacOS/minutes`, created 2026-07-19, before the cask was
   adopted. Nothing in chezmoi declares it; the application's own "install CLI" action made it. The
   Homebrew cask caveat says so explicitly: "For the CLI (record, stop, search from terminal): brew
   install silverstein/tap/minutes". That formula is not installed and not declared, and
   `/opt/homebrew/bin/minutes` does not exist. So a fresh machine applies the cask, gets the application,
   and has no `minutes` on PATH until somebody clicks through the application again.

PATH order decides how that gap gets closed. `dot_bashrc.tmpl:175` runs
`path_prepend "$HOME/.local/bin"`, under the comment "Order matters! These local (user-level) tools
should always take precedence", so the hand-made symlink shadows anything Homebrew installs. Declaring
the formula while leaving the symlink in place would install a second copy of the command-line interface
that never answers `minutes`, and would leave its version drifting invisibly against the application
bundle's copy.

### gnhf: already installed, undeclared, and two premises in its own ledger task are wrong

`gnhf` 0.1.49 is installed globally on the fnm default node (24.21.0) and is **not** in the `fnm`
packages list in `.chezmoidata/system_packages_autoinstall.yaml`. `~/.gnhf` does not exist, so it has
never been run here. Its own ledger entry at `docs/remaining-work.md:2161` (Todoist `6hW33VJ6rw9q7p9M`)
already carries the adoption plan, dated 2026-09-14 and requested by the operator, so this record does
not re-decide it. Four measured corrections belong to that task rather than to a new one.

1. **The drift has an asymmetric cost.** uu's npm lane runs `npm update -g` with no roster
   (`uu/crates/uu-adapters/src/config/lanes/npm.rs:3`: "There is no roster key, because `npm update -g`
   is already every globally" installed package), so an undeclared global still gets upgraded weekly. The
   apply-time lane is the opposite: `run_onchange_before_10-system-packages.sh.tmpl` iterates the
   declared packages only, and runs `fnm install`/`fnm default` for the declared node version, so a node
   bump installs the declared list into the new node's prefix and leaves an undeclared global behind in
   the old one. Today's state is therefore "upgraded but not reproducible", and the failure mode is a
   silent disappearance at the next node bump, not a stale version.

1. **The task's "(pinned, like the other npm tools there)" does not describe the lane.** Of the fifteen
   entries in that list exactly one is pinned, `"@a5c-ai/babysitter-sdk@6.0.3"`, and the comment above it
   explains why that pin is an exception. Pinning `gnhf` is a defensible choice, but it is a new
   convention for that list, not the existing one.

1. **Its rollback path is a gated command.** Upstream's README describes failed iterations rolling back
   with `git reset --hard`, preserving uncommitted work only when the commit itself failed.
   `git reset --hard` is on the operator's per-invocation destructive-action gate list, and gnhf runs it
   unattended by design. The `--worktree` flag and the `gnhf/` branch prefix (both confirmed in
   `gnhf --help` and the README) are what keep that reset inside throwaway state, which makes them
   constraints rather than options. `--max-iterations`, `--max-tokens`, `--max-rate-limit-wait` and an
   abort after three consecutive failures are the other rails upstream documents.

1. **Licensing is recorded in one place only.** The GitHub API reports `MIT` for `kunchenguid/gnhf` (4016
   stars, last pushed 2026-09-04); the npm registry record for `gnhf@0.1.49` carries no `license` field.
   That is weaker provenance than a package with both, and it is the same class of note the 2026-08-30
   dossier made for `herdr-tab-smart-rename`.

Telemetry matches the plan already written in the ledger: upstream says anonymous usage telemetry is on
by default and `GNHF_TELEMETRY=0` opts out.

### apple/container: no consumer here, and the strix coupling does not hold

Apple's `container` is real, current and eligible on this machine: the README calls it "a tool that you
can use to create and run Linux containers as lightweight virtual machines on your Mac", Homebrew core
carries `container` 1.4.1 as a bottled Apache-2.0 formula (the 2026-08-30 note's "brew 1.3.0" is one
minor version stale), and the README's minimum is macOS 26 while this machine runs 26.2. The Homebrew
caveat adds a step upstream's installer does not: "no kernel is installed automatically. Install the
recommended kernel before running containers: container system kernel set --recommended".

What it does not do is speak Docker. The README describes Open Container Initiative images and its own
Swift containerization package, and says nothing about a Docker-compatible daemon, socket or application
programming interface. Homebrew core carries the bridge separately: `socktainer` 1.2.1, "Docker-
compatible REST API on top of Apple container", which depends on `container`.

That is what breaks the old coupling. `strix-agent` 1.6.2 declares `docker>=7.1.0` in its PyPI
dependencies, which is the Docker software development kit for Python, and its own install script checks
`command -v docker` and `docker info` before pulling its sandbox image. So strix needs a Docker
application programming interface, `container` does not provide one, and "strix via uv+container as one
decision" is one decision only in the sense that the container half was never needed: the Docker Desktop
already installed (29.7.2, daemon live, declared as a cask) is what would serve strix.

Nothing else here wants a second runtime. The only declared Docker consumer is `act` (formula, line 48),
whose Linux path runs workflows in Docker containers per
`docs/research/2026-04-12-act-runner-isolation.md` ("Option 7"), and whose macOS path was answered by
Tart, also declared. Repository-wide, `docker` appears in tracked non-Rust source in exactly two places:
this package declaration and `dot_config/starship.toml` (the `docker_context` prompt module). No script,
recipe, LaunchAgent or hook runs `docker`.

### strix: nothing technical blocks it, three operator inputs do

Upstream is active and permissive: Apache-2.0, 62286 stars, last pushed 2026-09-13, release v1.6.2 on
2026-09-05. It is an autonomous penetration-testing agent that runs its own sandbox container, pointed at
a local directory, a GitHub repository or a live web application.

**The install path is already decided by this repository's own precedents, and it is not the install
script.** Reading the downloaded `https://strix.ai/install` (354 lines, not executed) shows it installs a
prebuilt release binary into `$HOME/.strix/bin` and then appends `export PATH=$INSTALL_DIR:$PATH` to
shell startup files including `$HOME/.bashrc`, which is a chezmoi-managed target rendered from
`dot_bashrc.tmpl`. The next apply erases that line and the command leaves PATH. It also deletes an older
pipx copy of `strix-agent` from `.local/bin`. This is the same hazard the 2026-08-30 dossier already
ruled on for plannotator ("never its curl installer which writes into chezmoi-managed targets"), so the
ruling carries over unchanged.

The alternative costs nothing in freshness. The GitHub release publishes both the standalone tarballs and
the Python wheels, and PyPI `strix-agent` 1.6.2 was uploaded seven minutes after the release was
published, so `uv tool install strix-agent` lands the same version the script would. It joins the
existing `uv` list in `.chezmoidata/system_packages_autoinstall.yaml`, where the package name and the
command name already differ for a declared entry (`graphifyy` provides `graphify`), and uu's `[lanes.uv]`
runs `uv tool upgrade --all` weekly with no roster to extend. Requirement: Python 3.12 or newer, which uv
supplies itself.

**What is missing is not plumbing.** Strix drives a model through LiteLLM and reads `STRIX_LLM` plus
`LLM_API_KEY` from the environment; results land in `strix_runs/<run-name>` and its own configuration
cache in `~/.strix/cli-config.json`. An `OPENROUTER_API_KEY` already exists in KeePassXC and is already
referenced by one managed template (`private_dot_hermes/private_dot_env.tmpl`), and upstream's own
example provider string is an OpenRouter model, so the key question is exposure and spend rather than
absence. The fully local path exists mechanically (`LLM_API_BASE` against `ollama`, declared and
installed at 0.33.3) but the only local model on this machine is a 4 billion parameter one, which is not
a serious driver for this workload.

Three inputs are genuinely the operator's: a first target, a decision about unattended credit spend, and
where the key is exposed. On the last one, exporting `LLM_API_KEY` from `dot_bashrc.tmpl` would widen a
vault secret to every interactive shell, where hermes keeps the same key scoped to one `.env` file.
direnv is already first in the bashrc init order, so a per-repository `.envrc` or a small wrapper that
reads the vault at call time keeps the current blast radius.

### The three tools the ledger line fences off

Herdr stays the multiplexer; nothing in this record touches that. gh-dash's deferral has a tracked
source: `docs/superpowers/plans/2026-04-19-dotfiles-improvements-v2.md:3963` records it as "cut from v2
scope, re-add if you want it". **git-absorb's rejection has no record in this repository at all**,
outside the ledger sentence itself: a repository-wide grep for "absorb" returns only unrelated uu source
identifiers, and the memory index has no entry for it. The rejection therefore rests on that one
sentence, which is worth knowing the next time it is questioned.

## Verdicts with reasons

**minutes: adopt, already adopted. The evaluation item is superseded by the 2026-07-27 declaration.** It
is declared, installed, tapped, trusted, upgraded weekly by uu's brew lane, and integrated with the vault
by its own `vault setup` command. The only open work is the fresh-machine command-line interface gap, and
it is one line in the formulae list plus removing one stray symlink by hand.

**gnhf: adopt, already decided. Its install work stays in its own ledger task (line 2161).** This record
adds the four corrections above to that task's inputs and does not duplicate it. The one thing that
should not wait for the rest of that task is the declaration: it is installed today and a node bump loses
it.

**apple/container: decline for now.** It would be a second Linux container runtime with no declared
consumer, on a machine where Docker Desktop is installed, running, declared, and the runtime `act`
expects. The one recorded reason to want it (strix) is measurably not a reason, because strix needs a
Docker application programming interface that `container` does not provide. Declining costs nothing that
can be named today.

**strix: defer, with the adoption path pre-cleared.** No technical blocker survived checking: the sandbox
runtime is already here, the key is already in the vault, the install lane and the weekly upgrader
already exist, and the only hazard (the install script's edit to a managed shell file) is avoided by the
lane this repository would use anyway. What is missing is a target, a spend decision and a key-exposure
decision, all three of which are the operator's. Installing it before those exist would put an autonomous
penetration-testing agent on the machine with nothing to point it at.

## Assumptions made in the operator's place

Each of these is a choice this record made rather than leaving the task unanswerable overnight. Each has
a named alternative.

1. **`minutes`: the command-line interface gap is closed by declaring the Homebrew formula
   `silverstein/tap/minutes` in the formulae list, and by removing the hand-made `~/.local/bin/minutes`
   symlink so one binary answers the name.** The tap is already declared and trusted, uu's brew lane then
   reports the version, and the caveat text is upstream's own instruction. *Alternative:* declare the
   symlink itself as a chezmoi `symlink_` entry under `dot_local/bin/`, which keeps exactly one binary
   (the application bundle's) and adds no second download, at the cost of breaking silently if the
   application moves or is renamed, and of putting a declaration in `~/.local/bin`, whose stated rule is
   "only what the OPERATOR TYPES" (which `minutes` is, so the rule is satisfied either way). *Third
   option, rejected:* change nothing and accept that a fresh machine needs the application's "install
   CLI" click. It works, and it is the only option with a manual step in the fresh-machine path.

1. **`apple/container` is declined as an addition, not evaluated as a replacement.** The ledger line
   asked for an adopt-or-decline verdict on adding it, so that is the question answered. *Alternative:*
   if the intent behind the original evaluation was to retire Docker Desktop (lighter runtime, no
   persistent virtual machine or Electron dashboard, Apache-2.0 instead of a commercial product), that is
   a different and larger decision: it costs `container` plus its kernel install plus `socktainer` for
   the Docker application programming interface, and it has to be proven against `act` and against any
   Docker software development kit client such as strix before Docker Desktop is removed. This record
   does not open that.

1. **`strix` is deferred rather than declined outright.** Nothing found argues against the tool; only its
   inputs are missing. *Alternative:* decline it now and delete the item. That is the right call if there
   is no asset the operator wants scanned, and it costs nothing to reverse, since the whole adoption is
   one line in the `uv` list.

1. **`gnhf`'s npm entry is recommended unpinned, matching the fourteen unpinned entries in that list,
   with the pin question handed back.** The ledger task says "pinned"; the lane's actual convention is
   unpinned except for one documented exception. *Alternative:* pin it, on the grounds that a 0.x tool
   that runs unattended overnight and calls `git reset --hard` is exactly where an unreviewed upgrade
   hurts most. This is the stronger argument on safety and the weaker one on convention, which is why it
   is the operator's call rather than this record's.

## What would change each verdict

- **minutes** would reopen only if the operator wants non-default settings (a different output directory,
  retention, templates, or the folder-watcher login service, which is not installed today). Then
  `~/.config/minutes/config.toml` becomes a chezmoi target and the "nothing to manage" finding expires.
- **apple/container** flips if a named consumer appears: a decision to retire Docker Desktop, a workload
  that wants per-container virtual machines, or a tool that speaks its API natively. A second trigger is
  strix adoption plus a decision to run its sandbox without Docker Desktop, which needs `socktainer` in
  the path and a live test, not an assumption.
- **strix** flips to adopt the moment a target, a spend ceiling and a key-exposure choice exist. It flips
  to decline if the operator has no asset to scan. It would also need re-checking if upstream drops the
  Python distribution: the install script already deletes old pipx installs, which is the direction of
  travel, and if PyPI stops tracking the releases then the `uv` lane stops being the clean path and the
  choice becomes a managed download rather than the install script.
- **gnhf** does not need a verdict change, only the declaration and its own task's execution. It would
  need re-examination if upstream removes `--worktree` or changes the rollback away from
  `git reset --hard`, in either direction.

## Open questions for the operator

1. **minutes command-line interface:** declare the `silverstein/tap/minutes` formula and remove the
   `~/.local/bin/minutes` symlink by hand, or declare that symlink in chezmoi instead? (Assumption taken:
   the formula.)
1. **apple/container:** is declining the addition the whole answer, or was the original intent to retire
   Docker Desktop? The second reading turns this into a separate design task.
1. **strix target:** is there an asset to point it at (a repository of yours, a homelab service, a
   deployed application), and is unattended OpenRouter spend acceptable on it?
1. **strix key exposure:** per-repository `.envrc` through direnv, a wrapper that reads KeePassXC at call
   time, or an export in the managed shell? The first two keep the current blast radius; the third widens
   a vault secret to every interactive shell.
1. **gnhf pin:** pin the npm entry (safer for an unattended tool that calls `git reset --hard`) or leave
   it unpinned (matches the lane)?
1. **git-absorb:** the rejection has no record beyond the ledger sentence. Should that sentence stand as
   the record, or is a one-line reason worth adding next to it so the question stops recurring?
