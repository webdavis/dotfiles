# The producer API

posture raises a security page one of three ways, chosen in `~/.config/posture/config.toml`. It knows
nothing about any particular notification engine: either it signs the page itself and posts it to a
hermes webhook route, or it hands the page to a command and reads that command's answer back, or nothing
leaves the machine and the page is raised on the local banner alone. The second mode is the producer API,
and this document is posture's reading of it. Any program may implement it.

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
`crates/posture-adapters/src/wire/` is posture's own reading of both, plain serde over the two documents
and no path dependency on any other workspace, because posture and each engine ship as separate projects
and neither can compile against the other. It carries the producer's half only: posture writes requests
and reads results, so the depth-limiting parser, the duplicate-field refusal and the structural walk an
ENGINE needs over input it did not build are all absent. The two golden documents under
`crates/posture-adapters/fixtures/` are what hold the two readings of the bytes together: they are the
same documents the engine side pins, so a change on either side that moves the bytes fails a test rather
than a delivery.

## What posture puts in a request

| Field        | posture's value                                                    |
| ------------ | ------------------------------------------------------------------ |
| `request_id` | `posture-<32 hex>`, derived from the finding's own occurrence seed |
| `producer`   | `posture`                                                          |
| `state`      | `blocked` for a page, `observation` otherwise                      |
| `detail`     | the title, a newline, then the body                                |
| `route`      | the route the page's own tier names (see below)                    |
| `class`      | `security`, on a page and never on an observation                  |

Nothing else is set. The finding's own event name and time still exist inside posture, feeding the
request-id seed and the hermes body, but they are no longer fields on the request itself. A repeat
submission of the same finding carries the ORIGINAL `request_id`, which is what makes a retry idempotent
for an engine that keys on it.

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

The severity of a finding names its route, and the route rides in the request for both delivering modes:

| Tier           | Route                |
| -------------- | -------------------- |
| critical       | `priority`           |
| notice, info   | the configured route |
| no tier at all | the configured route |

The configured route is `route` in the `[notify]` table, and it defaults to `posture-pages`. It carries
every page whose tier names no route of its own: a notice, an info finding, the heartbeat, the daily
digest and the cursor-reset warning.

In `hermes` mode the route also selects the signing key and the final URL path segment: a `posture-pages`
page is signed with the `posture-pages` key and posted to `<url>/posture-pages`. A route this machine
holds no key for refuses the page and raises the local banner, rather than posting something the gateway
will answer 401 to and calling it delivered.

## Choosing a mode

```toml
[notify]
mode = "hermes"         # or "command", or "off"
route = "posture-pages"

[notify.command]
path = "/path/to/engine"
arguments = ["send", "--json"]

[notify.hermes]
url = "http://127.0.0.1:8644/webhooks"

[notify.hermes.keys]
posture-pages = "..."
priority = "..."
```

`off` needs no table of its own, and it turns off DELIVERY rather than the page: nothing leaves the
machine and every finding is raised on the local banner instead. A banner that fired is an acceptance,
because the banner is the destination in that mode; a banner that did not leaves the finding
unacknowledged, exactly as an undelivered post does.

Every other key ships at its default, so switching modes is a one-word edit. An unknown key, table or
mode is refused by name at load; with no file at all, and with a file this build cannot use, the choice
falls to a hermes mode holding no key, and every page then refuses loudly. A security tool that silently
stops paging is the one failure it cannot afford.
