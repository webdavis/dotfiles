# osquery Security Alerting: Build Guidance (dresden)

Context for the build agent: this steers an osquery detection-and-alerting system on a single macOS
machine (dresden) that is both a development machine and a home server. Alerts post to a Hermes Agent
webhook, which forwards them to one Discord channel. The hard requirement is low noise. These rules are
the result of a design pass. Follow them and flag any you cannot meet.

## Fixed scope (do not expand without asking Stephen)

1. One host only: dresden, macOS, development machine plus server.
1. One output only: the Hermes webhook, which posts to a single Discord channel. No second notification
   channel.
1. No agent analysis yet. Get osquery calibration correct first. Do not build the analysis layer.
1. No Vault. It is not set up. Do not reference or depend on it.
1. Account-level security (Discord, Substack, email) is out of scope here. It is handled by two-factor
   authentication and platform login alerts.

## Core principles

1. Alert on specific, high-confidence signatures, not on raw activity. Every detection states a
   hypothesis: "if X appears, it almost certainly means Y." The prior system failed because it pushed
   broad "potential threat" events.
1. One severity gate sits in front of the webhook. Only findings that are almost certainly bad and
   actionable cross it. Everything else stays in osquery's log on disk, queryable, and is never pushed.
1. Noise is defined relative to dresden's normal, not in the abstract. Calibrate every detection against
   this machine's actual baseline before it is allowed to alert.
1. Silence must be trustworthy. A low-noise channel is one Stephen can stop watching, which is the actual
   goal.

## Operator constraints (designed around Stephen's ADHD)

Every rule in this file ultimately serves one constraint. Stephen has ADHD, and his failure mode is that
scattered or noisy notifications get ignored, while the backlog of ignored alerts quietly builds anxiety.
The system only works if the one channel stays rare, calm, and trustworthy enough that he can stop
watching it. Concretely:

1. One channel, no fragmentation. Already fixed in scope. Do not undermine it by routing anything
   security-related anywhere else.
1. Precision is weighted higher here than for a typical user. A single recurring false positive trains
   him to ignore the channel, and that ignore-response then spreads to real alerts. When unsure whether a
   detection is quiet enough, hold it at log tier until it has proven quiet rather than letting it page.
1. Every alert is self-contained and actionable. State what was found, where, when, and the single next
   action, for example "if you did not add this, do X; if you did, allowlist it." Never send an alert
   that forces him to reconstruct context or decide from scratch what to do, because that work gets
   deferred indefinitely.
1. Allowlisting must be near-zero friction. The weekly tuning loop only happens if marking a finding
   "expected" takes one action, such as a Discord reaction or a single command, not editing a config file
   and redeploying. High-friction tuning will be skipped, noise will persist, and the channel will lose
   trust. Treat the friction of the allowlist path as a design requirement, not an afterthought.
1. Alert copy stays calm and specific. State the facts and the next step. Avoid alarmist wording, which
   spikes anxiety without adding information.
1. The system must not require active health-checking. The daily heartbeat (see Delivery) is what
   passively confirms the pipeline is alive, so he never has to remember to verify it. Keep ongoing
   maintenance minimal for the same reason: a system that needs frequent fiddling becomes its own
   background anxiety source.
1. Immediate individual pages are acceptable only because these three detections are rare. Any future
   noisier detection goes to a daily digest or stays log-only. Never let a high-volume detection reach
   the page channel.

## The detection set (start with exactly these three)

These are the exact detections to begin with. Adapt them to your config layout. All run hourly, in
differential mode, so only added or removed rows are emitted. The SQL is \[UNVERIFIED\]: validate it on
dresden before trusting it (see Calibration, step 1).

1. Persistence (launchd). Highest signal. Caveat: on a development machine, app and Homebrew installs add
   user-level LaunchAgents, so this detection needs real tuning. Allowlist legitimate tools during
   calibration. If user-level LaunchAgents stay too chatty, split them to a lower tier and keep
   system-level LaunchDaemons at page tier.
1. New SSH access (authorized_keys). A new key is a new way in. Never put the raw key in an alert. Use
   username, algorithm, and file path only.
1. New admin account (users in the admin group, gid 80). Optional companion: alert on any newly created
   user account at all.

osquery.conf schedule stanza:

```json
{
  "options": {
    "logger_path": "/var/log/osquery",
    "schedule_splay_percent": "10"
  },
  "schedule": {
    "persistence_launchd": {
      "query": "SELECT label, path, program FROM launchd;",
      "interval": 3600,
      "snapshot": false
    },
    "new_ssh_key": {
      "query": "SELECT u.username, ak.algorithm, ak.key_file FROM users u JOIN authorized_keys ak ON u.uid = ak.uid;",
      "interval": 3600,
      "snapshot": false
    },
    "new_admin_user": {
      "query": "SELECT u.username, u.uid FROM users u JOIN user_groups ug ON u.uid = ug.uid JOIN groups g ON ug.gid = g.gid WHERE g.gname = 'admin';",
      "interval": 3600,
      "snapshot": false
    }
  }
}
```

## Calibration procedure (this is the part that must be right)

1. Validate and seed the baseline by hand. Run each query once with `osqueryi --json "<query>"` on
   dresden. This confirms the SQL works on macOS and shows current state. The interactive shell is
   snapshot-only, which is correct for this step.
1. Switch to the daemon for ongoing detection. Put the queries in osqueryd's scheduled config. The daemon
   emits differential (added or removed) events over time. The interactive shell does not.
1. Treat the first differential run as baseline, not alerts. On first run, osquery reports all current
   state as "added" (launchd alone is hundreds of rows). Discard this batch or seed the allowlist from
   it. It must not reach Discord.
1. Label for one week. Every added row that fires after baseline gets marked real or expected. Expected
   ones (a cron entry you added, a key you installed) go into the allowlist so they never fire again.
1. Consider it calibrated when a week passes with only real findings or none.

## Delivery and the alerter

1. osqueryd writes differential results to its results log as JSON lines. The macOS default is usually
   `/var/log/osquery/osqueryd.results.log`. Confirm this on the installed version.
1. The alerter reads new lines, keeps only rows with action "added" from these three query names, applies
   the allowlist, and posts the survivors to the Hermes webhook. Each posted alert must be self-contained
   and actionable (see Operator constraints): what, where, when, and the one next action.
1. Post on every match with no batching. A fired alert must leave the machine immediately, so that if the
   host is later tampered with, the alert has already reached Discord.
1. Everything else osquery logs stays on disk and queryable. It is never pushed.
1. Add a separate once-daily heartbeat job, at a consistent time, that posts "pipeline healthy, nothing
   to report" to the same webhook. This does two jobs: it makes silence mean safe rather than broken, and
   it passively confirms the pipeline is alive so Stephen never has to check on it. If the heartbeat does
   not arrive, that absence is itself the signal that something broke.
1. Match the POST body to whatever the Hermes webhook expects. This contract is still unconfirmed (see
   Resolve before going live).

## Guardrails (never do these)

1. Never push raw events or "potential" findings to Discord. Only calibrated, high-confidence detections.
1. Never include secrets or raw keys in an alert payload.
1. Never skip the baseline-discard step in calibration.
1. Never add a detection beyond the three without running it through the same calibrate-before-alert
   discipline.
1. Never add a second notification channel. One Discord channel only.
1. Never send an alert that is not self-contained and actionable. If it does not carry its own context
   and a next action, it is not ready to send.

## Known limits (accept these for now)

1. Single self-monitoring host: the watcher and the watched are the same machine. Root-level compromise
   can tamper with osquery or its log. Immediate off-box forwarding reduces this but does not remove it.
1. Host-based only: this sees dresden alone. There is no visibility into other devices on the network.
1. Hourly sampling catches recurring or persistent changes, not one-shot bursts between samples. This is
   acceptable for these slow-changing assets.

## Resolve before going live

1. Confirm the Hermes webhook payload contract and map the alerter's POST body to it.
1. Confirm the osquery results-log path and the differential-logging flags on dresden's installed
   version.
1. Run all three queries on dresden to replace [UNVERIFIED] with confirmed.

## Deferred (next, not now)

1. Beaconing detection (a new binary making outbound connections) and new-listening-port detection. Both
   are noisy on a development machine, because new binaries and new ports are normal here. Add them
   later, with baselining designed for that churn, and consider scoping them away from your day-to-day
   development activity.
