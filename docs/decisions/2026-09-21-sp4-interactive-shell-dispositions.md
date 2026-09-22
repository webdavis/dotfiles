# SP4 interactive shell dispositions

SP4's scope was set in the repository modernization roadmap
(`docs/superpowers/specs/2026-07-02-repo-modernization-roadmap-design.md`) and detailed in
`docs/superpowers/plans/2026-07-03-sp2-combine-and-split.md` as five workstreams. Two of them were
carried out; three were questions this page answers rather than code. It also reconciles three inherited
premises that have moved since the plan was written.

## The five workstreams

| #   | Workstream                                                                                          | Outcome                                                                                                                                 |
| --- | --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Consolidate every alias out of `~/.bashrc` into `~/.bash_aliases`                                   | Done. 22 of the 27 declared inline moved; five need a template conditional and stay.                                                    |
| 2   | `dot_fzf_bindings` quality plus five new selection bindings                                         | No new selector built. Each candidate's disposition is below.                                                                           |
| 3   | Invert the bindings architecture: one table generating the `bind` calls, an fzf menu, and the tests | Done. The table and the `bind` calls shipped earlier as `chord render bash`; the menu is `chord render menu` plus the rewritten picker. |
| 4   | Record a Charm-tools verdict                                                                        | Recorded before SP4 and unchanged. See below.                                                                                           |
| 5   | Generated per-binding tests, registration plus firing                                               | Dropped, not satisfied. See below.                                                                                                      |

## The five selection candidates, each declined

The ledger entry asked for these to be revisited "without treating every candidate as adopted". All five
are declined, because each is either already served or answers a need this machine does not have. The one
selector SP4 did build is the binding picker itself, which is workstream 3's menu.

**Directory selection: declined, already served three times over.** `dot_fzf_bindings` carries five
directory selectors (below the working directory including hidden entries, from the git root, from the
filesystem root, to a parent, and to a bookmark). `cd` is aliased to zoxide's frecency-ranked `z`, and
zoxide's own `zi` is an interactive picker over the same database. The binding table adds nine
fixed-destination `cd` rows. A sixth path would compete with all of it.

**Git stash selection: declined, the workflow does not stash.** The evidence is an absence measured
across every place a stash habit would leave a trace: zero of the 176 binding rows mention a stash, zero
of the 14 git aliases do, and none of the 25 fzf binding functions do. Work is isolated per worktree
here, which is what stashing substitutes for on a single checkout. Building a picker for an empty list is
a binding that never fires.

**Process selection: declined, two tools already cover it.** `btop` is installed and is an interactive
process browser that selects and signals; `procs` is installed and aliased over `ps` for the listing. A
third path would be a worse btop. Note also that a process killer is the one candidate where a mis-pick
is destructive, which argues for the tool with a confirmation step rather than an fzf binding that acts
on Enter.

**Worktree selection: declined, worktrunk owns it.** `wt switch` creates or switches in one verb, and
`wt` is a shell function precisely so it can change the calling shell's directory, which an fzf binding
in a subshell cannot do without help. Worktree layout is already reconciled between worktrunk and herdr
at `~/.herdr/worktrees/<repo>/<branch>`; a third writer of that path is how they fall out of agreement.

**Herdr workspace selection: declined, herdr ships a picker.** `workspace_picker` is bound to
`prefix+alt+space` in `dot_config/herdr/config.toml`, and the `herdr-workspace-jump` plugin covers nine
named workspaces on their own chords plus a most-recently-used toggle. The plugin also holds the
authoritative label-to-directory map, so a shell-side picker would need a second copy of it.

## Charm tools and the output style

The Charm verdict recorded with SP4's scope stands unchanged: gum no, bubbletea and lipgloss no, crush
no, vhs optional later, zero new dependencies now.

The reconciliation is a correction of one word rather than a reversal. What this repository adopted is
the "G2" look chosen from gum's rendered samples, a bold 256-color tool name with an optional faint
context and no boxes. It is implemented in `.chezmoitemplates/cli-print-style-lib.sh.tmpl` as three ANSI
SGR sequences and pulls in nothing. So gum informed a style choice and was not itself adopted; reading
the adopted style as "gum adopted" would invite a dependency the verdict already declined. The two places
`gum` appears in this repository are that library's own explanation of why it is absent and the test that
pins the escape codes.

## The dropped declaration tests

Workstream 5 asked for generated per-binding tests, "registration plus firing", so that a broken binding
failed at commit time. That requirement predates the 2026-08-05 behavior-only test policy and does not
survive it. A per-binding registration assert tests that a declaration exists, which is exactly the class
the policy names and deletes on sight: gutting the renderer's logic while leaving the table intact would
not turn such a test red, because it asserts against the table.

It is dropped rather than satisfied. What covers the same ground by testing our own logic instead:

- `chord check bash` and `chord check menu`, byte-equality of each generated file against what the table
  renders, in `just test-rust`. A renderer whose logic changed fails both.
- The renderer unit tests in the `chord` crate, which pin the rendering of every action kind and every
  mode case against golden text.
- `test/unit/bash-bindings-picker.sh`, which pins the picker's behavior: every record reaches it, and the
  action kind decides what running a selection means.

Firing is deliberately left unguarded, which is the accepted price the policy names. A chord that is
declared but does not fire is caught by the operator pressing it, not by a gate.

## The two shell verdicts

**Nushell: no-go, ratified 2026-07-09, unchanged.** The report is
`docs/research/2026-07-09-sp4-nushell-evaluation.md`. Reedline's one-event-per-binding limit cannot
express a chord grammar, which sinks a binding surface this size on its own. Nothing in SP4 reopens it.

**xonsh: no-go, approved 2026-09-21.** xonsh stays installed as an on-demand subshell and is not the
interactive shell. An evaluation task existing is not an adoption; SP4 assumes none.

Both verdicts point the same way for this work: bash is the interactive shell, so the binding table's
only rendered shell target is bash. The table is shell-agnostic anyway, which is what makes a future
shell question cheap rather than a rewrite, and that was the standalone argument for workstream 3.
