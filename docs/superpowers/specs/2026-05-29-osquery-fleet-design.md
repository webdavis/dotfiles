# Osquery Fleet Design Spec

**Date:** 2026-05-29
**Updated:** 2026-06-01 — reconciled with the as-built notification system (real-time alerting via
hermes webhook → Discord; the original daily-markdown-report design it described was replaced).
**Scope:** A migration plan from the current single-host osquery setup (one Mac, local `osqueryd`,
real-time security alerting) to a homelab-hosted [FleetDM](https://fleetdm.com/) deployment that
centralizes osquery query management, schedules, and posture across the laptop plus future Linux
servers.
**Out of scope:** Standing up the homelab infrastructure itself (covered by the homelab project —
see its `PLAN-v9.md` → Extras → Osquery for the live as-built record), MDM enrollment of corporate
devices (this is personal infra), distributed file-integrity hashing of large directories, FleetDM
enterprise-tier features, Apple-DEP enrollment.

> **Source of truth for current state:** the homelab `docs/plans/PLAN-v9.md` → *Extras → Osquery*
> section holds the authoritative as-built description. This spec is the forward-looking **FleetDM
> migration design**; the "standalone today" details below are summarized only enough to plan the
> migration.

## Background — what exists today (standalone, Mac `dresden`)

`osqueryd` runs locally as a system LaunchDaemon. It does **real-time, tiered security alerting** —
not the daily-markdown reports the first draft of this spec described (that apparatus was retired):

- **Three purpose-scoped packs** (chezmoi `dot_config/osquery/packs/`): `intrusion-detection` (8
  queries — persistence, non-loopback listeners, new kexts/system-extensions, unexpected suid,
  recent logins), `security-policy-regression` (10 — firewall/Gatekeeper/SIP/FileVault/screen-lock/
  sharing plus snapshot "floor" assertions), `installed-software-drift` (5 — apps, Homebrew,
  browser extensions). Mostly **differential** (a changed row is the signal).
- **Inline `file_events_recent` @10s** drains FSEvents for watched paths: `~/.ssh`, `/etc/sudoers*`,
  `/etc/ssh/sshd_config*`, and the LaunchAgents/LaunchDaemons dirs.
- **Notification:** findings are tiered 🔴 Critical / 🟡 Notice / 🔵 Info and delivered to **both**
  the local macOS notifier (`alerter`) and Discord `#osquery` via a hermes-agent webhook. Three
  LaunchAgents: `osquery-results-alerter` (WatchPaths + 300s sweep), `osquery-firewall-gatekeeper-
  monitor` (60s fast path), `osquery-uptime-watchdog` (900s, fail-loud if the pipeline dies), plus
  the `osquery-alert-dispatch` helper.

This works on one machine but does not survive contact with a fleet:

- Each new host means another `osqueryd` + notifier set to configure; the packs would live once per
  host with no central management.
- No cross-host comparison ("did the same SSH key appear on three boxes?").
- Ad-hoc live queries require SSHing into each host and running `osqueryi`.
- Per-host alerting only; no aggregated history for forensics.

### Approaches considered

| # | Approach | Verdict | Reason |
|---|----------|---------|--------|
| 1 | **FleetDM (this design)** | **Chosen** | Active maintenance ([fleetdm/fleet](https://github.com/fleetdm/fleet)), open source AGPL, community tier suitable for homelab per [official docs](https://fleetdm.com/docs/get-started/anatomy), pack-as-YAML via `fleetctl apply`, MDM data joins osquery results, Linux + macOS + Windows agent (`fleetd`). |
| 2 | Kolide Cloud / Kolide K2 | Rejected | Pivoted away from osquery fleet management; the original Kolide Fleet codebase became [FleetDM](https://github.com/fleetdm/fleet). |
| 3 | Doorman | Rejected | Abandoned (last meaningful commit pre-2020). |
| 4 | Self-rolled (rsync results + scripts) | Rejected | No live-query, no MDM, no auth — reinventing fleetctl badly. |
| 5 | ELK alone | Partial | ELK is the right **analytics/alert backend** (and the homelab will run it), but it does not *manage* agents (no config push, no live query). Pairs with — does not replace — a control plane (Fleet or Ansible). |
| 6 | Stay standalone forever | Rejected | Loses the centralization payoff of the homelab. |

## §1 — Target topology

```
┌─────────────────────────────────────────────────────────────────┐
│  Homelab (k3s on `lash`)                                        │
│  ─────────────────────────────────────────────────────────────  │
│  fleetdm/fleet            (control plane — config push, live qry)│
│  mysql + redis            (Fleet state + live-query cache)       │
│  ELK                      (results analytics, history, alerting) │
│  hermes-agent             (webhook → Discord, already planned)   │
└─────────────────────────────────────────────────────────────────┘
            ↑ enroll (TLS)              ↑ enroll (TLS)
   ┌────────┴────────┐         ┌────────┴────────────┐
   │  Mac dresden    │         │  Linux nodes        │
   │  fleetd /        │         │  fleetd (DaemonSet) │
   │  native osqueryd │         │                     │
   └─────────────────┘         └─────────────────────┘
```

| Component | Role |
|-----------|------|
| **Fleet server** | Receives results, web UI + REST/GraphQL, schedules packs per team, live queries. |
| **MySQL / Redis** | Fleet state (hosts, packs, results history) / live-query + session cache. |
| **ELK** | Layer-3 backend: ingest results (Filebeat/Fluent Bit → Elasticsearch), dashboards (Kibana), and **alerting rules** (Kibana/ElastAlert → hermes webhook). This is what replaces the Mac's local alerter scripts at fleet scale. |
| **fleetd agent** | Fleet's bundle: `osqueryd` + `Orbit` (auto-updater) + extensions. Replaces raw `osqueryd`. On Linux nodes runs as a privileged DaemonSet; the Mac stays native (EndpointSecurity/FSEvents need host access — a container would watch the VM, not macOS). |

## §2 — Agent strategy: fleetd vs. raw osqueryd

The Mac currently runs raw `osqueryd` via `/Library/LaunchDaemons/io.osquery.agent.plist` + a
chezmoi-written `/var/osquery/osquery.flags`. Migration:

1. Install the Fleet-signed `fleetd` package (bakes in server URL + enrollment secret).
2. fleetd's launchd unit + flagfile supersede the hand-written ones; remove the `osquery` Homebrew
   formula from `.chezmoidata/system_packages_autoinstall.yaml` (fleetd ships its own `osqueryd`).
3. The local `osquery.conf.tmpl` + `osquery.flags` chezmoiscript become obsolete; the packs move to
   Fleet-managed YAML.
4. The three notifier LaunchAgents (`osquery-results-alerter`, `-firewall-gatekeeper-monitor`,
   `-uptime-watchdog`) + `osquery-alert-dispatch` retire on the Mac **once ELK alerting covers their
   cases** — not before. Until then they coexist with fleetd (fleetd ships results to Fleet/ELK; the
   local scripts keep paging Discord).

## §3 — Query packs as YAML

The three purpose packs (`intrusion-detection`, `security-policy-regression`,
`installed-software-drift`, ~23 queries total) plus the `file_events` config become Fleet-managed
YAML applied via `fleetctl apply` (a one-shot `just fleet-apply` or CI). Same query content; only
the wire format and the schedule owner change. `file_paths` (the file-integrity watch list) maps to a Fleet
`file_paths` pack option. `fleet/` becomes the declarative home, sibling to `dot_config/osquery/`
during the transition.

## §4 — What Fleet + ELK add over standalone

| Capability | Standalone today | With Fleet + ELK |
|------------|------------------|------------------|
| Alerting | Real-time, tiered → local `alerter` **and** Discord `#osquery` (per-host) | ELK rules → webhook (fleet-wide, with history/correlation) |
| Centralized inventory | None — per-host only | Single web UI, queryable across all hosts |
| Live queries | Not supported (SSH + `osqueryi`) | First-class — query all hosts in seconds |
| FileVault state | `disk_encryption` reads correctly on this Mac (Apple Silicon); reliability varies across hardware | Joined from MDM data, authoritative fleet-wide |
| Software-inventory vuln matching | Drift detection only | Built-in `software` table + NVD matching |
| Query history / forensics | Bounded by rotated `results.log` | Time-series in MySQL/Elasticsearch |
| Host grouping | None | Per-team packs |
| Auth | None (anyone on box can `osqueryi`) | Fleet accounts, SSO |

## §5 — Migration trigger

Event-driven, not date-driven:

1. Homelab has a stable k3s node running 24/7.
2. A **second host** warrants osquery monitoring (a real server, not ephemeral). Single-host fleet
   is overkill.
3. The user wants centralized inventory / live queries rather than per-host local alerts.

Until then the standalone setup is correct. This spec exists so the "should I run a fleet?" question
isn't relitigated — yes, FleetDM, when the homelab is ready.

## §6 — Open questions for migration time

1. **TLS hostname.** Fleet wants a stable resolvable hostname for ACME (e.g. `fleet.webdavis.io`
   via Cloudflare Tunnel / Tailscale, or LAN-only with a private CA).
1. **MDM integration.** Fleet's macOS MDM adds FileVault key escrow + config-profile push. Worth it
   for this Mac? Personal Apple ID makes it awkward — separate decision.
1. **Backup.** Fleet's MySQL is the durability layer — Restic to the homelab backup target.
1. **Alert routing.** ELK rules → the existing hermes webhook (`#osquery`). **Per-route HMAC still
   applies** (see §8): each producer/source gets its own secret; ELK's alert webhook would use its
   own route + secret, not osquery's.
1. **Pack-as-YAML lifecycle.** `just fleet-apply` manually at first; CI later.
1. **Standalone cutover.** When fleetd lands on the Mac, retire the `osquery.conf.tmpl`, the flags
   chezmoiscript, and the three notifier LaunchAgents + dispatch helper **in one commit**, only
   after ELK alerting demonstrably covers their cases. The vault `security/osquery/` archive stays.

## §7 — What this spec does *not* do

- No timeline (trigger is event-driven).
- No specific VM/runtime pick (k3s is the homelab's choice; Fleet only needs containers).
- No future-Linux OS pin (fleetd supports them).
- No final pack enumeration — the current ~23 queries + `file_events` carry over; future additions
  (EndpointSecurity event tables once Full Disk Access is granted, broader inventory) are deferred
  until concrete need.

## §8 — Carry-over from standalone (not throwaway)

- The three purpose packs (~23 queries) become Fleet pack queries; the differential/snapshot choices
  and the severity tiers (🔴/🟡/🔵, direction-aware) inform ELK alert rules.
- The `file_events_recent` query + `file_paths` watch list become a Fleet `file_paths` pack option.
- **Secret model carries over and is load-bearing:** one HMAC key **per webhook route, never
  shared** (hard rule). The producer signs from its own file (`~/.config/osquery/webhook-secret`,
  mode 600); the hermes route verifies from its **inline** `secret:`. Per-route secrets **cannot**
  be sourced from `.env` on hermes (source-confirmed: webhook route config is never `${VAR}`-
  expanded; only one global `WEBHOOK_SECRET` is env-backed, and sharing it across routes breaks the
  rule). Dropping the global makes hermes **fail-closed**. At the homelab, **Ansible** templates the
  route + per-route secrets from the vault (which also removes the inline plaintext from
  `config.yaml`). Optional upstream ask: a feature request to NousResearch to extend env-expansion
  to `routes[*].secret`.
- Empirically-derived knowledge that informs Fleet design: event flags are CLI/flag-file-only (not
  config `options`); FSEvents file-events need no Full Disk Access; `disk_encryption` is usable on
  this Apple Silicon Mac (the earlier "always lies on Apple Silicon" claim was overstated — it
  reports `filevault_status='on'` correctly here).

Migration is additive, not a rewrite.
