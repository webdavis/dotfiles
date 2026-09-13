# Neovim overhaul acceptance

Audit date: 2026-09-13. Source: `adf14ff2`, the installed Neovim configuration, the September 1 overhaul
plan's task 63 and design sections 9.1 and 10. This records task 65 in
[`remaining-work.md`](../remaining-work.md). Acceptance remains incomplete.

## Verified checks

| Check            | Result                                                                                                                                                                                                   |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Installed editor | Neovim 0.12.5, LuaJIT 2.1.1788460057; Xcode 26.6, build 17F113.                                                                                                                                          |
| Plugin pins      | All 93 source lock entries match the deployed lock and installed Git revisions.                                                                                                                          |
| Lua suite        | 380 checks passed with private socket fixtures. Aggregate duration was 19.15 seconds; this does not certify each leaf's latency.                                                                         |
| Silent starts    | Five source and five installed-copy headless starts, each with empty stderr.                                                                                                                             |
| Deferred loading | VimEnter plus VeryLazy loaded all declared VeryLazy plugins, including which-key and claudecode, with no captured error or missing plugin.                                                               |
| Health           | `Lazy! load all` preceded the complete 40-section health capture. Exceptions are classified below.                                                                                                       |
| Language servers | Actual initialization and buffer attachment for lua_ls, bashls, gopls, basedpyright, rust_analyzer, sourcekit and clangd against private projects. All had empty stderr and matched their private roots. |
| Parsers          | Lua, Bash, Go, Python, Rust, Swift and C parsers resolved in the fixture.                                                                                                                                |
| Installed pns    | Version 0.1.0; help includes the flags the plugin emits. No notification was sent.                                                                                                                       |

Checks used private copies of configuration and installed plugin, Mason and parser data. Home, cache,
state, data and temporary paths were private, and agent/Herdr connection context was removed. Long socket
paths and sandbox bind refusals were corrected in the test environment before the unchanged suite passed.
No source defect was reproduced, and no package or live configuration was changed.

Python also attached pyright and ruff as currently configured. Sourcekit used a private Swift package;
attachment alone does not establish UIKit/Vapor semantic behavior, builds or formatting. The current
`vim.lsp.config()` and `vim.lsp.enable()` configuration agrees with installed help and the maintained
[Neovim language server documentation](https://neovim.io/doc/user/lsp.html#lsp-config). A framework
migration is unnecessary for this acceptance task.

## Performance

Five synthetic warm starts measured 94.373 ms for source and 95.845 ms for the installed copy. The
retained import-day baseline was 164.178 ms. Although the arithmetic meets `after < baseline - 10`, the
performance gate remains open: agents were active, paths differed from the fixed benchmark environment,
and no cold measurement ran. These timings are advisory. Record a quiescent comparison, cold timing and a
rendered Herdr start with every VeryLazy plugin loaded separately.

## Health and deployment differences

| Observation                                                          | Disposition                                                                                                                                                                                                                            |
| -------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Atlas/Octo authentication absent                                     | Private-home fixture omitted credentials. Live authentication was not tested.                                                                                                                                                          |
| Optional luarocks/hererocks environment missing                      | Already excluded by the plan.                                                                                                                                                                                                          |
| ESLint reported missing globally                                     | The configured generator resolves only a project's `node_modules/.bin/eslint`. Both absent and executable fixture cases were verified. The installed health checker inspects a different option location; no global install is needed. |
| pns missing under the private home                                   | Installed binary was checked separately through help and version.                                                                                                                                                                      |
| Snacks input/select/setup errors                                     | The health run had no UIEnter. Rendered checks remain.                                                                                                                                                                                 |
| Optional graphics, TeX, diagram, lazygit or terminal-protocol checks | Retain the plan's documented exclusions.                                                                                                                                                                                               |
| Xcode remote-debugger helper absent                                  | Deliberately omitted by the design.                                                                                                                                                                                                    |
| Overseer filename differs                                            | Source `literal_run_script.lua` and deployed `run_script.lua` have identical contents. Reconcile the target during operator deployment.                                                                                                |
| Neovim instructions and task notes are stale                         | Reconcile their eager-loading, neotest, annotation and autosave/format claims against current source.                                                                                                                                  |
| nvim-mcp reports a dirty build                                       | Installed 0.7.2 reports revision `0b5ace3b0369801c9bcb8eec68864427e6b1599c`; clean build provenance was not re-established.                                                                                                            |

Current which-key declarations are `x = xcode`, `X = diagnostics/quickfix`, `d = docker`, `A = herdr`,
`C = claude`, `t = test` and `L = lazy`. This corrects the stale expected groups for the rendered check.

## Remaining acceptance

- Verify rendered startup, which-key navigation and buffer-local key interactions.
- Run both real agent loops with unsaved Neovim buffers, including ClaudeCodeSend, annotations/paste and
  blocked-agent refusal.
- Verify UIKit/Xcode and Vapor semantic, build/test and on-save formatting behavior.
- Verify custom-plugin notification delivery and rendered ledger/annotation interactions.
- Complete fresh-user bootstrap/full apply and a quiet repeat apply. Cloned installed data is not a fresh
  installation.
- Finish the inventory-to-merged-pull-request acceptance mapping and the quiescent performance check.

The private audit receipt, complete pin inventory, health capture and exact probes are retained at
`/private/tmp/dotfiles-modernization/nvim-task65-1789297470562862000/`. The operator runs applies and
session/device checks. Headless checks do not close those items.
