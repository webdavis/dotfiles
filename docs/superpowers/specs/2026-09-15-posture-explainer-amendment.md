# Amendment: the posture explainer under direct hermes delivery

Status: amendment, written 2026-09-15. It amends
`2026-09-14-posture-critical-page-investigator-design.md` and replaces that document's Decision 1
and its Build plan. Everything else in the parent stands unchanged: the sandbox
(`platform_toolsets.webhook: ["no_mcp"]`), the fixed four-part structure, the label line, the
1200-character cap, the no-commands rule, the injection analysis, and the rule that the day this
route needs a tool is the day the investigation pipeline is the right answer instead.

## What changed under it

The parent assumed pns posts posture's pages and that the explanation is one more pns delivery
leg. That is no longer how a page leaves this machine. `dot_config/posture/private_config.toml.tmpl`
ships `mode = "hermes"`, so posture signs and posts its own pages with one key per route
(`posture/crates/posture-adapters/src/hermes.rs`), and `severity_route` in
`posture/crates/posture-domain/src/severity.rs` now sends `Critical` to `priority` and everything
below it to `posture-pages`. The held flip the parent named as a prerequisite has landed.

pns is out of posture's path on this host for the reason CLAUDE.md gives: pns commits its ledger
before it tries a destination, so a critical page the gateway refused comes back as an acceptance
and the cursor advances past a finding nobody saw.

The route table also moved. `private_dot_hermes/modify_private_config.yaml` declares five routes
(`general`, `pns-events`, `posture-pages`, `priority`, `uu-runs`), all `deliver_only`, and builds
BOTH KeePassXC titles from the route name: `Hermes :: Webhook Secret (#<route>)` and
`Discord (Uriel) :: Channel ID (#<route>)`. The map is written whole, so a route it does not
declare is removed by the next apply.

## Measured against the installed hermes, 2026-09-15

`hermes --version` reports version 0.17.0, dated 2026.6.19, at upstream commit a4091e49. Every
line below was read or run today against `~/.hermes/hermes-agent` and the live
`~/.hermes/config.yaml`.

- The tool surface resolves as the parent recorded. Calling `_get_platform_tools` from
  `hermes_cli/tools_config.py` against the live config: absent gives
  `clarify, codegraph, cua-driver, qmd, scalebar, vision, web`; `[]` gives
  `codegraph, cua-driver, qmd, scalebar`; `["no_mcp"]` gives nothing at all; `["no_mcp", "safe"]`
  gives `image_gen, safe, vision, web`. The live config declares no `webhook` key under
  `platform_toolsets`, so today's webhook agent surface is the seven.
- `deliver_only` is what makes a route post verbatim. `gateway/platforms/webhook.py` line 614
  branches into `_direct_deliver` and returns; without it the handler returns `202` and the
  AGENT'S RESPONSE is what reaches Discord. One POST resolves one route and runs one handler; there
  is no fan-out and no follow-on.
- The signature is validated before anything dispatches (the auth block ends at line 507, the
  agent task is created at line 713), so an unsigned probe answers 401 and starts no model run.
  `.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl` probes every route with an
  unsigned `{}` on every apply, which is therefore safe against an agent route.
- The delivery identifier is the first of `X-GitHub-Delivery`, `svix-id`, `X-Request-ID`, and
  failing all three a millisecond timestamp (lines 588 to 592). `Idempotency-Key`, which pns sends
  and posture does not, is ignored.
- **The duplicate cache is keyed on the delivery id ALONE, gateway-wide.** `_record_delivery_id`
  (line 331) indexes one `_seen_deliveries` dict shared by every route, and it runs before the
  `deliver_only` branch. Two posts carrying the same `X-Request-ID` within the one-hour window are
  one delivery even when they go to different routes: the second answers
  `{"status": "duplicate"}` and never runs.
- `deliver_extra` values are rendered by the same renderer as the prompt, and an unknown
  placeholder comes back as its own literal braces (`_render_prompt`, line 871). A thread id is
  attached only when the rendered value is truthy (lines 1018 to 1020), so an empty string degrades
  cleanly and an ABSENT key renders the literal `{thread_id}`, which does not.
- `agent.max_turns` is 150, `agent.gateway_timeout` is 1800, the root `agent.system_prompt` runs
  to roughly 2,800 characters and no route can override it, and `gateway.multiplex_profiles` is
  absent in both spellings.

## Decision 1: posture posts the explanation leg

**Recommendation: posture posts it, as a second signed post to the shared route, after its own post
came back delivered.** This is the parent's PR 2 moved from pns into `posture-adapters`.

The other two candidates are refused on measured grounds:

- **pns.** CLAUDE.md settles it: until a producer reports the destination's own answer rather than
  its ledger, posture posts its own pages, and pns never sees a posture page on this host.
- **The `priority` route in agent mode.** `deliver_only` is the only thing that makes a page post
  verbatim. Removing it to get an explanation would replace the deterministic page with model
  prose, and there is no second handler to add one next to it. This is not a tuning choice, it is
  the whole route.
- **A host watcher over posture's results log.** Unchanged from the parent: a sixth background job
  to do what a delivery already does.

The parent's constraint 6 said posture must not learn the feature exists. That form of the rule
cannot survive this change, because the only process holding both the page and the gateway's answer
is posture. The form that does survive, and is the one the `uu AND posture NAME NO ENGINE` ruling
actually protects, is that **posture learns about destinations, never about agents**. It already
holds two route names and a key for each. A third route name in the same table teaches it nothing
about what is behind it.

The spelling, one optional key in a table posture already parses
(`posture/crates/posture-adapters/src/delivery/schema.rs`):

```toml
[delivery.hermes]
url = "http://127.0.0.1:8644/webhooks"
# A second route a CRITICAL page is copied to, after its own post came back
# delivered. Absent is one post, which is the default.
critical_copy_route = "explain"
```

The copy is byte-identical to the page's own body, which is what the route's prompt renders over.
Three properties fall out of posting it second rather than first: the page is already in the channel
before the copy is attempted; a keyless or 404 copy route refuses loudly through the same path a
keyless page route already uses, without touching the page's own outcome; and an absent key is
exactly today's behavior.

## Decision 2: one shared `explain` route

**Recommendation: one route, named `explain`, serving posture and uu triage both.**

The senders are already two and will stay two. posture posts its own pages. uu does not: its failure
alert spawns pns (`uu/crates/uu-adapters/src/alert.rs`), and only its weekly record posts directly,
to `uu-runs`. So uu triage's explanation will be posted by pns, whose hermes body
(`pns/crates/pns-adapters/src/destinations/hermes.rs`) and posture's
(`posture/crates/posture-adapters/src/hermes/body.rs`) share seven keys: `agent`, `state`,
`project`, `detail`, `header`, `subheader`, `body`. One prompt over `{header}`, `{subheader}` and
`{body}` renders both.

What a second route would buy, and why none of it is worth a route:

- **Separate budgets.** It does not buy them. The cap is a sender-side rule either way (an agent
  route always replies, so silence lives in the sender), and the ChatGPT quota is one pool.
- **Separate prompts.** The instruction is the same instruction: explain a machine page. The day it
  is not, a second route is an additive change, not a migration.
- **Separate secrets.** True, and it costs a second vault entry the operator creates by hand plus a
  second copy of the prompt, for a blast radius that is one Discord channel either way.

The cost of one route is that its name is no longer its channel's name, which the modify template
currently assumes when it builds `Discord (Uriel) :: Channel ID (#<route>)`. Pay it by letting a
route declare which channel entry it reads, defaulting to its own name. That restructuring is needed
regardless, because an agent route also differs in `deliver_only` and in its prompt.

## Decision 3: what the operator does first

Exactly one new KeePassXC entry, created and populated BEFORE the route is declared. A title
`keepassxc-cli` cannot find aborts the whole apply, and an entry that exists with an empty Password
renders an empty secret, which makes the gateway refuse the request.

| Title | Field | Value |
| --- | --- | --- |
| `Hermes :: Webhook Secret (#explain)` | Password | a fresh random secret, 32 bytes or more |

Nothing else is needed from the vault: the channel is `#priority`, whose
`Discord (Uriel) :: Channel ID (#priority)` entry already exists and already renders a 19-digit
snowflake into the live route.

The hermes fields the route carries, all in `private_dot_hermes/modify_private_config.yaml`:

- `secret`, from the entry above.
- `deliver: discord`.
- **No `deliver_only` key at all.** Its absence is what makes this an agent route.
- `prompt`, the explainer instruction from the parent design, with the page wrapped in a delimiter.
- `deliver_extra.chat_id`, read from the `priority` channel entry.
- **No `deliver_extra.thread_id` yet.** posture's body carries no `thread_id` key, so the
  placeholder would render as its own literal braces and be sent to Discord as a thread id. Threads
  wait for a sender that always emits the key, as pns already does.
- `platform_toolsets.webhook: ["no_mcp"]`, a third path the modify template owns alongside the
  routes map and the voice id.

After the apply: `hermes gateway restart`. A route is loaded at gateway start, and run_after_68
reports a configured-but-unloaded route as a 404.

## Decision 4: the corrected build plan

**PR 1, hermes configuration and the checker carve-out.**
`private_dot_hermes/modify_private_config.yaml`: turn the name-to-prompt dict into a name-to-fields
dict, add the `explain` route as Decision 3 describes, and add the `platform_toolsets.webhook` path
to the owned set. `.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl`: add `explain` to
`expected_routes`, keep the secret and channel checks for it, and invert the `deliver_only` check
for that one route so the checker still catches a delivery route that silently became an agent
route. **No tests.** Both files are declarations, and a test asserting a config key exists is
deleted on sight under the 2026-08-05 scope ruling. The gate for the shell template is
`just lint-check`; the gate for the modify template is the operator's apply, then the resolver
reporting an empty webhook toolset and one signed hand POST to `/webhooks/explain`.

**PR 2, posture's copy leg.** `delivery/schema.rs` gains the optional key, `delivery.rs` carries it
into `DeliveryPath::Hermes` and the sink, `hermes.rs` posts the copy after a delivered critical
post, a new `hermes/window.rs` holds the rolling-hour decision over a timestamp file under
`~/.local/state/`, `signed_post.rs` gains the `X-Request-ID` header, and
`dot_config/posture/private_config.toml.tmpl` gains the `explain` key and the
`critical_copy_route` line. The request id reuses the derivation already in
`producer/request.rs`, lifted into a pure function both paths call, and **the copy carries a
DISTINCT id derived from the page's**, because the gateway's duplicate cache is keyed on the id
alone across routes and would otherwise swallow the copy for an hour. Behaviors worth a test, each
one behavior of a tool we wrote:

1. a delivered critical page produces exactly one copy, to the copy route, signed with that route's
   key;
1. a Notice page produces no copy;
1. a critical page whose own post failed produces no copy, and its failure is unchanged;
1. a copy route holding no key produces no copy and does not turn a delivered page into a failure;
1. the seventh critical page inside one rolling hour produces no copy and says so;
1. an absent `critical_copy_route` produces exactly one post;
1. every post carries `X-Request-ID`, and a page and its copy carry different ones.

**PR 3, documentation.** The ledger entry, and the hermes section of
`docs/runbooks/local-daemons.md`: the route under "The routes", posture as its second sender under
"Which sender reads which secret", and three gotchas worth adding to the three already there, which
are the `[]`-leaves-`cua-driver` trap, the cross-route duplicate cache, and the literal-braces
rendering of an absent placeholder.

## Assumptions made in the operator's place

1. **The route is named `explain`**, not `posture-explain`, because it serves two senders.
   *Alternative:* one route per sender, priced in Decision 2.
1. **posture learns a route name, not a feature.** *Alternative:* wait for the producer API to
   report the destination's answer so pns can own the leg again, which defers this indefinitely.
1. **The copy is posted after the page's own post reports delivered**, which is the ordering the
   parent said the build should enforce and this one can, because both posts are in one process.
1. **The cap is six per hour PER SENDER**, so two senders can produce twelve in the worst hour.
   *Alternative:* a shared budget, which needs a store both senders write and buys little while the
   two page types are this rare. hermes's own per-route limit is not usable as a budget: it is one
   gateway-wide `rate_limit` value applied to all five delivery routes.
1. **`critical_copy_route` ships uncommented naming `explain`**, per the defaults-visible ruling,
   with its absence as the off state for anyone else who installs posture.
1. **The rolling-hour window is a timestamp file under `~/.local/state/`**, matching where posture
   already keeps its cursor and digest spool.
1. **PR 2 changes posture only.** pns gains the same leg when uu triage is built, which is what the
   shared route makes additive.

## Open questions

1. Is `platform_toolsets.webhook: ["no_mcp"]` acceptable knowing every future webhook agent route
   on this gateway shares it, and that the only way to differ is `gateway.multiplex_profiles` plus
   the unwinding of two gateway LaunchAgents?
1. Six per hour per sender, or a shared budget across posture and pns?
1. Does `explain` keep its own channel entry pointing at the priority channel, or does the modify
   template learn a per-route channel-entry name? The second avoids a second copy of one snowflake
   in the vault and is the recommendation, but it changes a template the operator applies.
