# The ledger grammar

What the two verifier scripts accept. This is shared by all three strategies; each one's SKILL.md states
only its own deltas.

**The scripts are the contract and this file is a guide to them.** Where they disagree, the script wins,
because the script is what refuses the merge. Read them when the stakes are high; they are short:

- `~/.claude/pipeline/slice-checklist.sh` proves every STEP ran.
- `~/.claude/pipeline/findings-register.sh` proves every FINDING was resolved.

Both know exactly two strategies, `A` and `F`, which are the letters `open-loop` and `closed-loop` used
to be called. There is no third letter, so `orchestrator-loop` runs the ledgers as `F` and deviates the
step it does not have.

## Opening the ledgers

```bash
~/.claude/pipeline/slice-checklist.sh  new <slug> <A|F> [--security] [dir]
~/.claude/pipeline/findings-register.sh new <slug> <A|F> [dir]
```

Neither overwrites an existing file, so a second `new` on a slug that already has ledgers is an error
rather than a reset.

`--security` adds step 4a-s, a security lens. Pass it when the slice touches authentication,
credentials, secrets, a privilege boundary or untrusted input.

Careful: `slice-checklist.sh steps <A|F>` PRINTS 4a-s by default, while `new` OMITS it unless
`--security` is given. The slice is judged against what `new` wrote, so the printed table over-reports.
`steps <A|F> no` prints the non-security form.

## What the checklist accepts

Tick a box only when its EVIDENCE names something that exists. The verifier resolves evidence against
reality rather than pattern-matching its shape, because a checklist of invented references
(`wf_deadbeef "VERDICT: CLEAN"`) once printed "Clear to merge".

- `wf_<id>`: a transcript directory under `~/.claude/projects` must exist.
- a 7 to 40 character hex token: `git cat-file -e <sha>^{commit}` must succeed. Pass `--repo`, or no
  commit token can resolve and every commit-backed box fails.
- a path (containing `/`, or ending `.md`, `.txt`, `.sh`): the file must exist.

REVIEW steps additionally require a QUOTED VERDICT: the evidence must contain `VERDICT:`, a JSON
`"verdict":`, `NO_ISSUE`, `NEW_ISSUE` or `INCOMPLETE`. An agent id, a workflow id or the word "clean" is
rejected on its own. Quoting is what forces reading, and reading was the part actually missing when a
HIGH defect merged behind a completed review nobody opened.

The review steps are 2, 4a, 4a-s, 4b, 6v and 7. The script's own header comment lists five and omits
6v; the step table it enforces marks 6v a review, and the table is what runs.

```bash
~/.claude/pipeline/slice-checklist.sh verify <dir>/checklist-<slug>.md --repo <repo>
```

## What the register accepts

**Rows must begin with `| F`.** The verifier reads only lines with that prefix, so a finding numbered
`| 1 |` is invisible to it and evaporates silently. Use `F1`, `F2`, and so on. Any row whose summary
contains "delete this row" is skipped, which is how the generated example excuses itself.

Columns are `| id | step | severity | summary | disposition | evidence |`. Disposition is exactly one of:

- **FIXED**. Evidence needs three things together: a commit sha, a NAMED test, and the literal
  transition `RED ... GREEN` or `SURVIVED ... KILLED`. A test is named by path (`test/...`, `.sh`,
  `.bats`, `.rs`) or by name (`test some_function_name`, or `test "a quoted sentence"`). Two transitions
  because there are two kinds of finding: RED to GREEN closes a behaviour defect, SURVIVED to KILLED
  closes a test-quality defect where the code was already right and the test could not fail.
- **ACCEPTED**. Evidence is the written rationale.
- **FIXED-NOTEST**. A narrow escape for a fix with no surface a test can reach (a corrected comment, a
  regenerated artifact). Needs a commit AND a phrase saying why no test closes it (`no test`,
  `cannot be tested`, `untestable`, `not testable`, `measured`). If a test can be written, the
  disposition is FIXED.
- **TASK #\<n\>**. Strategy-A only. The number must appear in a tasks manifest passed as `--tasks
  <file>`, and whenever any row defers, that manifest is REQUIRED: its absence fails the gate.

The **Declared verdicts** table quotes each review step's verdict exactly as returned, and the verifier
reconciles it against the rows. It reads a count out of the quote: `CLEAN` or `NO_ISSUE` is zero,
otherwise the first `(N)`, otherwise the first `N finding`, otherwise `VERDICT: PASS` is zero. If a
reviewer returns `FINDINGS (5)` and the register holds three rows for that step, two findings vanished
between the review and the ledger, and the gate says so. A verdict it cannot parse fails, so carry the
count alongside a prose verdict (`3 findings`) rather than reshaping the quote. `n/a` in a verdict cell
means that step may have NO rows at all; rows against an `n/a` step are a contradiction and fail.

```bash
~/.claude/pipeline/findings-register.sh verify <dir>/findings-<slug>.md [--tasks <file>]
```

Deferring at least as many findings as you fix prints an OVER-DEFERRAL banner. Loud, not fatal: say why
in the PR body.

## Deviations

A step may be skipped only with a line under the checklist's **Deviations** heading naming it and giving
a reason, its box marked `[DEV]`, and the same reason repeated in its EVIDENCE field. A `[DEV]` box with
an unfilled evidence field fails exactly like an unticked one. An unexplained gap is a process failure,
not a shortcut.

## Merging

```bash
~/.claude/pipeline/pipeline-merge.sh <pr> <slug> <A|F> --repo <repo> --dir <dir> [--tasks <file>]
```

It runs both gates, refuses on either, and on success posts their output as a PR comment. It does not
merge for you: it prints the merge command to run. The comment is the artifact, so a PR merged around
the gate is visible afterwards by carrying none.
