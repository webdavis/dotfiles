# The calendar quiet source

## Scope

`[quiet.calendar]` is the table that lets a calendar switch the mute on and off. This document covers
which reader answers it, what each reader takes, and what a poll refuses. The mute it sets, and every
other mechanism that silences pns, are in `quiet-behavior.md`.

The table's `type` key picks a COMPILED-IN reader, the way a destination is picked by `type = "hue"` or
`type = "moshi"`. It defaults to `"command"`, so a file written before the google reader existed means
what it always did.

## The table

| Key             | Read under        | Default        | What it is                                                             |
| --------------- | ----------------- | -------------- | ---------------------------------------------------------------------- |
| `enabled`       | both              | `false`        | The switch. Off is off however complete the rest of the table is       |
| `type`          | both              | `"command"`    | `"command"` or `"google"`                                              |
| `command`       | `"command"`       | none           | Argv, never a shell string                                             |
| `calendars`     | `"google"`        | `["primary"]`  | The calendar ids read, as one union                                    |
| `client_id`     | `"google"`        | none, required | The OAuth client                                                       |
| `client_secret` | `"google"`        | none, required | The OAuth client's secret                                              |
| `refresh_token` | `"google"`        | none, required | The refresh token minted for that client                               |
| `poll_interval` | both              | `"2m"`         | How often one poll runs, bounded `"30s"` to `"30m"`                    |
| `deadline`      | both              | `"20s"`        | How long one poll may take, bounded `"1s"` to `"30s"`                  |

Four shapes are refused AT LOAD, each naming the key
(`src/config/quiet.rs:resolve_source`): a `type` word that is neither of the two; `type = "google"`
missing any of `client_id`, `client_secret` or `refresh_token`; `type = "google"` carrying `command`;
and `type = "command"` carrying any of the three credentials. `calendars` is admitted under either
type, because it ships live at its default in the generated template.

The three credentials are SECRETS. In `dot_config/pns/config-values.toml` each is written as a marker
table `{ keepassxc = "<entry>", field = "Password" }` and the render refuses a literal; the shipped
template carries the chezmoi action, never a value.

## `type = "command"`

Unchanged. The configured argv is run read-only under `deadline`, handed no input, and must answer on
standard output with `{"events": [{"start": <epoch>, "end": <epoch>, "busy": <bool>}]}`. Which calendar
that reads, and how, is the command's business (`src/calendar/command.rs`).

## `type = "google"`

Two calls per poll at most, both under the table's `deadline` and both bounded at 256 KiB of body
(`src/calendar/google.rs`).

1. **The access token.** `POST https://oauth2.googleapis.com/token`, `application/x-www-form-urlencoded`,
   with `grant_type=refresh_token`, `client_id`, `client_secret` and `refresh_token`; the answer's
   `access_token` and `expires_in` are read. The token and its expiry are cached at
   `<state>/quiet-calendar-google-token`, one line `<expiry> <token>`, mode `0600`, beside the feature's
   own `quiet-calendar` state file. A cached token is reused until it is within 60 seconds of expiry
   (`src/calendar/google/token.rs`).
2. **The busy intervals.** `POST https://www.googleapis.com/calendar/v3/freeBusy` with
   `Authorization: Bearer <access token>` and a body of `timeMin` (now), `timeMax` (now plus one hour)
   and one `items` entry per configured calendar. Each `calendars.<id>.busy[]` entry becomes one
   `Event { start, end, busy: true }` in epoch seconds, the union over every configured calendar
   (`src/calendar/google/freebusy.rs`). ONE HOUR is the whole window: an event further out cannot start
   before the next poll.

RFC 3339 times are parsed and written by `src/calendar/google/rfc3339.rs`, which the workspace carries
because it depends on no time crate.

### Fail-closed, exactly as the command reader is

A dropped interval is a meeting that silently does not mute, and it looks exactly like a clear calendar.
So the WHOLE answer is refused, never part of it, when: the token exchange answers non-2xx or carries no
`access_token`; the freeBusy call answers non-2xx or is not the document; any configured calendar
carries an `errors` entry; any configured calendar is absent from the answer; or any `start` or `end` is
not an RFC 3339 time. A refused poll changes nothing at all, and a mute already set stands until its own
expiry.

### Privacy

freeBusy carries NO event text: a subject and an attendee list never reach this machine. Every refusal
above is a fixed sentence naming the step or the shape, so no response body, client secret, refresh
token or access token can reach a log line through one. That is pinned end to end by
`src/calendar/google/tests.rs:no_refusal_on_any_path_carries_a_credential_or_a_response_body`, which
drives every refusal path through the scripted transport and asserts the secret bytes are absent.

## What comes later

`dam`, the operator's own task and calendar store, is this feature's primary owner: a `type = "dam"`
reader arrives in a later pull request, and the google reader is the alternative for a machine without
dam. The one-time consent verb that mints the refresh token arrives with it; until then the token is
supplied by hand.
