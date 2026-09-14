# Hermes sandbox revalidation, 2026-09-14

Verdict document for the `docs/remaining-work.md` task "Revalidate the old Docker/profile, trigger,
network and artifact-copy assumptions against supported Hermes interfaces" (section: Hermes security
investigation, recovered from #24). No code was written, no Hermes file was modified, and nothing was
installed or configured. Every probe in this record is either a read of the installed source, a `--help`
of the installed command-line interface, or a throwaway container that was removed in the same command.

## The question

The 2026-06-03 analysis-agent design
(`docs/superpowers/specs/2026-06-03-osquery-analysis-agent-design.md` on the `docs/osquery-design`
branch, sections 4, 5 and 11) asserts a specific set of containment properties for a sandboxed
investigator running under Hermes Agent, and treats them as facts drawn from Hermes's documentation. The
task asks one question of each: **is the property real at a revision this machine can actually run,
reachable through supported configuration rather than a patch, and what is the check that would prove it
on this host?**

A second question rides along, because the answer changes the shape of the work: does the old design's
own architecture (a webhook agent route, a Docker terminal backend, an added outbound allowlist, a prompt
that forbids de-escalation) survive contact with the installed interfaces, or does a different supported
shape do the same job with fewer moving parts?

## Verdict

**Reject the old plan's containment story as written. Adopt a corrected control set, of which most items
are reachable through supported configuration on this host, four are genuine gaps, and one is a tier
mismatch that Hermes's own security policy calls out explicitly.**

The single most important finding is not a missing flag. It is that **Hermes Agent's own `SECURITY.md`
says the posture the old design chose is the wrong one for this input class.** Verbatim, from the
installed revision's `SECURITY.md` section 2.2:

> Terminal-backend isolation is the right posture when the concern is LLM-emitted destructive shell or
> unwanted file-tool writes, and the operator is otherwise trusted.

and, of whole-process wrapping:

> This is the supported posture when the agent ingests content from surfaces the operator does not
> control [...]

A suspect launch item's label, path and `ProgramArguments`, and the strings inside a suspect binary, are
by definition content the operator does not control. The old design picked terminal-backend isolation.
Upstream names whole-process wrapping (Hermes's own container image and Compose setup, or NVIDIA
OpenShell for layer-7 outbound policy) as the supported answer for exactly this case, and is explicit
that nothing inside the agent process is containment:

> **The only security boundary against an adversarial LLM is the operating system.** Nothing inside the
> agent process constitutes containment - not the approval gate, not output redaction, not any pattern
> scanner, not any tool allowlist.

Three consequences follow, and they are the reasons for the verdict:

1. **Terminal-backend isolation does confine the tools the old design cares about** (shell, and the file
   tools, which are built on the shell contract). That part of the old design is correct and is
   configurable here.
1. **It confines nothing else the agent process does.** Upstream lists the exclusions: the code-execution
   tool, Model Context Protocol subprocesses, plugin loading, hook dispatch and skill loading all run in
   the agent's own interpreter or as host subprocesses. Any of those in the investigator's tool surface
   reopens the host.
1. **The outbound allowlist the old design proposed to build is unnecessary in the shape it proposed, and
   insufficient in the shape it promised.** Unnecessary because the model call is made by the host
   controller process, not from inside the container, so the container needs no outbound access at all
   and can run fully networkless (measured below). Insufficient because a Docker-network allowlist would
   not have covered the host-side leg, which is where the untrusted text actually leaves the machine.

The good news is that the corrected control set is smaller than the old one and needs no new
infrastructure: a dedicated profile, four terminal keys, one gateway key, a narrowed tool surface, and a
publisher this repository already knows how to write. The bad news is that four properties the old plan
counted on are simply absent, and one of them (unattended dangerous-command auto-approval) is the
opposite of what was assumed.

**Implementation stays blocked on the operator**, exactly as the ledger says. This document does not
unblock it; it replaces guesses with measurements so the decisions are made against facts.

## What was checked and how

**Installed Hermes.** `hermes --version` reports:

```text
Hermes Agent v0.17.0 (2026.6.19) · upstream a4091e49
Project: /Users/stephen/.hermes/hermes-agent
Python: 3.11.15
OpenAI SDK: 2.24.0
```

`git log -1 --format='%H%n%cI' a4091e49` in that checkout: `a4091e49f10ddceaac1a902848aabfb1b9aae210`,
committed **2026-06-25**. The launcher at `~/.local/bin/hermes` unsets `PYTHONPATH` and `PYTHONHOME` and
execs `~/.hermes/hermes-agent/venv/bin/hermes`, so the installed tree is the authority for behavior on
this machine.

**Upstream reference.** `b6b53c69` is the revision the 2026-09-13 supported-interface review read. It is
**not present in the local checkout** (`git cat-file -t b6b53c69` returns "Not a valid object name"), so
every upstream-only claim below is marked as such and was taken from the upstream file at that revision
through a fetch, not from local source.

**Host.** dresden, macOS (Darwin 25.2.0). Docker server **29.7.2**
(`docker info --format '{{.ServerVersion}}'`). Container probes used `alpine:3.20`, pulled for this
record. Three Hermes gateways are live (`pgrep -fl hermes`): the default profile plus `nicodemus` and
`butters`, each started with `gateway run --replace`. Existing profiles: `butters`, `concerned`,
`elaine`, `nicodemus`. There is **no** `security-analyst` profile and no Hermes LaunchAgent under
`Library/LaunchAgents/`.

**Source files read in the installed tree.** `tools/environments/docker.py`,
`tools/environments/local.py`, `tools/terminal_tool.py`, `tools/file_tools.py`,
`tools/credential_files.py`, `tools/env_passthrough.py`, `tools/code_execution_tool.py`,
`tools/approval.py`, `toolsets.py`, `hermes_cli/platforms.py`, `hermes_cli/tools_config.py`,
`hermes_cli/kanban_db.py`, `hermes_cli/kanban.py`, `gateway/platforms/webhook.py`,
`gateway/platforms/base.py`, `gateway/kanban_watchers.py`, `agent/file_safety.py`, `cli.py` (the terminal
config bridge), `SECURITY.md`, `cli-config.yaml.example`, `website/docs/user-guide/security.md`,
`website/docs/user-guide/messaging/webhooks.md`, `website/docs/user-guide/features/kanban.md`.

**Command-line interfaces exercised** (help output only, no board writes): `hermes kanban --help`,
`hermes kanban create --help`, `hermes kanban show|runs|tail|log|complete|notify-subscribe --help`,
`hermes profile --help`.

**Rendered, not guessed.** The container security flags were produced by calling the installed function
rather than reading the constant:

```text
$ ./venv/bin/python -c "from tools.environments.docker import _build_security_args; ..."
root-drop mode : ['--cap-drop', 'ALL', '--cap-add', 'DAC_OVERRIDE', '--cap-add', 'CHOWN',
                  '--cap-add', 'FOWNER', '--security-opt', 'no-new-privileges', '--pids-limit', '256',
                  '--tmpfs', '/tmp:rw,nosuid,size=512m',
                  '--tmpfs', '/var/tmp:rw,noexec,nosuid,size=256m',
                  '--tmpfs', '/run:rw,noexec,nosuid,size=64m', '--cap-add', 'SETUID',
                  '--cap-add', 'SETGID']
host-user mode : [... same, without SETUID/SETGID]
```

**Network probes.** Three throwaway containers, each removed in the same command.

```text
# 1. Hermes's default flag set, no --network flag (this is what Hermes passes today)
$ docker run --rm --cap-drop ALL --cap-add DAC_OVERRIDE --cap-add CHOWN --cap-add FOWNER \
    --security-opt no-new-privileges --pids-limit 256 alpine:3.20 \
    sh -c 'wget -q -O- -T5 https://example.com | head -c 60; echo; echo "egress-exit=$?"'
<!doctype html><html lang="en"><head><title>Example Domain</
egress-exit=0

# 2. Same, with --network=none appended the way terminal.docker_extra_args appends
egress-exit=1
lo                      # the only interface present

# 3. Home network reachability from the default bridge (host default gateway 192.168.1.1)
lan-gateway-ping-exit=0
host-internal-ping-exit=0          # host.docker.internal answers too

# 4. docker exec still works with --network=none, and /workspace is writable
exec works: 0:0
egress-exit=1
workspace writable
```

**Live configuration read** (secrets redacted at read time, never printed). `~/.hermes/config.yaml`
defines three webhook routes, all `deliver_only: true`: `priority` (template `{alert.title}` /
`{alert.detail}`), `pns`, and `unattended-upgrades`, each with its own Discord `chat_id`. The model block
is `provider: openai-codex`, `base_url: https://chatgpt.com/backend-api/codex`, `fallback_providers: []`.
`security.tirith_enabled: true`, `tirith_path: tirith`, `tirith_fail_open: true`. There is **no**
`posture` route.

**Producer side, in this repository.** `pns/crates/pns-adapters/src/destinations/hermes.rs` (body fields
and `channel_url`), `pns/crates/pns-hermes/src/post.rs` (headers), `posture/crates/posture/src/alert.rs`
(the fixed route name), and `.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl` (the tracked
route checker).

**Research input, as instructed.**
`~/Documents/Sandboxed_Agent_Prompt_Injection_Research_20260603/report.md` was read in full. It is not
re-litigated here; its conclusions are load-bearing for two sections below and are cited where they bind.

## Findings, control by control

Each control below names the old plan's claim, the measured reality, a verdict, and the acceptance check
that would prove it on this host. "Supported" means reachable through documented configuration keys or
documented command arguments, with no change to Hermes source.

### 1. Capability drop: supported, but not "all"

**Old claim (section 4):** "Drops **all** Linux capabilities."

**Measured:** `--cap-drop ALL` is applied, then `DAC_OVERRIDE`, `CHOWN` and `FOWNER` are added back
unconditionally, and `SETUID` plus `SETGID` are added back as well unless the container runs as the host
user. The rendered arrays are quoted above. The code comments give the reasons: bind-mount writes,
package managers, and the image's own privilege-drop helper.

**Verdict: supported with a correction.** The reduction is real and meaningful; "all" is wrong.

**Acceptance check:**
`docker inspect --format '{{json .HostConfig.CapAdd}} {{json .HostConfig.CapDrop}}' <container>` on a
live investigator container. Setting `terminal.docker_run_as_host_user: true` drops `SETUID`/`SETGID`
from the added set, which is the tighter of the two modes and costs nothing here because the investigator
installs no packages.

### 2. No-new-privileges and process limit: supported as claimed

`--security-opt no-new-privileges` and `--pids-limit 256` are unconditional. **Verdict: supported.**
**Acceptance check:** the same `docker inspect` read (`.HostConfig.SecurityOpt`,
`.HostConfig.PidsLimit`).

### 3. "No host environment variables in the container by default": supported, for a different reason

than the old plan gives

**Old claim (section 4):** "**No host environment variables** in the container by default;
`KEY`/`TOKEN`/`SECRET`/`PASSWORD` stripped from code execution."

**Measured:** two separate mechanisms were conflated.

- The **Docker terminal** forwards nothing implicitly. `DockerEnvironment._build_init_env_args` starts
  from the `docker_env` config dict only, then adds
  `explicit_forward_keys | (passthrough_keys - blocklist)`. With no `terminal.docker_forward_env`, no
  `terminal.env_passthrough`, and no skill declaring `required_environment_variables`, that set is empty
  and no host variable is injected. This is *stronger* than a name-pattern strip.
- The `KEY`/`TOKEN`/`SECRET`/`PASSWORD`/`CREDENTIAL` substring scrub is in `tools/code_execution_tool.py`
  (`_SECRET_SUBSTRINGS`), and governs the **code-execution** child process, not the Docker terminal.

Two bypasses are worth stating because they are silent. A skill's `required_environment_variables` are
auto-registered as passthrough when the skill is loaded, and any variable not on the provider-credential
blocklist is legitimately registerable (the blocklist itself is derived from the provider registry plus a
fixed list, and skill registration cannot override it; the code cites advisory GHSA-rhgp-j443-p4rf as the
reason). And `terminal.docker_forward_env` **deliberately wins over the blocklist**: the comment says
explicit entries "are an intentional opt-in and must win over the generic Hermes secret blocklist."
Values are read from the process environment first and then from `~/.hermes/.env`.

**Verdict: supported, conditional on the investigator loading no skills and declaring no
`docker_forward_env`.**

**Acceptance check:** `docker inspect --format '{{json .Config.Env}}'` on a live investigator container,
asserting it holds only the image's own variables. Run it after a real investigation, not after a bare
container start, because the injection happens at session initialization.

### 4. "Only a per-task `/workspace` is bound": gap, three automatic mounts exist

**Old claim (section 4 and 5):** "Only a per-task `/workspace` is bound"; "**No** host mounts, **no**
credential files, **no** `env_passthrough`."

**Measured:** `DockerEnvironment.__init__` adds, before any user configuration, up to three families of
bind mounts from `tools/credential_files.py`:

- **Credential files**, read-only, from skill `required_credential_files` declarations and
  `terminal.credential_files` config. Empty when neither exists.
- **The profile's skills directory**, read-only, at `/root/.hermes/skills`, plus every external skills
  directory. Mounted whenever `<HERMES_HOME>/skills` is a directory. There is a symlink-sanitizing copy
  step, which tells you bind mounts following symlinks was a real finding upstream.
- **Five host cache directories**, read-only: `cache/documents`, `cache/images`, `cache/audio`,
  `cache/videos`, `cache/screenshots`, each mounted if it exists on disk.

All three resolve against the **active profile's** `HERMES_HOME`, so a dedicated investigator profile
starts with none of them and acquires them only as that profile accumulates skills and cached media.

**Verdict: gap against the claim as written; manageable by profile hygiene.** The claim is true only for
a profile with no skills directory and no cache directories.

**Acceptance check:** `docker inspect --format '{{json .Mounts}}'` on a live investigator container. The
expected set is the workspace and home entries and nothing else. Re-run it after any change to the
investigator profile, because a skill installed into that profile silently adds a mount.

### 5. "Ephemeral tmpfs wiped on cleanup" and "cross-session isolation": gap under the defaults

**Old claim (section 4):** "**ephemeral tmpfs** wiped on cleanup; **cross-session isolation**."

**Measured:** both properties are off by default, for two independent reasons.

- `container_persistent` defaults to **true** (`TERMINAL_CONTAINER_PERSISTENT`, documented in
  `cli-config.yaml.example` as "Persist filesystem across sessions (false = ephemeral)"). In that mode
  `/root` and `/workspace` are host bind mounts under `<sandbox>/docker/<task-id>/`, not tmpfs.
- `docker_persist_across_processes` defaults to **true**. Containers are labeled `hermes-agent=1`,
  `hermes-task-id=<id>`, `hermes-profile=<name>`, and a new process **reuses** an existing container
  matching `(task-id, profile)`. Reuse matches on labels only; image, mounts and resource limits are
  deliberately not compared. `DockerEnvironment.cleanup` in persist mode is a **no-op**: the container is
  left running. Its docstring is explicit that this implements the documented "ONE long-lived container
  shared across sessions" contract, and that reclamation is left to an orphan reaper that removes labeled
  containers untouched for twice `lifetime_seconds` (default 300) at the next Hermes start.

`_resolve_container_task_id` collapses every ordinary task id to the literal string `default`, so the
label is not per-investigation either. Back-to-back investigations under one profile therefore share one
container, and whatever the previous run wrote is still there.

**Verdict: gap under defaults; closable with two documented keys.**
`terminal.container_persistent: false` makes the workspace and home tmpfs, and
`terminal.docker_persist_across_processes: false` restores stop-and-remove on cleanup. Both bridge from
`config.yaml` through `cli.py`'s `env_mappings` (to `TERMINAL_CONTAINER_PERSISTENT` and
`TERMINAL_DOCKER_PERSIST_ACROSS_PROCESSES`). `docker_persist_across_processes` is read by the code but is
**not** documented in `cli-config.yaml.example`, so it is an undocumented-but-supported key: worth a
comment in whatever configuration this repository ends up writing.

**Acceptance check:** run two investigations back to back; `docker ps -a --filter label=hermes-agent=1`
must show no surviving container between them, and the second run must not see a file the first wrote.

### 6. Outbound network: gap as stated, and the fix is smaller than the old plan's

**Old claim (section 4):** Hermes "already blocks calls to *other rooms in the house* - the home LAN +
homelab + cloud-metadata addresses (its SSRF protection) - which kills lateral pivot", and the one
addition needed is an outbound allowlist limiting the container to the model endpoint and the webhook.

**Measured:** both halves are wrong.

- The server-side request forgery protection in `tools/url_safety.py` is a **pre-flight check on
  Hermes-owned uniform resource locator tools**: `tools/web_tools.py`, `tools/vision_tools.py`,
  `tools/browser_tool.py`, `tools/skills_hub.py` and the platform adapters import `is_safe_url`. Its own
  docstring documents two limits (rebinding of the domain name system, and redirects on third-party
  search backends). Nothing routes a shell command through it. A `curl`, `wget` or `nc` inside the
  container never touches it. Probe 3 above confirms the consequence: the home gateway at 192.168.1.1 and
  `host.docker.internal` both answer from inside a container started with Hermes's flag set.
- `DockerEnvironment` accepts a `network` parameter that appends `--network=none`, but
  `tools/terminal_tool.py` **never passes it** (the `_DockerEnvironment(...)` construction has no
  `network=` argument). The flag is unreachable from configuration through that path.

The correction that matters: **the container does not need outbound access at all.** The agent loop, and
therefore the model application programming interface call, runs in the host `hermes` process; only tool
execution is dispatched into the container, over `docker exec`, which is not network-dependent. Probe 4
proves `docker exec` works and `/workspace` is writable with `--network=none`. So the old plan's
two-destination allowlist collapses to one flag, and that flag is expressible today:
`terminal.docker_extra_args: ["--network=none"]`. The example configuration documents `docker_extra_args`
as "extra flags passed verbatim to docker run (appended after security defaults)", and because Hermes
passes no `--network` of its own there is no conflicting flag to fight.

What `--network=none` does **not** cover is the host-side leg, and that leg is the one that carries the
untrusted text off the machine: the controller's model call, plus `web_search` and `web_extract` if they
are in the tool surface. Closing the host-side leg is a tool-surface decision (finding 10) and a provider
decision (open question 3), not a Docker flag.

**Verdict: gap as stated; the container half is fully closable with one documented key, and the claimed
lateral-pivot protection does not exist.**

**Acceptance check:** from inside a live investigator container, `wget -q -O- -T3 https://example.com`
must fail, `ping -c1 -W3 <home gateway>` must fail, and `ip -o addr` must list only `lo`. Then confirm
the investigation still completes, which proves the model call is not on that path.

### 7. Pre-execution command scanning: gap, inactive on this host right now

**Old claim (section 4):** "Pre-exec command scanning (Tirith) for pipe-to-interpreter / homograph
patterns."

**Measured:** the live configuration enables it (`security.tirith_enabled: true`, `tirith_path: tirith`,
`tirith_timeout: 5`, `tirith_fail_open: true`) but `command -v tirith` finds nothing. It is not installed
and is not declared in `.chezmoidata/system_packages_autoinstall.yaml`. With `tirith_fail_open: true`,
every command passes unscanned and silently.

**Verdict: gap, and a live misconfiguration independent of this task.** Either install the scanner
(upstream documents `brew install sheeki03/tap/tirith`) or set `tirith_fail_open: false` so the
configuration stops claiming a control that is absent. Note also that the prompt-injection research's
framing applies: a pattern scanner on an attacker-influenced string is a probability reduction, and
upstream's own `SECURITY.md` puts pattern scanners explicitly outside the containment boundary. Treat
this as hygiene, not as a load-bearing control.

**Acceptance check:** `command -v tirith` resolves, and a deliberately hostile command (a pipe to a
shell) is refused in a scratch session with `tirith_fail_open: false`.

### 8. Dangerous-command approval: gap, and the default is the opposite of the assumption

**Old claim (section 11a):** "dangerous-command auto-approval OFF."

**Measured:** `tools/approval.py` decides by context. When neither `HERMES_INTERACTIVE` nor the gateway
approval context is present, and the session is not a cron session with `approvals.cron_mode: deny`, a
dangerous command is **auto-approved** with a warning log:

```text
AUTO-APPROVED dangerous command in non-interactive non-gateway context (pattern: %s): %s
  - set HERMES_INTERACTIVE or HERMES_GATEWAY_SESSION to require approval.
```

A kanban worker is exactly that context. The dispatcher spawns `hermes -p <profile> chat -q ...` with
`stdin=subprocess.DEVNULL` and `start_new_session=True`; it sets no `HERMES_INTERACTIVE`. The gateway
context test reads `HERMES_GATEWAY_SESSION` from the process environment or a session-platform context
variable; only `tui_gateway/server.py` sets that environment variable, and context variables do not cross
a process boundary, so a dispatcher-spawned worker lands in the auto-approve branch.

The alternative is worse, not better: if the variable *were* set, the worker would submit a pending
approval with no listener and stall until its runtime cap fired. The cron-session comment in
`_is_gateway_approval_context` documents precisely that failure and is why cron is excluded.

**Verdict: gap. There is no supported "approval off" posture for an unattended worker, and approvals are
not a containment layer anyway** (upstream's section 2.2 names the approval gate as not containment).
Containment must come from the container plus the tool surface.

**Acceptance check:** none needed for the control, because the control does not exist. What is worth
asserting is the consequence: with `--network=none`, a tmpfs workspace and a two-toolset surface, an
auto-approved destructive command can damage only the throwaway container.

### 9. Profile binding and backend pinning: supported, and better than the old plan knew

**Old claim (section 11a):** each profile gets its own `HERMES_HOME` and `config.yaml`, so a dedicated
profile can set `terminal.backend: docker`, and kanban `--assignee` pins the worker to it.

**Measured: correct, and the mechanism is stronger than described.** `~/.hermes/profiles/butters/`
contains its own `config.yaml`, `auth.json`, `cache`, `home`, `hooks` and `gateway.pid`. The dispatcher's
`_default_spawn` sets `env["HERMES_HOME"] = resolve_profile_env(profile_arg)` before exec, with a comment
explaining that without it the child would fall back to the default profile root and ignore the
profile-specific configuration entirely. So `--assignee` does pin the profile, and the profile's
`terminal.backend` decides the sandbox.

Two additions the old design did not have:

- **`_resolve_worker_cli_toolsets`** resolves the assignee profile's effective command-line-interface
  tool surface at dispatch time and passes it as an explicit `--toolsets` pin, "so worker startup cannot
  fall back to a stale root/active-profile config." That makes `platform_toolsets.cli` in the
  investigator profile's `config.yaml` the supported lever for restricting host access.
- **Webhook profile multiplexing.** `gateway/platforms/webhook.py` registers both
  `/webhooks/{route_name}` and `/p/{profile}/webhooks/{route_name}`, the latter honored when
  `gateway.multiplex_profiles` is on, so an inbound event can be pinned to a named profile by path.

**One inheritance fact that must be recorded, because it bounds what "no host secrets" can mean.**
`_default_spawn` builds the child environment as `env = dict(os.environ)` and then overrides
Hermes-specific keys. The worker **controller process** therefore inherits the dispatcher gateway's full
environment and runs as the operator's user with the operator's filesystem reach. Only the terminal and
file tools are inside the container. "No host secrets" is true of the **container**; it is not true of
the **process**, and cannot be made true through configuration. The tool surface is what stands between
the model and that process.

**Acceptance check:** `hermes kanban show <id> --json` reports the run's `profile` as the investigator
profile; `ps` shows the worker's argv carrying `-p <profile>` and a `--toolsets` list with no unwanted
entry; and `docker ps --filter label=hermes-profile=<profile>` shows the container.

### 10. Tool surface: supported, and the decisive control

**Measured.** `toolsets.TOOLSETS` holds 57 named toolsets. Two facts decide the design.

- **The webhook platform's default tool surface is deliberately tiny.** `hermes_cli/platforms.py`
  registers `webhook` with `default_toolset="hermes-webhook"`, and that toolset is exactly
  `["web_search", "web_extract", "vision_analyze", "clarify"]`, with the comment: "Webhook events may
  originate from untrusted third-party content [...] Keep the default webhook toolset intentionally
  constrained to avoid local file/system execution by prompt injection." **A webhook agent route
  therefore cannot run `codesign`, `strings` or `otool` at all**, which is the entire point of the old
  design's section 4 "tool surface" paragraph. `hermes_cli/tools_config.py::_get_platform_tools` keys off
  the platform name generically, so `platform_toolsets.webhook: [...]` can widen it, but widening it is
  deliberately re-opening a door upstream closed on purpose.
- **The code-execution toolset must be excluded.** `tools/code_execution_tool.py`'s own architecture
  comment describes the remote-backend transport: the script runs inside the terminal backend, but "Tool
  calls are written as request files; a polling thread on the parent reads them via env.execute(),
  dispatches, and writes response files." That is a designed bridge from inside the container to
  host-side tool dispatch. Upstream's `SECURITY.md` lists the code-execution tool among what
  terminal-backend isolation does not confine.

**Verdict: supported, and this is where the real boundary is set.** The kanban worker path (a full
command-line-interface session under a pinned profile) is the only supported path that can run the
inspection commands, and its tool surface is restricted by `platform_toolsets.cli` in that profile.

**Acceptance check:** the worker's argv `--toolsets` list contains only what was declared (`terminal` and
`file` are sufficient for `codesign`, `strings`, `otool` and reading the evidence), and specifically
excludes `code_execution`, `browser`, `web`, `delegation`, `cronjob`, `memory`, `skills`, and every
messaging toolset. The kanban lifecycle tools are appended automatically when `HERMES_KANBAN_TASK` is
set, so they need not be declared.

### 11. Trigger: the old plan's two-routes-on-one-event is wrong; two supported shapes exist

**Old claim (section 11):** "Two routes can fire on one incoming webhook and deliver independently."

**Measured: false.** `_handle_webhook` resolves `route_config = self._routes.get(route_name)` from the
path segment and runs one handler. One post reaches one named route and one destination. This matches the
ledger's existing note.

Two supported shapes remain, and they differ in what they can do.

- **Shape A, two posts.** The producer posts twice: once to the existing `deliver_only` alert route, once
  to an agent route (optionally under `/p/<profile>/webhooks/<route>`). Cost: the agent route's tool
  surface is the four read-only tools above unless widened, so the investigator cannot inspect the
  artifact.
- **Shape B, alert post plus a local card.** The producer posts the alert to the `deliver_only` route as
  today, and separately invokes `hermes kanban create` locally. This is the shape that gets the full
  supported control set, because `create` carries every knob the investigation needs:

```text
--assignee <profile>          pin the profile, and therefore the Docker backend and the toolset
--workspace dir:<abs path>    the evidence directory (must be absolute; relative is rejected
                              explicitly to prevent confused-deputy traversal)
--max-runtime 5m              the dispatcher SIGTERMs then SIGKILLs and re-queues on overrun
--max-retries 1               trip the circuit breaker on the first failure, no retries
--idempotency-key <key>       "If a non-archived task with this key exists, its id is returned
                              instead of creating a duplicate"
--json                        emit the created task id for the producer to correlate on
```

**Verdict: Shape B is the supported trigger.** It is also strictly better on the non-negotiable
invariant: the alert delivery and the card creation are two independent local operations, so no failure
of one can delay the other.

**Acceptance check:** kill the gateway, fire a Critical finding, and confirm the alert still reaches
Discord (the `deliver_only` route is served by the gateway, so this actually tests the producer's
ordering and its failure handling) while the card either queues or fails loudly and separately.

### 12. Transport deduplication: a real collision, with a concrete cause

**Measured.** The webhook adapter's idempotency key is read from headers in this order:
`X-GitHub-Delivery`, then `svix-id`, then `X-Request-ID`, and **failing all three, a millisecond
timestamp** (`str(int(time.time() * 1000))`). `_record_delivery_id` holds seen ids for
`self._idempotency_ttl = 3600` seconds, hardcoded, in a **single dictionary that is not keyed by route**.

Against that, `pns/crates/pns-hermes/src/post.rs` sends `X-Webhook-Signature` (which matches Hermes's
generic hash-based message authentication code path, confirmed in `_validate_signature`) and
`Idempotency-Key`. **Hermes does not read `Idempotency-Key`.** So today every pns post falls through to
the millisecond-timestamp fallback: retries are not deduplicated at all, and two posts that landed in the
same millisecond would collide.

The trap for the investigation work is the opposite one. If pns starts sending the alert correlation key
as `X-Request-ID`, then a second post carrying the same correlation key **within an hour, on any route,**
is answered `{"status": "duplicate"}` and silently dropped. That is exactly the ledger's requirement, now
with a mechanism: the header must carry a **per-delivery-attempt** identifier, and the alert correlation
key belongs in the **body**.

**Verdict: gap on the producer side, fully in this repository's control.**

**Acceptance check:** post the same body twice with distinct `X-Request-ID` values and confirm two
deliveries; post twice with the same value and confirm the second returns `{"status": "duplicate"}`.

Two smaller transport facts worth carrying: the route rate limit defaults to 30 per minute
(`extra.rate_limit`), and the body cap defaults to 1,048,576 bytes (`extra.max_body_bytes`).

### 13. Alert and evidence metadata: confirmed gap, with the exact field list

**Measured.** `hermes_body` in `pns/crates/pns-adapters/src/destinations/hermes.rs` emits exactly
`agent`, `state`, `project`, `detail`, and `request_id`. There is no security class and no structured
artifact reference, which confirms the ledger. One addition: the body carries **no `event_type` or `type`
field**, and the adapter derives `event_type` from `X-GitHub-Event`, `X-GitLab-Event`,
`payload.event_type` or `payload.type`, falling back to `"unknown"`. So a route that declares an
`events:` filter would reject every pns post. Today's three routes declare no filter, so this is latent
rather than broken.

**Route names.** `posture/crates/posture/src/alert.rs` pins the route name `posture` ("the fixed posture
route is valid"). The live configuration has no `posture` route, so a posture alert posted through that
name would receive `404 {"error": "Unknown route: posture"}`. The tracked checker,
`.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl`, checks only `pns` and
`unattended-upgrades`, so nothing on this machine would report that.

That checker also encodes a standing invariant that the investigation work must not trip over: it warns
when a checked route is **not** `deliver_only`, on the grounds that "hermes feeds its messages to an
AGENT instead of posting them verbatim, and those messages carry third-party package and skill names." An
investigator route is an agent route by construction. If Shape A is ever chosen, that route needs a
deliberate carve-out in the checker, with the reason written down. Shape B avoids the conflict entirely,
because it adds no agent route.

**Verdict: gap, on the producer side, plus two route-name reconciliations (`posture` missing from the
configuration, `posture` and any investigator route missing from the checker).**

**Acceptance check:**
`curl -s -o /dev/null -w '%{http_code}' -X POST http://127.0.0.1:8644/webhooks/posture -d '{}'` returns
something other than 404, and the checker's `expected_routes` array covers every route a producer on this
machine names.

### 14. Artifact copy-in: supported, two mechanisms, each with a named cost

**Old open item (section 11b):** "how the suspect file [...] is mounted/copied into the helper's
container to read (kanban/Docker backends require attachment paths be mounted - confirm the mount path)."

**Measured. The open item resolves, and the answer is documented upstream.** The installed
`website/docs/user-guide/features/kanban.md` has a "File attachments" section that states the mechanism
and the catch:

> **What the worker sees** - when the dispatcher hands a task to a worker, the worker's context includes
> an **Attachments** section listing each file's name and its **absolute path**.

and, in an admonition:

> Attachment paths resolve directly on the **local** terminal backend, which is the default for Kanban
> workers. If you run workers on a remote backend (Docker, Modal), mount the board's `attachments/`
> directory into the sandbox so the absolute paths in the worker context are reachable.

The rendering code matches (`hermes_cli/kanban_db.py`, `build_worker_context`: an `## Attachments`
heading, then one bullet per file ending in the backtick-quoted `stored_path`). Storage is
`<hermes-home>/kanban/attachments/<task-id>/`, or the per-board path for a named board, overridable with
`HERMES_KANBAN_ATTACHMENTS_ROOT`.

**Revision delta, and a correction to the ledger.** The ledger records "Upstream has a separate
`kanban attach` command". At the reviewed upstream revision the capability is a pair of **agent-callable
tools**, `kanban_attach` (inline bytes plus name, 25 megabyte cap) and `kanban_attach_url`, not a
command-line subcommand; the upstream subcommand list contains no `attach`. At the **installed** revision
neither exists: `hermes kanban --help` lists no `attach`, `toolsets.py` declares no `kanban_attach` among
the kanban tools, and the only match in the tree is a dashboard-plugin test. Uploading is a dashboard
action at this revision. A producer cannot attach a file without either upgrading or writing the row and
the file itself, which would be reaching into Hermes's own storage rather than using an interface, and is
therefore out of scope by the task's own rule.

So the two mechanisms actually available here are:

- **Attachments plus a static read-only volume.** `terminal.docker_volumes` entries are passed verbatim,
  so `"<attachments root>:/evidence:ro"` is expressible and gives a genuinely read-only evidence mount.
  **Cost:** `docker_volumes` is a static configuration value with no per-task templating, so the mount is
  the whole attachments root. One investigation can read every other task's attachments. Acceptable only
  if that root is dedicated to this workflow.
- **`--workspace dir:<abs path>` plus cwd mounting.** `resolve_workspace` returns the given absolute path
  for `dir:` (creating it if missing, rejecting relative paths explicitly). The dispatcher exports
  `TERMINAL_CWD=<workspace>`; with `terminal.docker_mount_cwd_to_workspace: true`, `_get_env_config`
  recognizes a host path (the prefix list is `/Users/`, `/home/`, `C:\`, `C:/`), sets `host_cwd`, remaps
  the container cwd to `/workspace`, and `DockerEnvironment` binds `-v <host dir>:/workspace`. This is
  genuinely per-task and needs no attachment machinery: the producer creates the directory, copies the
  bounded evidence in, and names it on `create`. **Cost:** the bind is read-write, with no `:ro`, so the
  investigator can write into the evidence directory on the host. That directory should therefore be a
  throwaway under a dedicated root, never the live artifact's own location.

**`scratch` is not ephemeral.** `resolve_workspace` for `scratch` returns
`<board-root>/workspaces/<task-id>/`, created and persisted, with the path recorded back onto the task
row. It is per-task and path-stable, and it survives the run until garbage collection. The old design's
"per-task `--workspace scratch` (ephemeral)" is wrong on the ephemeral half.

**Verdict: supported, with the second mechanism preferred, and one documented cost to accept (read-write)
or engineer around.**

**Acceptance check:** `docker inspect --format '{{json .Mounts}}'` shows exactly one evidence mount at
the expected source and destination; inside the container, the evidence files are readable and a write
outside `/workspace` fails.

### 15. Advisory publication: the raw completion is already bounded, and a trusted publisher is supported

**Old claim (section 8):** the advisory is "posted as a **separate message beneath** the deterministic
alert (a reply/threaded under it)", prefixed with a fixed label and length-capped.

**Measured, and this is the most useful finding in the delivery half.** The kanban notifier
(`gateway/kanban_watchers.py::_kanban_notifier_watcher`) does **not** publish the model's text. It
publishes a fixed one-line message per terminal event, with the worker's summary truncated to its first
line and 200 characters:

```text
✔ Kanban <id> done — <title>\n<first line of summary, 200 chars>
⏸ Kanban <id> blocked<: reason, 160 chars>
✖ Kanban <id> gave up after repeated spawn failures<\nerror, 200 chars>
✖ Kanban <id> worker crashed (pid gone); dispatcher will retry
⏱ Kanban <id> timed out (max_runtime=<n>s); will retry
```

Two things follow.

- **"Failure or timeout must be distinguishable from an all-clear" is natively supported.** Five distinct
  terminal kinds with five distinct renderings, and the subscription is deliberately **kept alive** for
  the non-final kinds so a respawn notifies again.
  `hermes kanban notify-subscribe <task-id> --platform discord --chat-id <id> [--thread-id <id>]` is
  non-interactive.
- **The structured contract the ledger asks for already has a home.** `kanban complete` accepts
  `--metadata` ("JSON dict of structured facts"), stored on the closing run, and
  `hermes kanban show <id> --json` / `hermes kanban runs <id> --json` return each run's `outcome`,
  `summary`, `error` and `metadata` (verified in `hermes_cli/kanban.py`, the two `json.dumps` payload
  literals). So a **trusted publisher owned by this repository** can read the closing run, validate
  `metadata` against an allowed field list and a fixed response vocabulary, render its own text, and post
  that through the existing `deliver_only` route. The model's prose never reaches the channel. That moves
  enforcement out of the prompt, which is what the ledger asks for and what the prompt-injection research
  says a prompt cannot deliver.

**One capability the old design assumed that is not available.** Threading the advisory under the alert
message needs the alert's message identifier. `_deliver_cross_platform` passes only
`deliver_extra.message_thread_id` or `deliver_extra.thread_id` through as metadata, so a **pre-existing**
thread can be targeted but a reply to a specific just-posted message cannot. The achievable shape is a
separate message in the same channel that names the alert's correlation identifier in its own text.

**Verdict: supported, and better than the old plan assumed, with threading downgraded to correlation by
identifier.**

**Acceptance check:** an investigation whose worker writes hostile text into its summary produces a
channel message containing none of that text, only the publisher's rendering of validated `metadata`
fields.

### 16. Evidence upload: an unguarded exfiltration path, closable with one key

**Measured, and this is the finding that most deserves the operator's attention.** After delivering the
text notification for a `completed` event, the notifier calls `_deliver_kanban_artifacts`, which collects
file paths from three sources: `event_payload["artifacts"]`, **paths found in the summary text** by
`adapter.extract_local_files`, and paths found in the legacy `task.result`. It then uploads them to the
delivery channel with `send_multiple_images`, `send_video` or `send_document`.

The only filter is `BasePlatformAdapter.filter_local_delivery_paths`, which calls
`validate_media_delivery_path`. That function's docstring states the default posture plainly:

> Default mode (single-user / private gateway): accept any existing regular file that isn't under the
> credential / system-path denylist

The denylist is `/etc`, `/proc`, `/sys`, `/dev`, `/root`, `/boot`, `/var/log`, `/var/lib`, `/var/run`,
plus, under the home directory, `.ssh`, `.aws`, `.gnupg`, `.kube`, `.docker`, `.config`, `.azure`,
`.gcloud` and `Library/Keychains`, plus the credential files at the Hermes root. **Everything else in the
home directory is uploadable**, including `~/workspaces/**`, `~/Documents/**`, and `~/.claude.json`.

So an investigator that has been talked into naming an absolute path in its completion summary gets that
file uploaded to Discord by the **host-side gateway**, entirely outside the container, whatever the
container's network policy is. This is the concrete mechanism behind the ledger's "no [...] evidence
upload was enabled".

**The supported close is one key.** `gateway.strict: true` bridges to `HERMES_MEDIA_DELIVERY_STRICT`
(`gateway/run.py`), which requires every delivered file to sit under a Hermes-managed cache, under an
operator allowlist (`HERMES_MEDIA_ALLOW_DIRS`), or inside a recency window. The docstring names the exact
threat: "Suitable for public-facing bots where prompt injection from one user shouldn't be able to
exfiltrate the host's secrets to that same user." That is this threat, with the attacker upstream of the
alert instead of in the chat.

**Verdict: gap under defaults, closable with `gateway.strict: true`, and worth doing regardless of
whether the investigator is ever built,** because the path is live today for every agent route on this
gateway.

**Acceptance check:** with `gateway.strict: true`, complete a scratch card whose summary names
`/Users/stephen/.claude.json` and confirm the gateway log records "Skipping unsafe local file path" and
no attachment arrives. Then repeat with a file under an allowlisted evidence root and confirm it does.

### 17. Monotonic advisory and fixed vocabulary: gap in the prompt, supported in the publisher

**Old claim (sections 6 and 7):** the helper "may only ADD concern, explain, or recommend", is
"structurally forbidden" from de-escalating, and remediation comes "from a fixed safe vocabulary only".
Section 7 is candid that the prompt is "defense-in-depth, NOT load-bearing", and rests the safety case on
alert-fires-first, sandbox containment, and the monotonic rule.

**Measured.** The first two structural legs hold and are configurable (findings 11 and 1 to 6). The
third, the monotonic rule, is **not** structural in the old design: as written it lives in the prompt and
in "output handling" that was never specified. The prompt-injection research is unambiguous that this is
the wrong place for it, and its "Poisoning the Watchtower" citation names the exact attacker goals ("a
malicious event labeled benign, a summary that omits or downplays the attack, or a remediation that
recommends no action") with a measured residual success rate of 11.8 percent under the strongest tested
defense.

**The measured mechanism that makes it structural** is the one in finding 15: `--metadata` plus
`runs --json` plus a publisher this repository owns. A validator that accepts only an enumerated set of
fields, rejects any response outside a fixed vocabulary, and renders its own text, is code, not a prompt.
Recommendation 2 of the research report describes this design in as many words: the deterministic verdict
always ships, and the model's contribution is display text that cannot remove or downgrade it.

**Verdict: gap as designed; supported once the publisher is the enforcement point.** This is the
"separate trusted publisher" the ledger already prefers, now with the exact read interface it would use.

**Acceptance check:** feed the validator a metadata document containing a de-escalating verdict and a
remediation outside the vocabulary; it must refuse to publish and must emit a failure notice that is
distinguishable from an all-clear.

## Assumptions made in the operator's place

Each of these is a choice this document made so the analysis could be concrete. Each names the
alternative. None is settled.

1. **Assumed: Shape B (local `hermes kanban create`) is the trigger, not a second webhook route.**
   Rationale: it is the only supported path whose tool surface can run the inspection commands, and it
   brings `--max-runtime`, `--max-retries`, `--idempotency-key` and `--workspace dir:` with it.
   *Alternative:* Shape A, a webhook agent route, widened with `platform_toolsets.webhook`. That
   deliberately reopens a door upstream closed for untrusted third-party content, and collides with the
   tracked route checker's `deliver_only` invariant.
1. **Assumed: `--network=none` on the container rather than an outbound allowlist.** Rationale: the
   container needs no outbound access, measured. *Alternative:* an allowlist, which is strictly more
   machinery for strictly less containment, or whole-process wrapping under NVIDIA OpenShell, which is
   the only reviewed option that constrains the **host-side** leg and is what upstream names for this
   input class.
1. **Assumed: the evidence directory is a throwaway under a dedicated root, accepting the read-write
   bind.** Rationale: `docker_mount_cwd_to_workspace` produces no `:ro` variant and is per-task, which
   the read-only `docker_volumes` route is not. *Alternative:* the attachments root mounted `:ro`,
   accepting cross-task visibility, at the cost of needing an upgrade before a producer can attach
   anything programmatically.
1. **Assumed: the investigator's tool surface is `terminal` and `file` only.** Rationale: those cover
   `codesign`, `spctl`, `strings`, `otool` and reading the copied evidence, and both run inside the
   container. *Alternative:* adding `web` so the investigator can look up a hash or a bundle identifier.
   That restores a host-side outbound path for untrusted-derived queries and should be a deliberate
   decision, not a convenience.
1. **Assumed: `gateway.strict: true` is acceptable gateway-wide.** Rationale: the artifact-upload path is
   live for every agent route today. *Alternative:* leaving it off and relying on the publisher never
   forwarding a path, which leaves the notifier's own artifact scan unguarded on the same channel.
1. **Assumed: the advisory is a separate message correlating by identifier, not a threaded reply.**
   Rationale: the supported metadata path carries only a pre-existing thread identifier. *Alternative:* a
   dedicated Discord thread per alert, created by the producer, whose identifier is then passed as
   `deliver_extra.thread_id`. That is expressible but adds a thread-lifecycle problem nobody has scoped.
1. **Assumed: the installed revision (a4091e49, 2026-06-25) is the target, not an upgrade.** Rationale:
   the task asks what supported interfaces provide, and this is what is installed. *Alternative:* upgrade
   to at least the reviewed upstream revision to get the `kanban_attach` tools and the review
   subcommands, at the cost of a configuration migration that
   `.chezmoiscripts/run_after_59-hermes-config-migrate.sh.tmpl` would have to carry.

## What would change the verdict

- **A decision to run whole-process wrapping.** If the gateway (or a dedicated investigator gateway)
  moves inside Hermes's own container image or under NVIDIA OpenShell, most of findings 3, 4, 8, 10 and
  16 stop being per-key configuration problems and become properties of the wrapper. This is the posture
  upstream names for untrusted input, and it is the only reviewed option that constrains the host-side
  leg. It is also a much larger change than anything else in this document.
- **A Hermes upgrade past the reviewed upstream revision.** `kanban_attach` and `kanban_attach_url` would
  make programmatic evidence attachment a supported interface, which changes finding 14's mechanism
  choice. Anything else new in the roughly two and a half months between a4091e49 and b6b53c69 was not
  reviewed here.
- **A locally hosted model.** If the investigator's model runs on this machine, open question 3 dissolves
  and the host-side outbound leg shrinks to the delivery channel. The old design already listed this as
  out of scope for version one, at a quality cost.
- **A measurement that contradicts a probe here.** Every claim above names its check. The ones most worth
  re-measuring on a real investigator container, because they are the ones a configuration mistake would
  silently reverse, are the mount list (finding 4), the environment list (finding 3), and the container
  survival between runs (finding 5).
- **A decision that the advisory is not worth it.** The prompt-injection research's verdict was "keep the
  deterministic pipeline", with a bounded middle path (its recommendation 2) as the only concession. This
  document describes how to build that bounded middle path with supported interfaces. It does not argue
  that it should be built.

## Open questions for the operator

1. **Execution and network boundary: terminal-backend isolation with `--network=none`, or whole-process
   wrapping?** Upstream's own policy says the second for content the operator does not control. The first
   is a handful of configuration keys on this host; the second is a project. Which tier is this workflow
   held to?
1. **Permitted evidence: what may be copied into the box, and who resolves it?** The old design says only
   the artifact at the finding's structured `path` field, resolved deterministically, never chosen by the
   agent. Is the referenced binary or script in scope, are narrowly scoped logs in scope, and what is the
   byte ceiling per investigation?
1. **Model-provider disclosure.** The live configuration routes to `provider: openai-codex` at
   `https://chatgpt.com/backend-api/codex`. An investigator sends attacker-controlled artifact text to
   that endpoint under your account. Is that acceptable, does the investigator profile get a different
   provider, or does this wait for a local model?
1. **Credential policy for the investigator profile.** A profile carries its own `auth.json`. Does the
   investigator get its own credential (blast radius bounded, one more secret to manage through
   KeePassXC) or share an existing one?
1. **Evidence-upload posture: turn on `gateway.strict` now?** The unguarded artifact path in finding 16
   is live today for every agent route on this gateway, independent of this workflow. Turning it on is
   one key and needs an allowlist root decided for anything you do want delivered.
1. **Tirith: install it or stop claiming it?** The configuration enables a scanner that is not installed,
   with `fail_open: true`. Either is defensible; the current state claims a control that does not run.
1. **Route reconciliation: add a `posture` route, and extend the tracked checker?** Posture pins the name
   `posture`; the configuration has no such route; the checker covers only `pns` and
   `unattended-upgrades`. Separately, if an agent route is ever added, the checker's `deliver_only`
   invariant needs a documented carve-out for it.
1. **Advisory scope: does the fixed response vocabulary from the old design's section 6 stand?**
   Quarantine the file, disable the launch item by label, revert the setting in System Settings. The
   publisher can only enforce a vocabulary that has been written down.
1. **Producer transport identifiers.** Confirm the intended split: a per-delivery-attempt value in
   `X-Request-ID`, and the alert correlation key in the body. This is a pns change, small, and it also
   fixes the fact that pns's current `Idempotency-Key` header is ignored by Hermes entirely.
