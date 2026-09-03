# 0006: A word that names no command is refused, even though the producer parser is lenient

Status: accepted. Implemented by `src/main.rs:is_producer_argv` and the refusal in `src/main.rs:main`.

## The two rules that collided

The producer parser is deliberately lenient. It skips a token it does not recognise in flag position,
because a notification path must not fail the work it reports on and a stray token is not worth losing a
card over.

The subcommand table is not lenient. `pns nag` and `pns lights` already refused a verb they would not
vouch for.

Between them sat a hole. `pns stpo` carried no recognised flag, so the lenient parser skipped the word,
rendered an empty event, and delivered it. The operator got a card about nothing, with no sign that they
had mistyped.

The same hole reopened later for a dash-led typo. A dash-led first word used to be a free pass into the
producer path, so `--wat`, `-help`, and `--agent=claude` each delivered an empty event in silence.

## The rule

A word that names no command is a typo, never an event. The check runs where argv[1] is decided, before
any parsing, and it reads THE WHOLE OF ARGV rather than the first word alone:

- argv carrying a recognised producer flag anywhere is a producer invocation.
- argv carrying `--help` or `-h` anywhere is a producer invocation, so the parser's own help arm answers
  and there is no second copy of help above it.
- An EMPTY argv is the bare invocation, which is a valid empty event.
- Anything else prints `USAGE` to standard error and exits 2.

Reading the whole of argv is what keeps the fix from becoming the bug's mirror image: refusing on the
first word alone would drop real notifications whose argv happens to start with a stray token.

## Why this does not contradict the always-exit-zero contract

That contract governs EVENT deliveries: a notification must never fail the work it reports on. A word
naming no command never becomes an event, so refusing it costs no notification. Exit 2 here is the same
answer `pns nag` and `pns lights` already gave for a verb they did not recognise.

## Consequence for the refactor

This is command decoding, so it belongs in the command-line crate, above the legacy producer adapter. The
leniency stays inside the legacy adapter, where it is a compatibility contract with the flags that
already exist. The refusal stays above it, where the subcommand table is.
