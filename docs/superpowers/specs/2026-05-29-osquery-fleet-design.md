# Osquery Fleet Design Spec

**Date:** 2026-05-29
**Scope:** A migration plan from the current single-host osquery setup (one Mac, one local
`osqueryd`, one daily-report script writing into the Ivy vault) to a homelab-hosted
[FleetDM](https://fleetdm.com/) deployment that centralizes osquery query management, schedules,
and posture reporting across the laptop plus future Linux servers.
**Out of scope:** Standing up the homelab infrastructure itself (already covered by the homelab
project), MDM enrollment of corporate devices (this is personal infra), distributed FIM hashing of
large directories, FleetDM enterprise-tier features (vulnerability scanning UI, MDM payload
authoring), Apple-DEP enrollment.

## Background

The standalone osquery setup landed in commits `5538d13` (vault integration), `ebb6c2f` (security
pack), and `b60b559` (file_events FIM). It runs `osqueryd` locally, schedules ten posture queries
plus a `file_events_recent` event query at 6h cadence, and renders each tick into a markdown report
that lands in `~/workspaces/Ivy/security/osquery/YYYY-MM-DD.md`. That works on one machine; it does
not survive contact with a fleet:

- Each new host means another raw `osqueryd` to configure, another report script to wire up,
  another markdown file to read. The query pack lives twice — once on the Mac, once on each Linux
  server.
- There is no cross-host comparison. "Did the same SSH key just appear on three boxes?" is
  unanswerable without grep-across-vaults.
- Ad-hoc live queries ("which host is listening on 8443 right now?") require SSHing into each
  host and running `osqueryi`.
- The FileVault gap (the `disk_encryption` table mis-reports on Apple Silicon) cannot be closed by
  osquery alone; it needs MDM data, which FleetDM exposes natively.

### Approaches considered

| # | Approach | Verdict | Reason |
|---|----------|---------|--------|
| 1 | **FleetDM (this design)** | **Chosen** | Active maintenance ([fleetdm/fleet](https://github.com/fleetdm/fleet)), open source AGPL, community tier explicitly suitable for homelab per [official docs](https://fleetdm.com/docs/get-started/anatomy), pack-as-YAML via `fleetctl apply`, MDM data joins osquery results, Linux + macOS + Windows agent (`fleetd`). |
| 2 | Kolide Cloud / Kolide K2 | Rejected | Pivoted away from osquery fleet management to an identity-product company; the original Kolide Fleet codebase was forked by [the Fleet team](https://github.com/fleetdm/fleet) in 2022 and that fork is now FleetDM. |
| 3 | Doorman | Rejected | Last meaningful commit pre-2020; project effectively abandoned. |
| 4 | Self-rolled (rsync results + scripts) | Rejected | No live-query support, no MDM data, no auth, hand-rolling everything fleetctl already does. YAGNI in reverse — we'd reinvent FleetDM badly. |
| 5 | Stay standalone forever | Rejected | Loses the centralization payoff the user wants from the homelab buildout. |

## §1 — Target topology

```
┌─────────────────────────────────────────────────────────────────┐
│  Homelab VM (Linux, 2 vCPU / 4 GB RAM / 20 GB disk)             │
│  ─────────────────────────────────────────────────────────────  │
│  fleetdm/fleet:latest      (Go binary, single process)          │
│  mysql:8                   (state — hosts, packs, results)      │
│  redis:7                   (live-query cache, session)          │
│  caddy:2 (reverse proxy, TLS termination, ACME via Let's Enc.)  │
│                                                                 │
│  docker compose up -d  ←  declarative deploy                    │
└─────────────────────────────────────────────────────────────────┘
                              ↑                ↑
                              │ enroll         │ enroll
                              │ (TLS)          │ (TLS)
                              │                │
            ┌─────────────────┴─┐         ┌────┴────────────────┐
            │  This Mac         │         │  Future Linux       │
            │  fleetd 1.x       │         │  servers (fleetd)   │
            │  (osqueryd        │         │                     │
            │   + Orbit         │         │                     │
            │   + extensions)   │         │                     │
            └───────────────────┘         └─────────────────────┘
```

| Component | Role |
|-----------|------|
| **Fleet server** | Receives query results, exposes web UI + GraphQL/REST API, schedules packs per team. |
| **MySQL** | Persistent state: hosts, query packs, results history, users. |
| **Redis** | Live-query distribution + session cache. |
| **Caddy** | TLS termination + ACME for the public hostname. Fleet doesn't terminate TLS itself in homelab deployments. |
| **fleetd agent** | The bundle Fleet distributes — `osqueryd` (the query engine, same as today) + `Orbit` (auto-updater that keeps the agent current) + extension support. Replaces raw `osqueryd` on every enrolled host. |

VM sizing (2 vCPU / 4 GB / 20 GB) handles up to ~100 hosts per the official Fleet documentation,
which is roughly 30× the homelab's likely fleet size. Right-sized, not over-provisioned.

## §2 — Agent strategy: fleetd vs. raw osqueryd

The current setup runs raw `osqueryd` via `/Library/LaunchDaemons/io.osquery.agent.plist` plus a
hand-written `/var/osquery/osquery.flags`. Migrating means:

1. Download the Fleet-signed `fleetd` macOS installer from the Fleet server's UI (the server
   generates per-tenant installers that bake the server URL + enrollment secret in).
2. `sudo installer -pkg fleetd-base.pkg -target /`.
3. The installer replaces the launchd unit and the flagfile with Fleet-managed equivalents and
   removes the `osquery` Homebrew formula's interference.
4. The `osquery` Homebrew formula gets removed from
   `.chezmoidata/system_packages_autoinstall.yaml` (no longer needed; fleetd ships its own
   osqueryd).
5. The local `osquery.conf.tmpl` and `osquery.flags` chezmoiscript become obsolete and are deleted.
   The query pack moves to a YAML file under `fleet/packs/` in this repo.
6. The report script becomes **optional** — Fleet's web UI now shows the same data live. The script
   can survive as a "vault snapshot exporter" that pulls results from Fleet's API into Obsidian on
   a cron, or it can be retired entirely.

## §3 — Query pack as YAML

Fleet manages packs declaratively via `fleetctl apply -f <file>`. Our current `osquery.conf.tmpl`
becomes a `fleet/packs/security.yaml` that lives in this repo and gets applied to the Fleet server
via a CI job or a one-shot `just fleet-apply`.

Sketch (illustrative; column names match the schedule in current `osquery.conf.tmpl`):

```yaml
apiVersion: v1
kind: pack
spec:
  name: security
  description: Security-posture queries (SIP, FW, GK, listening ports, FIM)
  targets:
    labels: [All Hosts]
  queries:
    - query_name: firewall_state
      interval: 21600
      snapshot: true
    - query_name: gatekeeper_state
      interval: 21600
      snapshot: true
    # ... seven more snapshot queries + file_events_recent
```

The query bodies themselves live as separate `kind: query` documents in the same file or in
`fleet/queries/`. This is the same query content we already have — only the wire format changes.

`fleet/` becomes the new home for declarative Fleet config, sibling to `dot_config/osquery/`
during the transition (osquery.conf stays for the period when this Mac still runs standalone but
the future hosts already run fleetd).

## §4 — What Fleet adds on top of standalone osquery

| Capability | Standalone today | With Fleet |
|------------|------------------|------------|
| Centralized inventory | One markdown file per host, manually compared | Single web UI, queryable across all hosts |
| Live queries | Not supported | First-class — type a SQL query in the UI, get results from all hosts in seconds |
| FileVault state | Unreliable on Apple Silicon (osquery `disk_encryption` lies) | Joined from MDM data, accurate |
| Software-inventory delta | Snapshot comparison only, manual | Built-in `software` table with vulnerability matching against NVD |
| Alerting | `alerter` macOS notification, local only | Webhook → Slack/Discord/PagerDuty/anywhere |
| Host grouping ("teams") | None | Per-team packs (e.g. laptops get one pack, servers another) |
| Auth | None (anyone on box can osqueryi) | Fleet user accounts, SSO via Google/GitHub/SAML |
| Query history | None — once a snapshot rolls off the log, it's gone | Time-series via MySQL, queryable for forensics |

## §5 — Migration trigger

The migration is **not** "do this when the spec lands." The triggers are:

1. **Homelab has at least one stable Linux VM** capable of running Docker Compose 24/7.
2. **A second host** would benefit from osquery monitoring (a server, not a one-off ephemeral
   thing). Single-host fleet is overkill.
3. **The user wants centralized inventory** rather than per-host markdown reports.

Until those three line up, the standalone setup is correct. The spec exists so future-Stephen
doesn't relitigate "should I run my own fleet?" — the answer is yes, FleetDM, when the homelab is
ready.

## §6 — Open questions for migration time

These are deferred decisions, captured here so they don't get rediscovered fresh:

1. **TLS hostname.** Fleet wants a stable, publicly-resolvable hostname for ACME. Options:
   subdomain off the personal apex (`fleet.webdavis.io`) with a Cloudflare DNS record routed to the
   homelab via Tailscale Funnel or Cloudflare Tunnel, or LAN-only with a private CA.
1. **MDM integration.** Fleet's macOS MDM adds FileVault key escrow, configuration profile push, and
   software install. Worth turning on for this Mac at migration time? The personal Apple ID makes
   this awkward; punt to a separate decision.
1. **Backup story.** MySQL state is the durability layer. Restic to the homelab's existing backup
   target with a daily snapshot schedule.
1. **Vault integration.** The current daily-markdown-report flow served the "I notice osquery in
   Obsidian" need. After migration: keep a thin exporter that pulls a daily security-posture
   summary from Fleet's API into `~/workspaces/Ivy/security/osquery/`, or rely entirely on the
   Fleet UI?
1. **Pack-as-YAML lifecycle.** Does `just fleet-apply` run on commit-to-main via CI, or manually?
   Probably manual at first, CI when fleet config becomes part of regular workflows.
1. **Standalone deprecation cutover.** When fleetd is installed on this Mac, the chezmoi-managed
   `osquery.conf.tmpl`, `osquery.flags` chezmoiscript, and `osquery-report.sh` LaunchAgent should
   be removed in a single commit, not piecemeal. The vault `security/osquery/` folder remains; new
   reports come from the exporter or stop appearing.

## §7 — What this spec does *not* do

- It does not commit to a timeline. The migration trigger is event-driven, not date-driven.
- It does not pick a specific homelab VM platform. Proxmox VM, NixOS container, Talos node — all
  fine. Fleet only requires Docker.
- It does not specify the OS for the future Linux servers. Whatever the homelab settles on, the
  `fleetd` Linux build supports it.
- It does not enumerate every query that will live in the final pack. The current 10 + file_events
  carry over; future additions (es_process_events once FDA is granted, browser-extension tracking,
  Homebrew inventory) are out-of-scope until concrete need.

## §8 — Carry-over from standalone

The work already done in commits `5538d13`, `ebb6c2f`, and `b60b559` is **not throwaway**:

- The 10 posture queries become Fleet pack queries verbatim.
- The `file_events_recent` query and `file_paths` config become a Fleet `file_paths` pack option.
- The `~/workspaces/Ivy/security/` vault structure stays as the long-term home for security notes
  and any retained exporter output.
- The empirically-derived knowledge ("`disk_encryption` lies on Apple Silicon", "event flags are
  CLI-only", "FSEvents listeners need no FDA") goes directly into Fleet pack design.

Migration is additive, not a rewrite.
