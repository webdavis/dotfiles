# Request, result and egress protocol version 1

These requirements belong to the separate `protocol-v1/S001` through `protocol-v1/S029` namespace.
Requirements S001 through S022 record row 7.1, already implemented in 61faeb0c; S023 through S027 add row
7.2's egress contract. They are not the legacy inventory's S-statements or a claim that these
requirements were recorded before the inherited draft.

The source boundary is `crates/pns-protocol`. It depends on serde and serde_json, with private modules
and curated exports. It performs no policy decisions, transport, persistence, process creation, delivery,
logging or clock waits. Cancellation, process cleanup and external side effects belong to later
application and adapter rows. A request identifier is retained as data; executing idempotency belongs to
submission. Refusals are typed values, not process exit codes.

Plan basis: row 7.1 of `2026-09-05-pns-refactor-plan.md`, lines 496-502 at main
`50763deea9c48396d1d4356a5fee1cbf05992bd5`. The package requirements below remain usable after a
repository move. The seven signals and five kinds of bounds come from that row. Fixed ceilings, duplicate
rejection and advisory truncation are explicit version 1 policies recorded in the implementation and
tests, rather than preserved legacy behavior.

## protocol-v1/S001: Schema and major version

Given a schema string, when an envelope is decoded, then its nonempty name and unsigned decimal major
must parse as one name/major pair. Requests require the request name and major 1; results require the
result name and major 1. A missing or non-string schema is schema_missing, a malformed or wrong name is
schema_unknown, and an unsupported major, including 0 or 2, is major_unsupported. Encoding writes
pns.request/1 or pns.result/1.

Source: [`crates/pns-protocol/src/identifiers.rs`](../../crates/pns-protocol/src/identifiers.rs#L132),
[`crates/pns-protocol/src/envelope.rs`](../../crates/pns-protocol/src/envelope.rs#L67),
[`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L17),
[`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L13).

## protocol-v1/S002: Raw byte ceiling

Given input or an encoded envelope, when its byte length is checked, then 65,536 bytes is allowed and
65,537 is refused with the measured byte count. The raw-input check precedes parsing, including for
malformed bytes. A total envelope can exceed this limit even when each field fits.

Source: [`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L11),
[`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L49),
[`crates/pns-protocol/src/envelope.rs`](../../crates/pns-protocol/src/envelope.rs#L67),
[`crates/pns-protocol/src/envelope.rs`](../../crates/pns-protocol/src/envelope.rs#L93).

## protocol-v1/S003: Object field ceiling

Given an object anywhere in either envelope, including extensions, when structural bounds are checked,
then 64 fields is allowed and 65 is refused with the measured field count. The same bound applies to
encoding constructed requests.

Source: [`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L13),
[`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L65).

## protocol-v1/S004: Text ceiling

Given a JSON (JavaScript Object Notation) string or object key, when bounds are checked, then 8,000
Unicode characters is allowed and 8,001 is refused with the measured character count. This applies at
every depth, to unknown fields, and when encoding request text or destination notes. It counts
characters, not UTF-8 (Unicode Transformation Format, 8-bit) bytes.

Source: [`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L17),
[`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L93),
[`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L65).

## protocol-v1/S005: Array ceiling

Given an array anywhere in either envelope, when bounds are checked, then 64 elements is allowed and 65
is refused with the measured count. Encoding enforces the same cap; the explicit advisory-diagnostic
policy in `protocol-v1/S020` is the sole truncation rule.

Source: [`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L19),
[`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L65),
[`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L108).

## protocol-v1/S006: Container depth ceiling

Given nested objects or arrays, when parsing or checking structure, then the envelope container counts as
level 1, level 8 is allowed, and level 9 is refused with the measured depth. Parsing stops at the
excessive container before reading deeper values. Encoding constructed extensions follows the same rule.

Source: [`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L21),
[`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L101),
[`crates/pns-protocol/src/envelope/json.rs`](../../crates/pns-protocol/src/envelope/json.rs#L33),
[`crates/pns-protocol/src/envelope/json.rs`](../../crates/pns-protocol/src/envelope/json.rs#L95),
[`crates/pns-protocol/src/envelope/json.rs`](../../crates/pns-protocol/src/envelope/json.rs#L83).

## protocol-v1/S007: One JSON object

Given bounded bytes, when an envelope is opened, then the complete input must contain exactly one JSON
object. Malformed syntax, a scalar or array root, an empty input, or another top-level value is refused
as malformed_json. No request identifier is recovered from these failures.

Source: [`crates/pns-protocol/src/envelope/json.rs`](../../crates/pns-protocol/src/envelope/json.rs#L9),
[`crates/pns-protocol/src/envelope.rs`](../../crates/pns-protocol/src/envelope.rs#L67).

## protocol-v1/S008: Duplicate object fields

Given repeated object keys at any depth, when either envelope is parsed, then parsing fails as
malformed_json and no occurrence wins. This includes schema and request identifiers, identical repeated
values, result fields, and nested extensions. A duplicate request identifier is not recovered. This is a
deliberate version 1 boundary policy.

Source: [`crates/pns-protocol/src/lib.rs`](../../crates/pns-protocol/src/lib.rs#L26),
[`crates/pns-protocol/src/envelope/json.rs`](../../crates/pns-protocol/src/envelope/json.rs#L95),
[`crates/pns-protocol/src/envelope.rs`](../../crates/pns-protocol/src/envelope.rs#L67).

## protocol-v1/S009: Request identifiers

Given a producer-generated request identifier, when constructed or decoded, then it must contain 1
through 128 visible ASCII (American Standard Code for Information Interchange) characters. Empty,
over-cap, whitespace, control, or non-ASCII values are refused; valid values encode as plain JSON strings
unchanged. The codec preserves the producer identifier and does not generate or execute idempotency keys.

Source: [`crates/pns-protocol/src/identifiers.rs`](../../crates/pns-protocol/src/identifiers.rs#L13),
[`crates/pns-protocol/src/identifiers.rs`](../../crates/pns-protocol/src/identifiers.rs#L44),
[`crates/pns-protocol/src/identifiers.rs`](../../crates/pns-protocol/src/identifiers.rs#L69).

## protocol-v1/S010: Names

Given a producer, event, route, destination, or session name, when constructed or decoded, then it must
contain 1 through 64 Unicode characters and no control characters. Empty or over-cap names are refused.
Valid names encode as plain JSON strings unchanged.

Source: [`crates/pns-protocol/src/identifiers.rs`](../../crates/pns-protocol/src/identifiers.rs#L17),
[`crates/pns-protocol/src/identifiers.rs`](../../crates/pns-protocol/src/identifiers.rs#L44),
[`crates/pns-protocol/src/identifiers.rs`](../../crates/pns-protocol/src/identifiers.rs#L100).

## protocol-v1/S011: Request fields and defaults

Given a version 1 request, when decoded, then request_id, producer, event and signal are required and
must have their declared types. Invalid identifiers anywhere are refused as field_invalid. Absent
optional session, times, route and class become None; detail is empty, context fields are None, scope is
automatic, interaction is none, and extensions is an empty object. Request::new supplies those same
defaults.

`class` uses the same validated `Name` as the other short names: 1 through 64 Unicode characters, without
controls. A wrong type or invalid name is refused before effects, retaining the correlated request
identifier. An absent or null class is omitted when encoding, preserving the exact canonical bytes of
unmarked version 1 requests. A present class survives canonical encoding and the original producer
request retained by the ledger; changed class metadata under the same identity conflicts.

Source: [`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L107),
[`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L95),
[`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L147),
[`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L186).

## protocol-v1/S012: Signal words

Given a request signal, when encoded or decoded, then kind is exactly one of succeeded, failed,
needs_attention, approval_requested, resolved, observation or progress. Each word maps to its matching
variant in both directions. An unknown word is field_invalid; a missing signal is not defaulted.

Source: [`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L52),
[`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L186).

## protocol-v1/S013: Scope and interaction words

Given a request scope or interaction, when encoded or decoded, then scope is exactly automatic,
local_only or remote_only, and interaction.kind is exactly none or await_decision. Defaults are automatic
and none. Unknown words are refused. These fields describe a request; this codec performs no delivery or
blocking wait.

Source: [`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L65),
[`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L77),
[`crates/pns-protocol/src/lib.rs`](../../crates/pns-protocol/src/lib.rs#L9).

## protocol-v1/S014: Additive fields and inert content

Given a known-major envelope with bounded unknown fields, when decoded, then unknown fields are ignored;
unknown request top-level fields are also returned by name in DecodedRequest::ignored. Parsed values
under request.extensions and text within bounds are preserved without sanitizing or interpretation.
Unknown fields and extensions remain subject to all shared bounds.

Source: [`crates/pns-protocol/src/lib.rs`](../../crates/pns-protocol/src/lib.rs#L18),
[`crates/pns-protocol/src/lib.rs`](../../crates/pns-protocol/src/lib.rs#L22),
[`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L24),
[`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L196),
[`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L107).

## protocol-v1/S015: Unsigned request times

Given occurred_at or elapsed_secs, when decoded, then supplied values must be unsigned integral epoch or
duration seconds. Negative or fractional values are field_invalid, and omission remains None. The codec
does not choose a notification tier from elapsed time.

Source: [`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L107),
[`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L8).

## protocol-v1/S016: Request fixture and public construction

Given a valid Request built through the curated public exports or the package-owned `request-v1.json`
fixture, when encoded and decoded, then every defined request field and extension value round-trips
unchanged. The fixture decodes to its explicitly asserted request. Schema is supplied by the codec.

Source: [`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L167),
[`crates/pns-protocol/src/request.rs`](../../crates/pns-protocol/src/request.rs#L186),
[`crates/pns-protocol/src/lib.rs`](../../crates/pns-protocol/src/lib.rs#L41),
[`crates/pns-protocol/src/request/tests.rs`](../../crates/pns-protocol/src/request/tests.rs#L59),
[`crates/pns-protocol/tests/encoding.rs`](../../crates/pns-protocol/tests/encoding.rs#L24).

## protocol-v1/S017: Result fields and public construction

Given a version 1 result, when decoded, then status is required; absent request_id, decision_id and
interaction are None, and absent destination and diagnostic arrays are empty. Valid results round-trip
through the curated public exports and the package-owned `result-v1.json` fixture. A missing destination
note is omitted when encoded; a supplied note is preserved.

Source: [`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L68),
[`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L59),
[`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L121),
[`crates/pns-protocol/src/lib.rs`](../../crates/pns-protocol/src/lib.rs#L45),
[`crates/pns-protocol/src/result/tests.rs`](../../crates/pns-protocol/src/result/tests.rs#L41).

## protocol-v1/S018: Result words and decision codes

Given a result, when encoded or decoded, then status is accepted, degraded or rejected, destination
outcome is delivered, failed, silent or unlaunched, and interaction.kind is no_opinion or answered.
Answered carries its signed 32-bit code unchanged. Unknown status, outcome or interaction words are
field_invalid.

Source: [`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L27),
[`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L48),
[`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L39),
[`crates/pns-protocol/tests/decoding.rs`](../../crates/pns-protocol/tests/decoding.rs#L127).

## protocol-v1/S019: Correlated refusals

Given an object that passes parse and shared bounds, when schema or typed-field decoding fails, then a
valid request_id is retained for correlation; an invalid identifier is not recovered. Malformed or
over-bound input has no recovered identifier. ResultEnvelope::rejected returns status rejected, that
optional identifier, one stable diagnostic code, and no decision, interaction or destination outcomes.

Source: [`crates/pns-protocol/src/envelope.rs`](../../crates/pns-protocol/src/envelope.rs#L53),
[`crates/pns-protocol/src/envelope.rs`](../../crates/pns-protocol/src/envelope.rs#L107),
[`crates/pns-protocol/src/envelope.rs`](../../crates/pns-protocol/src/envelope.rs#L67),
[`crates/pns-protocol/src/envelope.rs`](../../crates/pns-protocol/src/envelope.rs#L34),
[`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L94).

## protocol-v1/S020: Advisory diagnostics

Given a constructed result with advisory diagnostic codes, when encoded, then the first 64 codes are kept
in order; 63 or 64 remain unchanged and 65 loses only its last code. Encoding does not mutate the caller
or change other result fields. This deliberate version 1 policy permits bounded advisory summaries; it is
not a legacy-compatibility requirement. The retained codes still obey text and total-byte bounds.

Source: [`crates/pns-protocol/src/lib.rs`](../../crates/pns-protocol/src/lib.rs#L28),
[`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L108),
[`crates/pns-protocol/src/result/tests.rs`](../../crates/pns-protocol/src/result/tests.rs#L81).

## protocol-v1/S021: Preserve delivery facts

Given a constructed result whose destination array or destination note exceeds a shared bound, when
encoded, then it is refused rather than dropping an outcome or shortening a note. At 64 destination
outcomes it remains encodable, subject to the other bounds; at 65 it is refused.

Source: [`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L105),
[`crates/pns-protocol/src/result.rs`](../../crates/pns-protocol/src/result.rs#L108),
[`crates/pns-protocol/tests/encoding.rs`](../../crates/pns-protocol/tests/encoding.rs#L68),
[`crates/pns-protocol/tests/encoding.rs`](../../crates/pns-protocol/tests/encoding.rs#L52).

## protocol-v1/S022: Bound refusal direction and codes

Given a structural violation, when an envelope is opened, then bounds are checked before its schema.
Violations retain their measured size and use bytes_over_cap, fields_over_cap, text_over_cap,
items_over_cap or depth_over_cap. Such input is refused, never accepted after silently discarding data.

Source: [`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L27),
[`crates/pns-protocol/src/bounds.rs`](../../crates/pns-protocol/src/bounds.rs#L35),
[`crates/pns-protocol/src/envelope.rs`](../../crates/pns-protocol/src/envelope.rs#L67).

## protocol-v1/S023: Egress envelope and correlation

Given a rendered event and an original request identifier, when encoded, then the envelope has schema
`pns.egress/1`, that unchanged `request_id`, and the rendered `body`. Decoding returns those same values.
No identifier is generated or replaced. Unknown names and unsupported majors are refused with S001's
typed errors, retaining a valid identifier under S019's rules.

Source: `crates/pns-protocol/src/egress.rs`.

## protocol-v1/S024: Legacy executable body

Given the same rendered event and reporting mode, when the version 1 body is serialized, then its bytes
equal the legacy executable-channel JSON (legacy S126). Its ten keys retain sorted order; `mode` is
`async` for a silent leg and `sync` for a reporting leg. Quotes, escapes, Unicode and empty strings
remain unchanged. Schema and request identifier surround the body and are not added to it. The existing
executable stdin line and trailing newline remain unchanged; adopting an envelope does not authorize
replacing that input with a nested object. The executable adapter sets only `PNS_REQUEST_ID` and
`PNS_PRODUCER` on the child command from the original delivery request, overriding any inherited values.
Event text stays on stdin; no serialized envelope is placed in the environment.

Source: `tests/egress.rs`, `crates/pns-protocol/src/egress.rs`.

## protocol-v1/S025: Required egress fields

Given egress input, when decoded, then `request_id` is a valid S009 identifier and `body` contains all
ten fields. Agent, branch, detail, message, pane, preview, project, state and title are strings; mode is
exactly `async` or `sync`. Missing or mistyped fields and unknown modes are refused as `field_invalid`.
No missing body field is defaulted.

Source: `crates/pns-protocol/src/egress/tests.rs`.

## protocol-v1/S026: Egress boundary validation

Given an egress envelope, when encoded or decoded, then all shared byte and structural limits apply to
the complete envelope, including the body. Over-limit content is refused without truncation. Duplicate
keys at any depth are refused before correlation, as in S008. The egress entry points use the same
validation as request and result envelopes.

Source: `crates/pns-protocol/src/egress.rs`, `crates/pns-protocol/src/envelope.rs`.

## protocol-v1/S027: Additive egress fields

Given a known-major egress envelope with unknown fields at the top level or in its body, when decoded,
then those fields are ignored within the shared limits. Known text fields remain inert data. The codec
performs no command execution, sanitization, logging, delivery or other external side effect.

Source: `crates/pns-protocol/src/egress/tests.rs`.

## protocol-v1/S028: JSON submission adapter

Given `pns submit --json`, when stdin contains one bounded request, then the adapter invokes the existing
submission callback once with the complete decoded request, including ignored field names. It reads no
more than 65,537 bytes. Requests through 65,536 bytes retain the decoder's existing bounds; an extra byte
is refused. Invalid arguments are rejected with `submit_usage` before reading stdin. An input read error
is rejected with `input_unreadable`, without exposing the input error or submitting partial input.
Decoder refusals retain their existing diagnostic codes and recovered request identifier.

The adapter writes the callback's result as one JSON line and returns its status to the composition root.
It preserves destination outcomes and diagnostic codes. An unencodable result or failed output write is
an output error; it does not fabricate acceptance or silently discard destination facts.

## protocol-v1/S029: Durable submission receipt

An `accepted` result with `ledger_committed` in diagnostics means the ledger committed the request before
dispatch and owns its delivery. A retained identical request qualifies through the existing ledger row.
The request identifier remains the producer's original identifier. A successful live send alone does not
qualify: when storage is unavailable and delivery runs without a committed row, the result is `degraded`
and does not contain `ledger_committed`. A decoded refusal is `rejected`.

Posture advances its own state only on a matching `accepted` result containing `ledger_committed`.
Acceptance is durable ownership, not proof of a destination acknowledgement. Per-destination outcomes
state the attempts separately, and retries retain the original identifier. Main dispatch, receipt
classification and the event workflow are composed by the existing root callback.

## protocol-v1/S030: Root submission composition

The JSON command encodes the decoded request before reading configuration, probing or delivering. If
canonical encoding exceeds a protocol bound, it returns a correlated rejection and causes no event side
effects. Otherwise, the ledger retains those canonical bytes beside the original producer and request
identifier. Source event names, occurrence time, session and extensions remain metadata. The normalized
signal selects the existing event state; observation and progress use the marker-neutral observation
path. Scope and context enter the same decision workflow as legacy events. Elapsed time selects the
existing 300-second long-running tier without suppressing a short JSON request.

JSON stdout contains exactly one result line. Human delivery lines and executable-channel stdout go to
stderr for that invocation, including its replay tail. Legacy stdout and the flat executable stdin body
remain unchanged. Accepted and degraded results exit zero; rejected requests and output errors exit two.
Destination results carry typed verdicts without echoing private transport text. Unknown top-level field
names follow an `ignored_fields` diagnostic. An awaited decision receives `no_opinion` because this
entrypoint has no applicable interaction forwarder; this does not complete the separate hook and approval
migration or the posture route. The configured class policy is specified in `quiet-behavior.md`, behavior
7\.

When legacy identity generation or the system clock is unavailable, the same application delivery body
attempts the planned channels without inventing an identifier or lease time. Native transports omit the
unavailable idempotency identifier, and legacy executables receive no request-identifier environment
variable. The versioned egress envelope still requires a valid request identifier. Unretained attempts
never produce a committed-ownership receipt.
