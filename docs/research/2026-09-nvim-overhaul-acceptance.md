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
| Neovim instructions and task notes were stale                        | Source notes now describe lazy defaults, configured adapters and autosave/format behavior. The inventory mapping below records the annotation plugin extraction.                                                                       |
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
- Complete the quiescent performance check. Inventory accounting is complete below; its open acceptance
  and language-coverage items remain open.

The private audit receipt, complete pin inventory, health capture and exact probes are retained at
`/private/tmp/dotfiles-modernization/nvim-task65-1789297470562862000/`. The operator runs applies and
session/device checks. Headless checks do not close those items.

## Inventory and merge receipts

Documentation reconciliation date: 2026-09-13, against source `b078de0f` and documentation base
`706a3143`. The September 1 inventory has 78 numbered entries, decisions A through H and four custom
items. All 90 are accounted for below. The
[canonical design appendix](../superpowers/specs/2026-09-01-nvim-overhaul-design-v4.md#13-appendix-a-inventory-item-to-section-and-pull-request)
uses planned PR (pull request) labels such as `PR 7a`; the links below name actual GitHub pull requests.

GitHub merge state, commit and body were read through gh-axi. Every linked merge commit is an ancestor of
this documentation base. These are implementation receipts, with historical verification attributed to
their pull-request bodies. This pass did not rerun their probes. The private audit above remains the
source for the 93-pin, 380-check and seven-language-server results; neither the merges nor this mapping
close the operator checks.

The old task list now describes configured behavior in
[`dot_config/nvim/docs/todo.md`](../../dot_config/nvim/docs/todo.md), and the local instructions describe
`defaults.lazy = true`. The separate language and routing follow-ups remain in
[`remaining-work.md`](../remaining-work.md). No configuration, binding or plugin changed in this pass.

| Item      | Source disposition and merge receipt                                                                                                                                                                                                      |
| --------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1         | GitHub username read repaired: [#303][pr303]; dereference guards followed in [#341][pr341].                                                                                                                                               |
| 2         | Duplicate `<C-g>bc` definition removed: [#318][pr318].                                                                                                                                                                                    |
| 3         | `OverseerWatchRun` binding repaired: [#305][pr305].                                                                                                                                                                                       |
| 4         | Default-branch string/table contract repaired: [#303][pr303].                                                                                                                                                                             |
| 5         | GitHub fallback format arity repaired: [#303][pr303].                                                                                                                                                                                     |
| 6         | Duplicate delegate setup became moot when delegate was deleted: [#315][pr315].                                                                                                                                                            |
| 7         | Duplicate checktime autocmd removed with the reload mechanism: [#307][pr307].                                                                                                                                                             |
| 8         | Struck conflict premise; the separate boole removal landed in [#328][pr328].                                                                                                                                                              |
| 9         | First commit word preserved: [#300][pr300].                                                                                                                                                                                               |
| 10        | Delegate Escape defect became moot with deletion: [#315][pr315].                                                                                                                                                                          |
| 11        | Unread harpoon v1 width setting deleted, rather than deferred: [#319][pr319].                                                                                                                                                             |
| 12        | Preview host derived from configuration: [#319][pr319]. Off-machine preview acceptance is not established by this audit.                                                                                                                  |
| 13        | Per-server settings registered through `vim.lsp.config`: [#298][pr298]. Private clangd and lua_ls attachment verified above.                                                                                                              |
| 14        | Struck none-ls crash premise; no repair required by the inventory.                                                                                                                                                                        |
| 15        | Dead Overseer bundles/log configuration removed: [#319][pr319]; full configuration followed in [#359][pr359].                                                                                                                             |
| 16        | hlslens deprecation fix pinned: [#317][pr317].                                                                                                                                                                                            |
| 17        | Catppuccin pin/name changed: [#317][pr317]; transition fallback [#331][pr331] and review corrections [#343][pr343].                                                                                                                       |
| 18        | Unused noice `inc_rename` preset disabled: [#325][pr325].                                                                                                                                                                                 |
| 19        | Reflection wrapper replaced with the explicit error boundary: [#297][pr297].                                                                                                                                                              |
| 20        | Helper error contracts corrected: [#297][pr297].                                                                                                                                                                                          |
| 21        | Blame mappings moved to gitsigns attachment: [#329][pr329]; unsaved-buffer and directory corrections [#343][pr343].                                                                                                                       |
| 22        | Informational noice comment; no implementation or acceptance check required.                                                                                                                                                              |
| 23        | Unused cspell dependency removed: [#326][pr326]. Codespell remains commented out in current source.                                                                                                                                       |
| 24        | gitmoji removed: [#326][pr326]. Commit-buffer emoji completion corrected in [#335][pr335].                                                                                                                                                |
| 25        | nvim-notify removed: [#326][pr326]; Snacks remains the notification backend.                                                                                                                                                              |
| 26        | gv.vim removed: [#318][pr318].                                                                                                                                                                                                            |
| 27        | git-messenger removed with static and walkable blame replacements: [#318][pr318].                                                                                                                                                         |
| 28        | Blame capabilities remapped before plugin removal: [#329][pr329]; corrections [#343][pr343].                                                                                                                                              |
| 29        | Telescope removed, Octo uses Snacks: [#328][pr328]. Rendered buffer-local picker/key acceptance remains.                                                                                                                                  |
| 30        | Boole augends moved to dial and boole removed: [#328][pr328]; corrections [#343][pr343].                                                                                                                                                  |
| 31        | claudecode.nvim with provider `none`: [#337][pr337]; startup trigger [#369][pr369] and quiet startup [#372][pr372]. Real session acceptance remains.                                                                                      |
| 32        | Snacks selected; fzf-lua not added: [#328][pr328].                                                                                                                                                                                        |
| 33        | nvim-surround moved to v4: [#317][pr317].                                                                                                                                                                                                 |
| 34        | No forced none-ls bump required. This decision is a no-op, not a missing merge; subsequent general pin refresh is [#377][pr377].                                                                                                          |
| 35        | Decision retained: dial, markview and toggleterm remain in source. Dial changes are [#328][pr328]/[#343][pr343]; no removal requested.                                                                                                    |
| 36        | Struck as a branch-migration request. Runtimepath correction [#330][pr330]; deliberate later Treesitter pin refresh [#394][pr394].                                                                                                        |
| 37        | Go and gopls declared: [#324][pr324]; private gopls attachment verified above.                                                                                                                                                            |
| 38        | Conform/nvim-lint migration remains deferred by the design; no implementation expected.                                                                                                                                                   |
| 39        | Owned raw-text Herdr send helper, with no vim-slime dependency: [#367][pr367] (stacked [#344][pr344]). Real blocked-agent/key acceptance remains.                                                                                         |
| 40        | External-write reload and re-arming: [#307][pr307]. Real agent-loop acceptance remains.                                                                                                                                                   |
| 41        | claude-tmux is moot under Herdr and was not added.                                                                                                                                                                                        |
| 42        | Autosave skips proposed diff buffers and automatic-write formatting: [#337][pr337]. Real workflow acceptance remains.                                                                                                                     |
| 43        | Swift/Xcode configuration: [#292][pr292] and [#363][pr363]. UIKit/Vapor semantics, build/test and save behavior remain operator checks.                                                                                                   |
| 44        | Neotest core [#334][pr334]; filetype adapters [#338][pr338]; Go correction [#347][pr347]; Bash delivery [#376][pr376] and beta pin [#435][pr435]. Rust/Zig/Java/Elixir remain open follow-ups.                                            |
| 45        | Unchanged source import: [#288][pr288]. Its handoff left archive, metadata removal and operator apply outstanding; this audit does not certify those actions.                                                                             |
| 46        | Lock tracked in [#288][pr288]; update checker disabled in [#294][pr294].                                                                                                                                                                  |
| 47        | LazyVim scaffolding removed and augroups renamed: [#294][pr294].                                                                                                                                                                          |
| 48        | Lazy defaults and triggers delivered through [#351][pr351], [#353][pr353], [#369][pr369], [#382][pr382] and [#386][pr386]; later replay/PATH repairs [#364][pr364]/[#371][pr371]. Quiescent/cold/rendered performance acceptance remains. |
| 49        | Superseded by the explicit helper work in items 54 through 59.                                                                                                                                                                            |
| 50        | Bootstrap preparation [#298][pr298] and installer/verifier [#385][pr385]. Fresh-user full apply and quiet repeat remain open.                                                                                                             |
| 51        | Neovim instructions and local-settings carve-out imported: [#288][pr288]. Formatting admitted in [#301][pr301]; source notes reconciled in this record.                                                                                   |
| 52        | Source-path exclusions delivered with import: [#288][pr288].                                                                                                                                                                              |
| 53        | Struck LazyVim premise; remaining scaffold removed in [#294][pr294].                                                                                                                                                                      |
| 54        | Explicit `try` boundary: [#297][pr297].                                                                                                                                                                                                   |
| 55        | Injected shell runners: [#300][pr300]; argv safety followed in [#314][pr314].                                                                                                                                                             |
| 56        | GitHub fallback moved to the GitHub helper: [#303][pr303].                                                                                                                                                                                |
| 57        | Keymap and Overseer helpers split: [#304][pr304]. The required command closure was retained after its removal failed verification.                                                                                                        |
| 58        | `latest_commit` returns `(table, err)`: [#297][pr297].                                                                                                                                                                                    |
| 59        | Early umbrella-module loading removed: [#314][pr314]; delegate side effect retired in [#315][pr315].                                                                                                                                      |
| 60        | `copy_url_to_clipboard` rename and caller: [#304][pr304].                                                                                                                                                                                 |
| 61        | Delegate deleted: [#315][pr315].                                                                                                                                                                                                          |
| 62        | nvim-mcp resolver decision [#336][pr336]; installation, socket routing and harness registration [#339][pr339]. Real agent-loop acceptance remains.                                                                                        |
| 63        | Buffer-edit guard and resolver diagnostics documented for both harnesses: [#339][pr339]. Real unsaved-buffer loops remain open.                                                                                                           |
| 64        | Agent Client Protocol hedge remains deferred by the design; no implementation expected.                                                                                                                                                   |
| 65        | Import preservation evidence is recorded in [#288][pr288]. Its remaining destructive/operator steps are not certified here; see item 45.                                                                                                  |
| 66        | Owned Lua harness [#297][pr297]; direct runner [#327][pr327]; bootstrap verification [#385][pr385]. Fresh-home execution remains open.                                                                                                    |
| 67        | Pure-helper and injected-runner coverage: [#297][pr297]/[#300][pr300].                                                                                                                                                                    |
| 68        | Treesitter runtimepath report corrected by [#330][pr330]. none-ls executable handling landed in [#335][pr335] and was corrected in [#347][pr347].                                                                                         |
| 69        | Implementation sequence is mapped here, including batching amendments [#309][pr309]/[#333][pr333]. Planned PR labels are not actual GitHub numbers or proof of final acceptance.                                                          |
| 70        | Overall success criteria remain open. [#385][pr385] delivers bootstrap code; the private checks above cover only their stated conditions.                                                                                                 |
| 71        | Lint infrastructure [#283][pr283]/[#301][pr301]; health/config repairs [#330][pr330]/[#335][pr335]/[#347][pr347]/[#398][pr398]. Private checks above passed with named health errors; rendered/live and performance checks remain.        |
| 72        | Open: fresh-user full apply, post-bootstrap checks and quiet repeat. [#385][pr385] explicitly leaves this acceptance outstanding.                                                                                                         |
| 73        | Open: real Claude and Codex unsaved-buffer read/edit loops and ClaudeCodeSend. Implementation is [#339][pr339]/[#337][pr337]; private probes do not close it.                                                                             |
| 74        | Unchanged import and preservation handoff: [#288][pr288]. Operator actions remain subject to items 45/65.                                                                                                                                 |
| 75        | Design reevaluation recorded in the canonical specification and amendments [#309][pr309]. This is a review directive, with no separate feature merge required.                                                                            |
| 76        | Settled decision: a separate program with its own specification and plan. No separate implementation merge required.                                                                                                                      |
| 77        | Workflow source: [#337][pr337] (provider/send/add), [#367][pr367] (launch), [#307][pr307] (reload), [#369][pr369] (server startup). Real workflow acceptance remains; claude-tmux stays moot.                                             |
| 78        | Eight research dispositions retained in the claudecode spec header: [#337][pr337]. No new adoption decision is implied.                                                                                                                   |
| A         | Both channels selected: evaluation [#336][pr336], Model Context Protocol (MCP) registration [#339][pr339], selection push [#337][pr337]. Real loops remain open.                                                                          |
| B         | Octo uses Snacks: [#328][pr328].                                                                                                                                                                                                          |
| C         | Blame capabilities retained through remapping: [#329][pr329]/[#343][pr343].                                                                                                                                                               |
| D         | gopls added: [#324][pr324].                                                                                                                                                                                                               |
| E         | Snake-case helper rename: [#304][pr304].                                                                                                                                                                                                  |
| F         | Autosave, neotest and Xcode are in scope: [#337][pr337], [#334][pr334]/[#338][pr338]/[#376][pr376]/[#435][pr435], [#292][pr292]/[#363][pr363]. Items 42 through 44 retain their acceptance gaps.                                          |
| G         | Settled separate-program decision; see item 76.                                                                                                                                                                                           |
| H         | Unchanged import before modernization: [#288][pr288].                                                                                                                                                                                     |
| custom #1 | Published pns.nvim ownership documented in [#387][pr387]; actual producer wiring [#465][pr465]. Notification delivery acceptance remains open.                                                                                            |
| custom #2 | `ReviewLedger[!]` quickfix reader: [#349][pr349]. Real findings-file navigation and rendered acceptance remain open.                                                                                                                      |
| custom #3 | Annotation composition [#350][pr350] and key [#369][pr369]; extracted published plugin [#391][pr391] owns current implementation. Rendered annotation/paste acceptance remains open.                                                      |
| custom #4 | Resolver row chosen in [#336][pr336] and shipped in [#339][pr339]. Planned PR 10b is skipped because the maintained server plus owned resolver was selected.                                                                              |

The struck premises remain struck: 8, 14, 36 as originally worded, 45's chezmoi.nvim rationale, 48's
eager-spec count and 53. Items 22, 38 and 64 retain their informational or deferred dispositions. No
missing merge is invented for those entries.

[pr283]: https://github.com/webdavis/dotfiles/pull/283
[pr288]: https://github.com/webdavis/dotfiles/pull/288
[pr292]: https://github.com/webdavis/dotfiles/pull/292
[pr294]: https://github.com/webdavis/dotfiles/pull/294
[pr297]: https://github.com/webdavis/dotfiles/pull/297
[pr298]: https://github.com/webdavis/dotfiles/pull/298
[pr300]: https://github.com/webdavis/dotfiles/pull/300
[pr301]: https://github.com/webdavis/dotfiles/pull/301
[pr303]: https://github.com/webdavis/dotfiles/pull/303
[pr304]: https://github.com/webdavis/dotfiles/pull/304
[pr305]: https://github.com/webdavis/dotfiles/pull/305
[pr307]: https://github.com/webdavis/dotfiles/pull/307
[pr309]: https://github.com/webdavis/dotfiles/pull/309
[pr314]: https://github.com/webdavis/dotfiles/pull/314
[pr315]: https://github.com/webdavis/dotfiles/pull/315
[pr317]: https://github.com/webdavis/dotfiles/pull/317
[pr318]: https://github.com/webdavis/dotfiles/pull/318
[pr319]: https://github.com/webdavis/dotfiles/pull/319
[pr324]: https://github.com/webdavis/dotfiles/pull/324
[pr325]: https://github.com/webdavis/dotfiles/pull/325
[pr326]: https://github.com/webdavis/dotfiles/pull/326
[pr327]: https://github.com/webdavis/dotfiles/pull/327
[pr328]: https://github.com/webdavis/dotfiles/pull/328
[pr329]: https://github.com/webdavis/dotfiles/pull/329
[pr330]: https://github.com/webdavis/dotfiles/pull/330
[pr331]: https://github.com/webdavis/dotfiles/pull/331
[pr333]: https://github.com/webdavis/dotfiles/pull/333
[pr334]: https://github.com/webdavis/dotfiles/pull/334
[pr335]: https://github.com/webdavis/dotfiles/pull/335
[pr336]: https://github.com/webdavis/dotfiles/pull/336
[pr337]: https://github.com/webdavis/dotfiles/pull/337
[pr338]: https://github.com/webdavis/dotfiles/pull/338
[pr339]: https://github.com/webdavis/dotfiles/pull/339
[pr341]: https://github.com/webdavis/dotfiles/pull/341
[pr343]: https://github.com/webdavis/dotfiles/pull/343
[pr344]: https://github.com/webdavis/dotfiles/pull/344
[pr347]: https://github.com/webdavis/dotfiles/pull/347
[pr349]: https://github.com/webdavis/dotfiles/pull/349
[pr350]: https://github.com/webdavis/dotfiles/pull/350
[pr351]: https://github.com/webdavis/dotfiles/pull/351
[pr353]: https://github.com/webdavis/dotfiles/pull/353
[pr359]: https://github.com/webdavis/dotfiles/pull/359
[pr363]: https://github.com/webdavis/dotfiles/pull/363
[pr364]: https://github.com/webdavis/dotfiles/pull/364
[pr367]: https://github.com/webdavis/dotfiles/pull/367
[pr369]: https://github.com/webdavis/dotfiles/pull/369
[pr371]: https://github.com/webdavis/dotfiles/pull/371
[pr372]: https://github.com/webdavis/dotfiles/pull/372
[pr376]: https://github.com/webdavis/dotfiles/pull/376
[pr377]: https://github.com/webdavis/dotfiles/pull/377
[pr382]: https://github.com/webdavis/dotfiles/pull/382
[pr385]: https://github.com/webdavis/dotfiles/pull/385
[pr386]: https://github.com/webdavis/dotfiles/pull/386
[pr387]: https://github.com/webdavis/dotfiles/pull/387
[pr391]: https://github.com/webdavis/dotfiles/pull/391
[pr394]: https://github.com/webdavis/dotfiles/pull/394
[pr398]: https://github.com/webdavis/dotfiles/pull/398
[pr435]: https://github.com/webdavis/dotfiles/pull/435
[pr465]: https://github.com/webdavis/dotfiles/pull/465
