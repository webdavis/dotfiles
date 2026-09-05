---
name: clean-code
description: "The language-neutral architecture standard for the custom tools this repository owns. Use when restructuring a tool into layered modules, deciding boundaries or where a seam goes, designing a versioned protocol between a tool and its callers, choosing between a database and the filesystem for durable state, planning the pull-request ladder for a large refactor, or reviewing work against SOLID, file-size and test-quality standards. Always pair it with the language skill for the tool at hand."
---

# Clean code

The method for building and restructuring a custom tool this repository owns. It is deliberately
language-neutral: the ladder, the boundaries, the test obligations and the delivery rules are the
same whatever the tool is written in.

**Read a language skill alongside this one. Always.** This skill states what must be true; the
language skill states how that is spelled and enforced in a particular toolchain:

- **Rust**: `~/.agents/skills/clean-code-rust/SKILL.md`
- **Swift**: `~/.agents/skills/clean-code-swift/SKILL.md`

Those paths are the canonical store and resolve identically from Claude Code, Codex and hermes. Where
this skill and a language skill disagree on a number or a mechanism, **the language skill wins**: it
was written against that toolchain's real behavior.

## The three references

Consult these; do not read them front to back.

- [`ARCHITECTURE.md`](ARCHITECTURE.md): the module roles and what belongs in each, the extension
  model, SOLID, typed outcomes, public API discipline, and the file-size standard.
- [`TESTING.md`](TESTING.md): test-first versus a pure move, mutation verification by hand, the speed
  gate, the rule that nothing reaches a real destination, and the test levels.
- [`PERSISTENCE.md`](PERSISTENCE.md): classifying durable state, a database versus the filesystem,
  delivery safety, configuration, and secrets.

## Scope of the work

Treat mature code as behaviorally mature and structurally concentrated. The existing tests carry
product, compatibility, privacy, concurrency, process-lifecycle and failure-direction
specifications. Preserve those. Do not preserve accidental internal structure merely because the
current implementation, or a mechanism-specific test, asserts it.

This is not a request for the smallest incremental cleanup. Implement the best final architecture.
Sequence the work so it stays verifiable, but do not stop after introducing facades, moving a few
helpers, or preparing a future migration.

Where the tool is planned to move to its own repository, design it as a standalone package now:
nothing outside its own folder may dictate its shape, and it may contain no path that reaches outside
that folder. Code outside the folder may be changed whenever the tool's design needs it, and every
such change lands in the same pull request as the change that needs it, with the reason named. Do not
perform the repository move itself.

## Engineering priorities

In this order:

1. Correct observable behavior and safety invariants.
2. Stable compatibility and protocol contracts.
3. Deterministic, meaningful tests.
4. Dependency direction and architectural boundaries.
5. Cohesion and clear ownership.
6. Small source files.
7. Reuse through demonstrated abstractions.
8. Performance improvements supported by evidence.

Do not damage a higher-priority property to satisfy a lower-priority metric such as line count,
interface count, or coverage percentage.

## Step 1: enumerate the consumers outside the folder

The tool leads and its consumers follow. Before touching anything, enumerate them and, for each, name
what may change, what stays fixed, and how to prove it. **Verify by running the real command, never by
reasoning about it.**

The usual set in this repository:

- **The chezmoi builder script** that compiles and installs the binary. Its build line and paths
  change with the layout; the deployed contract (where the source deploys, where the binary installs,
  and that the build runs from a committed lockfile) does not.
- **The justfile recipes** that invoke the build system with an explicit manifest path. Update them
  to the new shape and read which units ran, to prove every one is covered.
- **Any sibling package that depends on the tool by path.** Do not keep a legacy import path alive
  behind a facade: put the shared code where it belongs, update the sibling's manifest and imports in
  the same pull request, and add the sibling's own test command to your gates. The boundary you draw
  now is the one that sibling consumes once the tool moves repositories.
- **The command-line surface**, when it is a compatibility contract. Grep for every in-repo
  invocation of the installed binary path before touching argument handling. Some callers are not
  yours to change: a generated third-party config holding one pathname cannot be given a subcommand.
- **Any generated file the tool renders.** Regenerate it through its recipe whenever rendering
  changes, in the same pull request, and name the change in the report.

Freeze the tool's own backlog for the duration. Every open item is either absorbed as a named design
decision or re-filed against the new structure in the completion report. None is dropped silently.

## Step 2: establish the behavioral specification

Before moving production code, inventory the externally meaningful behaviors the tests and source
comments currently specify.

Record the baseline as the **set of leaf test names with their results, never a count**. A count
passes when one test is dropped and another added, and it passes a rename. Keep a table mapping every
permanent-contract test to its successor by name; a removed test appears in that table with its
reason.

Write concise specifications under `docs/specs/` and decision records under `docs/decisions/`, both
**inside the package**, so they travel with it when it is extracted.

Express each use case as observable scenarios (`Given` / `When` / `Then`), and for each identify:

- success behavior
- every meaningful failure source
- fail-open or fail-closed direction
- exact threshold behavior, and one step either side of each threshold
- required side effects and forbidden side effects
- cancellation or timeout behavior
- idempotency and duplicate behavior
- privacy requirements
- process ownership and cleanup
- which output and exit-code details are compatibility contracts

Do not invent new product behavior without identifying it as a deliberate design decision. Move long
historical explanations and measured investigation narratives into `docs/decisions/`. Keep concise
comments in production and tests stating the governing invariant, linking to the decision record when
more history helps. Do not delete rationale to reduce line count.

## Step 3: build the glossary from the source

Name modules and types after the tool's own concepts, capabilities, use cases, policies, protocols
and adapters. Derive the vocabulary from the source's own names during the specification step. Where
a circulated vocabulary list and the code disagree, **the code wins**: check each term against the
source before adopting it, because a term that names nothing in the code is either a new concept you
are introducing deliberately or a mistake.

Avoid `manager`, `processor`, `handler`, `service`, `helpers`, `utils`, `common` and `misc` where a
more precise domain name exists. They are not categorically forbidden; use one only when it
accurately describes a recognized role and nothing more precise exists. Do not import naming from
unrelated sample applications.

## Step 4: the ordered procedure

The final result must implement the full target architecture. This sequence keeps it correct along
the way:

1. Run and record the complete baseline suite, as a set of test names with results.
2. Extract the behavioral specifications into `docs/specs`.
3. Classify every existing test as a permanent behavioral contract, an adapter contract, an obsolete
   implementation-mechanism test, or a migration test.
4. Create the module boundaries and their dependency direction, updating the builder, the justfile
   and every dependent sibling in the same pull request, proved by running their commands.
5. Move pure policy into the domain module, with no infrastructure dependencies.
6. Define application use cases and consumer-owned ports.
7. Define the versioned protocols test-first.
8. Implement legacy command-line and hook adapters over the new use cases.
9. Implement real registries and remove central name-based dispatch.
10. Separate any stateful indicator into policy and infrastructure.
11. Introduce semantic repositories and their persistence.
12. Migrate or intentionally preserve existing durable state.
13. Split configuration parsing, validation, schema, rendering, setup and publication.
14. Move system, network, filesystem and process behavior into adapters.
15. Reduce the executables to command adaptation and composition.
16. Split unit and acceptance tests by behavior.
17. Remove obsolete compatibility code and mechanism tests.
18. Run every quality gate and verify file-size compliance.

Do not stop after creating new interfaces while the old modules still own the behavior. Do not leave
duplicate old and new architectures indefinitely. Do not write TODO-only adapters, placeholder use
cases, or unused protocols.

## Delivery: the pull-request ladder

The end state is not negotiable; how it lands is an ordered ladder of pull requests to `main`, one or
more per step of the procedure above. Every pull request:

- builds with the builder's build line as it stands after that pull request, and passes the tool's
  test recipe, `just lint-check`, and every dependent sibling's tests;
- leaves `main` deployable, because the builder rebuilds the binary on every apply;
- passes an **argument-surface differential** against the binary built from the previous `main`, over
  the frozen command-line surface, **with a control mutant the differential is shown to catch**. A
  differential without a failing control proves nothing: a previous harness compared a file with
  itself and reported zero mismatches against a broken binary;
- states which kind of work it is, new behavior or a pure move, and gives the matching evidence (see
  [`TESTING.md`](TESTING.md));
- stays small enough to review. Decompose by behavior before starting.

Old and new structure may coexist between pull requests, never indefinitely. No pull request may
leave a large module owning behavior that a new module claims to own.

## Quality gates

Run the repository's own commands, whose text may change in the pull request that changes the layout
they describe. What may not change is that each keeps covering every unit:

    just lint-check
    just ship

plus the tool's own test recipe, the test command of every dependent sibling, the builder's own build
line, and the regenerate-and-diff check for any generated file the tool renders. The language skill
names the exact recipe and the compiler-level gates.

`just lint-check` is the drift gate CI runs. treefmt has no dry run, so a red gate has already
written its fixes into the tree: stage them and rerun.

Do not add broad lint suppressions. A suppression must be narrow and explain why the lint is
incorrect at that specific location. **Do not claim a command passed unless it actually ran
successfully.**

## The sol review

Every pull request in a refactor of this size gets a `sol` review at ultra reasoning in addition to
the pipeline's own steps. Its scope is fixed: SOLID adherence, abstraction quality, composability,
test quality, and test environment quality. Correctness follows where it derives from those; style
nits do not.

    codex exec --model gpt-6-astra -c model_reasoning_effort=ultra --sandbox read-only "$(cat <prompt>)" </dev/null

The stdin redirect is load-bearing: without it the call hangs forever. The prompt carries the step's
diff, what the step is deliberately not doing, the target architecture, and the evidence the step
offers, and it asks sol to **disagree with the architecture where it thinks the architecture is
wrong** rather than to validate it. The verdict is recorded in the slice's findings register as a
review step like any other.

## Completion report

Report:

1. The final dependency graph.
2. The behavior specifications created.
3. Each protocol, including its versioning and compatibility policy.
4. How each legacy entry point maps into the normalized request.
5. The extension interfaces and registries introduced.
6. The use cases and consumer-owned ports introduced.
7. Which persistent state moved stores, and to what.
8. Which filesystem protocols remained, and why the filesystem is part of their contract.
9. State and configuration migration behavior.
10. Every removed central switch or name-based dispatch path.
11. Before-and-after line counts for every previously oversized file.
12. Every file remaining above the review threshold, with its justification.
13. Tests added, moved, replaced or removed, as the name-mapping table, including why each
    mechanism-specific test became obsolete. Per pull request: which kind of work it was, and the
    matching evidence.
14. Exact commands executed and their results.
15. The argument-surface differential result, with the control that proves it can fail.
16. Every generated-file change, with its byte diff.
17. What the operator must verify live after their own apply. Agents never apply.
18. Every change made outside the tool's folder, with the need that drove it.
19. Any unresolved behavior or risk.

Do not describe the work as complete while a known oversized module, duplicate architecture, failing
test, protocol ambiguity, or unowned process remains.

## Standing repository rules that apply throughout

- No em-dashes anywhere: code, comments, documents, commit messages.
- Conventional Commits, one logical change per commit, never a `Co-Authored-By` trailer or a
  generated-with footer.
- `trash`, never `rm`, including scratch directories you create yourself.
- Never `git push --force`, never `chezmoi apply`, never `launchctl kickstart` or `bootout` a real
  agent. The operator runs applies and live drills; agents never verify a binary against real
  destinations.
