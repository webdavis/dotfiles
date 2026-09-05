# uu tooling lanes: Neovim plugins and the rest of the toolchain

Status: design, no code. Written 2026-09-05 against the crate as it stands after the clean-code
refactor. Every command below was checked against the copy installed on this machine, not recalled.

uu today runs four lane kinds: `brew`, `npm`, `uv`, and the generic `command` producer API. The
operator has asked that it also carry Neovim plugin upgrades and the rest of the tooling that updates
itself. This states what each candidate lane would run, how it would know it failed, what its record
row carries, and the one question that decides whether several of them can exist at all.

## The hard problem: chezmoi owns the pin

Exactly one of the seven candidates upgrades state that this repository is the source of truth for,
and it is the one the operator asked for first. Two more carry a pin held somewhere other than a
committed file, which is the same trap wearing a different hat.

The clearest case is Neovim. `dot_config/nvim/lazy-lock.json` is tracked here and deploys to
`~/.config/nvim/lazy-lock.json`, which is exactly the path lazy.nvim writes its lockfile to
(`lockfile = vim.fn.stdpath("config") .. "/lazy-lock.json"`, verified in the installed
`lua/lazy/core/config.lua`). The committed file holds 83 pinned commits. So an unattended
`Lazy! update` on Sunday rewrites the deployed lock, and the operator's next `chezmoi apply` reverts
it to the committed pin. The plugins on disk then disagree with the lock that is supposed to describe
them, silently, until something notices.

There are only three honest answers to that, and every lane below is assigned one of them:

- **Apply.** uu performs the upgrade. Only safe where nothing in this repository pins the result.
- **Report.** uu asks what is available, records it, and changes nothing. This is what
  `report-plugin-updates` already does for the plugins Claude Code auto-updates: the record is the
  product, and the operator acts on it.
- **Write back.** uu performs the upgrade and then writes the new pin into the chezmoi source
  directory, so the next apply carries the bump forward rather than reverting it.

Write-back is the one that deserves suspicion. It would make an unattended weekly job commit to the
dotfiles repository, which means uu would need the repository path, a clean working tree, a commit
identity and a push credential, and a failure halfway through leaves the source and the deployed copy
disagreeing in the other direction. It also breaks uu's own standing rule that the tool is generic
and standalone with no chezmoi assumptions inside it. A `command` lane the operator writes could do
it without uu knowing anything about chezmoi, which is the point of the producer API, but nothing in
uu should learn what a source directory is.

## Neovim plugins, via lazy.nvim

**Command.** `nvim --headless "+Lazy! update" +qa`. The bang is load-bearing:
`lua/lazy/view/commands.lua` sets `wait = cmd.bang == true`, so the plain `:Lazy update` returns
immediately and a headless nvim would quit mid-clone.

**Failure detection.** Not from the exit code. nvim exits 0 whether or not a plugin failed to fetch,
so a lane that trusted the status would report a clean week for a broken update forever. lazy.nvim
does keep the state: `require("lazy.core.plugin").has_errors(plugin)` answers per plugin. So the lane
must run a small Lua script through `nvim -l` that performs the update, walks the plugin list, prints
one line per failure, and calls `os.exit(1)` when any plugin has errors. That script is the lane's
own artifact, not something uu ships.

**Record row.** The count of plugins updated, the count that failed, and the name and reason of each
failure. The commit range per plugin is available and is too much for one record.

**Verdict: REPORT.** The lock is committed here, so applying is the revert trap above and writing back
is the credential problem above. What uu can do safely is run the update in a scratch data directory,
or run `:Lazy check`, and record which of the 83 pins have moved upstream. The operator then updates
and commits deliberately, which is also when a breakage is attributable to a change they made.

## Mason tools

**Command.** `nvim --headless "+MasonToolsUpdateSync" +qa`. The synchronous variant is required and it
exists: `mason-tool-installer.nvim/plugin/mason-tool-installer.lua` registers both `MasonToolsUpdate`
and `MasonToolsUpdateSync`. The asynchronous one returns before the installs finish. Separately,
mason.nvim's own `MasonUpdate` refreshes the registry index rather than the installed packages, so
both are wanted, registry first.

**Failure detection.** Same shape as lazy: nvim's exit code says nothing. mason-tool-installer emits a
`MasonToolsUpdateCompleted` user autocommand carrying the list it actually installed, so the lane's
script subscribes to that, compares against the configured `ensure_installed` roster, and exits
non-zero when a tool the roster names is still missing.

**Record row.** Tools updated, tools already current, and any tool that failed to install with the
reason Mason gave.

**Verdict: APPLY.** Nothing in this repository pins a Mason package version. The roster in
`dot_config/nvim/lua/plugins/lsp.lua` names tools, not versions, so an upgrade is not reverted by an
apply and there is no pin to write back. This is the cleanest candidate of the seven.

## Neovim treesitter parsers

**Command.** Not `:TSUpdate` from a headless nvim. This config runs nvim-treesitter on the `main`
branch (`branch = "main"` in `dot_config/nvim/lua/plugins/treesitter.lua`), where
`lua/nvim-treesitter/install.lua` defines `M.update` as `a.async(...)`, returning a task with a
`:wait(timeout)` method. A headless `+TSUpdate +qa` therefore quits before the compiles finish. The
lane runs a Lua script that calls `require("nvim-treesitter.install").update(nil, { summary = true })`
and then `:wait()` on the returned task.

**Failure detection.** The task's `:wait()` errors on failure and the summary reports per-parser
results, so the script can exit non-zero. Parsers compile C, so this lane is the slowest of the seven
and the one most likely to fail for a reason outside Neovim, such as a missing compiler after an Xcode
update.

**Record row.** Parsers updated, parsers already current, and each parser that failed to compile with
the tail of the compiler's own error.

**Verdict: APPLY.** No parser revision is committed here. `lazy-lock.json` pins the nvim-treesitter
plugin, not the grammars it installs, and those live under the Neovim data directory, which this
repository does not track.

## uv tools

**Command.** `uv tool upgrade --all`. Already shipped as the `uv` lane.

**Failure detection.** The exit code, which is what the lane already uses. uv narrates on stderr and a
clean run prints nothing, which is why the lane records a line of its own saying it ran.

**Record row.** Already shipped: one line naming the binary and whether the upgrade succeeded.

**Verdict: APPLY, and it already does.** Listed here only to close the set. No uv tool version is
pinned in this repository; `.chezmoidata/system_packages_autoinstall.yaml` names tools under
`packages.macos.uv` without versions.

## fnm-managed npm globals

**Command.** `npm update -g`, run with fnm's default-alias bin directory first on PATH. Already
shipped as the `npm` lane, and the PATH rule is the whole reason that lane exists: npm is an
`#!/usr/bin/env node` script, so whichever node PATH answers with is the node it installs into.

**Record row.** Already shipped.

**Verdict: APPLY, and it already does.** The node version is pinned in
`.chezmoidata/system_packages_autoinstall.yaml`, but the npm globals riding on it are not, so the
weekly upgrade moves nothing the apply would revert. Note the one hazard the existing lane already
documents: bumping the pinned node moves every tool to a new runtime, which is a deliberate operator
action and not this lane's business.

## cargo-installed binaries

**Command.** There is no upgrade subcommand in cargo itself. `cargo install --list` reports what is
installed (on this machine: `fd-find`, `herdr-navigator`, `nu`, `selene`), and the usual answer is
`cargo install-update -a` from the `cargo-update` crate, which is **not installed here**: `cargo
install-update --version` answers "no such command". Adding it means adding a dependency whose whole
job is to resolve and rebuild, which is a real build cost every Sunday and a new thing that can break.
The alternative with no new dependency is `cargo install --force <crate>` per entry, which
unconditionally rebuilds whether or not there is a newer version, so it pays the full compile cost
every week for nothing.

There is a second problem this machine already has. `cargo install --list` shows `herdr-navigator`
installed from a git URL at an explicit rev. An upgrade would move it off that rev, which is a pin,
just held in the install rather than in a lockfile.

**Failure detection.** The exit code, plus per-crate parsing, because `install-update -a` continues
past a crate that failed to build and still exits zero in some versions. The lane would have to read
its summary rather than trust the status.

**Record row.** Each crate with its old and new version, and each crate that failed to build with the
tail of the compiler error.

**Verdict: REPORT, and accept `cargo-update` as a dependency only if the operator wants applying.**
`cargo install-update --list` reports what is outdated without building anything, which is cheap
enough to run weekly and answers the question. The git-pinned entry is the reason not to apply blindly.

## rustup

**Command.** `rustup update`.

**Failure detection.** The exit code, which rustup sets honestly.

**Record row.** Each toolchain with the version it moved from and to, which rustup prints.

**Verdict: REPORT, and this one is the most dangerous to apply.** The default toolchain on this machine
is `nightly-aarch64-apple-darwin`, and that is the toolchain the apply-time builders use to compile
pns and uu. An unattended nightly bump can therefore break the next `chezmoi apply` on a machine whose
operator changed nothing, and the failure surfaces as a build error in a script they did not run
deliberately. Continuous integration runs stable, so a nightly regression is not caught anywhere
before the apply. Reporting the available update and letting the operator take it when they can watch
the build is the right trade until the toolchain is pinned in a `rust-toolchain.toml`.

## The recommendation, in one place

| Lane                | Verdict          | Why                                                          |
| ------------------- | ---------------- | ------------------------------------------------------------ |
| Neovim plugins      | report           | `lazy-lock.json` is committed here, so an apply reverts it   |
| Mason tools         | apply            | no version is pinned in this repository                      |
| treesitter parsers  | apply            | parsers live in the data directory, which is untracked       |
| uv tools            | apply, shipped   | no tool version is pinned                                    |
| npm globals         | apply, shipped   | the node version is pinned, the globals are not              |
| cargo binaries      | report           | one entry is git-pinned, and applying needs a new dependency |
| rustup              | report           | a nightly bump can break the next apply's builds             |

Three lanes to build, two of them Neovim-hosted and sharing the same script shape, plus two reporting
lanes with no Neovim involvement. That is a smaller program than the seven-lane list suggests, and the
two already-shipped rows are there only to show the set is closed.

**One shape serves all five new lanes.** Each is a program that runs, prints what it did, and exits
non-zero on failure, which is exactly the `command` producer API's contract. None of them needs a
built-in lane kind in uu. The Neovim ones become scripts under `~/.local/libexec`, the reporting ones
likewise, and the config declares each as `[lanes.<name>]` with `type = "command"`. Building them as
built-in kinds would put Neovim, Mason and rustup knowledge inside a tool whose stated design is to
stay generic and standalone, for no gain over a script the same repository already knows how to
deploy.

## Decisions the operator must make

1. **Is report-only acceptable for Neovim plugins?** If the answer is no and the bump must be
   unattended, the follow-up question is whether uu may write to the chezmoi source directory, and
   that is a policy change to the tool's stated boundary, not a feature.

2. **Should `cargo-update` be installed?** Applying cargo upgrades needs it. Reporting also uses it
   (`--list`), so the dependency is wanted either way unless cargo binaries are dropped from scope
   entirely.

3. **Should the Rust toolchain be pinned before rustup is touched at all?** A `rust-toolchain.toml`
   in this repository would make the rustup lane safe to apply and would also end the current
   nightly-versus-stable split between this machine and continuous integration. That is worth doing
   on its own merits and it changes this lane's verdict.

4. **How long may the Neovim lanes take?** Treesitter compiles C for every parser. The lane deadline
   default is six hours and the run deadline is twenty-four, so there is room, but a first measured
   run should decide whether these lanes get a `deadline_secs` of their own.

5. **Does a reporting lane deserve its own record state?** Today a lane is clean, failed or deferred.
   A lane whose whole product is "here are twelve updates you have not taken" is clean every week, so
   the record reads identically whether there is nothing to report or a year of neglect. Either that
   is accepted, or reporting lanes need a fourth state, which is a change to the record contract.
