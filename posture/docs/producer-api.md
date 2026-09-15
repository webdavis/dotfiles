# The producer API

posture delivers a security page one of two ways, chosen in `~/.config/posture/config.toml`. It knows
nothing about any particular notification engine: either it signs the page itself and posts it to a
hermes webhook route, or it hands the page to a command and reads that command's answer back. The second
path is the producer API, and this document is posture's reading of it. Any program may implement it.

## The contract

A producer runs one command and exchanges two JSON documents with it.

| Direction | Carrier                 | Document                    |
| --------- | ----------------------- | --------------------------- |
| in        | the command's stdin     | one request envelope        |
| out       | the command's stdout    | one result envelope         |
| out       | the command's exit code | whether the run itself held |

The command's arguments are not part of the contract. An engine spells its own submit path however it
likes, so both the command and its arguments come from config, passed verbatim.

Both envelopes are versioned. Their schema identifiers are `pns.request/1` and `pns.result/1`, which are
the names version 1 was published under: they are data on the wire, not a dependency, and posture keeps
emitting them verbatim so that every engine already serving version 1 keeps working.
`crates/posture-producer-wire` is posture's own copy of both, with no path dependency on any other
workspace, because posture and each engine ship as separate projects and neither can compile against the
other. The two golden documents under `crates/posture-producer-wire/fixtures/` are what hold the two
readings of the bytes together: they are the same documents the engine side pins, so a change on either
side that moves the bytes fails a test rather than a delivery.

## What posture puts in a request

| Field         | posture's value                                                    |
| ------------- | ------------------------------------------------------------------ |
| `request_id`  | `posture-<32 hex>`, derived from the finding's own occurrence seed |
| `producer`    | `posture`                                                          |
| `event`       | the source event, such as `alert` or `heartbeat`                   |
| `signal`      | `needs_attention` for a page, `observation` otherwise              |
| `occurred_at` | when the finding happened, where that is known                     |
| `detail`      | the title, a newline, then the body                                |
| `route`       | the route the page's own tier names (see below)                    |
| `class`       | `security`, on a page and never on an observation                  |

Nothing else is set. A repeat submission of the same finding carries the ORIGINAL `request_id`, which is
what makes a retry idempotent for an engine that keys on it.

## What posture requires of a result

An accepted submission must be the engine's promise of a retriable obligation for THIS request, taken
before any destination is tried. Posture reads that as all three of: a `request_id` equal to the one it
sent, `status: accepted`, and a `ledger_committed` diagnostic. Destination outcomes are not a substitute,
because a page that reached no channel yet is still a page the engine owes.

Anything less leaves posture's own state where it was, so the next run re-reads the same findings. A
correlated `status: rejected` is a protocol refusal and stays quiet; an engine that could not be run,
timed out, or answered bytes that are not a result envelope raises the local banner instead, because a
broken delivery path is itself something the operator has to know.

## Routes

The severity of a finding names its route, and the route rides in the request for both delivery paths:

| Tier           | Route           |
| -------------- | --------------- |
| critical       | `priority`      |
| notice, info   | `posture-pages` |
| no tier at all | `posture-pages` |

"No tier at all" is the heartbeat, the daily digest and the cursor-reset warning, which keep whatever
route the command built its sink with.

On the direct path the route also selects the signing key and the final URL path segment: a
`posture-pages` page is signed with the `posture-pages` key and posted to `<url>/posture-pages`. A route
this machine holds no key for refuses the page and raises the local banner, rather than posting something
the gateway will answer 401 to and calling it delivered.

## Choosing a path

```toml
[delivery]
mode = "producer" # or "hermes"

[delivery.producer]
command = "/path/to/engine"
arguments = ["submit", "--json"]

[delivery.hermes]
url = "http://127.0.0.1:8644/webhooks"

[delivery.hermes.keys]
posture-pages = "..."
priority = "..."
```

Every key ships at its default, so switching paths is a one-word edit. An unknown key, table or mode is
refused by name at load; with no file at all, and with a file this build cannot use, delivery falls to a
hermes path holding no key, and every page then refuses loudly. A security tool that silently stops
paging is the one failure it cannot afford.
