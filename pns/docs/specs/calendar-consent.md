# `pns calendar consent`: the one-time grant

One subcommand, run once per machine, that mints the refresh token
`[quiet.calendar] refresh_token` reads. It prints an authorization URL for the operator to open, waits
on a loopback port for the single redirect the browser makes, exchanges the code, and prints the refresh
token once. It writes the token nowhere: the operator stores it in KeePassXC, and the config references
that entry.

It is a MODE in `command_resume.rs`'s sense: it takes no decision from any event, delivers nothing, and
nothing on the event path reaches it. `pns mute calendar` is the poll this grant arms;
`calendar-quiet-source.md` is that poll.

## The flow

Google's OAuth 2.0 flow for an installed application, read from
<https://developers.google.com/identity/protocols/oauth2/native-app> on 2026-09-21 along with the
`freebusy.query` scope list at
<https://developers.google.com/workspace/calendar/api/v3/reference/freebusy/query>.

1. A `TcpListener` binds `127.0.0.1` on an ephemeral port. LOOPBACK ONLY: the authorization code is a
   credential, and a listener on any other interface offers it to the network.
1. A verifier and a state are minted from `/dev/urandom`, 32 bytes each, written base64url with no
   padding: 43 characters, which is the shortest verifier the flow admits.
1. The authorization URL is printed, every parameter percent-encoded:

   | Parameter               | Value                                                     |
   | ----------------------- | --------------------------------------------------------- |
   | endpoint                | `https://accounts.google.com/o/oauth2/v2/auth`            |
   | `client_id`             | the OAuth client                                          |
   | `redirect_uri`          | `http://127.0.0.1:<the bound port>`                       |
   | `response_type`         | `code`                                                    |
   | `scope`                 | `https://www.googleapis.com/auth/calendar.freebusy`       |
   | `code_challenge`        | base64url, unpadded, of the verifier's SHA-256            |
   | `code_challenge_method` | `S256`                                                    |
   | `state`                 | this run's state                                          |
   | `access_type`           | `offline`, which is what makes a refresh token be issued  |
   | `prompt`                | `consent`, so a client that already has a grant mints one |

   `calendar.freebusy` ("view your availability") is the NARROWEST of the four scopes the freeBusy query
   accepts. It reads no event text, which is the property the poll itself relies on.

1. The one redirect is read, its `state` compared with this run's, and the browser window answered with a
   plain page either way.
1. The code is exchanged at `https://oauth2.googleapis.com/token`, as
   `application/x-www-form-urlencoded`, with `grant_type=authorization_code`, `code`, `client_id`,
   `client_secret`, `code_verifier` and the same `redirect_uri`. The answer's `refresh_token` is printed.

The exchange rides `GoogleCalendarSource::production_config`, the poll's own agent semantics: TLS
verified, NO REDIRECTS, and a 30 second bound on the call. The answer is read under the poll's 256 KiB
body cap.

## What it takes

| Invocation                                                  | Where the client comes from                             |
| ----------------------------------------------------------- | ------------------------------------------------------- |
| `pns calendar consent`                                       | `[quiet.calendar]`, when it states `type = "google"`    |
| `pns calendar consent --client-id <id> --client-secret-stdin` | argv for the id, standard input for the secret          |

`--client-secret <value>` is REFUSED BY NAME, and the refusal says where the secret goes instead: a
secret in argv is readable by every process on the machine. The two flags are given together or neither
is; half of a stated client, a flag this walk does not take, and a verb that is not `consent` each earn
the usage text on stderr and exit 2, which is `pns resume`'s code.

## Fail-closed, exactly as the poll is

Every failure is one fixed sentence naming the step, so no response body, client secret, authorization
code, verifier or token reaches a log line through one:

| When                                             | The sentence                                                                 |
| ------------------------------------------------ | ---------------------------------------------------------------------------- |
| no loopback port binds                           | no loopback port could be opened for the redirect, so nothing was asked for  |
| `/dev/urandom` will not read                     | no random bytes could be read, so nothing was asked for                      |
| the browser never connects, or the read fails    | the browser redirect did not arrive, so no code was exchanged                |
| the redirect's `state` is not this run's         | the redirect did not carry this run's state, so it was not answered          |
| the redirect carries no `code` (a declined grant)| the redirect carried no authorization code, so the consent was refused       |
| the exchange answers non-2xx, or not the document| the token exchange was refused                                               |
| the answer carries no `refresh_token`            | the exchange answered with no refresh token                                  |

A refused walk mints nothing, writes nothing and leaves the config as it was.

## What the operator does with it

The printed refresh token goes into KeePassXC, beside the OAuth client's id and secret, and
`[quiet.calendar]` references all three the way every other secret in `config-values.toml` is
referenced, as `{ keepassxc = "<entry>", field = "Password" }`. The token is printed ONCE, to the
terminal that asked for it: pns writes it to no file, and a second copy is a second walk.

## What is out of scope

Creating the OAuth client. That is a Google Cloud console step the operator takes once, and this walk
takes the id and the secret it produced.
