# pns delivery failure reporting: the pull-request breakdown

Companion to `docs/superpowers/specs/2026-09-08-pns-delivery-failure-reporting-design.md`. The design is
approved; this file says how it is built and in what order. Read both canonical standards first:
`~/.agents/skills/clean-code/SKILL.md` and `~/.agents/skills/clean-code-rust/SKILL.md`.

Seven pull requests, one per task 30 to 36 in `docs/remaining-work.md`. Each lands green on its own and
leaves the tree shippable, so the ladder can stop anywhere.

## What the tree already has, verified on 2026-09-09

Facts that change the plan, all read out of `main` at `b4e60f5d`:

- **Backoff already landed.** `pns_domain::retry::RetryBackoff` exists with `base_secs: 60` and
  `random_secs: 60`, and `retry_at(now, retries, sample)` adds `base_secs * retries` plus a jitter
  sampled from a 15-bit value. The design keeps the backoff and drops the jitter, so PR 30 removes
  `random_secs` rather than adding the whole type. `pns-adapters/src/config/retry.rs` parses both keys
  and has to lose one.
- **`DeadletterReason` has two variants**, `Attempts` and `Age`, in `pns/crates/pns-domain/src/retry.rs`.
  `RetryLimits::exhausted` returns them. There is no permanent case and no place to put one.
- **An `http_status` column already exists** on both `ledger_legs` and `ledger_attempts`, added by
  `retain_http_status` at schema version 7. Its CHECK constraint admits only `(401,403,404,413)`, which
  is narrower than the design's permanent set and rejects every temporary code, so PR 31 has to widen it.
  SQLite cannot alter a CHECK constraint in place; the column is recreated.
- **The migration runner is a version ladder** in `pns/crates/pns-adapters/src/persistence/sqlite/migrations.rs`,
  `if version < N { ... }` per step with `VERSION` bumped at the end. PR 31 adds step 8.
- **`PostOutcome`** in `pns/crates/pns-hermes/src/post.rs` is already the three-way split the design needs:
  `Status(u16)`, `NoResponse`, `NoStatus`.
- **The banner click slot is free.** `click_command(herdr_path, pane)` in
  `pns/crates/pns-adapters/src/destinations/banner.rs` returns the no-op `:` for an event with no pane,
  and every delivery failure is such an event. `verbatim_argument` is the existing escaper.
- **Subcommands dispatch by first word** in `pns/crates/pns/src/invocation.rs` (`pulse` at 48, `doctor` at
  66, `setup` at 113). `failures` and `click` join that list.

## The order and why it is this order

30 before 31, because the ledger stores a classification the domain has to be able to name. 31 before 32,
because the message renders a stored record and there must not be a formatter that reads something the
ledger cannot hold. 32 before 33, because `pns failures` prints the full form and would otherwise ship a
second formatter that drifts. 34 is independent of 30 to 33 and can overtake them if anything stalls; it
only needs the gateway client. 35 needs 33, since a click opens the view. 36 needs 32, since the page is a
thin render of the same record, and it is last because it is the one rung the design calls optional.

## PR 30, task 30: permanent versus temporary in `pns-domain`

The classification, written once so the retry loop and the reporting read the same rule.

- `pns-domain/src/retry.rs` gains `FailureClass { Permanent, Temporary }` and a total function from a
  `PostOutcome`-shaped input to a class. The domain crate does not depend on `pns-hermes`, so the input is
  a domain value (`DeliveryOutcome`), and the adapter maps `PostOutcome` onto it. Permanent: 400, 401,
  403, 404, 405, 410, 422 and no-status. Temporary: no response, 408, 429 and every 5xx. Any other status
  is temporary, because an unrecognized refusal that might heal is the safer default for a page.
- `DeadletterReason` gains `Permanent`. `RetryLimits::exhausted` keeps its two existing reasons; the
  permanent case dead-letters before `exhausted` is consulted, which is what makes a 404 stop on attempt
  one instead of twenty.
- `RetryBackoff` loses `random_secs` and `retry_at` loses its `sample` argument. `config/retry.rs` stops
  accepting `retry_random_secs` and refuses it by name rather than ignoring it, matching how the config
  parser treats every other unknown key.

Tests, all in `pns-domain` unit modules: every code in the design's two tables maps to its stated class;
an unlisted 4xx is permanent and an unlisted 5xx is temporary; `retry_at` is now a pure function of `now`
and `retries`; a permanent outcome dead-letters at attempt one while a temporary one does not.

Size: under 150 lines of implementation. No I/O, no new dependency.

## PR 31, task 31: the ledger columns and the persisted failure record

The only part of the design with real weight, because it is a schema change.

- Migration step 8 in `migrations.rs`. `ledger_legs` and `ledger_attempts` get their `http_status` CHECK
  widened to any 3-digit status, which means recreating the column: SQLite has no `ALTER COLUMN`. Add the
  design's four new columns on `ledger_legs`: `last_status`, `last_outcome` (`status` | `no-status` |
  `no-response`), `route`, `failed_at`. Widen `deadletter_reason`'s CHECK from `('attempts','age')` to
  include `'permanent'`. All nullable with defaults, so rows written before this survive with no data
  migration.
- The write path in `ledger/outcomes.rs` records the class, the status, the outcome kind and the route on
  every failed attempt, not only on the dead-letter.
- A read path returning one stored failure and the newest N, ordered newest first, for PR 33 to print.
  The id is `ledger_legs.id`, which is already `INTEGER PRIMARY KEY`; the design requires ids never be
  reused, so this PR asserts `AUTOINCREMENT` on the table that hands out failure ids and adds it if the
  column lacks it.

Tests: a version-7 database opens, migrates and keeps its rows; a 502 round-trips through the widened
CHECK that the old one rejected; a permanent failure stores `deadletter_reason = 'permanent'`; two
failures get distinct ids and a delete does not let the next one reuse the freed number.

Size: the migration is the bulk. Split into a second PR if the column recreation and the read path
together push past 400 lines.

## PR 32, task 32: the message and both render forms

One record, two renderings, no third formatter anywhere.

- A new `pns-protocol` module owning the record's field order and the two renderers, `full` and
  `notification`, since the protocol crate is where output contracts live.
- The per-destination meaning tables, keyed by destination and code together. The design's rule that every
  meaning names its concrete subject means these are formatting functions over the route, the config key
  and the address, never constant strings.
- The temporary variant's `fix` line, `nothing to do, pns will retry (N of M attempts used)`.
- The surface-dependent `fix` line for the permanent case: five rows in the design's table, branching on
  the surface and, for the phone, on `[failures] serve` and whether the hermes leg is the one that failed.
- The notification triple varies per failure, using the failure id as the discriminator, so a second
  failure cannot displace the first. This is the fix for the `identifiers.rs` keying behavior the design
  measured.

Tests: the notification form is at or under 256 characters for a worst-case route and agent name, counting
newlines; the full form's field order matches the design; a rename of `[plugins.hermes] key` breaks a test
rather than the message, which means the test reads the live config key name rather than a literal; two
failures render two distinct triples.

## PR 33, task 33: `pns failures` and the `pns doctor` routing

- `pns failures` with no argument lists the last twenty, newest first, in the design's five columns.
  `pns failures <id>` prints one in full through PR 32's `full` renderer.
- The interaction model: infer from the tty, `--interactive` and `--non-interactive` override it,
  `PNS_NON_INTERACTIVE=1` overrides inference but loses to a flag. `--interactive` with no terminal is an
  error with a non-zero exit, never a hang.
- `pns doctor` reports the failure count and names `pns failures`, staying a summary.
- A synchronous producer gets a non-zero exit when its own page did not land, which is design rung 5 and
  the reason posture can tell that its page was lost.

Tests: the listing is capped at twenty and ordered newest first; a piped invocation never prompts;
`--interactive` without a tty exits non-zero; `pns doctor` names the subcommand when the count is above
zero and stays quiet when it is zero.

## PR 34, task 34: the `pns doctor` route check

Independent of 30 to 33; it needs only the gateway client.

`pns doctor` asks the gateway whether each configured route exists. A route that answers is ok, a route
that does not is a named failure, and an unreachable gateway reports **unknown** for every route rather
than failing them, because a gateway that is down is a different problem and reporting it as missing
routes would be a false alarm on every restart.

Tests: three states over a stub gateway, and the unknown state is asserted to name no route as missing.

## PR 35, task 35: the banner click

- `pns click <failure-id>` reads `[banner.click]` and opens the chosen view.
- Three types: `herdr` opens a pane in the running session, `window` runs
  `open -na Ghostty.app --args -e ...` (never `ghostty -e`, which Ghostty's own help says is unsupported
  on macOS), `command` runs the operator's string with `{id}` substituted.
- The default is inferred: `herdr` when herdr is present, `window` otherwise.
- `click_command` returns the fixed `pns click <id>` string for a failure event instead of the no-op `:`.
- A failed view is logged with the command it tried, falls back to `window`, and only if that also fails
  raises a banner. **That banner carries no click**, which is what stops the loop.
- The configuration key documents that a click runs in a bare launchd context with no PATH, so a
  configured command needs absolute paths.

Tests: each type produces its expected argv; an absent herdr infers `window`; a command that does not
exist produces a log line, one fallback and no second click.

## PR 36, task 36: the local page

- `[failures] serve` and `port`, both shipped uncommented at their defaults per the operator's
  defaults-visible ruling.
- A listener on `127.0.0.1` only, because the moshi SSH forward is the trust boundary.
- The page is a thin render of the same stored record, reusing PR 32's renderer. There must not be two
  formatters that can drift.

Tests: the served page's field values match the terminal view for the same record; `serve = false` starts
no listener; the listener refuses a non-loopback bind.

## Verification for the whole ladder

Per PR: `just test-rust`, `just lint-check` and `just test-unit` in the foreground, CI green on the pushed
ref, and merge with the `Merge pull request #N from webdavis/<branch> (#N)` subject.

Two of the design's four verification items cannot be reached by a test and are operator drills after the
ladder merges and an apply lands: the 256-character budget on a real banner and a real phone card, and a
click that opens a real pane. Both are recorded in the design as hand measurements and are re-measured
only if moshi or macOS changes.
