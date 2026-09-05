# uu tooling lanes: Neovim plugins and the rest of the toolchain

Status: design, no code. Written 2026-09-05 against the crate as it stands after the clean-code
refactor. Every command below was checked against the copy installed on this machine, not recalled.

uu today runs four lane kinds: `brew`, `npm`, `uv`, and the generic `command` producer API. The
operator has asked that it also carry Neovim plugin upgrades and the rest of the tooling that updates
itself. This states what each candidate would run, how it would know it failed, what its record row
carries, and the one question that decides whether several of them can exist at all.

## The hard problem: chezmoi owns the pin

Exactly one of the seven candidates upgrades state that this repository is the source of truth for,
and it is the one the operator asked for first. Two more carry a pin held somewhere other than a
committed file, which is the same trap wearing a different hat.

The clearest case is Neovim. `dot_config/nvim/lazy-lock.json` is tracked here and deploys to
`~/.config/nvim/lazy-lock.json`, exactly where lazy.nvim writes its lockfile
(`lockfile = vim.fn.stdpath("config") .. "/lazy-lock.json"`, in the installed
`lua/lazy/core/config.lua`). The committed file holds 83 pinned commits, so an unattended
`Lazy! update` rewrites the deployed lock and the next `chezmoi apply` reverts it. The plugins on
disk then disagree with the lock that describes them, silently, until something notices.

There are only three honest answers to that, and every lane below is assigned one of them:

- **Apply.** uu performs the upgrade. Only safe where nothing in this repository pins the result.
- **Report.** uu asks what is available, records it, and changes nothing. This is what
  `report-plugin-updates` already does for the plugins Claude Code auto-updates: the record is the
  product, and the operator acts on it.
- **Write back.** uu performs the upgrade and then writes the new pin into the chezmoi source
  directory, so the next apply carries the bump forward rather than reverting it.

Write-back deserves suspicion. It would make an unattended weekly job commit to the dotfiles
repository, needing the repository path, a clean working tree, a commit identity and a push
credential, and a failure halfway through leaves source and deployed copy disagreeing in the other
direction. It also breaks uu's rule that the tool is generic and standalone with no chezmoi
assumptions inside it. A `command` lane the operator writes could do it without uu knowing what a
source directory is, which is the point of the producer API.

## Neovim plugins, via lazy.nvim

**Command.** `nvim --headless "+Lazy! check" +qa`, which fetches and reports without touching a
working tree. The bang is load-bearing on either verb: `lua/lazy/view/commands.lua` sets
`wait = cmd.bang == true`, so the plain form returns immediately and a headless nvim quits
mid-fetch.

**Not `Lazy! update`, and redirecting the data directory does not make it safe.** `M.install`,
`M.update` and `M.clean` in `lua/lazy/manage/init.lua` each end by calling
`require("lazy.manage.lock").update()`; `M.check` does not. The lockfile that call writes lives under
the CONFIG directory, not the data directory, so pointing the plugin tree somewhere scratch still
leaves lazy.nvim rewriting the real `~/.config/nvim/lazy-lock.json`. Isolating an update means
redirecting the config directory too, and then the plugins it resolves are not the ones this machine
runs.

**Failure detection.** Not from the exit code. nvim exits 0 whether or not a plugin failed to fetch,
so a lane trusting the status reports a clean week for a broken check forever. lazy.nvim keeps the
state: `require("lazy.core.plugin").has_errors(plugin)` answers per plugin. So the lane runs a Lua
script through `nvim -l` that performs the check, walks the plugin list, prints one line per failure,
and calls `os.exit(1)` when any plugin has errors.

**Record row.** How many of the 83 pins have commits waiting upstream, and the name of each. The
per-plugin commit range the check also gathers is too much for one record.

**Verdict: REPORT.** The lock is committed here, so applying is the revert trap above and writing
back is the credential problem above. The lane records which pins have moved; the operator then
updates and commits deliberately, which is also when a breakage is attributable to a change of
theirs.

## Mason tools

**Command, and there are two rosters, not one.** `nvim --headless "+MasonToolsUpdateSync" +qa` covers
mason-tool-installer's own `ensure_installed`, which is the linters, formatters and debug adapters.
It does not touch the twenty-two language servers this config declares separately through
mason-lspconfig in `dot_config/nvim/lua/plugins/lsp.lua`, among them `basedpyright`, `bashls` and
`clangd`. mason-lspconfig ensures a server is installed and has no update command at all, so a lane
that ran only the first command would report a clean week while every language server stayed at
whatever version it was first installed at.

The synchronous variant is required and it exists: the plugin registers both `MasonToolsUpdate` and
`MasonToolsUpdateSync`, and the asynchronous one returns before the installs finish. mason.nvim's
`MasonUpdate` refreshes the registry index rather than the packages, so it runs first.

Covering both rosters means the script also walks mason-lspconfig's `ensure_installed`, translates
each server name into its Mason package name through mason-lspconfig's own mapping (`bashls` is the
package `bash-language-server`), and installs each through mason.nvim's registry. That mapping is
the cost. The alternative is to narrow the lane and say in the record that it covers tools and not
servers.

**Failure detection. Completion is not success.** mason-tool-installer's `init.lua` inserts a package
into its completion list at line 127, BEFORE calling `p:install` on the next line, so a package whose
upgrade failed still appears in the list the `MasonToolsUpdateCompleted` event carries, with the old
installation still on disk to satisfy any "is it installed" check. A lane built on that event alone
reports success for a failed upgrade.

The verdict is per package instead: the same file registers `install:success` and `install:failed`
handlers on each package object. The script subscribes to those, collects each failure with the
reason Mason gave, and treats `MasonToolsUpdateCompleted` only as the signal that every attempt has
finished.

**Record row.** Tools and servers updated, those already current, and each one that failed with the
reason Mason gave.

**Verdict: APPLY.** Neither roster pins a version, so an upgrade is not reverted by an apply and
there is no pin to write back.

## Neovim treesitter parsers

**Command.** Not `:TSUpdate` from a headless nvim. This config runs nvim-treesitter on its `main`
branch, where `install.lua` defines `M.update` as `a.async(...)`, returning a task with a
`:wait(timeout)` method, so `+TSUpdate +qa` quits before the compiles finish. The lane runs a Lua
script that calls `require("nvim-treesitter.install").update(nil, { summary = true })` and waits on
the task it returns.

**Failure detection. A compile failure does not throw.** `install.lua`'s installer returns
`done == #tasks`, a plain boolean, so a parser that failed to compile makes the task's result `false`
and `:wait()` returns it normally. `:wait()` throws only for a task exception or a timeout, so
error-only handling reports a clean week for a broken compile. The script must reject the value:
`assert(require("nvim-treesitter.install").update(nil, { summary = true }):wait())`. Parsers compile
C, so this lane is the slowest of the seven and the one most likely to fail for a reason outside
Neovim, such as a missing compiler after an Xcode update.

**Record row.** Parsers updated, parsers already current, and each parser that failed to compile with
the tail of the compiler's own error.

**Verdict: APPLY, as reconciliation rather than as an upgrade.** No parser revision is committed in
this repository, so nothing an apply carries is overwritten. But the revisions are pinned all the
same, one level further out: `lua/nvim-treesitter/parsers.lua` inside the plugin holds 320 `revision`
entries, and `needs_update` compares each installed grammar against the revision that file names. The
lane therefore reconciles the installed grammars to what the LOCKED plugin pins; it can never fetch a
grammar newer than that. A newly released grammar reaches this machine only through an
operator-approved nvim-treesitter bump, which the plugin lane above is report-only about. So the
lane is worth running for a narrower reason than keeping parsers current: it repairs a missing or
half-compiled parser, and it picks up the new revisions after the operator does take a bump.

## uv tools

**Command.** `uv tool upgrade --all`. Already shipped as the `uv` lane.

**Failure detection.** The exit code. uv narrates on stderr and a clean run prints nothing, which is
why the lane records a line of its own saying it ran. The record row is that line.

**Verdict: APPLY, and it already does.** No uv tool version is pinned here;
`.chezmoidata/system_packages_autoinstall.yaml` names tools under `packages.macos.uv` without
versions.

## fnm-managed npm globals

**Command.** `npm update -g`, run with fnm's default-alias bin directory first on PATH. Already
shipped as the `npm` lane, and the PATH rule is the whole reason that lane exists: npm is an
`#!/usr/bin/env node` script, so whichever node PATH answers with is the node it installs into.

**Verdict: APPLY, and it already does.** The node version is pinned in
`.chezmoidata/system_packages_autoinstall.yaml`, but the globals riding on it are not, so the weekly
upgrade moves nothing an apply would revert. Bumping the pinned node moves every tool to a new
runtime, which is a deliberate operator action and not this lane's business.

## cargo-installed binaries

**Command, and no new dependency is needed.** Plain `cargo install <crate>` already is the upgrade.
The installed `cargo-install(1)` man page: "If the package is already installed, Cargo will reinstall
it if the installed version does not appear to be up-to-date", reinstalling when the package version
and source, the binary names, or the chosen features change. An unchanged package is skipped at no
compile cost; a newer one is built. `cargo install --list` names the roster, here `fd-find`,
`herdr-navigator`, `nu` and `selene`. That rules out two things. `--force` is wrong, because it
removes exactly that up-to-date check and rebuilds everything weekly for nothing. And `cargo-update`
(which provides `cargo install-update`, **not** installed here) is a convenience whose `--list` is a
cheaper outdated check, not a prerequisite.

**The git-sourced entry is a pin.** `cargo install --list` shows `herdr-navigator` from a git URL at
an explicit rev. Source is one of the values cargo compares, so re-running the same `--git --rev` is
a no-op and dropping the rev moves the install off the pin. The lane walks registry entries only and
records each git-sourced entry as skipped, so re-pinning stays the operator's deliberate act.

**Failure detection.** The exit code per crate, because the lane invokes cargo once per crate rather
than once for the roster, which also keeps one failed build from stopping the rest.

**Record row.** Each crate with its old and new version, each already current, each git-sourced entry
skipped with its rev, and each failed build with the tail of the compiler error.

**Verdict: REPORT.** Applying needs no new dependency and could be reconsidered, but its weekly cost
is a real compile for every crate that moved, on the machine that also compiles pns and uu at apply
time. Reporting which registry crates are behind answers the question at no build cost.

## rustup

**Command.** `rustup update`.

**Failure detection.** The exit code, which rustup sets honestly.

**Record row.** Each toolchain with the version it moved from and to, which rustup prints.

**Verdict: REPORT, and this one is the most dangerous to apply.** The default toolchain here is
`nightly-aarch64-apple-darwin`, and that is what the apply-time builders compile pns and uu with. An
unattended nightly bump can break the next `chezmoi apply` on a machine whose operator changed
nothing, surfacing as a build error in a script they did not run deliberately, and continuous
integration runs stable so nothing catches it first. Reporting the update and letting the operator
take it when they can watch the build is the right trade until a `rust-toolchain.toml` pins it.

## The recommendation, in one place

| Lane               | Verdict        | Why                                                            |
| ------------------ | -------------- | -------------------------------------------------------------- |
| Neovim plugins     | report         | `lazy-lock.json` is committed here, so an apply reverts it     |
| Mason tools+servers| apply          | neither roster pins a version                                  |
| treesitter parsers | apply          | reconciles to the revisions the locked plugin already pins      |
| uv tools           | apply, shipped | no tool version is pinned                                       |
| npm globals        | apply, shipped | the node version is pinned, the globals are not                 |
| cargo binaries     | report         | one entry is git-pinned, and a weekly rebuild is a real cost    |
| rustup             | report         | a nightly bump can break the next apply's builds                |

Five lanes to build, three of them Neovim-hosted and sharing one script shape. The two shipped rows
are there only to show the set is closed.

**One shape serves all five.** Each is a program that runs, prints what it did, and exits non-zero on
failure, which is exactly the `command` producer API's contract, so none needs a built-in lane kind.
They become scripts under `~/.local/libexec` and the config declares each as `[lanes.<name>]` with
`type = "command"`. Building them in would put Neovim, Mason and rustup knowledge inside a tool whose
stated design is to stay generic, for no gain over a script this repository already deploys.

## Decisions the operator must make

1. **Is report-only acceptable for Neovim plugins?** If the answer is no and the bump must be
   unattended, the follow-up question is whether uu may write to the chezmoi source directory, and
   that is a policy change to the tool's stated boundary, not a feature.

2. **Should the cargo lane apply rather than report?** Plain `cargo install <crate>` upgrades a
   registry package and skips an unchanged one, so no dependency stands in the way any more. What
   stands in the way is the weekly compile cost on the machine that also builds pns and uu at apply
   time, and the git-pinned entry the lane would have to keep skipping.

3. **Should the Rust toolchain be pinned before rustup is touched at all?** A `rust-toolchain.toml`
   here would make the rustup lane safe to apply and would end the nightly-versus-stable split
   between this machine and continuous integration. It is worth doing on its own merits and it
   changes this lane's verdict.

4. **How long may the Neovim lanes take?** Treesitter compiles C for every parser. The six-hour lane
   default leaves room, but a first measured run should decide whether these get their own
   `deadline_secs`.

5. **Does a reporting lane deserve its own record state?** Today a lane is clean, failed or deferred.
   A lane whose product is "here are twelve updates you have not taken" reads clean every week, so
   the record cannot tell nothing-to-report from a year of neglect. Either that is accepted or the
   record contract gains a fourth state. Three of the five new lanes report, so this is not a corner
   case.

6. **Is the mason-lspconfig name mapping worth carrying?** Covering the twenty-two language servers
   means translating lspconfig names into Mason package names through mason-lspconfig's own table.
   The alternative is a lane that says plainly it covers tools and not servers, leaving the servers
   frozen at whatever version first installed them.
