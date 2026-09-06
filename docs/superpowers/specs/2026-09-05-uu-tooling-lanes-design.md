# uu tooling lanes: Neovim plugins and the rest of the toolchain

Status: design, settled. First written 2026-09-05 against the crate as it stands after the clean-code
refactor; amended 2026-09-06 with the operator's rulings from the grill of 2026-09-05 and 2026-09-06.
Every command below was checked against the copy installed on this machine, not recalled. The
implementation plan is `docs/superpowers/plans/2026-09-06-uu-tooling-lanes-plan.md`.

uu today runs four lane kinds: `brew`, `npm`, `uv`, and the generic `command` producer API
(application programming interface). This design adds nine more, in three groups. The Neovim group
covers the plugins lazy.nvim pins, the tools Mason installs, the treesitter parsers, and a smoke test
of the whole config. The toolchain group covers cargo-installed binaries and rustup. The third group is
the three bash weekly jobs this repository still runs beside uu (the skills store refresh, the Claude
Code plugin record, and the entry helper they share) plus the hourly log rotation. All of them become
uu lanes, so that one binary, one config and one record cover every unattended job on the machine.

## The hard problem: chezmoi owns the pin

Exactly one of the candidates upgrades state that this repository is the source of truth for, and it
is the one the operator asked for first. Two more carry a pin held somewhere other than a committed
file, which is the same trap wearing a different hat.

The clearest case is Neovim. `dot_config/nvim/lazy-lock.json` is tracked here and deploys, as a
regular file, to `~/.config/nvim/lazy-lock.json`, exactly where lazy.nvim writes its lockfile
(`lockfile = vim.fn.stdpath("config") .. "/lazy-lock.json"`, in the installed
`lua/lazy/core/config.lua`). The committed file holds 83 pinned commits, so an unattended
`Lazy! update` rewrites the deployed lock and the next `chezmoi apply` reverts it. The plugins on disk
then disagree with the lock that describes them, silently, until something notices.

There are only three honest answers to that:

- **Apply.** uu performs the upgrade. Only safe where nothing in this repository pins the result.
- **Report.** uu asks what is available, records it, and changes nothing. The record is the product,
  and the operator acts on it.
- **Write back.** uu performs the upgrade and then writes the new pin into the chezmoi source
  directory, so the next apply carries the bump forward rather than reverting it.

Write-back deserves suspicion. It makes an unattended weekly job commit to the dotfiles repository,
needing the repository path, a clean working tree at the file and a commit identity, and a failure
halfway through leaves source and deployed copy disagreeing in the other direction. The operator's
ruling: Neovim plugins are report-only. The exact pins stay committed in `lazy-lock.json`, the operator
bumps them through a pull request, and uu reports what moved. Write-back still ships, as the
`auto_commit` key on that lane, default off, described with the lane below. It commits and never
pushes, so even switched on it leaves the pull request to the operator.

## What a lane may do: apply, report, verify

Every lane below is assigned one of three verbs. Apply and report are the two above. The third is
**verify**: uu installs nothing the machine keeps and changes nothing it runs on; it exercises a
candidate and says whether the candidate passed. The smoke test is the one lane of that kind.

### The `pending` record state

A run used to be `completed`, `deferred` or `failed`. A report lane whose product is "twelve pins have
moved and you have not taken them" read `completed` every week, so the record could not tell a quiet
week from a year of neglect. The run's state gains `pending`: at least one lane found something the
operator has to take, and nothing failed or deferred. A failure anywhere wins over a deferral, a
deferral wins over pending, and pending wins over completed, because the record is read at a glance and
the graver reading has to be the one on top.

A pending lane did run, and did succeed at the job it has, so a pending run still moves the
last-successful-run marker. Only a failure, a deferral or a lost record holds the marker back. In the
detail, a pending lane's verdict line reads `pending`, never `0 failure(s)`, and the closing line counts
pending lanes beside failures and deferrals.

How a lane says so: a built-in lane marks its own report pending. A `command` lane's child, and the Lua
behind every Neovim lane, exit **100**. That number sits outside the sysexits range (64 to 78, where 75
already means deferred), below the shell's reserved 126 and 127, and below the signal range that starts
at 128, so nothing else on the machine returns it by accident. The shipped config documents it beside
the existing note on 75.

### Escalation

A lane that has been pending for `escalate_after_runs` consecutive runs fires one alarm of its own,
saying how long the updates have been waiting. The default is 3, which on the shipped weekly schedule
is three weeks. The alarm trips exactly once per streak, on the run that reaches the threshold, and a
run with nothing pending resets the count, which is the shape the staleness alarm already has. The
count lives beside the staleness streak, one small file per lane at
`~/.local/state/uu/lanes/<name>/pending`.

The key sits beside `deadline_secs`, read before the lane's type is dispatched, so every lane type
carries it and none has to remember to. It ships written out at `3` on the lanes that can be pending;
removing it falls back to 3, and a value that is not a positive whole number is refused by name.

## Alarms: one path, two destinations

uu raises three alarms today, all through the pns engine named in `[alerts]`: a lane failed, a lane
went stale (three runs without a success), and the weekly record could not be delivered. The
escalation above joins them as the fourth. Every alarm goes through the one `send_alert` path, which is
what makes the next paragraph a single change rather than four.

Failures may also go to a separate webhook. `[records]` gains `failure_webhook`: a URL that receives a
second signed POST for every alarm, signed with the same `key` the record uses and carrying the same
four-field body (`agent` is `uu`, `state` names the alarm as `failed`, `stale`, `record-lost` or
`pending`, `project` is the host, `detail` is the sentence pns was handed). All four alarms go there;
the operator ruled against a subset. Writing the URL alone opts in, and the empty string means off.

That makes `failure_webhook` the one key in this schema where an empty string is legal. Everywhere else
a blank value is refused, because it reads as a setting that was made; here the operator ruled that
empty means off, and the key ships written out as `failure_webhook = ""` so the option is visible
without being on. The parser admits `""` and whitespace-only for this key alone and says so in its
comment.

## How uu drives Neovim

The Neovim-side logic is Lua, and it lives in the Neovim config under `lua/uu/`, one module per lane:
`plugins.lua`, `mason.lua`, `parsers.lua` and `smoke_test.lua`, with their pure parts (line
composition, the keymap dump and its diff, the instance count, the exit codes) in `report.lua`. There
is no shell wrapper anywhere. uu runs

```text
nvim --headless -u <config>/init.lua -l <config>/lua/uu/<module>.lua [args]
```

where `<config>` is the lane's `config` key, `~/.config/nvim` on this machine. The `-u` is
load-bearing: `:help -l` on the installed 0.12.5 says the script runs "after processing any preceding
Nvim cli-arguments" and "skips user config unless -u was given", so without it lazy.nvim, Mason and
nvim-treesitter are not on the runtime path at all. A Lua error exits 1 on its own; the modules exit 0,
1 or 100 deliberately, and print their record lines to stdout, which uu keeps whatever the exit was.

The pure halves are pinned by headless specs under `dot_config/nvim/tests/uu_*_spec.lua`, run by
`just test-nvim` under `--clean` exactly like the `custom_api` specs (the runner puts `lua/` on
`package.path`). The halves that drive lazy.nvim, Mason and the treesitter installer are proved by one
real headless run at pull-request time against this config, because a fake plugin manager would be a
test of a plugin nobody here wrote.

Running while a Neovim is open is fine. A plugin tree swapped under a live instance costs nothing until
that instance loads a module it had not loaded yet, and the operator ruled it acceptable. The record
carries a restart notice instead: each apply lane counts the other running instances by listing the
sibling directories of `vim.fn.stdpath("run")` (Neovim keeps one per instance under
`$TMPDIR/nvim.<user>/`, each holding its `nvim.<pid>.0` socket) and, when there are any, adds
`N Neovim instance(s) were running during this update; restart them to load the new versions`.

Two consequences for the LaunchAgent. Mason installs its npm-based packages with whatever `npm` PATH
answers with and its cargo-based ones with `cargo`, so `com.webdavis.uu.plist` gains
`~/.local/share/fnm/aliases/default/bin` and `~/.cargo/bin` on its PATH; the fnm path is the
version-free one this repository already keeps for LaunchAgents. And lanes run in NAME order, so the
four Neovim lanes are named to run mason, parsers, plugins, smoke test: `nvim-mason`, `nvim-parsers`,
`nvim-plugins`, `nvim-smoke-test`. Parsers before plugins is fine even with `auto_commit` on, because
lazy.nvim runs `:TSUpdate` as nvim-treesitter's own build step (`build = ":TSUpdate"`,
`lua/plugins/treesitter.lua`), so a plugin bump recompiles its parsers as part of the bump.

## Neovim plugins, via lazy.nvim (`nvim-plugins`)

**Command.** The check `:Lazy! check` performs, from Lua so the result can be read back:
`require("lazy.manage").check({ wait = true, show = false })`. It fetches and reports without touching
a working tree. The wait is load-bearing on either verb: `lua/lazy/view/commands.lua` sets
`wait = cmd.bang == true`, so the plain form returns immediately and a headless nvim quits mid-fetch.

**Not `Lazy! update`, and redirecting the data directory does not make it safe.** `M.install`,
`M.update` and `M.clean` in `lua/lazy/manage/init.lua` each end by calling
`require("lazy.manage.lock").update()`; `M.check` does not. The lockfile that call writes lives under
the CONFIG directory, not the data directory, so pointing the plugin tree somewhere scratch still leaves
lazy.nvim rewriting the real `~/.config/nvim/lazy-lock.json`. Isolating an update means redirecting the
config directory too, which is exactly what the smoke test does.

**Failure detection.** Not from the exit code. nvim exits 0 whether or not a plugin failed to fetch, so
a lane trusting the status reports a clean week for a broken check forever. lazy.nvim keeps the state:
`require("lazy.core.plugin").has_errors(plugin)` answers per plugin. The module walks the plugin list
after the check, prints one line per failure, and exits 1 when any plugin has errors.

**Record row.** How many of the 83 pins have commits waiting upstream, and the name of each. The
per-plugin commit range the check also gathers is too much for one record. When any pin has moved the
module exits 100 and the lane is pending; the escalation fires after three pending weeks.

**Verdict: REPORT, settled.** The lock is committed here, so applying is the revert trap above. The
lane records which pins have moved; the operator then updates and commits through a pull request, which
is also when a breakage is attributable to a change of theirs.

**`auto_commit`, default off.** The write-back answer ships behind this key. When it is `true` the
module runs `require("lazy.manage").update({ wait = true, show = false })` against the live tree, which
rewrites the live lock, then copies `<config>/lazy-lock.json` to `<repo>/dot_config/nvim/lazy-lock.json`
and commits it there with `SKIP_AI_COMMIT=1 git -C <repo> commit`, a fixed message and the operator's
hooks running as they would for anyone. It never pushes; the pull request stays the operator's. `repo`
names the chezmoi source checkout and is required when `auto_commit` is on. Two preconditions are
checked BEFORE the update runs, because the update is what moves the live tree past the committed lock:
`git status --porcelain -- dot_config/nvim/lazy-lock.json` must print nothing, and HEAD must be a
branch. When either fails the lane falls back to the check for that run and says why, so the machine is
never left on plugins its committed lock does not describe. The record then names the commit, the lane
is completed rather than pending, and the restart notice applies.

## Mason tools (`nvim-mason`)

**Command.** `MasonUpdate` first, which refreshes the registry index rather than the packages, then
`MasonToolsUpdateSync`, the synchronous variant, which the plugin does register beside the asynchronous
`MasonToolsUpdate` that returns before the installs finish. That covers mason-tool-installer's own
`ensure_installed`: the linters, formatters and debug adapters.

**Language servers are Mason's, not uu's.** This config declares twenty-two servers separately through
mason-lspconfig in `dot_config/nvim/lua/plugins/lsp.lua`, among them `basedpyright`, `bashls` and
`clangd`; mason-lspconfig installs a server that is missing and has no update command. Covering them
from uu would mean translating each lspconfig name into its Mason package through mason-lspconfig's own
mapping (`bashls` is the package `bash-language-server`). The operator ruled against carrying that
mapping: Mason covers the servers, and upgrading one is the operator's action in `:Mason`. The lane
says so in its record every week, so the row never implies a completeness it does not have.

**Failure detection. Completion is not success.** mason-tool-installer's `init.lua` inserts a package
into its completion list at line 127, BEFORE calling `p:install` on the next line, so a package whose
upgrade failed still appears in the list the `MasonToolsUpdateCompleted` event carries, with the old
installation still on disk to satisfy any "is it installed" check. The verdict is per package instead:
the same file registers `install:success` and `install:failed` handlers on each package object (lines
113 and 116). The module subscribes to those, collects each failure with the reason Mason gave, and
treats `MasonToolsUpdateCompleted` only as the signal that every attempt has finished.

**Record row.** Tools updated, tools already current, each one that failed with the reason Mason gave,
the sentence about servers, and the restart notice when it applies.

**Verdict: APPLY.** The roster pins no version, so an upgrade is not reverted by an apply and there is
no pin to write back.

## Treesitter parsers (`nvim-parsers`)

**Command.** Not `:TSUpdate` from a headless nvim. This config runs nvim-treesitter on its `main`
branch, where `install.lua` defines `M.update` as `a.async(...)`, returning a task with a
`:wait(timeout)` method, so `+TSUpdate +qa` quits before the compiles finish. The module calls
`require("nvim-treesitter.install").update(nil, { summary = true })` and waits on the task it returns.

**Failure detection. A compile failure does not throw.** `install.lua`'s installer returns
`done == #tasks`, a plain boolean, so a parser that failed to compile makes the task's result `false`
and `:wait()` returns it normally. `:wait()` throws only for a task exception or a timeout, so
error-only handling reports a clean week for a broken compile. The module rejects the value:
`assert(require("nvim-treesitter.install").update(nil, { summary = true }):wait())`. Parsers compile C,
so this lane is the slowest of the Neovim four and the one most likely to fail for a reason outside
Neovim, such as a missing compiler after an Xcode update.

**Record row.** Parsers updated, parsers already current, each parser that failed to compile with the
tail of the compiler's own error, and the restart notice when it applies.

**Verdict: APPLY, as reconciliation rather than as an upgrade.** No parser revision is committed in
this repository, so nothing an apply carries is overwritten. But the revisions are pinned all the same,
one level further out: `lua/nvim-treesitter/parsers.lua` inside the plugin holds 320 `revision`
entries, and `needs_update` compares each installed grammar against the revision that file names. The
lane reconciles the installed grammars to what the LOCKED plugin pins; it can never fetch a grammar
newer than that. A newly released grammar reaches this machine only through an operator-approved
nvim-treesitter bump. So the lane is worth running for a narrower reason than keeping parsers current:
it repairs a missing or half-compiled parser, and it picks up the new revisions after the operator does
take a bump.

## The smoke test (`nvim-smoke-test`)

The plugins lane says which pins have moved. This lane answers the question that decides whether to
take them: does the config still start, with those versions, on this machine? It is opt-in, the
operator wants it on here, and its block in the shipped config carries a comment naming what it costs.

**What it costs.** A second plugin tree on disk. Measured on 2026-09-06, `~/.local/share/nvim/lazy`
holds 93 plugins in 417 MB, so the candidate tree is about that size and stays on disk between runs
under `~/.cache/uu/nvim-smoke-test/`, so a week costs a fetch rather than a fresh clone. Mason's tree
is 2.1 GB and is NOT duplicated: the candidate's `data/nvim/mason` is a symlink to the live one, which
the smoke test only reads (mason-tool-installer's `run_on_start` is off in this config, so nothing
installs at startup). Deleting the cache directory reclaims the space; commenting the block out stops
paying it.

**Command.** uu copies `<config>` to `<cache>/config/nvim` (a plain recursive copy, so the copied lock
is the committed one), then runs the module with the four base directories redirected through
`/usr/bin/env`, which needs no new spawn seam:

```text
/usr/bin/env XDG_CONFIG_HOME=<cache>/config XDG_DATA_HOME=<cache>/data XDG_STATE_HOME=<cache>/state
  XDG_CACHE_HOME=<cache>/cache nvim --headless -u <cache>/config/nvim/init.lua
  -l <cache>/config/nvim/lua/uu/smoke_test.lua
```

Inside, `stdpath("config")` is the copy, so the lock lazy.nvim rewrites is the copy's. The module runs
`require("lazy.manage").update({ wait = true, show = false })`, which installs the candidate versions
into the scratch data directory, then runs `checkhealth` and reads the health buffer back, then dumps
every global keymap (`vim.api.nvim_get_keymap(mode)` for each mode letter) as one
`mode <tab> lhs <tab> rhs-or-description` row per mapping.

**Failure detection.** The module exits 1 when any plugin has errors after the update
(`has_errors`, as above) or when the config raised a Lua error on the way up. `checkhealth` lines
marked `ERROR` do not fail the lane on their own, because some are permanent on this machine (an
optional provider that is not installed); they are counted and named so the operator can compare weeks.

**Record row.** Pass or fail, the count of `ERROR` and `WARNING` lines in the health report, the count
of mappings in the dump, and how many mappings were added or removed since the previous run's dump,
by lhs and mode, which is what catches a plugin update that took a key. The full health report and the
dump are written beside the cache at `<cache>/checkhealth.txt` and `<cache>/keymaps.tsv`, and the
record names those paths rather than carrying their contents.

**Verdict: VERIFY.** Nothing the machine runs on changes. With `auto_commit` off the candidate is
exactly the pull request the operator would open; with it on, the candidate is what the plugins lane
just installed, and this lane's failure is the alarm to revert it.

## uv tools and fnm-managed npm globals

Shipped as the `uv` and `npm` lanes, unchanged by this design. Both apply: no uv tool version is
pinned here, and while the node version is pinned in `.chezmoidata/system_packages_autoinstall.yaml`,
the globals riding on it are not, so the weekly upgrade moves nothing an apply would revert. The npm
lane's whole reason to exist is the PATH rule: npm is an `#!/usr/bin/env node` script, so the lane runs
it with fnm's default-alias bin directory first on PATH.

## cargo-installed binaries (`cargo`)

**Command, and no new dependency is needed.** `cargo install --list` names the roster, here `fd-find`,
`herdr-navigator`, `nu` and `selene`, each crate line followed by its binaries indented beneath it. For
each registry crate the lane asks `cargo search <crate> --limit 1`, whose first line is the newest
published version in the shape `fd-find = "10.5.0"    # fd is a simple, fast ...` (checked
2026-09-06). Plain `cargo install <crate>` is the upgrade: the installed `cargo-install(1)` man page
says an already-installed package is reinstalled "if the installed version does not appear to be
up-to-date", so an unchanged crate costs no compile and `--force` is wrong because it removes exactly
that check. `cargo-update` (which provides `cargo install-update`, not installed here) is a convenience,
not a prerequisite.

**The git-sourced entry is a pin.** `cargo install --list` shows `herdr-navigator` from a git URL at
an explicit rev. Source is one of the values cargo compares, so re-running the same `--git --rev` is a
no-op and dropping the rev moves the install off the pin. The lane walks registry entries only and
records each git-sourced entry as skipped with its rev, so re-pinning stays the operator's deliberate
act.

**`compile`, default off.** The lane compiles only when asked. With `compile = false` a crate that is
behind produces this line, in the operator's own wording, and the lane is pending:

```text
fd has a new version: 8.4.0 → 10.5.0. Run the following command to compile it: cargo install fd-find
```

The subject is the binary when the crate installs exactly one (that is what `cargo install --list`
lists under `fd-find`), otherwise the crate. With `compile = true` the lane runs `cargo install <crate>`
once per crate that is behind, so one failed build does not stop the rest, and the exit code per crate
is the verdict.

**Record row.** Each crate with its old and new version, each already current, each git-sourced entry
skipped with its rev, and, when compiling, each failed build with the tail of the compiler error.

**Verdict: REPORT by default, APPLY when opted in.** The weekly cost of applying is a real compile for
every crate that moved, on the machine that also compiles pns and uu at apply time, so the default
answers the question at no build cost and leaves the compile to the operator's command line.

## rustup (`rustup`), and the stable pin that makes it safe

**Command.** `rustup update`, which also updates rustup itself. It prints one summary line per
toolchain in the shape `stable-aarch64-apple-darwin updated - rustc 1.98.1 (... 2026-09-01) (from
rustc 1.88.0 ...)` or `unchanged - ...`; the exit code is honest.

**The pin comes first.** The default toolchain here is `nightly-aarch64-apple-darwin`, and until now
that is what the apply-time builders compiled pns and uu with, so an unattended nightly bump could break
the next `chezmoi apply` on a machine whose operator changed nothing, and continuous integration (CI)
runs stable so nothing caught it first. The operator ruled for the pin: each of the four crates this
repository owns (pns at its workspace root, uu, and the two herdr plugins) gains a
`rust-toolchain.toml` with `channel = "stable"`. rustup's `cargo` proxy at `~/.cargo/bin/cargo`, which
is the path every builder already uses, honours the file, so the builders and `just test-rust` move to
stable with no other change. Nothing in the four crates uses a nightly feature (no `#![feature]`,
checked 2026-09-06), and CI has always run them on stable. The one thing that changes for the operator
is Miri, which stays a nightly tool and is invoked as `cargo +nightly miri` from then on; the
`clean-code-rust` skill's line about the local toolchain being nightly is corrected in the same pull
request.

**Record row.** Each toolchain with the version it moved from and to, and `rustup` itself.

**Verdict: APPLY, once the pin has landed.** The pin removes the failure the original design refused
to risk: a stable bump is the same bump CI takes, and a plain `cargo build` does not run clippy, so a
new stable cannot turn the builders red the way a new nightly could. The lane's pull request lands after
the pin's.

## The three bash weekly jobs become lanes

The operator ruled to port all three: `update-skills.sh` (the skills store refresh),
`report-plugin-updates.sh` (the Claude Code plugin record) and `helpers/log-entries.sh` (the entry
shape both source). They were written before uu existed and each carries its own copy of what uu now
provides: a serialize lock, a last-successful-run marker with a gap sentence, a weekly record posted
through pns, and an alert route. Porting them retires those copies, the three LaunchAgents and the
three retry and gating mechanisms that only existed because each job scheduled itself.

Two things change for every port and are stated once here. The schedule becomes uu's: one Sunday slot
under uu's run lock, retried a week later, instead of a job's own hourly slots and week stamp. A job
that keeps failing is caught by uu's staleness alarm after three runs, which replaces each job's own
"retry budget exhausted" wording. And the record is uu's: one entry per run for every lane, so a job no
longer posts a `deferred` entry of its own or claims a week with a guard file; a run that could not post
is uu's record-lost alarm.

### The skills store refresh (`skills`)

`update-skills.sh` is 3,806 lines of bash with real guarantees, and the port keeps them: the store is
one live generation, a candidate is built as a fake HOME under `~/.agents/.skills-generations/<id>/`,
the npx and clawhub lanes run against it under a clean environment, the Codex tier overlays are
asserted, the candidate is validated whole, and it is published with ONE atomic exchange so any lookup
during or after the swap sees a complete tree from exactly one generation. A lane or validation failure
discards the whole candidate and the live generation is untouched. Exactly one previous generation is
retained. The fan-out to Claude (respecting `claudeDelivery` `"none"`) and to the hermes profiles, the
hermes registry-update phase, the fork drift check, the app-owned `cua-driver` pack refresh and the
superpowers routing assertion all stay. The roster gate stays fail-closed: a missing, unparseable or
schema-broken `custom-skill-lock.json`, or one tracking zero skills, refuses the run.

What changes. The exchange primitive is `renameatx_np` with `RENAME_SWAP`, through libc, which is
the macOS system call `GNU mv --exchange` wraps; uu is already macOS-only, so resolving a GNU mv at run
time and probing it with a swap goes away. The `--dry-run` mode goes away; nothing in this repository
calls it and uu has no preview mode to mirror. The `--install-only` mode becomes `uu bootstrap skills`
(below), which is what the apply-time chezmoiscript runs. Per-skill failure streaks go away: the weekly
record names every skill that failed and the failure alarm fires each week it does, so the escalating
wording they produced adds nothing. Fork drift is not a failure of the lane, it is work for the
operator ("compare and port by hand"), so it makes the lane pending rather than raising a separate
alert state per fork. `assert-hermes-superpowers-routing.sh` stays bash, because `live-reconcile.sh`
also calls it; the lane runs it as a command.

**Record row.** The header the bash job printed (which skills were added, removed or changed, capped
at twelve names with the rest counted), the generation published, each lane's failed skills with the
reason, the hermes phase per profile, each fork's drift state, and the routing assertion's verdict.

**Verdict: APPLY.** Nothing here is pinned in this repository beyond the roster itself.

### The Claude Code plugin record (`claude-plugins`)

Claude Code refreshes its marketplaces and installed plugins at startup by itself; what it does not do
is leave a record. The lane reads `~/.claude/plugins/installed_plugins.json` the way the bash job did:
exactly one JSON document (serde's `from_str` refuses trailing content, so the bash job's slurp-and-count
comes for free), a `plugins` object with at least one entry, every install record shape-checked before
the scope filter, USER-scope records only, and a fingerprint that is `version` when the marketplace
publishes a real one, else `gitCommitSha`, else the literal `unknown`. A degraded file fails the lane
rather than reading as a quiet week. The reading is compared with the previous one, kept at
`~/.local/state/uu/lanes/claude-plugins/snapshot.tsv`, through the same change section the brew lane
renders (added, removed, changed, twelve names then a count, third-party text in code spans), and the
caveat about project-scope plugins and `unknown` fingerprints is restated on every entry.

One rule moves. The bash job moved its snapshot only after the gateway accepted the entry, so a change
consumed by a run that reported it nowhere would be reported by the next one. uu posts one record for
the whole run after every lane has finished, so a lane cannot know whether the record landed; the
snapshot moves when the lane runs, and a record the gateway refused is uu's record-lost alarm, with the
full detail in `~/.local/log/uu/uu.log`. That is a deliberate trade, and it is the one the brew lane
already makes.

The first reading is a baseline and compares nothing. The apply-time seed stays, as `uu bootstrap
claude-plugins`, for the reason the bash job spells out: the same apply that deploys the record also
turns marketplace auto-updates on, and baselining at the first scheduled run instead would bake the
first week's changes into the baseline and report them never. Seeding is idempotent.

**Record row.** The change section, or the baseline notice, or the reason the file could not be read.

**Verdict: REPORT, completed rather than pending.** There is nothing for the operator to take; the
record itself is the product, as it always was.

### The shared entry helper (`log-entries.sh`)

Retired once both callers are ported. Every behavior in it already has a home in uu: the marker that
stores epoch plus ISO 8601 on one line and the gap sentence with its three states (`record::marker`),
the elapsed rendering with its unit boundaries, the change tuples and the sentence with the twelve-name
cap and code-span quoting (`lanes::brew::changes` and `sections`, lifted to a shared `lanes::changes`
so three lanes read one copy), the host name, and the once-a-week alarm that the record channel itself
is broken (uu's record-lost alarm). The week-claim guard has no successor because uu runs once per week
under one lock and posts once per run. The route name `unattended-upgrades` stays; it is uu's
`DEFAULT_RECORD_URL`, and `run_after_68-hermes-log-route-status.sh.tmpl` points at that constant
instead of at the helper.

## Log rotation (`rotate-logs`)

The operator asked why logs are rotated at all and accepted the two reasons: bounded disk, and a
recent history that stays readable. `compress-and-truncate-local-logs.sh` becomes a uu lane with the
same mechanics. Truncate in place rather than rename, because every log under `~/.local/log` is a
launchd `StandardOutPath` redirect whose descriptor the daemon holds open and never reopens, so a rename
leaves it writing into the renamed inode forever while the new file stays empty (measured on macOS 26.2
against a live writer). Compress to a `.partial` sibling and rename it into generation `.1` so an
interrupted compress never leaves a half archive the next pass mistakes for a good one, shift oldest
first, prune every generation outside `1 <= index <= keep - 1` on both sides (index 0 is how
`newsyslog` numbers its own), refuse a symlink, and never truncate a log whose archive could not be
written. An unwritable log and an unreadable size are named in the record and fail the lane.

**The logs are declared explicitly.** The bash job scanned the whole root. The lane rotates exactly the
paths its `logs` key lists, and the shipped config lists every `StandardOutPath` the tracked
LaunchAgents write (twelve, once the three retired agents are gone), plus the post-commit graphify log.
A file the list does not name is never touched, which has two consequences the operator accepted: a
new LaunchAgent's log has to be added to the list by hand, and the leftovers of retired tools under the
root (`gha-watcher.log`, `paseo-daemon.log.1.gz`) are the operator's to delete. The root-owned osquery
daemon logs the bash job reported as unmanageable every hour are simply not listed.

**The cadence becomes weekly**, because uu is weekly and the rule is that a lane runs in uu's run. Each
log is then bounded at the threshold plus one week of growth rather than one hour. Measured on
2026-09-06, the fastest-growing declared log is `yt-dlp/pot-provider.log`, whose five archives are
dated one day apart, so it reaches the 10 MiB threshold about daily and will sit near 80 MiB before a
Sunday rotation; every other declared log grows under a megabyte a week. With `archives_kept = 5` the
history kept becomes five weeks instead of five hours, which serves the second reason better than the
hourly job did. The alternative, an hourly LaunchAgent running `uu run rotate-logs`, was rejected: a
single-lane run posts a record and moves the marker like any other, and an hourly record is noise the
channel exists to not carry.

**Record row.** Each log rotated with its size, each skipped as under threshold (counted, not named),
and each failure with its reason.

**Verdict: APPLY.**

## `uu bootstrap <lane>`

Two of the ported jobs have an apply-time step that is not a weekly run: the skills lane installs any
roster skill the store lacks (the bash job's `--install-only`), and the plugin record seeds its
baseline. `uu bootstrap <lane>` runs that step for one lane, under the run lock, and posts no record,
moves no marker and counts no streak, because nothing was compared and a manual run must never make a
dead schedule look alive. It exits 0 when the step completed and 1 when it did not, which is all the
chezmoiscripts that call it need for their retry markers. A lane whose type has no bootstrap step is
refused with exit 1 and a sentence naming the type, so a misspelled lane in a loader is loud rather
than a silent no-op. It is the one new verb this design adds.

## The config, in one place

The shipped template (`dot_config/uu/private_config.toml.tmpl`) gains the blocks below, in the house
style: every defaulted key written out at its default, opt-in features commented out, `type` selecting
the implementer, and lanes in the name order they run in. `[schedule]`, `[alerts]` and the four
shipped lanes are unchanged and not repeated. `HOME` stands for the rendered home directory and `SRC`
for the chezmoi source directory.

```toml
[records]
url = "http://127.0.0.1:8644/webhooks/unattended-upgrades"
key = "..."
# A second signed POST for every alarm uu raises (a failed lane, a stale lane,
# a lost record, updates pending past the escalation point). The URL alone opts
# in; the empty string is off, the one key here where empty is not refused.
failure_webhook = ""

# cargo-installed binaries. Registry crates are compared against crates.io;
# git-pinned entries are skipped by name. With compile = false a crate that is
# behind is reported with the exact command to compile it and the lane is
# pending; compile = true runs that command per crate.
[lanes.cargo]
type = "cargo"
cargo = "HOME/.cargo/bin/cargo"
compile = false
escalate_after_runs = 3
deadline_secs = 21600

# What Claude Code's own plugin auto-update changed since last week, from
# ~/.claude/plugins/installed_plugins.json. The first reading is the baseline;
# `uu bootstrap claude-plugins` seeds it at apply time.
[lanes.claude-plugins]
type = "claude-plugins"
inventory = "HOME/.claude/plugins/installed_plugins.json"
deadline_secs = 21600

# Mason's linters, formatters and debug adapters (mason-tool-installer's
# ensure_installed). Language servers are Mason's own and are upgraded in :Mason.
[lanes.nvim-mason]
type = "nvim-mason"
nvim = "/opt/homebrew/bin/nvim"
config = "HOME/.config/nvim"
deadline_secs = 21600

# Treesitter parsers, reconciled to the revisions the locked nvim-treesitter
# pins. Compiles C, so it is the slowest of the Neovim lanes.
[lanes.nvim-parsers]
type = "nvim-parsers"
nvim = "/opt/homebrew/bin/nvim"
config = "HOME/.config/nvim"
deadline_secs = 21600

# Which lazy.nvim pins have moved upstream. REPORT ONLY: lazy-lock.json is
# committed in the dotfiles repository and the operator bumps it through a pull
# request. auto_commit = true instead updates the live tree and commits the new
# lock into `repo` (never pushes); it requires a clean lock file and a branch.
[lanes.nvim-plugins]
type = "nvim-plugins"
nvim = "/opt/homebrew/bin/nvim"
config = "HOME/.config/nvim"
auto_commit = false
repo = "SRC"
escalate_after_runs = 3
deadline_secs = 21600

# Does the config still start on the candidate plugin versions? Installs them
# into a second plugin tree under `cache` (about 420 MB, measured 2026-09-06,
# kept between runs so a week costs a fetch), runs checkhealth and dumps every
# keymap. Delete the directory to reclaim the space, comment the block out to
# stop paying it.
[lanes.nvim-smoke-test]
type = "nvim-smoke-test"
nvim = "/opt/homebrew/bin/nvim"
config = "HOME/.config/nvim"
cache = "HOME/.cache/uu/nvim-smoke-test"
deadline_secs = 21600

# Bound every declared log at rotate_at_bytes plus a week of growth, keeping
# archives_kept compressed generations. Only the paths listed here are touched.
[lanes.rotate-logs]
type = "rotate-logs"
rotate_at_bytes = 10485760
archives_kept = 5
compressor = "/usr/bin/gzip"
logs = [
  "HOME/.local/log/atuin-daemon.log",
  "HOME/.local/log/graphify/dotfiles-post-commit.log",
  "HOME/.local/log/happy-daemon.log",
  "HOME/.local/log/osquery/alert-drainer.log",
  "HOME/.local/log/osquery/digest.log",
  "HOME/.local/log/osquery/firewall-gatekeeper-monitor.log",
  "HOME/.local/log/osquery/heartbeat.log",
  "HOME/.local/log/osquery/results-alerter.log",
  "HOME/.local/log/osquery/tailscale-monitor.log",
  "HOME/.local/log/osquery/uptime-watchdog.log",
  "HOME/.local/log/pns-daemon.log",
  "HOME/.local/log/uu/uu.log",
  "HOME/.local/log/yt-dlp/pot-provider.log",
]
deadline_secs = 21600

# Every toolchain rustup manages, updated. Safe because the four crates this
# repository builds at apply time pin `channel = "stable"` in rust-toolchain.toml.
[lanes.rustup]
type = "rustup"
rustup = "HOME/.cargo/bin/rustup"
deadline_secs = 21600

# The cross-harness skills store: one live generation, a candidate built and
# validated whole, published with one atomic exchange, fanned out to Claude
# and the hermes profiles. `uu bootstrap skills` installs absent roster skills
# at apply time.
[lanes.skills]
type = "skills"
lock = "HOME/.agents/custom-skill-lock.json"
agents = "HOME/.agents"
claude_skills = "HOME/.claude/skills"
hermes = "HOME/.hermes"
npx = "HOME/.local/share/fnm/aliases/default/bin/npx"
skills_cli_version = "1.5.22"
clawhub = "HOME/.local/share/fnm/aliases/default/bin/clawhub"
hermes_cli = "HOME/.local/bin/hermes"
cua_driver = "/opt/homebrew/bin/cua-driver"
routing = "HOME/.local/libexec/unattended-upgrades/agent-skills/assert-hermes-superpowers-routing.sh"
deadline_secs = 21600
```

The lanes then run in this order: `brew`, `cargo`, `claude-plugins`, `herdr`, `npm`, `nvim-mason`,
`nvim-parsers`, `nvim-plugins`, `nvim-smoke-test`, `rotate-logs`, `rustup`, `skills`, `uv`. Nothing
in that order matters except the four Neovim lanes, covered above, and rotation running after the lanes
that write most of the logs it rotates, which it does.

`uu doctor` prints every new lane the way it prints a command lane today: on, its type, and whether the
program it drives (`nvim`, `cargo`, `rustup`, `npx`, the compressor) resolves, with the standing note
that doctor resolves on this shell's PATH and the weekly run on the plist's. The pending file and the
escalation threshold join the doctor's per-lane line.

## What the record says

The entry keeps its shape: `run at <iso> on <host>`, the gap sentence, one block per lane in name
order with the lane's verdict line (`N failure(s)`, `deferred` or `pending`) and its own rows, and the
closing `=== done, N failure(s), N deferred, N pending ===`. The rows per lane are the record rows
above; in one place:

| Lane              | Rows                                                                            |
| ----------------- | ------------------------------------------------------------------------------- |
| `cargo`           | each crate behind (the sentence with the command), current, skipped, built      |
| `claude-plugins`  | the change section with its caveat, or the baseline notice                      |
| `nvim-mason`      | updated, current, failed with Mason's reason, the servers sentence, restart     |
| `nvim-parsers`    | updated, current, failed with the compiler tail, restart                        |
| `nvim-plugins`    | pins moved (count and names), plugins with errors, the commit when committed    |
| `nvim-smoke-test` | pass or fail, health ERROR and WARNING counts, mapping count and diff, paths    |
| `rotate-logs`     | each log rotated with its size, the under-threshold count, failures             |
| `rustup`          | each toolchain from and to, rustup itself                                       |
| `skills`          | the change section, the generation, failed skills, hermes phase, forks, routing |

## What this deliberately does not do

It never pushes and never opens a pull request; `auto_commit` stops at the commit. It does not manage
language servers. It has no dry-run mode. It does not rotate hourly and does not scan the log root. It
does not apply a Neovim plugin bump unless `auto_commit` is on. It keeps no per-skill failure streaks.
And it adds one verb, `bootstrap`, and no other command-line surface.
