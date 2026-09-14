# The credential access boundary

Status: design, written 2026-09-14 with the operator asleep. Nothing here is approved. No code was
written or changed. Every choice made in the operator's place is listed in "Assumptions made in your
place" with its alternative, and the decisions that are genuinely theirs are in "Open questions".

Ledger task: `docs/remaining-work.md`, "Safe agent credentials and Infisical client integration",
the bullet beginning "Define and verify the actual access boundary" (line 2143 at the time of
writing).

## Why this exists

The ledger asks for a boundary: which secrets an agent may reach, which operations it may perform,
how long an approval lasts, how it is revoked, what the audit record looks like without secret
values in it, and what denial outside the permitted set looks like. It also fixes the threat model
in advance: arbitrary shell access, readable rendered configuration, and agent-editable templates
and scripts. And it forbids one specific bad answer, that hiding a value from the chat transcript or
putting it in a child process environment keeps it away from an unrestricted process running as the
same user.

This document does the measurement first, because the measurement changes the answer. The headline
is not that the current arrangement is weak at the edges. It is that the current arrangement contains
a live path from an agent-written file to the entire KeePass database, triggered by the operator
running the review command that is supposed to catch it.

## What was measured, and how

Every claim below was measured on dresden on 2026-09-13 and 2026-09-14 (macOS 25.2, chezmoi 2.72.1
from Homebrew, keepassxc-cli 2.7.12). Anything unverified is labeled.

### 1. A process environment is readable by any same-user process

`ps eww -p <pid>` and `ps -E -p <pid>` both printed the full environment of a separate process owned
by the same user, including a marker variable set only for that process. A full command line
including a `--token=<value>` argument was likewise visible through `ps -o args=` and `ps auxww`.

This is the claim the ledger names and rejects, now with evidence. On macOS there is no `/proc`, but
`ps` needs none. Passing a secret to a child process, in its environment or on its command line, is
publication to every process the operator's account runs.

### 2. Cross-process memory reading is gated, and is not the interesting path

`DevToolsSecurity -status` reports developer mode disabled, and an `lldb -p` attach against a
same-user process did not complete inside a twenty second budget (it appears to block on
authorization rather than fail). The account is in the `_developer` group, so this is a policy toggle
rather than a hard wall. It is recorded for completeness and is not relied on anywhere below, because
the paths in sections 3 through 6 need no debugger.

### 3. Every rendered secret target is readable by the agent, and the file mode is irrelevant

All fifteen chezmoi targets that pull secrets through `keepassxc` exist on disk. Fourteen are mode
0600; `~/.gitconfig` is 0644, and that one is fine, because its only KeePassXC lookup is
`keepassxcAttribute "GitHub (Webdavis) :: GPG :: Signing key" "Public Signing Subkey ID"`, which is a
public subkey identifier and not a secret. So the mode audit comes back clean.

Clean, and beside the point. Mode 0600 owned by `stephen:staff` denies other users. There are no
other users in this threat model. The agent runs as `stephen`. `cat ~/.hermes/.env` succeeds. The
rendered configuration is, in full, agent-readable, and no permission change on those files alters
that. The Claude Code deny list covers four of the fifteen (`~/.config/pns/config.toml`,
`~/.hermes/.env`, `~/.aws/credentials`, and any `*.kdbx` under the home directory) and the settings
template already records, in its own comment, that deny only covers the reads Claude Code recognizes
while `Bash(cat *)` and `Bash(grep *)` sit in the allow list, and that all of it is moot at
`permissions.defaultMode = bypassPermissions`. That comment is correct. The deny list is a guardrail
against accident, not a boundary against intent.

### 4. The source tree contains a pre-review code execution path

`~/.config/chezmoi/chezmoi.toml` sets `hooks.read-source-state.pre.command` to
`{{ .chezmoi.sourceDir }}/.install-password-manager.sh`, a file inside the repository the agent edits
all day.

Measured with a throwaway source directory and home directory, the `read-source-state` pre hook fires
on `chezmoi status`, `chezmoi diff`, `chezmoi managed`, `chezmoi execute-template` and
`chezmoi apply --dry-run`. It did not fire on `chezmoi source-path`. The hook also receives
`CHEZMOI_COMMAND`, so a hostile hook can choose which command it wakes up for.

`just d` is `chezmoi diff`. So the operator's first act of reviewing what an agent changed executes a
file that agent could have changed, before the diff is rendered and before the operator has seen
anything.

### 5. The vault client is replaceable by the agent, on PATH and at its absolute path

The effective `keepassxc` configuration on this machine is `command = "keepassxc-cli"` (unqualified),
`mode = "cache-password"`, `prompt = true`. In a login shell, `$HOME/.local/bin` is PATH position 2
and `/opt/homebrew/bin` is position 9, and `$HOME/.local/bin` is writable. So a file named
`keepassxc-cli` dropped in `~/.local/bin` wins over the real client.

Pinning the absolute path does not fix it. `/opt/homebrew/bin` is mode `drwxrwxr-x` owned by
`stephen:staff` and is writable by the agent, and `/opt/homebrew/bin/keepassxc-cli` is a
user-owned symlink. The agent can replace the target of the absolute path too. The same is true of
`~/.cargo/bin` (the four Rust tools), `~/.local/libexec` (the osquery and posture pipelines) and the
`chezmoi` binary itself.

### 6. In the mode this machine uses, chezmoi hands the master password to that client on stdin

From chezmoi upstream, `internal/cmd/keepassxctemplatefuncs.go`, `keepassxcOutputCachePassword`:

```go
cmd := exec.Command(c.Keepassxc.Command, cmdArgs...)
if c.Keepassxc.password == "" && c.Keepassxc.Prompt {
    password, err := c.readPassword(...)
    ...
    c.Keepassxc.password = password
}
if c.Keepassxc.password != "" {
    cmd.Stdin = bytes.NewBufferString(c.Keepassxc.password + "\n")
}
```

`cache-password` is the documented default and the measured effective mode here. chezmoi prompts once,
holds the password in its own memory for the run, and writes it to the standard input of every
`keepassxc-cli` invocation. The `builtin` mode instead decodes the database in process with
`gokeepasslib` and spawns nothing; `open` drives a persistent `keepassxc-cli open` console.

### The chain these five facts form

1. The agent edits `.install-password-manager.sh`, or any file the hook reaches, or adds a
   `run_before_` script, or adds a `keepassxc` call to any template.
1. That file writes a `keepassxc-cli` shim into `~/.local/bin`.
1. The operator runs `just d` to review the agent's work. The hook fires before the diff renders.
1. chezmoi renders the templates, calls `keepassxc-cli`, and the shim receives the master password on
   standard input.
1. The shim now holds the master password and the database path, and the database file itself is
   readable at `~/Library/Mobile Documents/iCloud~com~strongbox/Documents/keepass.kdbx`.

The result is not access to a permitted subset. It is the whole database, every entry, including the
personal passwords the operator has explicitly decided stay out of agent reach, plus the age identity
that decrypts every age-encrypted source file.

So the honest answer to "what is the current permitted set" is: everything, on the operator's next
review command. Any boundary document that starts anywhere else is describing a system that does not
exist.

One softening, stated plainly because it matters for how urgently this is treated: this chain needs
the agent to write a hostile file and the operator to not notice it in the diff. It is a
review-evasion attack, not a background leak. It is also exactly the attack the threat model names,
because the review being evaded is the diff the hook runs ahead of.

## Constraints this design has to hold

From `CLAUDE.md`, the ledger, and recorded operator rulings:

- The operator runs applies. Agents do not. This holds until a reviewed replacement is approved.
- `--exclude=templates` is retired and must not be revived, and a by-name apply is not a supported
  shortcut, because the osquery known-good manifests derive from source and a partial apply pages a
  false critical alert.
- Personal passwords stay in the KeePass database used by KeePassXC and Strongbox, including iPhone
  AutoFill and offline access. No automatic synchronization between stores.
- No removal mechanisms are built in this repository (no `.chezmoiremove`, no `remove_` entries).
- Third-party tools are configured, never patched or forked. KeePassXC, chezmoi, Infisical and
  moshi-hook are all third-party here.
- The design bar: no steady state may depend on the operator remembering a recurring manual step, and
  result quality decides between designs, not implementation effort.
- Declaration-versus-declaration consistency tests are out of test scope by the 2026-08-05 ruling.
  A verification tool this repository writes is in scope, and its own behavior is testable. This
  distinction shapes the verification section below and is called out there.
- Demonstrate with dummy credentials first, including locked and denied access, secret-free output,
  and a complete render, deployment and manifest cycle.

## What is already built, and what the ledger already settled

Built and in use:

- chezmoi renders fifteen targets from twenty-four distinct KeePassXC entries, twenty-three of them
  secret-bearing. The full entry list is reproduced in the appendix, because the permitted-set
  question cannot be answered without it.
- Whole-file secrets use age encryption, with the identity at `~/.config/chezmoi/key.txt` and the
  identity itself held in the vault, restored by `run_before_05-restore-age-key.sh.tmpl`. That script
  is careful in the ways that matter for a secret-free rendered body: it keeps the value out of the
  script text and out of any command line, streams it into a 0600 file, validates rather than
  clobbers, and refuses a symlink. It calls `keepassxc-cli` directly rather than through the template
  function, which is a second call site the boundary has to cover.
- `run_once_after_60-moshi-hook-setup.sh.tmpl` renders a device token into the script body with
  `{{ $token := (keepassxc "moshi-hook :: Device Token").Password }}` and passes it as
  `moshi-hook pair --token <value>`. Measured: chezmoi renders `run_` scripts to a 0700 file inside a
  0700 directory under the per-user temporary directory, executed with the destination directory as
  the working directory. So the token sits in a same-user-readable file for the length of the apply
  and appears in `ps` output for the length of the pair call. Both are same-user-readable, which
  section 1 already showed is public to the account.
- Claude Code ships a fourteen-entry deny list and `defaultMode = bypassPermissions`, with the
  limitation recorded in the template's own comments.
- `gitleaks git --staged` blocks staged plaintext secrets at pre-commit. That is a commit gate, not an
  access boundary, and it does nothing about a value read at render time.

Settled by the operator on 2026-09-12, and not reopened here:

- Self-host Infisical for selected agent credentials. This is a selected service, not a product
  comparison.
- Personal passwords stay in KeePass and Strongbox.
- Server deployment, identities, policies, networking and recovery belong to `webdavis/homelab` A6.
  This repository owns laptop clients, non-secret harness configuration, and uu upgrades.
- kpxc-cli is an unadopted candidate, and Touch ID support and masked output are not proof of
  isolation.

## What the Infisical Agent Proxy actually is

Verified against Infisical's own documentation, because the whole Tier 1 claim below rests on it.

Agent Proxy is a forward proxy for hypertext transfer protocol traffic, secure and plain. For secure
destinations it terminates the connection with its own certificate authority, applies the real
credential to requests bound for a configured "proxied service", and re-originates to the true host.
The agent's clients are pointed at it by proxy environment variables plus a trusted certificate
authority certificate. Three modes exist: `run` (proxy and agent in one local process, with operating
system sandboxing), `start` (a long-lived network service), and `connect` (an agent routed through a
proxy already running elsewhere, authenticating with a machine identity).

Two documented properties carry real weight:

- Under `connect`, "Real secret values are never placed in the agent's environment"; the agent gets
  proxy routing, certificate authority trust, and placeholders.
- `connect` refuses to start if the agent identity can itself read a secret that a proxied service
  brokers to it, because that would make the proxy pointless. The refusal is overridable with
  `--allow-readable-brokered-secrets`.

That second property is the important one. It means the use-without-read separation is checked by the
server rather than asserted by configuration prose, which is the only kind of protection this
document is willing to call a boundary.

Destination restriction is `--unmatched-host`, which defaults to `allow` (unmatched hosts are
forwarded untouched) and can be set to `block` (403 for anything with no proxied service).

Two limitations, both material:

- The trust model is trust-the-proxy. A compromised proxy exposes the credentials it brokers. Under
  `connect` the proxy and its certificate authority private key live on the home server, so a
  compromised laptop does not yield them. Under `run` they are on the laptop, protected by the
  sandbox, which puts a certificate authority private key that is trusted for arbitrary hosts inside
  the blast radius of the agent. That is a strong argument for `connect` over `run` in this threat
  model, independent of latency or convenience.
- Infisical's published plans gate the controls this boundary needs. Audit logs are absent on Free
  (30 days on Pro, 90 on Advanced, streaming on Enterprise); role-based access control is not on
  Free, with custom roles, multi-role and temporary access on Advanced; secret rotation is not on
  Free. Free caps identities at five, humans and machines combined. The core is self-hostable at no
  cost under an MIT license, with supported licensed self-hosting sold separately. The homelab plan
  already flags this and requires confirming self-hosted entitlements before committing.

So the boundary's two central requirements, use-without-read enforced by policy and an audit record
without secret values, both appear to sit above the free tier. That is a cost decision, not an
engineering one, and it is an open question below rather than an assumption.

## Three approaches

### Approach A: harden the chezmoi and KeePassXC path in place

Pin `keepassxc.command` to an absolute path, or switch `keepassxc.mode` to `builtin` so no child
process receives the password at all. Move the `read-source-state` pre hook out of the source tree,
or delete it (its only job is installing KeePassXC when missing, best effort). Make the tool
directories non-agent-writable. Split the database so an agent-adjacent one holds only the entries
agents need.

What it buys: the chain in section 6 breaks at several points at once. `builtin` removes the shim
target entirely. A hook outside the source tree removes pre-review execution.

What it does not buy: `/opt/homebrew/bin` is agent-writable, and so is the `chezmoi` binary in it.
Making that directory root-owned fights Homebrew's own model and breaks the unattended weekly `brew`
lane that uu runs, which needs to write there without a password. And even with a perfect toolchain,
the rendered targets stay agent-readable, because rendering a secret into a file the agent can read is
what the mechanism is for. Approach A raises the cost of the attack and changes nothing about the
steady state.

Honest summary: necessary, insufficient, and cheap. Not a boundary.

### Approach B: two tiers, with the network as the only isolation mechanism

Declare two tiers and stop pretending they are the same thing.

Tier 1, brokered: a credential an agent may use but never read. It lives in Infisical, it is applied
to outbound requests by the Agent Proxy on the home server, and the laptop never holds its value. The
isolation mechanism is a different machine across a network boundary, enforced by a server-side
policy and by Infisical's refuse-to-start check.

Tier 2, rendered: a credential chezmoi writes into a file on the laptop. The isolation mechanism is
none. Tier 2 is agent-readable by construction, and the document says so in those words. What governs
Tier 2 is not isolation but three other things: the master password never existing in any file, so
the vault cannot be opened without the operator; operator review of source changes, hardened by
Approach A so that review actually happens before any agent code runs; and rotation, on the standing
assumption that a Tier 2 value may already be known to anything that has run on the machine.

The work then becomes moving entries from Tier 2 to Tier 1 until Tier 2 holds only what genuinely
cannot be brokered.

What it buys: a claim that survives contact with the measurements. Every Tier 1 protection names a
mechanism on another host. Every Tier 2 value is honestly labeled.

What it costs: Tier 1 only covers credentials used as authentication on outbound network requests to
a fixed host. It does nothing for a credential a local program needs as a value: the age identity, a
mail client's raw password in a configuration file, a Hue bridge application key on the local
network, the espanso personal data. Those stay Tier 2 forever. It also introduces a dependency on the
home server for protected operations, which the homelab plan already accepts and requires to fail
loudly rather than fall back to a local copy.

### Approach C: stop the agent from being the same user

Run agents under a separate macOS user id, or inside a container, so that file modes and process
visibility become real boundaries and Tier 2 becomes genuinely protected.

What it buys: the only version of this where a Tier 2 isolation claim is true. Mode 0600 on
`~/.hermes/.env` would actually mean something.

What it costs: the whole toolchain is built around one home directory. herdr workspaces, worktrunk
worktrees at `~/.herdr/worktrees`, the Rust tools in `~/.cargo/bin`, the osquery and posture pipelines
under `~/.local/libexec`, atuin history, the Claude Code and Codex configuration, the skills store at
`~/.agents/skills`, every launchd agent in `~/Library/LaunchAgents`. A second user id means either
duplicating that or sharing it across a boundary that then has to be designed. It also makes the
agent unable to edit the chezmoi source tree in the operator's checkout, which is most of what agents
do here. Apple's own sandboxing of a second account is strong; the integration cost is the objection,
and it is a large one.

Honest summary: the correct answer to the question as asked, at a cost that would stall every other
line of work in the ledger. Recorded as the upgrade path, recommended as deferred, and explicitly not
dismissed.

## Recommended design

Approach B as the boundary model, with the Approach A items that are provable adopted immediately as
the Tier 2 hygiene floor, and Approach C recorded as the only route to a true Tier 2 isolation claim
and deliberately deferred.

### The boundary, stated as one paragraph

An agent running as the operator's user id on dresden may use, and never read, the credentials
registered as Tier 1 in Infisical, and only against the destinations declared as proxied services,
for as long as its machine identity's access lifetime lasts, with every use recorded server-side. It
may read, and is assumed to have read, every Tier 2 credential rendered into its own home directory.
It may not open the KeePass database, because the master password exists only in the operator's head
and in nothing chezmoi or any tool writes to disk. It may not obtain the master password through a
substituted vault client, because the client is no longer spawned. Everything outside the Tier 1
permitted set is denied by the proxy, and everything inside Tier 2 is not denied at all and is
governed by review and rotation instead.

### Behaviors, each one a test

Written as behaviors so implementation can be test-first. The unit under test is in parentheses.

Tier 1, use without read:

1. An approved request to a declared proxied service succeeds, and the response is correct.
   (integration probe against the homelab proxy with a dummy credential)
1. The same request made with the proxy environment removed fails to authenticate, proving the
   credential was applied by the proxy and is not present locally. (integration probe)
1. A direct read of the brokered secret through the agent's own machine identity is refused.
   (integration probe; this is also the condition Infisical's refuse-to-start check enforces)
1. `connect` refuses to start when the agent identity can read a brokered secret, and the refusal is
   visible rather than silent. (integration probe, deliberately misconfigured identity)
1. A request to a host with no proxied service is refused with 403 under `--unmatched-host block`.
   (integration probe)
1. A revoked machine identity cannot obtain new brokered access, and the failure names revocation
   rather than a generic network error. (integration probe)
1. The audit record for a brokered use names the identity, the destination and the time, and contains
   no secret value. (inspection of the server-side record, asserted as a field allowlist)
1. With the home server unreachable, a protected operation fails loudly and ordinary local
   development still works. (integration probe with the proxy address unroutable)

Tier 2, honest labeling and hygiene:

9. Rendering the managed target set spawns no external vault client process. (verification tool,
   observing process creation during a dummy-credential render)
1. A `keepassxc-cli` shim placed earlier on PATH is never executed during a render. (verification
   tool, with a shim that records its own invocation)
1. A source-tree file cannot be reached by a chezmoi hook, because no hook command resolves inside
   the source directory. (verification tool, reading the effective configuration)
1. Every KeePassXC entry named anywhere in managed source appears in the register, and every register
   entry names its tier, its consumers and its rotation owner. (verification tool)
1. A template that calls `keepassxc` on an unregistered entry fails the check with a message naming
   the file and the entry. (verification tool)
1. Rendered output for the full managed target set contains no value from a Tier 1 entry. (verification
   tool)

The 2026-08-05 test scope ruling matters here and is being respected rather than worked around.
Behaviors 12 and 13 look like declaration-consistency checks, and as bare tests they would be exactly
what that ruling purged. The tool is the product: a boundary checker this repository writes, whose
behavior on fixtures is what the suite tests, and which then runs as a gate. The suite asserts that
the checker fails on a fixture with an unregistered entry and passes on a clean one. That is testing
our own tool's behavior, which is in scope. If the operator would rather not have the checker at all,
behaviors 12 and 13 drop and the register becomes documentation reviewed by eye.

### Configuration surface

New:

- `.chezmoidata/credential_register.yaml`, the register. One record per credential: the KeePassXC
  entry name or Infisical path, the tier, the consuming targets, the rotation owner, and whether it
  is assumed already agent-exposed. Data only, read by the checker and by the documentation; it
  contains no secret values and no master password, and it is committed.
- `dot_config/infisical/` for the non-secret client configuration: the self-hosted domain, the private
  proxy address, and the trust settings. The machine identity's own client identifier and secret are
  not rendered here; see the failure modes below for why that placement is the hardest remaining
  question.
- The Infisical client added to `.chezmoidata/system_packages_autoinstall.yaml` and to uu's upgrade
  lane, which is the ledger's separate bullet and is only referenced here.

Changed:

- `.chezmoi.toml.tmpl`: `keepassxc.mode = "builtin"`, which removes the child process the master
  password is currently written to. This is the single highest-value change in the document and it is
  one line.
- `.chezmoi.toml.tmpl`: remove the `hooks.read-source-state.pre` entry, or repoint it outside the
  source tree. Removing it is preferred. Its job is installing KeePassXC when absent on a fresh
  machine, best effort, and `run_once_before_00-install-homebrew.sh.tmpl` plus the packages script
  already own that class of work at a point where the file is not agent-editable during review.

Unchanged, deliberately:

- The operator-runs-applies rule. Nothing here proposes an agent apply. A brokered Tier 1 credential
  does not need one, and a Tier 2 credential is agent-readable anyway, so an agent apply would buy
  nothing and would hand the agent the render step.
- The single KeePass database. Splitting it is discussed under assumptions and is not recommended.

### Failure modes

- Home server or tailnet unreachable: Tier 1 operations fail with a message naming the broker.
  Ordinary local development continues. No local credential copy is created as a fallback, per the
  homelab plan.
- Agent machine identity revoked or expired: new brokered access is refused. An in-flight session's
  behavior is an upstream property that must be measured, not assumed, and it is listed as an
  operator step.
- Proxy certificate authority certificate rotated or expired: every brokered request fails
  certificate validation. This looks like a broken agent rather than a credential problem, so the
  failure needs to name certificate trust explicitly. Route it through pns as a delivery failure so
  it is not diagnosed as a network outage for an hour.
- KeePass database locked or the operator cancels the prompt: `builtin` mode fails the render, and a
  failed render aborts the apply. That is the existing behavior and is correct. It must not be
  softened into a skip, because a skipped secret target renders an empty credential into a
  configuration file.
- Fresh machine: the age identity is Tier 2 and must be restorable before anything age-encrypted can
  be read, which is a bootstrap ordering the existing `run_before_05` script already handles. Under
  `builtin` mode that script's own direct `keepassxc-cli` call is the one remaining external vault
  client invocation in the repository, so it keeps the shim exposure unless it is converted or
  accepted. The sibling ledger bullet owns that conversion; this document only records that
  `keepassxc.mode = "builtin"` does not cover it.
- Register drift: a template adds a `keepassxc` call for an unregistered entry. The checker fails the
  gate, naming file and entry.

### Security notes

- The KeePass database file is agent-readable and can be copied off the machine. Its security against
  the agent reduces to the offline resistance of one passphrase and the database's key derivation
  settings. That is a different claim from access control, and it should be stated as such wherever
  the vault is described as protecting something. Confirming the format and key derivation
  parameters needs `keepassxc-cli db-info`, which needs the master password, so it is an operator
  step rather than something this document measured.
- The espanso identity match file and the two Proton Mail Bridge logins are personal data and
  personal account credentials, not service tokens. They are Tier 2 and unbrokerable. If the operator
  is uncomfortable with agents being assumed to have read a home address and phone number, the answer
  is Approach C or removing them from managed rendering, not a permission bit.
- `--unmatched-host block` is required, not optional. Under the default `allow`, an agent keeps
  arbitrary outbound reachability through the proxy, which is an exfiltration channel for the Tier 2
  values it can already read.
- `connect` over `run`, so the certificate authority private key stays on the home server. `run`'s
  sandbox is protecting a key that is trusted for arbitrary hosts, on the machine with the hostile
  process on it.
- The agent's own Infisical machine identity credential is itself a secret on the laptop, and if it is
  passed in the environment then section 1 applies and every same-user process can read it. It must
  be scoped so that reading it grants proxy use and nothing else, which is precisely what Infisical's
  refuse-to-start check enforces. There is no arrangement on this machine where the agent can start a
  proxy session and also cannot read the credential that starts it; the mitigation is that the
  credential is worth only brokered use.

## Denial demonstration

The ledger's completion criterion is that the design "shows denial outside the permitted set". Denial
is demonstrated by a named, repeatable probe set run against the homelab instance with dummy
credentials before any real credential moves. Each probe states the action, the expected refusal, and
the evidence recorded.

| Probe | Action | Expected | Evidence |
| --- | --- | --- | --- |
| D1 | Read a brokered secret with the agent identity | refused | client error naming permission |
| D2 | Start `connect` with a readable brokered secret | refuses to start | startup refusal message |
| D3 | Request an undeclared host through the proxy | 403 | proxy response status |
| D4 | Approved request with proxy environment removed | authentication failure | upstream 401 |
| D5 | Approved request after identity revocation | refused | client error naming revocation |
| D6 | Approved request with the proxy unroutable | loud failure, no local fallback | client error, no new local file |
| D7 | Render targets with a `keepassxc-cli` shim earlier on PATH | shim never invoked | shim's own log empty |
| D8 | Render a template naming an unregistered entry | gate fails | checker output naming file and entry |
| D9 | Inspect audit records for D1 and D4 | no secret values present | field allowlist assertion |

D7 and D8 run locally and can be built now. D1 through D6 and D9 need the homelab A6 deployment and
are gated on it.

## Out of scope

- Deploying, licensing, networking or backing up Infisical. That is homelab A6.
- The NetBird cutover. Tailscale remains the path until an approved migration.
- kpxc-cli adoption. It is its own ledger bullet and an unadopted candidate.
- Passage. Background research, not a selected migration.
- The full treatment of the age-key restore call site and the moshi token placement. That is the
  sibling ledger bullet; this document states what the boundary requires of them and stops.
- Any change to the operator-runs-applies rule.
- Rotating any real credential. This document recommends assuming Tier 2 values are exposed; acting
  on that is an operator decision with real cost across twenty-three secret-bearing entries.
- Patching or forking chezmoi, KeePassXC, Infisical or moshi-hook.
- Hermes investigation sandboxing, which has its own recorded security boundary work.

## Assumptions made in your place

Each is a choice made so the document could be written tonight. Each names its alternative.

1. **Threat model is a hostile or prompt-injected agent running as your user id on dresden.** Not a
   remote attacker, not another local user, not a stolen laptop. *Alternative:* include device theft
   and multi-user, which would make the file modes in section 3 meaningful again and would add
   full-disk-encryption and screen-lock questions this document does not touch.
1. **Tier 2 is declared agent-readable rather than defended.** *Alternative:* pursue Approach C now
   and make the Tier 2 claim true, accepting the integration cost across herdr, worktrunk, the Rust
   tools, launchd and the skills store.
1. **`connect` against the homelab proxy, not local `run`.** *Alternative:* `run` locally, which is
   simpler and puts a broadly trusted certificate authority private key on the machine with the
   hostile process.
1. **`--unmatched-host block`.** *Alternative:* `allow`, which keeps arbitrary outbound reachability
   and is the upstream default.
1. **`keepassxc.mode = "builtin"`.** *Alternative:* keep `cache-password` and pin the command to an
   absolute path, which does not help, since `/opt/homebrew/bin` is agent-writable; or `open`, which
   still spawns the client.
1. **Remove the `read-source-state` pre hook rather than repoint it.** *Alternative:* move it to a
   path outside the source tree and keep the fresh-machine KeePassXC install, which keeps a hook but
   removes the agent-editable part.
1. **Approval is per-identity-lifetime, not per-request.** *Alternative:* per-request operator
   approval for high-value operations, which conflicts with unattended overnight work and with the
   no-manual-intervention design bar, and which the proxy model does not obviously support.
1. **Keep one KeePass database.** *Alternative:* split an agent-adjacent database out, which reduces
   the blast radius of a master-password capture but creates a second database to back up, unlock and
   keep in sync with Strongbox, and which the no-automatic-synchronization ruling makes awkward.
1. **Operator diff review is the Tier 2 gate, hardened by removing the pre-review execution path.**
   *Alternative:* a mechanical gate that refuses to render when secret-touching source paths changed
   without a recorded review, which is stronger and adds a state file and a bypass to design.
1. **Tier 2 values are assumed already exposed, and the register records that assumption.**
   *Alternative:* treat them as clean until there is evidence of a specific compromise, which is
   cheaper and is the weaker posture.
1. **Homebrew's prefix is left agent-writable.** *Alternative:* root-own `/opt/homebrew/bin`, which
   would break the unattended weekly `brew` lane uu runs and fights Homebrew's own model.

## Open questions

These need your answer before anything is built.

1. **Do you approve the isolation claim as stated?** Tier 1 is isolated by a different machine across
   a network boundary. Tier 2 is not isolated at all and is governed by review and rotation. The
   ledger asks you to approve the boundary and the isolation claim it rests on, and this is that
   claim.
1. **Paid Infisical edition, or no audit trail and no granular roles?** Audit logs and role-based
   access control both appear to sit above the Free tier, and both are load-bearing for this
   boundary. Self-hosted entitlements need confirming against a real instance, which is a homelab
   step, but the cost decision is yours.
1. **Approach C: pursue, defer, or reject?** It is the only route to a true Tier 2 isolation claim.
   My recommendation is defer and shrink Tier 2 instead, and I want that recorded as your choice
   rather than mine.
1. **Which of the twenty-four entries move to Tier 1?** The appendix lists them. My reading is that
   the Anthropic, OpenRouter, Tavily, ElevenLabs, Composio, Discord bot token and Google OAuth entries
   are brokerable, and the rest are not. Confirm or correct per entry.
1. **Root-own the Homebrew binary directory?** It closes the last substituted-client path and breaks
   the unattended weekly `brew` lane. I lean toward leaving it writable and accepting the assumption,
   because `builtin` mode removes the thing the shim was for.
1. **Remove the `read-source-state` pre hook, or repoint it?** Removing loses a best-effort
   fresh-machine KeePassXC install.
1. **Any appetite for per-request approval on specific high-value operations?** If yes, name them; the
   proxy model brokers by destination, not by request, so this may need something else entirely.
1. **Rotate all twenty-three secret-bearing entries now on the assumption they are already exposed,
   or only on evidence?** Rotation cost is real and spread across many providers.
1. **Build the boundary checker, or keep the register as reviewed documentation?** The checker is a
   tool this repository owns and its behavior is testable, so it is inside the test scope ruling, but
   it is still a new mechanism to maintain.

## Appendix: the twenty-four registered entries

Every distinct KeePassXC entry named in managed source, with the target that consumes it. Names only,
no values. Fixture entries from `test/fixtures/render-coverage/` are excluded.

| Entry | Consumed by |
| --- | --- |
| `Anthropic :: Auth Token` | `~/.hermes/.env` |
| `chezmoi :: Private Key :: age` | `run_before_05-restore-age-key.sh.tmpl` (direct call) |
| `Composio :: API Key :: CLI` | `~/.composio/user_data.json` |
| `Composio :: API Key :: MCP` | `~/.claude.json`, `~/.codex/config.toml` |
| `Discord (Uriel) :: Bot Token (Bob)` | `~/.hermes/.env` |
| `Discord (Uriel) :: Channel ID (#general)` | `~/.hermes/.env` |
| `Discord (Uriel) :: Channel ID (#osquery)` | `~/.hermes/.env` |
| `Discord (Uriel) :: User ID (Stephen)` | `~/.hermes/.env` |
| `Dotfiles (bashrc) :: HISTIGNORE Regex` | `~/.config/atuin/config.toml` |
| `ElevenLabs :: API Key` | `~/.hermes/.env` |
| `GitHub (Webdavis) :: GPG :: Signing key` | `~/.gitconfig` (public subkey id, not a secret) |
| `Google Cloud (webdavis.io) :: OAuth :: google-workspace-mcp` | `~/.claude.json`, `~/.codex/config.toml`, Claude desktop config |
| `Google Cloud (webdavis.io) :: OAuth :: openclaw-gog-cli` | `~/.config/gogcli/credentials.json` |
| `Hermes :: Webhook Secret :: #pns` | `~/.config/pns/config.toml`, `~/.config/uu/config.toml` |
| `Karl M. Davis (justdavis) :: AWS (Access Key CLI) :: steve (Admin IAM User)` | `~/.aws/credentials` |
| `moshi-hook :: Device Token` | `~/.config/pns/config.toml`, moshi pairing script |
| `OpenHue :: API Key (hue-bridge-pro)` | `~/.config/openhue/config.yaml`, `~/.config/lights/config.toml`, `~/.config/pns/config.toml` |
| `OpenRouter :: API Key (Hermes Agent)` | `~/.hermes/.env` |
| `Personal :: Address` | espanso identity match |
| `Personal :: Phone` | espanso identity match |
| `Proton Mail Bridge - IMAP Login` | `~/.config/himalaya/config.toml` |
| `Proton Mail Bridge - SMTP Login` | `~/.config/himalaya/config.toml` |
| `Tavily :: API Key` | `~/.hermes/.env` |
| `UniFi :: API Key (dresden-udr)` | `~/.config/pns/config.toml` |

Twenty-four rows, twenty-three secret-bearing: the GitHub signing-key row is a public identifier and
is listed because the checker in behavior 12 will see it and it should be registered as non-secret
rather than reported as a finding.

One inert reference exists and must not become a twenty-fifth row.
`dot_config/herdr/plugins/config/tab-smart-rename/private_provider.env` contains
`# OPENAI_API_KEY={{ (keepassxc "OpenAI API").Password }}` on a commented line, and the file has no
`.tmpl` suffix, so chezmoi never renders it and the template text is deployed literally. Verified by
reading the source file and its deployed copy, which are the same 2539 bytes. The checker has to
distinguish a real call site from dead text in a non-template file, or it will report an entry that
is never read and send the operator looking for a credential that is not in use.
