# Home probe

## Scope

The home probe asks the router whether the operator's own device is on the home wifi, and `pns home` is
the one mode that reads it and says the answer out loud. This document covers the probe end to end: the
transport and its authentication (two GET calls to the UniFi integration API over a verification-disabled
TLS connection, the key in an `X-API-KEY` header), the identity model (three configured keys, at least
one required, strongest naming the match), the staleness policy (a Home verdict with a configured key
pointing at another client or at nobody), the episode memory that keeps the warning to once per state,
the stale alert that memory gates, every failure source, and every deadline and byte ceiling. It also
settles what the router IS in this crate: `[plugins.router]` is registered as a SENSOR, an input, never a
delivery destination, and the evidence is behavior 1. Everything below is derived from the crate at
`dot_local/share/pns` and its tests only. Where the code does not settle a question the line begins
`NOT ESTABLISHED:` and names what was looked for.

## The identifiers

The probe never matches on a device. It matches CONFIGURED VALUES against FIELDS of the clients the
router listed. Three keys, at least one required, and any one of them matching any listed client reads
Home (`src/home.rs:home_reading`).

| Config key        | What kind of identifier                                                  | Field it is compared against       | How it is normalized                                                                                                                                                                                                                                             | What makes it go stale                                                                                                                                                                                                                                                                          | Tests that pin it                                                                                                                                                                                                                                          |
| ----------------- | ------------------------------------------------------------------------ | ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `device_mac`      | The device's media access control address, as the router sees it         | `macAddress` on each listed client | BOTH SIDES through `src/home.rs:normalized_mac`: six groups of two hexadecimal digits under ONE separator (all `:` or all `-`), joined with colons and lowercased. A bare twelve-character run and a mixed separator are refused as typos rather than guessed at | The address the router sees is minted per network and can rotate. iOS ships private wifi addresses, and the phone in the live capture of 2026-08-20 carries a locally administered one (`src/home.rs` module comment). A rotated address matches nobody; a reassigned one matches somebody else | `src/home.rs:a_mac_only_identity_reads_home_through_the_one_normalized_spelling`, `src/home.rs:a_well_formed_mac_in_any_case_or_separator_validates_to_one_spelling`, `src/home.rs:a_malformed_device_mac_is_refused_naming_the_key_and_quoting_the_value` |
| `device_hostname` | The client NAME the router shows, which is a label the operator can move | `name` on each listed client       | None. The comparison is exact, case-sensitive and whole-string, deliberately: anything looser would let `mister-2` answer for `mister` (`src/home.rs:client_carries`)                                                                                            | The operator renames a client, or the client that carried the name leaves the network. A device the router has not identified carries no `name` at all                                                                                                                                          | `src/home.rs:a_hostname_match_is_exact_so_a_sibling_device_cannot_answer_for_the_phone`, `src/home.rs:a_table_carrying_only_a_hostname_yields_it_and_the_retired_key_is_not_read`                                                                          |
| `device_ipv4`     | Today's lease, an IPv4 address                                           | `ipAddress` on each listed client  | BOTH SIDES parsed to `std::net::Ipv4Addr` and compared as ADDRESSES, never as text. The config value is parsed at read (`src/home.rs:device_identity`) and the router's own spelling is parsed at compare (`src/home.rs:client_carries`)                         | It drifts under DHCP (dynamic host configuration protocol) by design, and the old lease is handed to another client (`src/home.rs` module comment)                                                                                                                                              | `src/home.rs:an_ipv4_only_identity_reads_home_against_the_client_carrying_that_address`, `src/home.rs:a_malformed_device_ipv4_is_refused_naming_the_key_and_quoting_the_value`                                                                             |

PRECEDENCE runs media access control address, then client name, then address, and it is the statement
order of three `if let` blocks in `src/home.rs:home_reading` and nothing else. The declaration order of
`src/home.rs:DeviceKey`'s variants is explicitly NOT the rule: the module comment states that reversing
the enum leaves the whole suite green. Precedence is only ever observable on a disagreement, because any
one match reads Home.

A CONFIGURED KEY IS NEVER A CLIENT FILTER. An entry the probe cannot read (a garbage address, a malformed
media access control address, no `name`) is a client that matches nothing, never a listing that failed:
"the ROUTER is not the operator" (`src/home.rs:Client`,
`src/home.rs:an_ipv4_only_identity_reads_home_against_the_client_carrying_that_address`).

## The failures

| Failure source                                                                                  | What the reading becomes                                           | What the operator sees                                                                                                                                                                             | Fail direction                                                                                                                                                                                |
| ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No config file                                                                                  | No reading is taken                                                | `home: not configured (no config file)`                                                                                                                                                            | Refuse to read, exit 0                                                                                                                                                                        |
| Config could not be parsed                                                                      | No reading is taken                                                | `home: config error (<detail>)`                                                                                                                                                                    | Refuse to read, exit 0                                                                                                                                                                        |
| No `[plugins.router]` table                                                                     | No reading is taken                                                | `home: not configured (no [plugins.router] table)`                                                                                                                                                 | Refuse to read, exit 0                                                                                                                                                                        |
| `[plugins.router] enabled = false`                                                              | No reading is taken                                                | `home: [plugins.router] is present but enabled = false`                                                                                                                                            | Refuse to read, and told apart from the missing table on purpose, because the two are different edits (`src/home.rs:enabled_router_table`)                                                    |
| `type` absent or empty                                                                          | No reading is taken                                                | `home: no type in [plugins.router] (the only type is "unifi")`                                                                                                                                     | Refuse to read                                                                                                                                                                                |
| `type` names no compiled-in backend                                                             | No reading is taken                                                | `home: [plugins.router] has type "asus", which no compiled-in backend answers (the only type is "unifi")`                                                                                          | Refuse to read, quoting what was written                                                                                                                                                      |
| `router_url` missing, empty, or not a string                                                    | No reading is taken                                                | `home: the [plugins.router] table is present but router_url is missing, empty, or not a string`                                                                                                    | Refuse to read                                                                                                                                                                                |
| All three device keys absent                                                                    | No reading is taken                                                | `home: no device to look for in [plugins.router] (set at least one of device_mac, device_hostname, device_ipv4)`                                                                                   | Refuse to read. A probe with nothing to look for would read NotHome forever (`src/home.rs:device_identity`)                                                                                   |
| A device key present but empty, the wrong TOML type, or unreadable as its shape                 | No reading is taken                                                | `home: device_ipv4 = "192.168.1" in [plugins.router] is not an IPv4 address (a dotted quad, e.g. "192.168.1.169")`, and the sibling lines for the media access control address and the client name | Refuse by NAME. A blank key read as absent would be a silent typo the output could not show (`src/home.rs:read_device_key`)                                                                   |
| `api_key` absent, empty, or not a string                                                        | No reading is taken                                                | `home: no api_key in the [plugins.router] table (the probe is not set up)`                                                                                                                         | Refuse to read; every way the key is missing is one line (`src/home.rs:router_api_key`)                                                                                                       |
| Router unreachable, connection refused, deadline passed, or the body cannot be read as a string | `Unknown`                                                          | `home: unknown (router unreachable or its answer unreadable)`                                                                                                                                      | Unknown, NEVER NotHome                                                                                                                                                                        |
| The router answers a redirect                                                                   | `Unknown`                                                          | The same unknown line                                                                                                                                                                              | Unknown, and the redirect target is never contacted (`src/home.rs:a_redirecting_router_is_never_followed_and_reads_unknown`)                                                                  |
| Either body exceeds 1,000,000 bytes                                                             | `Unknown`                                                          | The same unknown line                                                                                                                                                                              | Unknown rather than swallowing the stream (`src/home.rs:ROUTER_BODY_CAP`)                                                                                                                     |
| The sites answer carries no usable id, or an id that is not hexadecimal digits and dashes       | `Unknown`, and the clients call is never made                      | The same unknown line                                                                                                                                                                              | Unknown; a corrupt or hostile id must never become a URL path segment (`src/home.rs:first_site_id`)                                                                                           |
| The clients answer is not JSON, or carries no `data` array (the unauthorized shape)             | `Unknown`                                                          | The same unknown line                                                                                                                                                                              | Unknown; distinct from an empty wifi on purpose (`src/home.rs:parse_clients`)                                                                                                                 |
| The page is incomplete (`totalCount` greater than the entries returned)                         | `Unknown`                                                          | The same unknown line                                                                                                                                                                              | Unknown; a device beyond the page would read as departed, which is a false transition                                                                                                         |
| One listed client is unreadable                                                                 | Nothing. That client matches nothing and the listing still answers | Whatever the reading was                                                                                                                                                                           | The listing is judged whole; a bad entry is not a bad listing                                                                                                                                 |
| `stale_alert_channel` is not a usable route name                                                | The reading is untouched                                           | `pns: config error (stale_alert_channel = "../alert" in [plugins.router] is not a usable route name); the stale alert posts to the default route` on stderr, and the alert still goes out          | LOUD-WARD. A config typo must not silence the very warning it is configuring (`src/home.rs:stale_alert_channel`)                                                                              |
| The state directory cannot be read or written                                                   | The reading is untouched                                           | The verdict, the evidence and the warning, unchanged, exit 0. The cost is that the same state is news again on every run                                                                           | FAIL-QUIET, and pinned as a cost rather than a crash (`tests/dispatch.rs:a_state_directory_that_cannot_be_used_leaves_the_whole_diagnostic_standing`)                                         |
| The stale alert's delivery is refused by the gateway                                            | The reading is untouched                                           | Nothing extra on the probe's own surface                                                                                                                                                           | The episode is consumed anyway. Fire and forget is this engine's contract for every producer, and the printed line has already told the human who typed the command (`src/main.rs:home_mode`) |

THE FAIL DIRECTION IN ONE SENTENCE, stated in the module comment: "every failure to read is `Unknown`,
never `NotHome`", because the future consumers suppress or replay on TRANSITIONS, and inventing "the
device left" out of an unreachable router would fire a false one. `Unknown` is the reading that changes
nothing.

## Behaviors

### 1. The router is a sensor, never a delivery destination

Given the compiled-in roster When the config enables `[plugins.router]` and an event is dispatched Then
the plugin is selected and named as known, no leg is planned for it, and no event ever reaches it

- Success: `src/registry.rs:ROSTER` declares `name: "router"` with `kind: PluginKind::Sensor`, first in
  the list, with the comment "an INPUT, so it holds no delivery order to state". `PluginKind::Sensor`
  carries no `Routing` at all, so "a sensor never becomes a leg" is unrepresentable rather than filtered
  (`src/registry.rs:PluginKind`), and the plan builder's exhaustive match answers
  `PluginKind::Sensor => None` (`src/routing.rs`). The doctor turns a selected sensor into a skip and
  prints `router: skipped, a sensor and never a delivery destination` (`src/doctor.rs:kind_of`,
  `src/doctor.rs:A_SENSOR`). Pinned by `tests/dispatch.rs:the_binarys_own_roster_knows_the_router_sensor`
  (a recording stub is installed under the name `router` so a rogue leg would leave a trace, and it does
  not fire), `tests/dispatch.rs:only_a_home_reading_alerts_and_the_sensor_is_never_a_destination` (the
  alert ABOUT the reading is never delivered back to the sensor),
  `tests/dispatch.rs:the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one`
  and `src/doctor.rs:a_selected_sensor_is_a_skip_because_no_leg_can_ever_reach_one`.
- Failure sources: a config naming a plugin nothing registered is refused by name
  (`src/registry.rs:Registry::enabled`); registering one name twice panics at the composition root
  (`src/registry.rs:build_registry`).
- Fail direction: refuse and name the offender. A typo'd plugin name that silently no-ops is a
  notification quietly turned off (`src/registry.rs` module comment).
- Thresholds: Not applicable. Nothing here is timed or counted.
- Required side effects: the name `router` occupies the one plugin namespace, so `[plugins.router]` is a
  known table and the roster fallback is not triggered by it.
- Forbidden side effects: no event may be delivered to `router`, under any flag, fallback, or later kind.
  The `filter_map` match in `src/routing.rs` is exhaustive, so a third kind has to state its own answer
  rather than inherit delivery from a catch-all.
- Timeout and cancellation: Not applicable. Registration is a compiled-in list.
- Idempotency and duplicates: a duplicate registration is `RegistryError::Duplicate` and panics at
  startup, deterministically, so it cannot reach an operator's machine.
- Privacy: Not applicable. The roster carries names, never values.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the config table is spelled `[plugins.router]`. The retired top-level `[home]`
  table is refused whole by the config layer, naming the tables that do work
  (`tests/dispatch.rs:every_way_the_home_probe_is_not_set_up_says_which_one_it_is`), and the retired
  `brand` and `phone` keys are refused by name (`src/config.rs`, keys served: `api_key`,
  `device_hostname`, `device_ipv4`, `device_mac`, `enabled`, `router_url`, `stale_alert_channel`,
  `type`).

### 2. The reading is an input that nothing in the delivery plan consumes

Given a Home, NotHome or Unknown reading When any other event is dispatched on this machine Then the
reading changes nothing about where that event lands

- Success: the module comment states it outright: "The reading is a sensor only: nothing in the delivery
  plan consumes it yet, because no row of the confirmed matrix changes on home-ness until
  catch-up-on-return and the quiet window (part 2's B and C) arrive to spend it. Building the integration
  ahead of the consumer was considered and declined on 2026-08-25." A search of the crate finds `home::`
  referenced outside `src/home.rs` in exactly these places, none of them a delivery decision:
  `src/main.rs:home_mode` (the diagnostic), `src/main.rs:disabled_backend_warnings` (a doctor warning),
  `src/setup.rs:router_is_armed` and `src/setup.rs`'s own test (the wizard), and `src/config.rs` plus
  `src/channels/moshi.rs` in comments only. The presence gate that DOES suppress a leg reads the idle
  counter, the phone marker, the phone's pty access time, the console lock and the multiplexer view
  (`src/probes.rs`), and none of those is the router.
- Failure sources: Not applicable. There is no consumer to fail.
- Fail direction: Not applicable, and deliberately so. A reading nothing consumes cannot fire a false
  transition in either direction.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: the reading must not be turned into a presence verdict. NOT ESTABLISHED: no
  test asserts the absence of a consumer, so this behavior rests on the module comment and on the absence
  of call sites rather than on a guard.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the one exception is the stale alert of behavior 12, which is an event ABOUT
  the reading and not the reading itself.

### 3. One reading is fetch, then parse, then judge

Given a configured device identity and a router When `pns home` takes one reading Then the clients
listing is fetched through the `Router` seam, parsed, and judged, in that order

- Success: `src/home.rs:read_home` is three calls in one expression:
  `home_reading(router.clients_json().as_deref().and_then(parse_clients), device)`. A fetch that answered
  `None` and a body that would not parse are the SAME input to the judge, which is what makes the Unknown
  arm one arm. Pinned by `src/home.rs:one_reading_runs_fetch_parse_judge_in_order`, which drives a fake
  router answering the live capture and then answering nothing.
- Failure sources: the fetch (behavior 4), the parse (behavior 6).
- Fail direction: Unknown. `src/home.rs:home_reading`'s first statement returns `HomePresence::Unknown`
  with an EMPTY key list, because "NOTHING WAS SEARCHED, so there is nothing to say a key found: an
  unreachable router is not a listing in which every key came up empty."
- Thresholds: Not applicable at this layer; the transport's are in behavior 4.
- Required side effects: none. A reading takes no state and writes none.
- Forbidden side effects: none of the three steps may write.
- Timeout and cancellation: inherited from the transport (behavior 4). There is no cancellation path: a
  reading in flight runs to its deadline.
- Idempotency and duplicates: a reading is read-only against the router (two GET calls) and can be
  repeated freely.
- Privacy: the seam carries the listing as a `String`. It is never logged, never printed whole, and never
  written to disk on any path in this crate.
- Process ownership and cleanup: no subprocess. The reading is in-process HTTP.
- Compatibility contract: `pub trait Router` with one method, `fn clients_json(&self) -> Option<String>`,
  is the seam a second backend enters through (`src/home.rs:Router`). The one compiled-in implementation
  is `src/home.rs:UniFiRouter`.

### 4. The adapter walks sites then clients, with the key in a header

Given a `router_url` and an `api_key` When one reading is taken against the UniFi backend Then
`/proxy/network/integration/v1/sites` is requested first, its first site id is validated and joined into
`/proxy/network/integration/v1/sites/{site}/clients?limit=200`, and both requests carry
`X-API-KEY: <api_key>`

- Success: `src/home.rs:UniFiRouter::clients_json`. The site is resolved by listing rather than
  configured, because site ids are per-install. Pinned by
  `src/home.rs:the_adapter_sends_the_key_and_walks_sites_then_clients`, which runs the REAL ureq pipeline
  over a scripted transport (`Agent::with_parts`, so the URL building, the header, the redirect policy
  and the body cap are all real and only the wire is fake) and asserts the sites request precedes the
  clients request, that the clients path carries the extracted site id, and that `x-api-key: k-123` is on
  the wire.
- Failure sources: a connection that cannot be made; a call ureq reports as an error; a body that will
  not read as a string; a sites answer with no usable id; a body past the cap; a redirect.
- Fail direction: every one of them yields `None` from the seam and therefore `Unknown` from the reading.
  NOT ESTABLISHED: whether a 401 status alone (rather than the body shape) is what makes an unauthorized
  answer `None`. `src/home.rs:a_json_answer_without_the_data_list_is_no_answer` pins the unauthorized
  BODY shape (`{"error":"unauthorized"}`) reaching Unknown through the parser, and ureq's own
  status-to-error behavior is not asserted anywhere in this crate.
- Thresholds: `src/home.rs:ROUTER_DEADLINE` is `Duration::from_secs(5)`, set as `timeout_global` on the
  agent, and the code states "both calls ride one agent and one deadline each"
  (`src/home.rs:impl Router for UniFiRouter`), so one reading can take up to about ten seconds in total.
  `src/home.rs:ROUTER_BODY_CAP` is `1_000_000` bytes per response, far below ureq's own 10MB default. A
  body well under the cap is read and judged; a body of 1,100,000 bytes that is otherwise VALID and would
  have parsed into Home reads Unknown instead
  (`src/home.rs:a_body_past_the_cap_reads_unknown_rather_than_being_swallowed`). NOT ESTABLISHED: the
  exact inclusive or exclusive boundary at 1,000,000 bytes, since no test sits at the cap and ureq's
  `limit` semantics are not asserted here. NOT ESTABLISHED: no test measures the five-second deadline.
- Required side effects: exactly two GET requests per reading, in that order, each carrying the key.
- Forbidden side effects: no redirect is followed (`max_redirects(0)`), so the key header can never be
  sent to a `Location` target; no request is made after a failed sites call; nothing is written anywhere.
- Timeout and cancellation: the five-second global timeout per call is the only bound. There is no
  cancellation.
- Idempotency and duplicates: both calls are GETs and change nothing on the router. Two concurrent
  readings are two independent walks.
- Privacy: the API key travels from the config into the request HEADER and nowhere else: never argv,
  never a child's environment, never an error string (`src/home.rs:UniFiRouter`). It is read by its own
  function so it never joins `RouterSettings` in a type that could be dumped whole
  (`src/home.rs:router_api_key`, `src/main.rs:home_mode`), and `UniFiRouter` derives no `Debug`. Pinned
  end to end by `tests/dispatch.rs:the_alert_carries_no_secret_and_no_raw_router_text`, which asserts the
  router key `k-123` and the hermes signing key appear in neither the delivered event, nor stdout, nor
  stderr.
- Process ownership and cleanup: in-process HTTP over ureq. No child, no file, no socket left behind by
  this crate's own code.
- Compatibility contract: TLS verification is DISABLED, exactly as the hue bridge's is and for the same
  stated reason: the router serves a self-signed certificate for its own local network address and no
  certificate authority vouches for it (`src/home.rs:UniFiRouter::new`). The scripted transport reaches
  into `ureq::unversioned`, which is exempt from semantic versioning; `Cargo.lock` pins the version and
  these tests are what catch a deliberate bump.

### 5. A router answer becomes part of a URL exactly once, and is validated first

Given a sites listing from the router When the first site's id is extracted Then it is accepted only when
it is non-empty and made entirely of hexadecimal digits and dashes

- Success: `src/home.rs:first_site_id` navigates `data[0].id`, requires a string, and then requires every
  byte to be an ASCII hexadecimal digit or `-`. Pinned by
  `src/home.rs:the_first_sites_id_is_extracted_from_the_live_shape`.
- Failure sources: an empty body, a body that is not JSON, `data` empty, `data[0]` with no `id`, an `id`
  that is not a string (`src/home.rs:a_sites_answer_without_a_usable_id_is_no_answer`).
- Fail direction: `None`, so the clients call is never made and the reading is Unknown.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: a path-escaping or query-injecting id must never be joined into the clients
  URL. `../evil`, `a/b`, `a?x=1`, `a#frag`, `id with space` and the empty string are all refused
  (`src/home.rs:an_id_that_could_escape_the_url_path_is_refused_outright`).
- Timeout and cancellation: Not applicable. This is a pure function over the fetched text.
- Idempotency and duplicates: pure.
- Privacy: the site id is an install-local identifier. It reaches the request URL and nothing else: it is
  never printed by `pns home` and never carried in an alert.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: this is the trust boundary the comment names: "this is the one place a router
  answer becomes part of a URL".

### 6. An unreadable or incomplete listing is no answer, and an empty one is an answer

Given the clients body the router returned When it is parsed Then a complete listing becomes a list of
clients (possibly empty), and anything else becomes no answer at all

- Success: `src/home.rs:parse_clients` requires JSON, requires `data` to be an array, and then reads
  `name`, `ipAddress` and `macAddress` off each entry, every field optional because every field is
  optional on the wire. An unnamed client STAYS in the listing, because it still carries a media access
  control address and an address a configured key can match on
  (`src/home.rs:every_client_in_the_live_capture_is_read_with_all_three_of_its_fields`). An empty list is
  a parsed answer meaning "nobody is on the wifi"
  (`src/home.rs:a_parsed_empty_list_is_an_answer_and_not_a_failure`).
- Failure sources: text that is not JSON, including the router's captive-portal page; JSON with no
  `data`; `data` that is not an array; a page the device could be beyond
  (`src/home.rs:a_listing_that_is_not_json_is_no_answer_rather_than_an_empty_wifi`,
  `src/home.rs:a_json_answer_without_the_data_list_is_no_answer`).
- Fail direction: `None`, which reads Unknown. Unparseable and empty must stay distinct, because empty
  would read as the device having LEFT.
- Thresholds: completeness is `totalCount` against the number of ENTRIES returned. `totalCount` absent
  means the page is taken whole (`src/home.rs:a_listing_without_total_count_is_taken_whole_as_before`).
  `totalCount: 2` with two entries is complete
  (`src/home.rs:a_complete_page_with_unnamed_clients_still_answers`); `totalCount: 201` with one entry is
  incomplete and returns `None`
  (`src/home.rs:a_page_the_phone_could_be_beyond_is_no_answer_rather_than_not_home`). KNOWN CEILING: the
  clients call asks for `limit=200` (`src/home.rs:UniFiRouter::clients_json`) and the probe paginates no
  further, so on a network the router counts as holding more than 200 clients every reading is Unknown.
  NOT ESTABLISHED: no test exercises a 200-entry page against a larger `totalCount` through the live
  adapter.
- Required side effects: none.
- Forbidden side effects: an entry the probe cannot read must not be dropped from the listing and must
  not fail the listing.
- Timeout and cancellation: Not applicable. Pure over an already-fetched string.
- Idempotency and duplicates: pure.
- Privacy: the parsed clients hold other people's device names, addresses and media access control
  addresses. They stay in memory for the length of one reading; only the label of a client a DISAGREEING
  key matched can reach stdout, and then only escaped (behavior 9).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: nothing about a client is validated. "an entry this probe cannot read is a
  client that matches nothing, never a listing that failed" (`src/home.rs:Client`).

### 7. Any one configured key matching reads Home, and the strongest that matched names it

Given a parsed listing and one to three configured keys When the verdict is derived Then the first key in
precedence order that matched any client is the verdict's `matched_by`, carrying the value the operator
configured

- Success: `src/home.rs:home_reading` builds `configured` in the fixed order media access control
  address, client name, address, then takes `find_map(first_match)`. ANY match is Home, so the order is
  only ever read on disagreement. Pinned by
  `src/home.rs:any_one_configured_key_matching_reads_home_while_the_others_match_nothing` and
  `src/home.rs:on_keys_matching_different_clients_the_verdict_names_the_strongest`.
- Failure sources: none of its own. The verdict is derived from an already-parsed listing.
- Fail direction: Not applicable at this step; a listing that could not be had never reaches it.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: the verdict must not be a second scan beside the evidence. It is the first key
  of the SAME scan that found anything, "so the line naming the winner and the lines describing the keys
  cannot drift apart" (`src/home.rs:home_reading`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure over the listing. A listing carrying one value twice cannot flip the
  reading by ordering: see behavior 9.
- Privacy: the `value` inside `HomePresence::Home` is the OPERATOR'S OWN configured value, not the
  router's text.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: a fourth `DeviceKey` variant compiles without reaching three hand-edited places
  (the key read in `device_identity`, the scan in `home_reading`, and the key list in the
  `NoDeviceIdentifier` line), and none of them fails when missed. The compiler asks only about
  `DeviceKey::config_key` and the shape in `setup_report` (`src/home.rs:DeviceKey`).

### 8. A complete listing that no key matched is the only NotHome there is

Given a parsed, complete listing When no configured key matched any client Then the verdict is `NotHome`,
and every lesser answer is `Unknown`

- Success: `src/home.rs:home_reading`'s `None` arm. Pinned by
  `src/home.rs:a_complete_listing_no_key_matched_is_not_home_and_anything_less_is_unknown`, which asserts
  NotHome for a complete one-client listing and for `{"totalCount":0,"data":[]}`, and Unknown for the
  captive-portal page, the unauthorized shape, and the incomplete page.
- Failure sources: Not applicable. This arm is reached only when the listing was had.
- Fail direction: THE CENTRAL ONE. A router that will not answer reads UNKNOWN, which is neither home nor
  away. The code says why in the module comment: the future consumers suppress or replay on transitions,
  so "inventing 'the device left' out of an unreachable router would fire a false transition; Unknown is
  the reading that changes nothing." The operator-facing line for that state is
  `home: unknown (router unreachable or its answer unreadable)`, which names both causes without claiming
  either.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: no failure of any kind may produce `NotHome`.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure.
- Privacy: the NotHome line quotes nothing at all:
  `home: NOT on the home network (no configured identifier matched a client)`.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `HomePresence::Home` carries its own evidence (`matched_by` and `value`) INSIDE
  the variant, so there is no matched-key field for a `NotHome` to fill in (`src/home.rs:HomePresence`).

### 9. Every configured key reports what it found, in precedence order

Given a Home, NotHome or complete-listing reading When the evidence is built Then there is exactly one
`KeyReading` per CONFIGURED key, in precedence order, each saying whether it matched the client the
verdict names, matched a different client (named), or matched nothing

- Success: `src/home.rs:home_reading` maps `configured` (already in precedence order) into `KeyReading`s,
  and `src/home.rs:report` prints one line each, reading
  `home:   <key> "<value>" matched the client the verdict names`,
  `home:   <key> "<value>" matched a different client "<label>"`, or
  `home:   <key> "<value>" matched no client`. Pinned by
  `src/home.rs:a_reading_carries_one_entry_per_configured_key_in_precedence_order` (an unset key is
  skipped rather than reported absent, and a table listing the address first still reports client name
  before address),
  `src/home.rs:every_key_that_found_the_client_the_verdict_names_is_marked_as_this_device`,
  `src/home.rs:a_key_that_found_another_client_than_the_verdict_names_says_which_one`,
  `src/home.rs:not_home_says_every_key_matched_no_client_and_unknown_says_nothing_at_all` and
  `src/home.rs:the_evidence_under_the_verdict_says_what_each_key_found_escaping_the_label`.
- Failure sources: none of its own.
- Fail direction: an `Unknown` reading carries an EMPTY key list, not a list of keys that found nothing.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: "this device" must be decided by MEMBERSHIP IN THE WINNER'S ENTRY, never by an
  index comparison against wherever a key first matched. A listing can carry one value twice, and asking
  "which entry did this key find first" would answer out of the router's listing ORDER, so reversing two
  entries would flip the same physical state between agreeing and stale and the episode would flap on
  nothing (`src/home.rs:home_reading`). Pinned by
  `src/home.rs:a_key_the_winners_own_entry_carries_is_this_device_in_either_listing_order`. The
  `MatchedDevice` line also refuses to claim identity with the operator's hardware: it says "matched the
  client the verdict names", because when the winning key is itself the stale one the entry it names
  belongs to somebody else (`src/home.rs:report`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: order-independent by construction, which is the point above.
- Privacy: a `MatchedOtherClient` outcome carries another client's LABEL, chosen as the first identifying
  field: its name, else its media access control address, else its address, and spelled with Rust's debug
  escape at the point it is captured rather than at the print (`src/home.rs:client_label`). A field that
  is present but empty is not identifying. A client carrying no identifying field at all reads as
  `an unnamed client`. That label reaches the terminal only, never an alert (behavior 12).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the evidence is never withheld. A hand-run diagnostic answers "why did it read
  that" as much as "what did it read", so every configured key says what it found on every run, however
  many times it has said it before (`src/home.rs:report`).

### 10. A Home verdict with a key pointing elsewhere is a staleness

Given a reading When the verdict is Home and at least one configured key did not match the client the
verdict names Then that is a `Staleness` naming the winner and every disagreeing key, and nothing else is

- Success: `src/home.rs:stale_identifiers` returns `None` unless the presence is `Home`, then keeps every
  key whose outcome is not `MatchedDevice`. Pinned by
  `src/home.rs:a_staleness_is_a_home_verdict_with_a_key_pointing_somewhere_else`, whose four negative
  cases are: every key found the same client, only one key is configured, NotHome, and Unknown.
- Failure sources: none of its own.
- Fail direction: AWAY IS NOT STALE and UNKNOWN IS NOT STALE. A NotHome reading has every key matching
  nothing, which is what being away IS, and an Unknown searched nothing at all: warning on either would
  fire every time the operator left the house. ONE configured key is never stale either, because a lone
  key has nothing to disagree with (`src/home.rs:stale_identifiers`).
- Thresholds: one configured key can never be stale; two or more can. There is no time threshold at all:
  staleness is a disagreement inside ONE reading, never an age.
- Required side effects: none.
- Forbidden side effects: no attempt is made to merge two listed entries into one device.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure over the reading.
- Privacy: a `Staleness` holds `KeyReading`s, so it can hold another client's label. Only the KEY NAMES
  leave it (behavior 12).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: KNOWN CEILING, stated in the module comment as FALSE STALENESS: one physical
  device listed TWICE (wired beside wireless, or a roaming re-association the router has not aged out)
  puts the keys on different entries legitimately, and this reads that as a disagreement. It takes two
  entries carrying DIFFERENT fields to get there, because a key the winner's entry also carries is read
  off that entry, so a duplicate answering to the same name changes nothing and cannot flip the reading
  by being listed first. Merging is not attempted, "because the router gives no answer to 'are these the
  same device' that is not a guess"; the evidence names the other client instead, so the operator can see
  a duplicate for what it is.

### 11. The episode identity spells the state and never a value that can move on its own

Given a staleness When it is given an identity for the memory Then the identity is the winner's key name
followed by each disagreeing key name and whether it found another client or nothing, in precedence order

- Success: `src/home.rs:episode_id` produces, for the operator's own case,
  `device_mac device_hostname=none device_ipv4=other`. Pinned by
  `src/home.rs:an_episode_identity_spells_the_state_and_never_the_values_that_moved`, whose second half
  moves every value (a different stale address, a different client under the name, a different label on
  it) and asserts the identity does not change, and by
  `src/home.rs:a_changed_stale_set_outcome_or_winner_each_spell_a_different_identity`, which produces
  four distinct identities from one listing by moving the stale SET, the outcome, and the winner.
- Failure sources: none. The function is total. The `MatchedDevice` arm is unreachable from
  `stale_identifiers` and is spelled `device` rather than panicked on, "because an identity is not the
  place to discover an impossible state".
- Fail direction: Not applicable. There is no failure to direct.
- Thresholds: Not applicable, and deliberately: there is no time in the identity, so a state that has
  stood for a day is the same state it was.
- Required side effects: none. This function only spells.
- Forbidden side effects: no matched value, no client label, no client count and no timestamp may enter
  the identity, because `device_ipv4` drifting under DHCP is the same stale state it was yesterday and
  the operator has already been told.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the same state always spells the same identity, which is the whole
  mechanism.
- Privacy: the identity is made of compiled-in config key names and the words `device`, `other` and
  `none`. This matters because the identity is the ONE thing this probe writes to disk (behavior 13).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: a change to the spelling invalidates every remembered episode on the machine,
  which costs one repeated warning.

### 12. A staleness is said out loud once per state, in the terminal and as one alert

Given a Home reading with a disagreement When `pns home` runs Then the verdict and the full evidence
print every time, and the one warning sentence prints and is delivered only when its episode differs from
the remembered one

- Success: `src/main.rs:home_mode` derives the staleness once, spells the episode once, decides news once
  (`src/home.rs:is_new_staleness`), and feeds ONE `Option` to both the printed report and the alert, so
  "there is no second condition that could deliver what was not printed, or print what was not
  delivered". The sentence itself is one function with two readers, `src/home.rs:stale_warning`,
  byte-identical in the terminal and in the alert body. The terminal reads, verbatim:
  `home: an identifier looks stale: device_hostname, device_ipv4 disagree with device_mac`, and the verb
  agrees with what it names (`disagrees` for one key). The delivered event carries `agent` `pns`, `state`
  `stale`, `title` `pns · stale`, `detail` and `message` both the warning sentence, and empty `pane`,
  `project` and `branch`. Pinned by
  `tests/dispatch.rs:a_new_stale_state_is_delivered_as_one_alert_carrying_the_warning_sentence`,
  `tests/dispatch.rs:the_same_stale_state_alerts_once_and_a_returning_one_alerts_again`,
  `src/home.rs:the_stale_warning_is_one_sentence_that_agrees_with_the_keys_it_names` and
  `src/home.rs:the_staleness_line_names_the_disagreeing_keys_and_prints_only_when_it_is_news`.
- Failure sources: an unreadable or unwritable state directory (the same state is news every run); a
  delivery the gateway refuses (the episode is consumed anyway); an unusable `stale_alert_channel` (the
  route falls back to the default).
- Fail direction: TOWARD DUPLICATES, chosen and stated. The dispatch happens BEFORE the remember, so a
  crash, a wedged channel or a kill between the two re-alerts on the next run instead of losing the
  alert, and two overlapping hand runs that both read the memory before either writes both alert:
  "Duplicates are the direction to fail in" (`src/main.rs:home_mode`).
- Thresholds: no time threshold anywhere. The dedupe is over the episode VALUE, not over a window or a
  count. Zero disagreeing keys is no staleness; one is a staleness with the singular verb.
- Required side effects: on news, exactly one event through `src/main.rs:run_event` with
  `EventArgs { agent: "pns", state: "stale", detail: <the warning>, channel: <the route>, ..Default::default() }`,
  plus whatever that shared event path records for any event. On a Home reading, the memory write of
  behavior 13.
- Forbidden side effects: the alert must not narrow itself and must not raise a pulse: "Nothing narrows
  it and it is not long-running, so it raises no pulse" (`src/main.rs:home_mode`). A RESOLVED staleness
  must not be announced: an all-clear for a warning the operator may never have read is one more thing to
  read (`src/home.rs:is_new_staleness`). The alert can never be delivered to the router itself (behavior
  1).
- Timeout and cancellation: the alert rides the ordinary event path and inherits its deadlines. The
  delivery outcome is deliberately not consulted before the memory is written.
- Idempotency and duplicates: one alert per episode, by the memory. Two concurrent runs can both alert,
  by design (see fail direction).
- Privacy: NOTHING THE ROUTER SAID IS IN THE ALERT. Every value in the sentence is a compiled-in config
  key name, so no client label, no address and no matched value can ride it out to a channel; the
  evidence that does carry router text stays in the terminal, escaped (`src/home.rs:stale_warning`).
  Pinned by `tests/dispatch.rs:the_alert_carries_no_secret_and_no_raw_router_text`, which feeds a client
  whose name carries a quote and an ANSI screen clear and asserts the terminal shows the escaped form
  (`matched a different client "mo\"use\u{1b}[2J"`) while the delivered `detail` is the key-names-only
  sentence, and that neither the router key nor the hermes signing key appears on stdout, on stderr, or
  in the delivered event.
- Process ownership and cleanup: `src/main.rs:home_mode` builds `system_probes()` and hands it to
  `run_event`; whatever children that path starts are its own contract, not the probe's.
- Compatibility contract: the alert goes to a hermes ROUTE, never a URL. `stale_alert_channel` names the
  route; unset means the default route (`/webhooks/pns`), the same spelling `--channel` and
  `hermes_url_for` use (`src/home.rs:stale_alert_channel`). A value that is not a usable route name (not
  a string, empty, or carrying anything but ASCII letters, digits, `-` and `_`, per
  `src/safety.rs:route_name_is_usable`) falls back to the default route with one complaint on stderr and
  the alert still goes out, because "a diagnostic that can be taken down by its own settings is not one".
  Pinned by `src/home.rs:a_usable_stale_alert_channel_is_read_back_as_the_route_verbatim`,
  `src/home.rs:no_stale_alert_channel_at_all_asks_for_the_default_route_in_silence`,
  `src/home.rs:a_stale_alert_channel_that_is_not_a_usable_route_complains_and_falls_back`,
  `tests/dispatch.rs:an_unusable_stale_alert_route_complains_and_still_delivers_the_alert`, and, over a
  real proxied gateway, `tests/native.rs:the_stale_alert_posts_to_the_hermes_route_the_config_named`,
  which asserts the request line is `POST /webhooks/priority HTTP/1.1` while the `Host` header is still
  the compiled-in `127.0.0.1:8644`, so naming a route never moves the gateway.

### 13. Only a Home reading may change the remembered staleness

Given a remembered episode on disk When a reading comes back NotHome or Unknown Then the memory is left
exactly as it was, and only a Home reading writes or clears it

- Success: `src/main.rs:home_mode` guards the write with
  `if matches!(reading.presence, HomePresence::Home { .. })`. `src/main.rs:remember_staleness` writes the
  episode line when there is one and unlinks the file when there is not.
  `src/main.rs:remembered_staleness` reads the file, trims it, and treats an empty file as no memory. The
  file is `<state>/home-staleness` (`src/main.rs:STALENESS_MEMORY`), one line, published by rename at
  mode `0600` (`src/main.rs:publish_state_line`, `src/main.rs:STATE_FILE_MODE`). The state directory is
  `$HOME/.local/state/pns` unless `PNS_STATE_DIR` overrides it (`src/main.rs:state_dir`). Pinned end to
  end by
  `tests/dispatch.rs:the_home_diagnostic_always_shows_the_evidence_and_warns_once_per_stale_state`, which
  asserts the file holds `device_mac device_hostname=none device_ipv4=other` after the first sighting,
  that a resolved state deletes it, that a trip AWAY leaves it in place, and that an UNREADABLE answer
  leaves it in place.
- Failure sources: a state directory that cannot be created, read or written (the integration test
  substitutes a regular FILE for the directory, which breaks every read and every write of it).
- Fail direction: FAIL-QUIET. "an unwritable state directory must never change a verdict, fail the
  diagnostic, or crash. The cost of a failed write is one repeated warning"
  (`src/main.rs:remember_staleness`). Pinned by
  `tests/dispatch.rs:a_state_directory_that_cannot_be_used_leaves_the_whole_diagnostic_standing`, which
  asserts the same warning twice, exit 0 both times, and the blocking file left as it was.
- Thresholds: none. The memory has no expiry, no age and no cap. It holds one line.
- Required side effects: at most one file, `<state>/home-staleness`, created with mode `0600` and
  re-narrowed on the open HANDLE after creation, published by a rename from a pending path in the SAME
  directory carrying this process's id.
- Forbidden side effects: a NotHome or Unknown reading must not write and must not unlink. "Away and
  unreadable leave the memory untouched, so the warning stays once per STATE rather than once per
  homecoming" (`src/main.rs:home_mode`). Without that guard the warning would fire once per homecoming,
  which for a phone is once a day.
- Timeout and cancellation: Not applicable. Local file operations only.
- Idempotency and duplicates: writing the same episode twice is the same file. A rename that fails
  removes the pending file so nothing half-written is left for the next run to trip over
  (`src/main.rs:publish_state_line`).
- Privacy: the file holds ONLY the episode identity: config key names and the words `device`, `other`,
  `none`. No device identifier, no client label, no timestamp, no session id. There is one memory per
  machine because one config names one device (`src/main.rs:STALENESS_MEMORY`).
- Process ownership and cleanup: the pending file is named for this process id so two runs publishing at
  once cannot share one, and it is removed on a failed rename.
- Compatibility contract: `PNS_STATE_DIR` relocates the whole state directory, which is how the tests
  drive it.

### 14. The device identity is validated once, at the config read

Given a `[plugins.router]` table When the settings are read Then `type` is settled first, then
`router_url`, then the device keys, and every value that reaches a comparison has already been parsed

- Success: `src/home.rs:router_settings` settles `type` before reading anything else, "because every
  setting under it belongs to whichever router it names", then requires a non-empty string `router_url`,
  then calls `src/home.rs:device_identity`, which reads the client name as free text, the address through
  `str::parse::<Ipv4Addr>`, and the media access control address through `src/home.rs:normalized_mac`,
  and refuses a table setting none of the three. `src/home.rs:DeviceIdentity` is constructible only
  through that function, "so an unparsed value cannot reach a comparison". Pinned by
  `src/home.rs:an_enabled_unifi_router_table_yields_its_url_and_device`,
  `src/home.rs:a_router_table_naming_no_device_at_all_is_refused_naming_every_key`,
  `src/home.rs:a_well_formed_mac_in_any_case_or_separator_validates_to_one_spelling`, and the two
  malformed-key tests.
- Failure sources: the ten members of `src/home.rs:SetupFailure`, each with its own line (see the failure
  table).
- Fail direction: refuse and name the edit. "one 'not configured' covering both a missing table and a
  mistyped value sent the operator to write a table they already had" (`src/home.rs:SetupFailure`). A key
  present but EMPTY is refused rather than read as absent, because a blank key read as absent is the
  silent typo the output cannot show (`src/home.rs:read_device_key`).
- Thresholds: at least one of the three device keys. Zero is `NoDeviceIdentifier`; one is enough to read,
  and is the case that can never be stale (behavior 10).
- Required side effects: none. Every function here is value in, value out.
- Forbidden side effects: the API key must not join `RouterSettings`. It stays its own read "so it never
  joins the settings in a type that could be dumped whole" (`src/main.rs:home_mode`,
  `src/home.rs:router_api_key`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure over the table.
- Privacy: a refusal QUOTES what the operator wrote, escaped: a string in Rust's debug quotes, anything
  else by its TOML type (`<integer>`, `<boolean>`) because "a non-string has no spelling worth echoing
  back and its TYPE is what has to change" (`src/home.rs:spell`). Control bytes in a quoted value reach
  stdout as their escape, pinned by
  `src/home.rs:an_unknown_type_with_control_bytes_is_escaped_like_every_other_spelled_value`. The
  `api_key` is never quoted back on any path: its absence is reported without its value
  (`src/home.rs:setup_report` for `NoApiKey`).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `src/home.rs:UNIFI_TYPE` is `"unifi"` and is the only type a compiled-in
  backend answers. It is validated and then DISCARDED, because `trait Router` is the seam a second
  backend enters through. It is public for one diagnostic: `pns doctor` names it when it warns about a
  router table that is switched off and names no backend (`src/main.rs:disabled_backend_warning`,
  `src/main.rs:disabled_backend_warnings`). The setup wizard refuses to arm a probe whose backend nothing
  answers (`src/setup.rs:router_is_armed`,
  `src/setup.rs:a_backend_the_home_probe_cannot_answer_declines_the_probe_rather_than_arming_it`).

### 15. `pns home` is a diagnostic: it always exits 0 and always says which state it is in

Given any machine, configured or not When the operator types `pns home` Then exactly one report is
printed on stdout, the exit code is 0, and the api_key never appears

- Success: `src/main.rs:main` dispatches the first argv token `home` to `src/main.rs:home_mode`, which
  returns rather than exiting with a code, so the process exits 0 on every path. Every cause is decided
  in the library so each line is pinned by a value-in, value-out test, and the wiring only chooses which
  one to print (`src/main.rs:home_mode`). Its own doc comment states the contract: "A DIAGNOSTIC FIRST:
  it always exits 0 and says what it found, including every way it can be unconfigured, because its job
  is to answer 'why did the probe not read' as much as 'is the device home'. The key itself is never
  printed, on any path." Pinned by
  `tests/dispatch.rs:every_way_the_home_probe_is_not_set_up_says_which_one_it_is`, which asserts eleven
  exact lines through the real binary, and by
  `src/home.rs:every_setup_failure_line_names_what_to_look_at`, which asserts every line starts with
  `home: ` and names the thing to look at.
- Failure sources: everything in the failure table. None of them changes the exit code.
- Fail direction: always exit 0, always print a line. A diagnostic that failed silently would be the
  defect it exists to find.
- Thresholds: Not applicable at this layer.
- Required side effects: exactly one `println!` of the report (the verdict, then one evidence line per
  configured key, then the warning when it is news). On an unusable route, one complaint on STDERR before
  the report, said on every run rather than only on a run that has something to deliver
  (`src/main.rs:home_mode`).
- Forbidden side effects: the api_key must never be printed, on any path. A stale alert is the only
  delivery this mode can make, and only from a Home reading that is news.
- Timeout and cancellation: bounded by the transport's deadlines (behavior 4) plus whatever the event
  path spends when an alert is raised.
- Idempotency and duplicates: re-running is safe and is the normal way to use it. The only run-to-run
  state is the staleness memory.
- Privacy: stdout carries the operator's OWN configured identifiers (debug-quoted) on every evidence
  line, and, when a key disagrees, one other client's label out of the router, escaped. Stdout carries no
  api_key, no site id, no full listing, and no other client's fields beyond that one label.
- Process ownership and cleanup: no long-lived child. The mode returns after at most one event dispatch.
- Compatibility contract: the usage text names it as
  `pns home                         one reading of the router, said out loud` (`src/main.rs:USAGE`). The
  word `home` is matched as argv's FIRST token and the rest of argv is ignored, so `pns home --help`
  takes a reading rather than printing the usage: the help arm lives in the producer parser, which this
  mode never reaches (`src/main.rs:main`, `src/main.rs:is_producer_argv`). NOT ESTABLISHED: no test pins
  the behavior of `pns home` with extra arguments.
