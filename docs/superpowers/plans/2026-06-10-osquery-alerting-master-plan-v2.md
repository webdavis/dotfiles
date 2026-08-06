# osquery Alerting — Master Implementation Plan (v2)

> **For agentic workers:** REQUIRED SUB-SKILLS: superpowers:test-driven-development (failing test → run
> red → minimal implementation → run green → full suite green → commit per task; a previously-green test
> turning red is a regression and is **not allowed**) and superpowers:subagent-driven-development (or
> executing-plans). Steps use `- [ ]` checkboxes. **This is the implementation plan; the planning run
> that produced it was docs-only. Building it changes code and requires the normal human approval gates
> (esp. anything touching `dot_hermes/`).**

**Goal:** Re-shape the osquery alerter from two tiers (page / log-only) to **three** (`page/core`,
`digest/suspicious`, `log-only/noisy`) so the one watched Discord channel stays calm, trustworthy, and
ADHD-compatible — pages only for immediate/high-confidence/actionable/**rare** threats;
useful-but-ambiguous events become an empty-suppressed daily digest; noisy/forensic events stay log-only.

**North star:** the calm channel is a security requirement — a recurring false positive trains the
ignore-response and then spreads to the real alerts. Silence must mean "nothing needs you." (Spec §1.)

**Scope:** **Dresden single-host**, via this chezmoi/dotfiles repo. Multi-host/fleet is the deferred
homelab migration *out* of this repo; v2 builds migration seams only (Spec §2). **No fleet features
here.**

**Architecture:** one default-deny gate, three outcomes — page (fall through to `send_alert`), digest
(`_digest_append` then `continue`), log-only (`continue`). Separation is structural: the digest store has
exactly one append call site. (Spec §3.)

**Artifact map:** Spec `specs/2026-06-10-osquery-alerting-master-spec-v2.md` · Tier matrix
`specs/2026-06-10-osquery-alerting-tier-matrix-v2.md` · Test matrix
`specs/2026-06-10-osquery-alerting-test-matrix-v2.md` · Decision addendum
`decisions/2026-06-10-osquery-alerting-v2-decision-addendum.md`. v1 plan/spec remain as history
(superseded tiering).

**Tech stack:** bash, jq, osquery (official cask, ES-entitled), chezmoi, **bats-core**, pytest,
shellcheck/shfmt, the Nix `run` shell. (`pkgs.bats` + `pytest` already in the flake from v1 Task 0.)

______________________________________________________________________

## v1 critique summary (what v2 fixes)

Evidence-backed against the live `results.log` and files (full detail in the decision addendum +
matrices):

- **Calm/FP:** `kernel_extensions_new` pages on a 657-event load/unload firehose (wrong signal);
  `system_extensions_new` pages on app-upgrade churn; `pipeline_integrity` pages on **every
  `chezmoi apply`**; user-LaunchAgents page on routine tool installs; sudoers (19 real events) lumped
  with quiet authorized_keys/sshd_config.
- **Dead/false-assurance pages:** `sip_state` (SIP off → no transition; + a stale `security-regression`
  name silently dropped), `remote_access_sharing_state` (never emits a deliverable row),
  `screenlock_state` (0 rows ever).
- **Missing middle tier:** no digest at all → ambiguous signals had nowhere calm to go.
- **Allowlist:** regex rejects the live label `homebrew.mxcl.postgresql@17` (no `@`); the alerter reads
  `launch-allowlist.txt` while the plan writes `page-launchd-allowlist.txt` (**every allow is a silent
  no-op**); no `deny`/`list`; no confirmation feedback; the allowlist file (a page-suppressor) is itself
  unwatched; the Hermes plugin API is **unverified** yet written as concrete code with pytest mocking the
  assumptions.
- **Delivery/TDD:** Task 8's spool test can't work (the harness stubs `send_alert` entirely); the
  `_drain_spool` runs under `set -e` and could abort the alerter; protection-off tests assert only the
  *negative* (the one behavior justifying the pack is unverified); fixtures are toy
  `{name,action,columns}` missing the real 9-key envelope; a phantom `new_ssh_key` edit; the launchd
  query is a replacement of an 11-column COALESCE query (load-bearing `removed:true` not flagged).
- **Doc hygiene:** an orphan trailing code fence (fence count 65 = odd) and stale two-tier "page is the
  only ping" prose.

## Detection tiering summary

Per the **tier matrix** (authoritative). **page/core (9 + poller):** new_admin_user, suid_bin_unexpected,
agent_exposure_changed, file_events authorized_keys + sshd_config, persistence_launchd **system**
daemons, filevault OFF, **webhook-secret + paseo `daemon-keypair.json`** (split), **pipeline_integrity on
content-mismatch** (vs a source-derived, root-owned baseline manifest); firewall/gatekeeper OFF via the
60s poller. **digest/suspicious (6):** persistence_launchd **user** agents, system_extensions_new,
agent_binary_changed, agent_authfile_changed(−secret,−paseo-keypair), sudoers, screenlock OFF.
**log-only/noisy:** kernel_extensions_new, sip_state, remote_access_sharing_state, es_launchd_writes, +
the existing drift/ports set.

## File structure

| Path                                                                   | Responsibility                                                                                          | Change |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | ------ |
| `test/osquery-alerter/lib.bash`                                        | H1 harness + realistic `row()`/`file_event_row()` builders + `digest_store`/`run_digest`/assert helpers | modify |
| `test/osquery-alerter/test_*.bats`                                     | per-tier + delivery + allowlist bats (test matrix)                                                      | create |
| `test/hermes-delivery/test_gateway_*.py`                               | Dresden-only integration (unsigned/badsig/dedup)                                                        | create |
| `dot_local/bin/executable_osquery-results-alerter.sh`                  | 3-outcome gate + `_digest_append` + render                                                              | modify |
| `dot_local/bin/executable_osquery-alert-dispatch.sh`                   | hostname in body; spool drain `set -e`-guarded; secrets never logged                                    | modify |
| `dot_local/bin/executable_osquery-digest.sh`                           | the digest builder (new)                                                                                | create |
| `dot_local/bin/executable_osquery-uptime-watchdog.sh`                  | guard the digest agent; drain spool                                                                     | modify |
| `Library/LaunchAgents/com.webdavis.osquery-digest.plist.tmpl` + loader | daily digest agent (chezmoi cadence)                                                                    | create |
| `.chezmoitemplates/osquery/osquery.conf` + packs                       | query tiering, file_paths categories, kext/sip notes                                                    | modify |
| `dot_config/osquery/page-launchd-allowlist.txt`                        | the allowlist (plain tracked file, NOT `.tmpl`)                                                         | create |
| `.chezmoi.toml.tmpl` / `.chezmoidata`                                  | `[data.osquery].digestHour/digestMinute`                                                                | modify |

______________________________________________________________________

## Task 0 — Test harness + fixture realism (commits green)

**Files:** `test/osquery-alerter/lib.bash`, a green smoke test. The flake already has bats+pytest.

- [ ] **Step 1 — realistic fixture builders** in `lib.bash` (closes the toy-JSON gap; real rows have a
  9-key envelope):

```bash
# differential row with the REAL envelope: row <name> <action> <counter> <columns-json>
row() { jq -cn --arg name "$1" --arg action "$2" --argjson counter "$3" --argjson columns "$4" \
  '{name:$name,action:$action,counter:$counter,columns:$columns,hostIdentifier:"dresden",calendarTime:"Tue Jun 10 17:00:00 2026 UTC",epoch:0,numerics:false,unixTime:1780000000}'; }
# evented file_events row: file_event_row <category> <target_path> <fsverb CREATED|UPDATED|DELETED>
file_event_row() { jq -cn --arg category "$1" --arg target_path "$2" --arg file_action "$3" \
  '{name:"file_events_recent",action:"added",counter:1,columns:{action:$file_action,category:$category,target_path:$target_path,sha256:"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",time:"1780000000"},hostIdentifier:"dresden",unixTime:1780000000}'; }
```

- [ ] **Step 2 — H2 delivery harness + digest helpers**: a `with_real_dispatch` helper that sources the
  real `executable_osquery-alert-dispatch.sh` with `curl`/`openssl`/`base64` shims on `PATH` and
  `OSQUERY_WEBHOOK_SECRET_FILE` → a fixture; `digest_store()` (cats `$OSQUERY_DIGEST_STORE`),
  `run_digest()` (runs the builder with the fixture env). The curl shim records URL+headers+body and
  returns a settable code.
- [ ] **Step 3 — green smoke test** (`test_smoke.bats`): `row new_admin_user added 0 '{}'` (a counter==0
  baseline) → `assert_no_page`. Green today.
- [ ] **Step 4:** `just test` → PASS. Commit
  `test(osquery-alerter): realistic fixtures + delivery/digest harness`.

**Acceptance gate:** every later task's fixtures use `row`/`file_event_row` (never hand-typed envelopes).

______________________________________________________________________

## Task 1 — The three-outcome gate (page / digest / log-only) + hostname

**Files:** the alerter (gate + `_digest_append`), the dispatch helper (hostname). Tests:
`test_gate.bats`, plus the per-detector page/digest tests land in Tasks 4–7.

- [ ] **Step 1 — failing tests:** `T-SEP-page-not-in-digest` (admin c1 pages AND digest store empty),
  `T-SEP-logonly-not-in-digest` (drift neither pages nor stores), `T-SEP-baseline` (counter 0 digest
  detector not stored). All red (no gate/digest yet).
- [ ] **Step 2 — `_digest_append`** in the alerter (best-effort, never fails the alerter):

```bash
ALLOWLIST="${OSQUERY_LAUNCHD_ALLOWLIST:-$HOME/.config/osquery/page-launchd-allowlist.txt}"
DIGEST_STORE="${OSQUERY_DIGEST_STORE:-$HOME/.local/state/osquery-digest-spool/digest.ndjson}"
PIPELINE_MANIFEST="${OSQUERY_PIPELINE_MANIFEST:-/var/osquery/pipeline-known-good.sha256}"  # root-owned baseline (layer 2)
_digest_append() {
  local finding="$1"
  mkdir -p "$(dirname "$DIGEST_STORE")" 2>/dev/null; chmod 700 "$(dirname "$DIGEST_STORE")" 2>/dev/null
  jq -c --arg timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{timestamp:$timestamp,detector:.query_name,category:(.columns.category//""),id:(.columns.label//.columns.identifier//.columns.target_path//.columns.path//.columns.username//"?"),action:.action,summary:(.query_name+" "+((.columns.label//.columns.identifier//.columns.target_path//.columns.path//.columns.username)//"?"))}' \
    <<<"$finding" >>"$DIGEST_STORE" 2>/dev/null || true
  chmod 600 "$DIGEST_STORE" 2>/dev/null || true
}
```

- [ ] **Step 3 — the gate** (in the enrich loop where `$query_name`/`$category`/`$severity`/`$action` are
  in scope; replace v1's two-outcome gate). Page arms set `severity=CRIT` and fall through; digest arms
  `_digest_append` then `continue`; everything else `continue`s. **Rename as you go — no abbreviations
  (readability over the v1 house style):** when you touch v1's raw-findings producer + enrich loop, emit
  and read the finding object with the readable keys this plan uses —
  `{severity, query_name, action, columns, enrichment_path}`, **not** `{sev, q, act, cols, ep}` — and
  name the loop variables `finding`/`severity`/`query_name`/`category`/`action`/`enrichment_path` so
  producer and consumer match. Likewise rename v1's dispatch identifiers `pcount`/`ocount` →
  `page_count`/`other_count`, `sig` → `signature`, and `reqid` → `request_id`. (The osquery 9-key
  envelope keys — `name`/`action`/`counter`/`columns`/`hostIdentifier`/… — are osquery's own schema and
  stay.)

```bash
action=$(jq -r '.action // ""' <<<"$finding")
case "$query_name" in
  new_admin_user | suid_bin_unexpected | agent_exposure_changed)
    severity="CRIT" ;;                                         # page/core
  agent_authfile_changed)
    case "$(jq -r '.columns.path // ""' <<<"$finding")" in
      */webhook-secret | */daemon-keypair.json) severity="CRIT" ;;  # page: pipeline HMAC key + paseo daemon auth
      *) _digest_append "$finding"; continue ;; esac ;;        # digest: .env, codex/config.toml, cli-client-id
  file_events_recent)
    case "$(jq -r '.columns.action // ""' <<<"$finding")" in CREATED|UPDATED) : ;; *) continue ;; esac
    case "$category" in
      authorized_keys | sshd_config) severity="CRIT" ;;        # page
      pipeline_integrity)                                      # page ONLY on content-mismatch (layers 1+2)
        hash_value=$(jq -r '.columns.sha256 // ""' <<<"$finding")
        [ -n "$hash_value" ] && grep -qiF -- "$hash_value" "$PIPELINE_MANIFEST" 2>/dev/null && continue || severity="CRIT" ;;  # matches source-derived manifest → silent
      sudoers | allowlist_file) _digest_append "$finding"; continue ;;  # digest (allowlist is runtime-mutable → no source baseline)
      *) continue ;; esac ;;                                   # launch dirs etc → log-only
  persistence_launchd)
    [[ $action == added ]] || continue
    label=$(jq -r '.columns.label // ""' <<<"$finding"); path=$(jq -r '.columns.path // ""' <<<"$finding")
    case "$path" in
      /System/Library/*) continue ;;                           # Apple churn → log-only
      */LaunchDaemons/*) severity="CRIT" ;;                    # page: system daemon (not allowlistable)
      *) grep -qxF -- "$label" "$ALLOWLIST" 2>/dev/null && continue || { _digest_append "$finding"; continue; } ;;
    esac ;;                                                    # user agent → digest (was page in v1)
  system_extensions_new | agent_binary_changed | screenlock_state)
    _digest_append "$finding"; continue ;;                     # digest
  filevault_state)
    [[ $severity == CRIT ]] || continue ;;                     # page only on OFF (protection_off set severity)
  firewall_state | gatekeeper_state | sip_state | remote_access_sharing_state | kernel_extensions_new)
    continue ;;                                                # log-only (poller owns firewall/gatekeeper)
  *) continue ;;                                               # default-deny → log-only
esac
```

- [ ] **Step 4 — hostname** in the shared `send_alert` (carried from v1 Spec §4): inject
  `host="${OSQUERY_HOSTNAME:-$(scutil --get LocalHostName 2>/dev/null || hostname -s)}"` and build the
  body `{event_type,host:$host,alert:{title,detail}}` with `jq -cn` (`--arg host "$host"`). Confirm the
  Hermes gateway renders `alert.host`, else prefix `[$host] ` into the title (no gateway change). Test:
  dispatch body carries `host`.
- [ ] **Step 5 — pass-1 filter:** admit the bare page/digest names; **do not** instruct removing a
  `new_ssh_key` token (it does not exist — v1's phantom edit). Grep the real `select(.name ...)` anchor
  first.
- [ ] **Step 6:** `just test` → green. Commit
  `feat(osquery-alerter): three-outcome page/digest/log gate + hostname`.

**Acceptance gate:** the digest store has exactly **one** writer (`_digest_append`), called only from
explicit `digest)` arms; page arms never call it (structural separation, T-SEP-\*).

______________________________________________________________________

## Task 2 — Digest store + builder + summary path

**Files:** `executable_osquery-digest.sh` (new). Tests: `test_digest.bats` (T-DIGM-\*).

- [ ] **Step 1 — failing tests:** T-DIGM-send (non-empty store → 1 grouped message), T-DIGM-empty (no
  store → silent, exit 0), T-DIGM-group, T-DIGM-rollup (cap + `+K more`), T-DIGM-clear (keeps `.last`),
  T-DIGM-route (CRIT + empty sound). Red (no builder).
- [ ] **Step 2 — the builder:** atomic-rotate → empty-suppress (twice) → group-by-detector concise render
  → one silent send → clear keeping `.last`:

```bash
#!/usr/bin/env bash
set -euo pipefail
source "$HOME/.local/bin/osquery-alert-dispatch.sh"
store="${OSQUERY_DIGEST_STORE:-$HOME/.local/state/osquery-digest-spool/digest.ndjson}"
[ -s "$store" ] || exit 0                                     # guard 1: empty → silent
work_file="$store.$(date -u +%s).build"; mv -f "$store" "$work_file" 2>/dev/null || exit 0   # atomic rotate
[ -s "$work_file" ] || { rm -f "$work_file"; exit 0; }        # guard 2: whitespace/zero-byte
item_count=$(grep -c . "$work_file" 2>/dev/null || echo 0)
body=$(jq -rs 'group_by(.detector)[] as $group | "**\($group[0].detector)** (\($group|length))",
  ($group[0:10][] | "- \(.id) — \(.summary)"),
  (if ($group|length)>10 then "… +\(($group|length)-10) more" else empty end), ""' "$work_file" 2>/dev/null | head -c 1800)
title="🗒️ osquery daily digest · $(date -u +%Y-%m-%d) · ${item_count} item(s)"
send_alert CRIT "$title" "$body" ""                           # CRIT=channel selector; ""=silent (non-interruptive)
mv -f "$work_file" "$store.last" 2>/dev/null || rm -f "$work_file"
```

- [ ] **Step 3:** `just test` → green. Commit
  `feat(osquery): daily digest builder (grouped, empty-suppressed, silent)`.

**Acceptance gate:** an empty store produces ZERO dispatch calls; a non-empty store produces exactly ONE
`send_alert CRIT … sound=""`. (Routing wart documented in a script comment — Spec §4.)

> **Alternative delivery (documented, not the default — decision addendum D-V2-12):** instead of
> `send_alert` → webhook, the builder can print the digest to stdout and be scheduled by **Hermes
> `no_agent` cron**
> (`hermes cron create "0 9 * * *" --no-agent --script osquery-digest.sh --deliver discord`) via a
> one-line wrapper in `~/.hermes/scripts/` (`exec "$HOME/.local/bin/osquery-digest.sh" "$@"`). That gives
> native secret-redaction + native `[SILENT]` empty-suppression, at the cost of a Hermes-gateway-uptime
> dependency (the macOS watchdog can't see it). The rotate/group/render core is unchanged — only the
> delivery tail differs — so it is a config swap, not a redesign. The **page tier always uses the webhook
> \+ spool**.

______________________________________________________________________

## Task 3 — log-only/noisy retention (no-deliver proofs)

**Files:** the alerter (already drops these via Task 1's `continue`); tests `test_logonly.bats`.

- [ ] **Step 1 — failing tests:** T-LOG-kext-no-deliver (kext added → no page AND digest store empty),
  T-LOG-sip-no-page (both `pack_security-policy-regression_sip_state` **and** the vestigial
  `pack_security-regression_sip_state` → no page), T-LOG-sharing, T-LOG-es. Red if any leaks.
- [ ] **Step 2:** the gate's log-only arms (Task 1) already `continue`; confirm no digest leak. Add the
  `sip_state` reconciliation note to the pack (remove/rename the vestigial `security-regression` entry so
  two names can't diverge — docs/config-plan; the alerter's `pack_security-policy-regression_` prefix
  would silently drop the other).
- [ ] **Step 3:** `just test` → green. Commit
  `test(osquery): log-only detectors never deliver (kext firehose, dead sip/sharing)`.

______________________________________________________________________

## Task 4 — launchd tiering + the allowlist contract

**Files:** `packs/intrusion-detection.conf` (query), the alerter (render). Tests: `test_launchd.bats`,
`test_allowlist.bats`.

- [ ] **Step 1 — query replacement (flag it):** v2's `persistence_launchd` is a **replacement** of the
  live 11-column COALESCE query (which has **no `removed` key**, default false). v2 selects
  `label, path, program, program_arguments` with **`removed:true`** (load-bearing — the gate's
  `removed → log-only` branch depends on differential removes; the live query never emits them). Document
  this as the behavioral change it is.
- [ ] **Step 2 — failing tests:** T-PAGE-launchd-sysdaemon (`/Library/LaunchDaemons` label → page even if
  allowlisted), T-NEG-launchd-apple (`/System/Library/*` → no page), T-DIG-launchd-user
  (`~/Library/LaunchAgents` label ∉ allowlist → digest, no page), T-DIG-launchd-allow (∈ allowlist →
  silent). Fixtures via `row` with the real launchd columns (`program` empty, command in
  `program_arguments`).
- [ ] **Step 3 — allowlist path + regex fixes (T-AL-path, T-AL-regex-at):** standardize the file on
  `page-launchd-allowlist.txt` and **update the alerter's `ALLOWLIST_FILE`/env in the same change** (the
  live alerter reads `launch-allowlist.txt` → every allow is a silent no-op until reconciled). The
  validation regex (writers) is `^[A-Za-z0-9][A-Za-z0-9._@-]+$` — **the `@` is required**
  (`homebrew.mxcl.postgresql@17` is live).
- [ ] **Step 4 — render:** launchd page → header "New startup item (system daemon)"; digest summary line
  for user agents → "new login item: `<label>` (reply `allow <label>` to silence)".
- [ ] **Step 5:** `just test` → green. Commit
  `feat(osquery): launchd tiering (system→page, user→digest) + allowlist path/regex fix`.

______________________________________________________________________

## Task 5 — file_events tiering

**Files:** `osquery.conf` (`file_paths` categories), the alerter (render). Tests: `test_fileevents.bats`.

- [ ] **Step 1 — reconcile the live `ssh` category:** the live alerter pages/renders on category `ssh` in
  two places (verified). v2 uses category `authorized_keys`; **delete the stale `ssh` render/gate
  branches** so a real authorized_keys event matches the new branch (not the old `ssh` one or nothing),
  and ensure the watch is scoped to the `authorized_keys` file, not a broad `~/.ssh/%%` (which
  re-introduces known_hosts noise).
- [ ] **Step 2 — failing tests:** T-PAGE-authkeys (CREATED → page), T-NEG-authkeys-delete (DELETED → no
  page), T-PAGE-sshd, T-DIG-sudoers (UPDATED → digest), **T-PAGE-pipeline-mismatch** (a
  pipeline_integrity event whose `sha256` ∉ the manifest → page), **T-NEG-pipeline-match** (a
  pipeline_integrity event whose `sha256` ∈ the manifest — a legit `chezmoi apply` → no page, no digest),
  T-AL-watched (a write to the allowlist file is a pipeline_integrity event → same content-mismatch
  rule). Fixtures via `file_event_row` (set `columns.sha256`).
- [ ] **Step 3 — config:** add `file_paths` categories `authorized_keys`; `pipeline_integrity` (the
  alerter's own scripts/plists → **page-on-content-mismatch**); and `allowlist_file` (the
  `page-launchd-allowlist.txt` page-suppressor → **digest**, NOT page-on-mismatch — it legitimately
  mutates at runtime via `allow`/`deny`, so it has no fixed source baseline; editing it is still
  surfaced, just in the daily digest). `sudoers`/`sshd_config` already present. CREATED/UPDATED routed
  per Task 1; DELETED → log-only.
- [ ] **Step 3a — the baseline manifest (pipeline_integrity layers 1+2 — D-V2-13):**
  - **Baseline script** `dot_local/bin/executable_osquery-pipeline-baseline.sh` (deployer-agnostic):
    hashes the **source artifacts** (layer 1 — what the files *should* be, from the chezmoi source state,
    NOT the live deployed files) for the watched set and writes `sha256`-per-line to
    `/var/osquery/pipeline-known-good.sha256`.
  - **Root-owned manifest (layer 2):** `/var/osquery` is `root:wheel`, so the manifest is root-owned; the
    script writes it as root. Trigger: a chezmoi `run_after` hook invokes the baseline script (via a
    tightly-scoped NOPASSWD `sudo` entry or a root LaunchDaemon — `chezmoi apply` runs as the user).
    **Deployer-agnostic seam:** Homelab later calls the *same* script post-deploy; the detector (Task 1
    gate, reads `$PIPELINE_MANIFEST`) is unchanged. (Layer 3 — off-host heartbeat-absence — is deferred
    to Homelab.)
  - The gate (Task 1) pages when the event `sha256` ∉ manifest, silent when ∈.
- [ ] **Step 4:** `just test` → green. Commit
  `feat(osquery): file_events tiering — authkeys/sshd + pipeline_integrity(content-mismatch)→page, sudoers→digest; source-derived root-owned baseline`.

______________________________________________________________________

## Task 6 — admin / suid / kext / sysext / security-posture tiering

**Files:** `osquery.conf`, packs, the alerter. Tests: `test_admin.bats`, `test_posture.bats`,
`test_extensions.bats`, `test_logonly.bats`.

- [ ] **Step 1 — page (positive + negative):** T-PAGE-admin, T-PAGE-suid; **T-PAGE-filevault-off — a real
  OFF transition driven through the actual `severity` pipeline asserts it DOES page** (v1 shipped only
  negatives — the one behavior justifying the pack was unverified). Keep the existing
  re-enable/firewall-off negative tests.
- [ ] **Step 2 — digest:** T-DIG-sysext (`system_extensions_new`, `state='activated_enabled'` non-Apple,
  with identifier-seen-before dedup in the digest so a re-activation of a known sysext collapses),
  T-DIG-screenlock.
- [ ] **Step 3 — log-only:** T-LOG-kext-no-deliver. **Document that `kernel_extensions_new` is
  intentionally not delivered** (load-state firehose, 657 real events); the open-question install-state
  redesign is **not** built in v2.
- [ ] **Step 4 — zero-row acceptance gates (Spec §6):** add explicit steps to **confirm
  `screenlock_state` emits rows under `osqueryi --json` on Dresden** (else it is a no-op page — leave at
  digest) and to **rebuild `remote_access_sharing_state` as a Remote-Login/Screen-Sharing ON-transition
  detector** before it can move off log-only. A page detector that never emits a deliverable row is false
  assurance.
- [ ] **Step 5:** `just test` → green. Commit
  `feat(osquery): posture/extension tiering + positive filevault-off test`.

______________________________________________________________________

## Task 7 — agent / Hermes surface tiering

**Files:** `osquery.conf` (queries), the alerter (render). Tests: `test_agent.bats`.

- [ ] **Step 1 — page:** T-PAGE-exposure (`agent_exposure_changed`, off-loopback 8644/8181).
  T-PAGE-webhooksecret **and T-PAGE-paseokey** — split BOTH the **webhook-secret** and the paseo
  **`daemon-keypair.json`** out of `agent_authfile_changed` → page (the pipeline's HMAC key + the paseo
  daemon's auth, your primary access path).
- [ ] **Step 2 — digest:** T-DIG-agentbin (`agent_binary_changed` — keep v1's resolved-native-binary
  scope, not launcher symlinks; the Hermes editable-tree remains a stated gap, not a noisy per-file
  hash), T-DIG-authfile (the **remaining** credential set — `.env`, `codex/config.toml`, `cli-client-id`
  — → digest; `assert_page_lacks` the sha256).
- [ ] **Step 3 — intervals:** keep v1's explicit intervals (agent_binary 3600, agent_exposure 600,
  authfile 600).
- [ ] **Step 4:** `just test` → green. Commit
  `feat(osquery): agent-surface tiering (exposure + webhook-secret + paseo-keypair→page; binary/authfile→digest)`.

______________________________________________________________________

## Task 8 — Delivery security + durable spool (H2 harness)

**Files:** the dispatch helper, the watchdog. Tests: `test_spool.bats`, `test_secrets_redaction.bats`,
`test_localhost_boundary.bats` (all H2), `test/hermes-delivery/*.py` (integration).

- [ ] **Step 1 — failing tests:** T-SEC-spool-retry, T-SEC-spool-idem, T-SEC-no-secret-log,
  T-SEC-localhost, **T-SEC-drain-setE** (an empty/malformed spool must not abort the alerter under
  `set -euo pipefail`). Red.
- [ ] **Step 2 — spool + guarded drain:** keep v1's spool-on-final-failure; make `_drain_spool`
  **`set -e`-safe** (wrap the loop body so a malformed entry or empty dir returns 0; never abort the
  caller — a delivery feature must not cause a detection outage). Replay the **stored** request-id
  verbatim (idempotent at the gateway); recompute the signature from the stored body.
- [ ] **Step 3 — redaction proof:** drive `agent_authfile_changed` with a sentinel secret + a sha256;
  assert no file under `~/.local/log/osquery/` or the spool dir contains the secret/sha256 and the
  payload carries only the basename. (Backs Spec §4 "secrets never written.")
- [ ] **Step 4 — integration (Dresden-only / manual gate):** T-SEC-gw-unsigned, T-SEC-gw-badsig (+
  correct-key sibling), T-SEC-gw-dedup. Do **not** hard-code the reject status — confirm on Dresden. **No
  Hermes edits.**
- [ ] **Step 5 — watchdog:** drains the spool each tick; pages if a spool entry is older than one tick.
- [ ] **Step 6:** `just test` (bats) green; integration documented as a manual gate. Commit
  `feat(osquery): set-e-safe spool drain + delivery-security + redaction tests`.

______________________________________________________________________

## Task 9 — Digest cadence + empty suppression (the agent)

**Files:** `Library/LaunchAgents/com.webdavis.osquery-digest.plist.tmpl`, the loader,
`.chezmoidata`/`.chezmoi.toml.tmpl`, the watchdog. Tests: `test_digest.bats` (T-DIGM-cadence,
T-DIGM-empty, T-DIGM-heartbeat-sep).

- [ ] **Step 1 — failing tests:** T-DIGM-cadence (the script has **no time-gate**; Hour/Minute live only
  in the rendered plist), T-DIGM-empty (no store → silent), T-DIGM-heartbeat-sep (a digest send emits no
  ✅; an empty day still lets the heartbeat fire — distinct agents/titles). Red.
- [ ] **Step 2 — plist (cadence via chezmoi, `RunAtLoad=false`):**

```xml
{{ if eq .chezmoi.os "darwin" -}}
… <key>Label</key><string>com.webdavis.osquery-digest</string>
   <key>ProgramArguments</key><array><string>/opt/homebrew/bin/bash</string><string>{{ .chezmoi.homeDir }}/.local/bin/osquery-digest.sh</string></array>
   <key>StartCalendarInterval</key><dict><key>Hour</key><integer>{{ .osquery.digestHour | default 18 }}</integer><key>Minute</key><integer>{{ .osquery.digestMinute | default 0 }}</integer></dict>
   <key>RunAtLoad</key><false/>
   <key>StandardErrorPath</key><string>{{ .chezmoi.homeDir }}/.local/log/osquery/digest.log</string> …
{{- end }}
```

backed by `[data.osquery] digestHour = 18 / digestMinute = 0` (evening review; decoupled from the 09:00 ✅
heartbeat). Loader cloned from the watchdog loader (bootout `com.webdavis.osquery-digest` → bootstrap).

- [ ] **Step 3 — watchdog guards the digest agent:** append `"com.webdavis.osquery-digest"` to the
  `AGENTS` array (its silence must be a guarded safety signal).
- [ ] **Step 4:** `just test` green. Commit
  `feat(osquery): daily digest LaunchAgent (chezmoi cadence, RunAtLoad=false) + watchdog guard`.

______________________________________________________________________

## Task 10 — Allowlist curation path (manual file now; tap buttons + `/osquery` skill = PR #2)

**Files:** `dot_config/osquery/page-launchd-allowlist.txt` (plain tracked file) + the **one** shared
getopts tool `dot_local/bin/executable_osquery-allowlist.sh` (PR #1 ships its `-a` add verb; PR #2 adds
`-d`/`-l`), docs; **no `dot_hermes/` edits in PR #1**. Tests: `test_allowlist.bats` (the tool is the
security boundary, reused by every PR #2 caller). The PR #2 tap-button bot + `/osquery` skill ship under
their own plan (D-V2-15); the retired plugin's `test_handler.py` is dropped.

- [ ] **Step 1 — the file (the durable floor):** create `dot_config/osquery/page-launchd-allowlist.txt`
  as a **plain `.txt`** (NOT `.tmpl` → no KeePassXC/TTY prompt; applies from a headless remote shell).
  Seed it empty with a header comment documenting the contract.
- [ ] **Step 2 — failing tests (T-AL-\*) for the tool's `-a` verb:** regex-accepts-`@`, reject-junk (`*`
  / `../etc` / empty / embedded space / an Apple `com.apple.*` label), exact-full-line match, dedup,
  path-agreement (reader==writer==`page-launchd-allowlist.txt`), allowlist-file-is-watched (T-AL-watched,
  in Task 5). (Owner+channel auth is a PR #2 button-bot test — T-AL-owner-channel.)
- [ ] **Step 3 — implement the tool's `-a` verb (makes Step 2 green):**
  `dot_local/bin/executable_osquery-allowlist.sh -a <label>` — getopts; validate
  `^[A-Za-z0-9][A-Za-z0-9._@-]+$` (fail-closed), **refuse Apple/system labels** (`com.apple.*`; true
  system daemons page by path in the gate regardless of the allowlist), dedup, then append the **bare**
  label — one exact label per line so the reader's `grep -qxF` matches (write history is git on the
  source file; an inline audit comment would break the exact-match). This is the **one security
  boundary** every caller (manual edit, the PR #2 button bot, the skill) uses. `just test` → T-AL-\*
  green.
- [ ] **Step 4 — the manual path (works today):** append via `osquery-allowlist.sh -a <label>` (or a
  hand-edit honoring the identical bare-label contract), commit (git audit),
  `chezmoi apply ~/.config/osquery/page-launchd-allowlist.txt`. This is the interim path the
  **calibration week depends on** — independent of the PR #2 UX.
- [ ] **Step 5 — `just test` green** (the `-a` verb + file-contract). Commit
  `feat(osquery): one allowlist tool (osquery-allowlist.sh -a, exact-label, @-aware, watched) + manual curation path`.

**Out of scope for PR #1 — the PR #2 Discord UX (separate plan, D-V2-15):** the tap Approve/Deny buttons
(Butters bot, pending-scoped daemon) and the `/osquery allow|deny|list` skill (rides Bob) are built under
their own plan and both call the Step 3 tool (`osquery-allowlist.sh`). The retired `pre_gateway_dispatch`
plugin (its API was verified in D-V2-12) is **superseded** by that buttons+skill design and is not built.

______________________________________________________________________

## Task 11 — chezmoi / secrets / config (docs-plan; no secret edits)

**Files:** carries v1 Task 11 forward + the v2 additions. **No live `~/.hermes` / secret mutation.**

- [ ] Carry v1's `.env` (KeePassXC) + `config.yaml` seed-once `create_` template + webhook-secret
  template unchanged (already designed; nothing in v2 changes them).
- [ ] Add `[data.osquery] digestHour/digestMinute` to `.chezmoi.toml.tmpl`/`.chezmoidata`.
- [ ] Add the allowlist plain file (Task 10). Confirm `chezmoi diff` shows only additions; the allowlist
  file is plain (no KeePassXC prompt). Value-based leak gate before any commit (carried from v1).
- [ ] Commit `chore(chezmoi): digest cadence data + allowlist file (no secret changes)`.

______________________________________________________________________

## Task 12 — Lint, full suite, calibration gates

- [ ] `nix develop .#run --command ./scripts/lint.sh` green (shellcheck/shfmt the
  alerter/dispatch/digest/watchdog; `chezmoi execute-template … | jq empty` for the conf + packs;
  **markdown fence parity even** in every doc).
- [ ] `just test` → all bats green; the Dresden-only integration pytest is a documented manual gate.
- [ ] **Acceptance gates before trusting the page tier:** (a) confirm `screenlock_state` emits rows under
  `osqueryi` on Dresden, else keep it digest; (b) `remote_access_sharing_state` rebuilt as a working
  ON-transition detector or left log-only; (c) the allowlist reader/writer path agree (else every allow
  is a no-op); (d) the positive filevault-OFF test is green (the pack's reason-for-being).
- [ ] **Calibration (Dresden only):** discard the counter==0 baseline; one week labeling page rows; new
  user-LaunchAgent labels flow to the **digest** (not pages) and are allowlisted via the **manual file
  path** (no plugin dependency). Confirm a quiet channel + the daily ✅ + a daily digest that is
  empty-suppressed when nothing suspicious occurred. **No fleet rollout** (homelab migration is out of
  this repo). Open the PR.

______________________________________________________________________

## Self-review checklist

- [ ] Calm-channel/ADHD is the stated primary requirement (Goal, North star, Spec §1). ✓
- [ ] Three tiers implemented: page/core (Task 1 gate + Tasks 4–7), digest/suspicious (Tasks 1–2, 9),
  log-only (Task 3). ✓
- [ ] Everything not immediate/high-confidence/actionable/rare is demoted out of page (tier matrix; Tasks
  4–7). ✓
- [ ] Configurable daily-default digest with empty suppression (Tasks 2, 9; T-DIGM-empty/cadence). ✓
- [ ] Hermes plugin challenged; manual file in PR #1; PR #2 = spare-bot approval buttons (pending-scoped
  daemon) + `/osquery` skill typed fallback (Task 10; addendum D-V2-6/7/12/14/15). ✓
- [ ] Allowlist: auditable (git), reversible (`deny`/revert), exact-label, no wildcards, `@`-aware regex,
  owner/channel scoped, watched, test-covered (Task 10; T-AL-\*). ✓
- [ ] Delivery: HMAC/localhost/dedup/idempotent-spool/no-secret-logging/`set -e`-safe-drain tests (Task
  8; T-SEC-\*). ✓
- [ ] TDD with realistic 9-key-envelope fixtures (Task 0 `row`/`file_event_row`; test matrix realism
  contract). ✓
- [ ] dotfiles-now vs homelab-later separated, no fleet scope creep (Scope; Spec §2). ✓
- [ ] Every page detector has BOTH a positive and a negative test (test matrix). ✓
- [ ] v1 left intact except a tiny pointer + the orphan-fence fix; no code/config/secret/Hermes edits in
  this planning task. ✓

## Open questions (proposed answers)

1. **pipeline_integrity** → **RESOLVED:** page on content-mismatch vs a source-derived, root-owned
   baseline manifest (layers 1+2 in round 1; D-V2-13). Task 5 Step 3a + Task 1 gate.
1. **screenlock / remote-sharing** → confirm-or-rebuild before page (Task 6 Step 4).
1. **kext** → install-state redesign for any future delivery; log-only now (Task 6 Step 3).
1. **agent_authfile split** → **RESOLVED:** page on webhook-secret **and** paseo `daemon-keypair.json`;
   digest the rest (Task 7).
