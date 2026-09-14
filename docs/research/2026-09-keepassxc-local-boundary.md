# KeePassXC access for the local credential boundary, 2026-09-14

The ledger task at `docs/remaining-work.md` line 2052, in the "Safe agent credentials and Infisical
client integration" section, asks for this. It evaluates the supported ways a tool on this machine can
reach the KeePass database for the operations that stay local, chezmoi's file rendering first among them,
and records why an outbound credential proxy does not reach that boundary.

Nothing was installed, no configuration was changed, no template was edited, and the operator's real
vault was never unlocked. Every behavioral claim below was measured against a throwaway database built in
that session's scratchpad with dummy credentials, under a `kpxc-probe/` directory. The database, its key
file and the throwaway chezmoi configurations were left in place and hold nothing but the strings
`dummy-master-pw`, `dummy-entry-secret` and `simple-secret`. Deleting that directory is one of the
operator steps handed over with this document, because the destructive-action rule puts removing a
directory behind the operator's own confirmation rather than an agent's judgment, scratch directories an
agent made itself included.

## Verdict

**Reject** the proposition that a supported KeePassXC integration can move the local credential boundary
on this machine, and **defer** the one configuration that could, pending a hardware purchase and an
operator decision.

The reason is not a gap in KeePassXC's integrations. It is that every one of them, and every alternative,
terminates in the same place: a process running under the operator's own user id. chezmoi, the agent's
shell, `keepassxc-cli`, and the browser-protocol socket all run as `stephen`, and a secret that one of
them can read is a secret all of them can read. No configuration of a credential store changes that,
because the store is not what is missing. A process boundary is.

Three concrete results carry the verdict:

- **Unattended access is already available and already rejected by its own cost.** A key-file-only
  database plus two documented chezmoi settings gives a fully silent, zero-prompt render (measured). It
  also hands the whole vault to anything that can read one 128-byte file, which is the definition of
  handing an agent the password rather than letting a tool use it.
- **The unlock gate only ever covered the render step, and it never covered the whole surface.** After
  any successful apply all fifteen vault-backed targets sit in `$HOME` as cleartext at mode 0600 owned by
  `stephen`, which an agent's `cat` reads. The five age-encrypted source files need no vault at all once
  `~/.config/chezmoi/key.txt` exists, which it does: `chezmoi cat ~/.hermes/config.yaml` succeeded here
  with no unlock. So the operator-only apply rule buys the freshness of one render, not the secrecy of
  the values.
- **The source tree is the real boundary, and it is agent-writable today.** An agent that edits a
  template adds a vault read that the operator's next unlocked apply performs. An agent that edits
  `.install-password-manager.sh` gets code execution inside every chezmoi command the operator runs,
  because `.chezmoi.toml.tmpl` wires that file as `hooks.read-source-state.pre`. Nothing in the test
  suite or the lint gates catches either edit.

What a supported integration **can** cover without the operator, today, with no change at all: the
whole-vault-free subset of chezmoi's read side (`managed`, `unmanaged`, `cat-config`, and path-scoped
`status` and `diff` on any non-vault target), plus the five age-encrypted targets. What still needs the
operator: whole-tree `status` and `diff`, every apply that renders one of the fifteen vault-backed
targets, and the two scripts that read the vault outside template lookup. The full table is below.

The deferred candidate is `keepassxc.mode = "open"` with a YubiKey challenge-response factor on the
database. It is the only supported configuration in which the unlock cannot be supplied by software
running as the operator, and it works with this repository's existing template calls unchanged (measured,
minus the hardware). It costs a hardware purchase, an experimental chezmoi mode, and a recovery story,
and it gates per run rather than per secret.

## The question

The task splits into four, and this document answers all four:

1. Which local operations can a supported KeePassXC integration cover, and which still need the operator?
   (The task's `done_means`.)
1. Why does an outbound credential proxy not solve that boundary by itself?
1. What is the difference between handing an agent a password and letting a tool use it for an approved
   operation, in mechanisms that exist on this machine?
1. Which supported integration should be preferred over a custom broker?

"Local operations" means the work that happens on the laptop with no network destination: chezmoi
rendering a config file, chezmoi decrypting an age file, a script restoring the age identity, a script
pairing a device, and the lint gates that render templates to check them.

## What was checked, and how

### Versions on this machine, dresden

| Component             | Version                               | Source                                        |
| --------------------- | ------------------------------------- | --------------------------------------------- |
| chezmoi               | `v2.72.1`, built 2026-08-30, Homebrew | `chezmoi --version`                           |
| keepassxc-cli         | `2.7.12`                              | `keepassxc-cli --version`                     |
| KeePassXC application | `2.7.12`                              | `Info.plist` `CFBundleShortVersionString`     |
| `keepassxc-proxy`     | present, running                      | `/Applications/KeePassXC.app/Contents/MacOS/` |
| Operator vault format | KDBX 4.1                              | header bytes `03d9a29a 67fb4bb5 0100 0400`    |
| Probe database format | KDBX 3.1                              | header bytes `03d9a29a 67fb4bb5 0100 0300`    |
| `ykman`               | absent                                | `command -v ykman`                            |

KDBX is the KeePass database file format. Its version came from reading the first twelve bytes of the
file, which are the two format signatures and the minor and major version words. No key material was
touched and nothing was decrypted. Note the asymmetry: every mode probe below ran against a KDBX 3.1
database, because `keepassxc-cli db-create` has no flag to select the key derivation function and the
resulting file is 3.1. The operator's real vault is 4.1 and was not probed.

### Upstream sources read

- chezmoi configuration variables, the `[keepassxc]` table:
  <https://www.chezmoi.io/reference/configuration-file/variables/>
- chezmoi KeePassXC template functions and the `mode` values:
  <https://www.chezmoi.io/reference/templates/keepassxc-functions/>
- chezmoi KeePassXC user guide, including the non-password-protected and YubiKey paths:
  <https://www.chezmoi.io/user-guide/password-managers/keepassxc/>
- chezmoi common command-line flags, for the `--exclude` entry types:
  <https://www.chezmoi.io/reference/command-line-flags/common/>
- The KeePassXC browser protocol: <https://github.com/keepassxreboot/keepassxc-browser> protocol document
- `kpxc-cli`: <https://github.com/mietzen/keepassxc-cli> README
- Infisical standalone Agent Proxy:
  <https://infisical.com/docs/documentation/platform/agent-proxy/standalone-agent-proxy>
- `passage`: <https://github.com/FiloSottile/passage>
- The Secret Service platform question was checked by search rather than a single page, because the
  KeePassXC user guide's own section does not state a platform. The load-bearing facts are that the
  feature implements the freedesktop.org Secret Service specification, which is a D-Bus interface, and
  that KeePassXC's secure shell (SSH) agent section names Linux, macOS and Windows separately while the
  Secret Service section names no platform at all. Sources: KeePassXC issues 11342, 3945, 6274 and the
  user guide contents.

### This repository's vault-backed surface

Fifteen chezmoi targets pull a value through the `keepassxc` or `keepassxcAttribute` template function,
which matches the count `CLAUDE.md` states. Enumerated by source path:

`dot_aws/private_credentials.tmpl`, `dot_composio/private_user_data.json.tmpl`,
`dot_config/atuin/private_config.toml.tmpl`, `dot_config/himalaya/private_config.toml.tmpl`,
`dot_config/lights/private_config.toml.tmpl`, `dot_config/openhue/private_config.yaml.tmpl`,
`dot_config/pns/private_config.toml.tmpl`, `dot_config/private_gogcli/private_credentials.json.tmpl`,
`dot_config/uu/private_config.toml.tmpl`, `dot_gitconfig.tmpl`,
`Library/Application Support/Claude/modify_private_claude_desktop_config.json`,
`Library/Application Support/espanso/match/private_identity.yml.tmpl`, `modify_private_dot_claude.json`,
`private_dot_codex/modify_private_config.toml`, `private_dot_hermes/private_dot_env.tmpl`.

Two scripts reach the vault outside template lookup:

- `.chezmoiscripts/run_before_05-restore-age-key.sh.tmpl` calls `keepassxc-cli show -s -a Password`
  itself, at execution time, streaming one entry into a 0600 file. Its rendered body carries no secret,
  which is the design its own docblock records.
- `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl` binds the token at render time
  (`{{- $token := (keepassxc "moshi-hook :: Device Token").Password -}}`) and puts it in a command
  argument: `moshi-hook pair --token {{ $token | quote }}`. So the value lands in the rendered script
  body and in the process argument list.

One further mention is inert: `dot_config/herdr/plugins/config/tab-smart-rename/private_provider.env`
contains a commented `keepassxc` call, and the file has no `.tmpl` suffix, so chezmoi never executes the
action. It is literal text in a comment.

### Probes run against the dummy database

Every command below ran with a throwaway `HOME` and a throwaway `sourceDir`, so the repository's own
configuration and its read-source-state hook were not in play.

| Probe                                                             | Result                                            |
| ----------------------------------------------------------------- | ------------------------------------------------- |
| `keepassxc-cli db-create -p`, password on standard input          | succeeded, no terminal needed                     |
| `keepassxc-cli show -q -s -a Password`, password on stdin         | printed the value, exit 0                         |
| Same, wrong password, with `-q`                                   | exit 1, no message at all                         |
| Same, empty standard input                                        | exit 1, "Invalid credentials were provided"       |
| `mode` default (`cache-password`), two lookups, one password      | both values, exit 0                               |
| `mode = "open"`, two lookups, one password line                   | both values, exit 0                               |
| `mode = "builtin"`, correct password                              | empty map `{}`, every entry, every name form      |
| `mode = "builtin"`, wrong password                                | "Wrong password? Database integrity check failed" |
| `keepassxcAttribute` under `cache-password` and under `open`      | both returned the attribute                       |
| Key-file-only database, `prompt = false`, `--no-password -k`      | value, zero prompts                               |
| `chezmoi managed` against the real configuration, no vault        | exit 0                                            |
| `chezmoi status` against the real configuration, no vault         | exit 1 at the first vault read                    |
| `chezmoi status --exclude=templates`, no vault                    | exit 1, at a `modify_` template                   |
| `chezmoi status ~/.bashrc` and `chezmoi diff ~/.bashrc`, no vault | exit 0                                            |
| `chezmoi cat ~/.hermes/config.yaml`, no vault, piped to `shasum`  | exit 0, real content                              |

The `keepassxcAttribute` probe used the built-in `Title` attribute rather than a custom one, because
`keepassxc-cli` 2.7.12's `edit` subcommand has no flag that sets a custom attribute, so no dummy fixture
with one could be built from the command line. The code path is the same `show -a <name>` in both cases,
but a genuine custom attribute under `mode = "open"` remains unverified. That is listed under what would
change the verdict.

## Findings

### 1. chezmoi has exactly one KeePassXC backend, and all three of its modes want the whole database

The `[keepassxc]` table holds five keys and no more: `command`, `args`, `database`, `mode` and `prompt`.
`mode` takes three values. `cache-password`, the default, prompts once and caches the password for the
duration of the run. `open` runs `keepassxc-cli open` and queries the resulting console. `builtin` uses a
library compiled into chezmoi instead of `keepassxc-cli`.

All three take the database master key, not an entry-scoped grant. There is no chezmoi setting that says
"this run may read these three entries". The unit of authorization in every mode is the database.

### 2. Two of the three modes serve this repository's lookups, and `builtin` does not

`cache-password` and `open` both returned the right values for a title lookup and an attribute lookup,
with one password line serving several lookups in the same run. `builtin` decrypted the database (a wrong
password produced an integrity-check failure, so it really opened the file) and then returned an empty
map for every entry, under the plain title, under `Root/<title>` and under `/<title>`.

The failure is loud in the form this repository actually writes. `{{ (keepassxc "x").Password }}` on an
empty map exits 1 with `map has no entry for key "Password"`, so a `builtin` misconfiguration would abort
an apply rather than splice an empty string into a deployed config. It is still a disqualifier: the mode
does not work here, and the cause was not chased because the verdict does not turn on it. Note the probe
was KDBX 3.1 and the real vault is KDBX 4.1, so this result does not even establish which format is the
problem.

### 3. The supported unattended configuration exists, and its cost is the whole vault

`keepassxc-cli db-edit --set-key-file <path> --unset-password` converts a database to key-file-only
authentication. With `prompt = false` and `args = ["--no-password", "-k", <path>]`, both documented on
chezmoi's KeePassXC page for non-password-protected databases, the render becomes silent: no prompt, no
terminal, exit 0, correct value. Measured.

This is the whole of the "agent applies unattended" question, and it answers itself. The key file was 128
bytes at mode 0400. Any process that can read it can read every entry in the database, forever, with no
prompt and no record. That is not a scoped grant with an approval step; it is the master credential in a
file, which is what the ledger's own boundary task calls out: "Hiding a value from chat or putting it in
a child environment does not keep it inaccessible to an unrestricted process running as the same user."

### 4. Only the browser protocol rides the unlocked application, and chezmoi cannot speak it

KeePassXC's browser integration is the one path that reaches a database already unlocked in the running
graphical application. On this machine it is live: `keepassxc-proxy` is running for a Chrome extension,
and the socket is at `$TMPDIR/org.keepassxc.KeePassXC.BrowserServer`, mode `srwx------`, owner `stephen`.

Three facts make it unusable as a chezmoi backend:

- **chezmoi has no browser-protocol mode.** The only backends are `keepassxc-cli` and the compiled
  library. Reaching the protocol would mean pointing `keepassxc.command` at a wrapper that mimics
  `keepassxc-cli show` output. The knob is supported; the wrapper is a custom broker, which the task asks
  to avoid.
- **The protocol looks entries up by uniform resource locator (URL), not by title.** `kpxc-cli`'s own
  README states it plainly: entry lookup is by URL or hostname only, and title-based search is not
  supported by the protocol. Every one of the fifteen targets here looks up by title. Custom fields need
  the "Support KPH fields" setting enabled and a `KPH: ` prefix on the attribute, which is a rename of
  the two `keepassxcAttribute` fields this repository reads.
- **The association it relies on is a file under the agent's own user id.** `kpxc-cli` stores its
  association at `~/.keepassxc/browser-api.json` at owner-only permissions. The socket is 0700 under the
  same user. An agent with shell access reads the association and connects to the socket, so the
  confirmation dialog gates the first association, not each later request. Approvals can also be made
  permanent in settings.

So the browser protocol is not a boundary against a process running as the operator. It is a boundary
against a remote web page, which is what it was built for.

### 5. KeePassXC's own per-operation mechanism covers SSH keys only, and Secret Service is not on macOS

KeePassXC has exactly one integration that lets another program use a secret without receiving it: the
SSH agent integration, which loads a key into the agent so the private key material never leaves. It is
supported on macOS. It serves SSH keys and nothing else, so it does not touch a single one of the fifteen
targets.

The Secret Service integration is the freedesktop.org Secret Service specification over D-Bus. There is
no D-Bus session bus on macOS and no `org.freedesktop.secrets` to register, so the feature does not exist
on this platform. KeePassXC's own documentation reinforces the split by naming Linux, macOS and Windows
individually in the SSH agent section while naming no platform in the Secret Service section.

Quick Unlock with Touch ID is a graphical-application feature. It re-unlocks a database that was already
unlocked with the master password in the same session, and `keepassxc-cli` does not participate in it.

### 6. The read-only reporting path is blocked today, and a narrower one is already available

`private_dot_claude/agents/chezmoi-apply.md` defines an agent whose job is to run `chezmoi status` and
`chezmoi diff` and hand the operator a checklist. Measured against the real configuration with no vault:
`chezmoi status` exits 1 at the first vault read, and so the agent's own documented process cannot
complete. `--exclude=templates` does not rescue it, failing instead at a `modify_` template, which
independently re-confirms the measurement `CLAUDE.md` records from 2026-08-02. There is no entry type
that excludes `modify_` templates: the valid set is
`all, none, dirs, files, remove, scripts, symlinks, always, encrypted, externals, templates`.

What does work with no vault, measured: `chezmoi managed`, and `chezmoi status <path>` and
`chezmoi diff <path>` for any target that is not vault-backed. That is a real, supported, zero-risk
improvement to that agent's instructions and it needs no boundary change at all: the agent can diff the
non-vault set by path and report the vault-backed set as "not inspectable without the operator". It is
the only thing in this document I would call adopt-now, and it is a documentation edit, not a credential
change.

### 7. After any successful apply, the values are cleartext under the agent's own user id

All fifteen deployed targets were checked by `stat`, contents untouched. Fourteen are mode 0600 owned by
`stephen`; `~/.gitconfig` is 0644 and holds a public signing subkey identifier, not a secret.

Mode 0600 owned by `stephen` means readable by every process running as `stephen`, an agent's shell
included. Claude Code's deny list names `~/.aws/credentials`, `~/.config/pns/config.toml` and
`~/.hermes/.env` among the fifteen; the other eleven are not named. The deny list's own docblock records
that it "only covers the Bash commands Claude Code recognizes as file reads" while the allow list carries
`Bash(grep *)` and `Bash(find *)`, and that the whole question is moot at
`defaultMode = bypassPermissions`, which is this repository's setting.

The age-encrypted arm has no vault gate at all. `chezmoi cat ~/.hermes/config.yaml` piped to `shasum`
returned a hash and exit 0 with no unlock, because `~/.config/chezmoi/key.txt` is already on disk at
0600\. Five encrypted Hermes configuration files decrypt on demand for anything running as the operator.

The consequence is the important part: the operator-only apply rule protects the freshness of a render,
not the secrecy of a value. Any claim that the current arrangement keeps secrets away from an agent is
false today, before any change is proposed.

### 8. The source tree is the boundary that matters, and it is agent-writable

The vault unlock is supplied by the operator, but the instructions that consume it come from the source
tree, which an agent edits as ordinary work. Two paths:

- **A template edit.** Adding `{{ (keepassxc "<entry>").Password }}` to any templated target makes the
  operator's next apply fetch that value and write it to a file the agent reads. Nothing catches this:
  `gitleaks git --staged` blocks a staged plaintext secret, not a new vault read, and no test asserts
  which templates may call the function.
- **A hook edit.** `.chezmoi.toml.tmpl` declares
  `[hooks.read-source-state.pre] command = "{{ .chezmoi.sourceDir }}/.install-password-manager.sh"`, and
  that script lives in the repository. It runs before every chezmoi command that reads the source state,
  the read-only ones included, which is by the hook's own design. An edit to it is code execution inside
  the operator's unlocked apply.

This reframes the whole task. The credential store is not the weak link. The weak link is that the
instruction stream reaching the unlocked vault is writable by the party the boundary is supposed to
constrain. Any design that grants an agent credential access without addressing this is granting more
than it names.

### 9. Why the outbound credential proxy does not reach this boundary

Infisical's Agent Proxy, as specified upstream, wraps a process, sets `HTTPS_PROXY` and `HTTP_PROXY`, and
adds credentials to the wrapped process's outbound Hypertext Transfer Protocol (HTTP and HTTPS) requests
itself. Its documentation is explicit that "brokered credentials are never injected here; the Agent Proxy
adds them to each outbound request itself, so they reach the destination but never the agent." It brokers
HTTP and HTTPS only, cannot broker SSH, and a tool that ignores the proxy environment variables bypasses
it entirely.

That model is a good fit for exactly what it describes and a non-fit for everything in this task:

- **It has no file output.** Secrets are applied to requests in flight. It cannot write a value into
  `~/.aws/credentials`, `~/.gitconfig` or `~/.hermes/.env`, which is what chezmoi's rendering does.
- **It has no local-consumer story.** `himalaya` authenticating to a local Proton Mail Bridge, `openhue`
  and `lights` talking to a bridge on the local network, `atuin`'s history-ignore pattern, espanso's
  address and phone expansions, and the age identity restore are not outbound application programming
  interface (API) calls to a brokered destination. Several are not network operations at all.
- **It is bypassable by the same process it wraps.** Environment variables are not a boundary against a
  process that can unset them.

The homelab plan already says this in its own words, at track A6: "the proxy handles outbound requests,
so it does not itself authorize or secure `chezmoi apply`". That is correct, and this document's
contribution is the reason in mechanism terms rather than assertion. The proxy solves a different problem
well: it keeps a credential off the laptop for a network destination. Roughly six of this repository's
vault entries are outbound API keys of that shape (Anthropic, ElevenLabs, OpenRouter, Tavily, Composio,
the Hermes webhook secret), and those are genuine proxy candidates. The local rendering set is not, and
no amount of proxy configuration makes it one.

### 10. `passage` is a replacement store, not a KeePassXC integration

`passage` is a fork of `password-store` that uses age instead of GnuPG, storing secrets under
`~/.passage/store` and decrypting with identities at `~/.passage/identities`. It has no KeePassXC
integration. Adopting it would mean a second store to keep in step with the KeePass database the ledger
says stays canonical, which the ledger also forbids ("Do not introduce automatic vault synchronization").
It stays background research, unchanged by this document.

Its one contribution here is a pointer. `passage` supports `age-plugin-yubikey`, where the private key
lives on the hardware and a touch policy can require a physical touch per decryption. That is the shape
of a real per-operation boundary, and it is what makes the deferred candidate in the verdict worth a
decision rather than a rejection.

### 11. A side finding: the vault-exclusion logic in the lint formatter has no test

`scripts/treefmt/shellcheck-rendered-template.sh` skips any shell template that mentions `keepassxc`, or
that transitively includes a `.chezmoitemplates/` partial that does, because an interactive unlock cannot
happen headless. That decision is made by a `renders_unsafe` function with a transitive include walk and
a cycle guard.

`test/fixtures/render-coverage/` holds twenty-odd fixtures built for exactly that logic
(`excluded_keepassxc.sh.tmpl`, `chain_root.sh.tmpl`, `cyclic_a.sh.tmpl`, `partial_secret.tmpl` and so on)
and nothing references the directory. The only mention of the string `render-coverage` anywhere in
`scripts/`, `test/`, `treefmt.toml` or the `justfile` is a comment in the formatter recalling the retired
Nix-era classifier. `test/unit/rendered-template-shellcheck-wrapper.sh` drives the per-file body with
stubs, not the classifier.

This is in scope under the 2026-08-05 ruling, since it is behavior in a script this repository owns, and
gutting `renders_unsafe` would not turn anything red. It wants its own small task; it is not part of the
credential boundary, and this document does not propose a change to it.

## Handing over a password versus letting a tool use it

The task asks for this distinction, and it is the most useful thing in the document, so here it is stated
as a test rather than a definition.

**Handing over a password** means the secret, or a credential equivalent to it, comes to rest somewhere
the agent can read: a file, an environment variable, a command argument, a cached unlock in a process the
agent can inspect or restart. Anything the agent can read once, it can read again at a time of its
choosing, for any purpose, with no record. The permission is unbounded in time, in scope and in purpose.

**Letting a tool use a secret for an approved operation** means the secret is applied inside something
the agent cannot read out of, and the approval is bounded. It requires all three of:

1. a **use boundary**, some place the agent cannot reach into (a different user id, a different machine,
   a hardware element);
1. an **approval act** bound to the specific operation, not to a session or a machine;
1. a **record** of what was used for what, without the value.

Measured against that test, this is what exists on dresden today:

| Mechanism                      | Use boundary                  | Approval act       | Record       | Verdict                 |
| ------------------------------ | ----------------------------- | ------------------ | ------------ | ----------------------- |
| Operator types the password    | none (runs as the user)       | one act, whole run | none         | handing over, delayed   |
| Key-file-only database         | none                          | none               | none         | handing over            |
| Browser protocol or `kpxc-cli` | none (association file, 0600) | first association  | none         | handing over            |
| KeePassXC SSH agent            | yes, the agent process        | none per use       | none         | tool use, SSH keys only |
| Infisical Agent Proxy          | yes, the home server          | none per use       | server audit | tool use, outbound only |
| YubiKey plus `mode = "open"`   | yes, the hardware             | one touch per run  | none         | tool use, per run       |

Note what that table says about the current arrangement. The operator typing the master password at an
interactive apply is, mechanically, handing the secret to every process running as `stephen` for as long
as the values sit on disk. It is a good practice for a different reason (a human sees the diff, and the
timing is deliberate), and it should stay. It is not a use boundary, and the design work in the ledger
should not be built on a belief that it is.

## The operation table

This is the task's `done_means`: which local operations a supported integration can cover, and which
still need the operator. "Supported" excludes a custom wrapper, a key-file handover and a custom broker,
for the reasons above.

| #   | Local operation                                    | Covered without operator?    | Basis                            |
| --- | -------------------------------------------------- | ---------------------------- | -------------------------------- |
| 1   | `chezmoi managed`, `unmanaged`, `cat-config`       | yes, today, no change        | measured exit 0, no vault        |
| 2   | `chezmoi status <path>` / `diff <path>`, non-vault | yes, today, no change        | measured exit 0, no vault        |
| 3   | Whole-tree `chezmoi status` / `diff`               | no                           | exits 1 at first vault read      |
| 4   | `chezmoi apply` of the 15 vault-backed targets     | no                           | every mode takes the master key  |
| 5   | `chezmoi apply` of the two vault-reading scripts   | no                           | `run_before_05` shells the tool  |
| 6   | Decrypting the five age-encrypted source files     | yes, already agent-reachable | `chezmoi cat` ran with no vault  |
| 7   | Reading an already-deployed rendered secret        | yes, already agent-reachable | fourteen files at 0600           |
| 8   | Lint-rendering a vault-backed shell template       | partly, by author opt-in     | the `{{ if (env "CI") }}` branch |
| 9   | Age identity restore on a fresh machine            | no                           | KeePassXC has no per-entry grant |
| 10  | Moshi device pairing                               | no, worst-shaped of the ten  | token in the body and in argv    |

Rows 6 and 7 are the finding that matters most. Two of the ten operations are already fully available to
an agent and were never gated, which means the boundary the ledger is designing is narrower than it
looks: it governs the render step, not the secrets.

Row 8 deserves its own note because it is the one place a supported pattern is already working.
`Library/Application Support/espanso/match/private_identity.yml.tmpl` puts each vault read behind
`{{ if (env "CI") }}` with a `FAKE_ADDRESS` or `FAKE_PHONE` constant on the other branch, and
`scripts/treefmt/espanso-match-render.sh` renders it with `CI=1`, so the template is fully linted with no
vault. That is exactly the dummy-credential discipline the ledger asks for elsewhere, already shipped in
one file. The other fourteen targets are skipped instead.

Row 9 is worth separating from row 4 because it is the one operation with a genuinely narrow need. The
age restore wants one entry, `chezmoi :: Private Key :: age`, and nothing else. If KeePassXC had any form
of per-entry grant this would be the first candidate for it. It has none: the unit of authorization is
the database in every mode. And the restore is self-healing and idempotent by design, so the cost of it
needing the operator is one unlock on a fresh machine, which the bootstrap already requires for the other
fourteen targets. See `docs/runbooks/age-key.md`.

Row 10 is the one place in the repository where a secret is put somewhere it did not need to go, and it
is only half fixable. The token reaches two places: the rendered script body, which chezmoi materializes
as a temporary file to execute, and the process argument list, which is readable by any process running
as the operator. `moshi-hook pair --help` was checked: its only input for the token is `--token string`.
There is no standard-input form and no documented environment variable, and patching a third-party tool
is not permitted, so the argument-list exposure cannot be removed.

The body exposure can. `run_before_05-restore-age-key.sh.tmpl` in the same directory already demonstrates
the pattern: keep the rendered body secret-free and shell `keepassxc-cli` at execution time, assigning
the value to a shell variable the script then passes on. That would move `run_once_after_60` from "token
in the source render and in argv" to "token in argv only".

Whether the half fix is worth making is a judgment about a single-use credential. The token is consumed
by the pairing call, and the script's own docblock records a live failure when a consumed one was
replayed, so its value after the first successful apply is nil. Against that, the exposure today is
persistent rather than momentary, because the rendered value is part of the source state the script's
hash is taken over. This document does not propose the change; it records that the standard-input option
the obvious fix would have used does not exist.

## Verdict and reasons

**Reject, with one adopt-now documentation item and one defer.**

**Rejected, and why:**

- **`keepassxc.mode = "builtin"`.** It returned an empty map for every entry against a database
  `keepassxc-cli` 2.7.12 wrote. It does not work here, and even working it would change nothing about the
  boundary, since it also takes the database master key.
- **A key-file-only database for unattended applies.** It works perfectly, which is the problem. It is
  the master credential in a readable file, unbounded in time, scope, purpose and record. It fails all
  three clauses of the use-boundary test.
- **`kpxc-cli` and the browser protocol as a chezmoi backend.** Three independent blockers: chezmoi has
  no such mode, lookups are by URL and this repository looks up by title, and the association plus socket
  are readable by any process running as the operator, so it is not a boundary against the party it would
  be deployed against. Adopting it would also mean renaming entries and re-prefixing custom fields across
  the vault for no security gain.
- **A custom broker of any shape.** The task asks to prefer supported integrations, and the reason holds
  independently: a broker running as `stephen` with the vault key inherits every property of the key
  file. A broker only becomes a boundary when it runs somewhere the agent is not, and at that point it is
  Infisical, not a local KeePassXC wrapper.

**Adopt now, zero risk, no boundary change:** correct `private_dot_claude/agents/chezmoi-apply.md` to
reflect what is measurably possible. Whole-tree `chezmoi status` and `chezmoi diff` cannot complete
without the operator, so the agent's documented step 1 fails as written. Path-scoped `status` and `diff`
on non-vault targets do work. This is a documentation fix to an agent definition. It grants nothing and
it needs no credential decision, though it is still the operator's edit to approve because it is a
behavior contract.

**Deferred, and the reason it is worth deciding rather than rejecting:** `keepassxc.mode = "open"` with a
YubiKey challenge-response factor on the database. chezmoi's own user guide describes this as the
experimental YubiKey path and states it requires `mode = "open"` plus a `--yubikey` argument. Measured
here: `mode = "open"` works with this repository's existing title and attribute lookups, unchanged, with
no template edits. It is the only supported configuration in which the unlock cannot be produced by
software running as the operator, because the challenge-response needs a physical touch. It converts row
4 of the operation table from "needs the operator at a keyboard" to "needs the operator to touch a key",
which is a smaller ask and a genuine boundary.

Its costs are real and the operator should hear them before the benefit: it is an experimental chezmoi
mode; the touch authorizes the whole run, not one secret, so an apply that renders fifteen targets is one
touch; a lost or broken key locks the vault unless a second factor or a recovery path exists; `ykman` and
the hardware are both absent from this machine today; and it does nothing for rows 6, 7 and 8, because
those never needed the vault. It is deferred rather than recommended because it costs money and a
recovery design, and because the 2026-05-01 secrets research already logged `age-plugin-yubikey` on macOS
as an open gap (its item 841) that nothing has closed since.

## Assumptions made in the operator's place

Each of these is a choice this document made so it could reach a verdict overnight. Each has its
alternative stated, and each is the operator's to reverse.

1. **"Supported" was read as "documented by chezmoi or KeePassXC and reachable through their own
   configuration", excluding a wrapper behind `keepassxc.command`.** The alternative reading is that
   `keepassxc.command` is a supported knob and therefore a shim pointed at it is a supported integration.
   I rejected that reading because the shim is code this repository would own, maintain and be exposed
   by, which is a custom broker with a supported socket. If the operator prefers the other reading, the
   `kpxc-cli` rejection in this document weakens to "possible, at the cost of a title-to-URL migration
   across the vault and a shim with no security gain", and the `kpxc-cli` evaluation task at ledger line
   2056 should be re-scoped accordingly.
1. **The threat model assumed is an agent with arbitrary shell as the operator's own user, which is the
   configuration this machine runs.** `defaultMode = bypassPermissions` plus `Bash(cat *)` in the allow
   list is what makes findings 7 and 8 bite. The alternative model, an agent whose reads are actually
   constrained, would change the verdict materially, and it is reachable: tightening `defaultMode` and
   dropping the broad `Bash` allow entries is a settings change, not a credential change. I did not
   propose it because it is a large behavioral change to how the operator works and it belongs to the
   boundary-definition task at ledger line 2058, not to this evaluation.
1. **The verdict treats "the agent can already read every deployed secret" as a fact to design around
   rather than a defect to fix first.** The alternative is to treat it as the primary defect, in which
   case the whole Infisical direction gets reordered behind it: there is little point brokering a
   credential to keep it off the laptop while fourteen others sit at 0600 in `$HOME`. I lean toward the
   alternative and did not act on it, because reordering an approved plan is the operator's call.
1. **The Moshi pairing token was treated as a defect worth naming but not worth proposing a fix for.**
   The alternative is to file a fix task now. I held back because the available fix is partial: it
   removes the token from the source render but not from the argument list, since `moshi-hook pair` takes
   the token only as `--token` and patching a third-party tool is forbidden. A partial fix to a
   single-use credential is a judgment about how much a persistent exposure of a spent token matters,
   which is the operator's to make rather than mine.
1. **The YubiKey path was written up as a deferred candidate rather than left out.** The alternative is
   to leave hardware out of a software evaluation. I included it because without it the document says
   only "no supported integration helps", which is true and not actionable, and because it is the only
   mechanism that passes the use-boundary test for chezmoi's rendering at all.
1. **The scratchpad probe directory was left in place.** The alternative is to trash it now. The
   destructive-action rule puts directory removal behind the operator's confirmation, including scratch
   directories an agent made itself, so leaving it and naming it is the conservative reading. It holds
   only dummy strings.

## What would change the verdict

- **chezmoi gains an entry-scoped or grant-scoped KeePassXC mode.** This is the single change that would
  move rows 4, 5 and 9 of the table. Nothing upstream suggests it is coming; the table of five
  configuration keys has no room for it today.
- **`mode = "builtin"` turns out to work against KDBX 4.1.** The probe here was KDBX 3.1 only. If builtin
  works on the real format it still does not change the boundary, but it would remove one rejection and
  would give a vault read that does not depend on `keepassxc-cli` being installed. Worth knowing, not
  worth acting on alone.
- **A genuine custom attribute under `mode = "open"` fails.** The attribute probe used the built-in
  `Title` field, because `keepassxc-cli` 2.7.12 cannot create a custom attribute from the command line.
  If the real thing fails under `open` mode, the deferred YubiKey candidate dies with it, because
  `dot_gitconfig.tmpl` and `dot_composio/private_user_data.json.tmpl` both read custom attributes and
  `open` is the mode YubiKey requires. This is the cheapest thing to check and the operator can do it in
  one command with the vault unlocked.
- **A process boundary appears on this machine.** A second user id that owns the applies, or a sandboxed
  harness whose file reads are actually constrained, would turn several "handing over" rows in the
  mechanism table into "tool use" rows without changing the credential store at all. That is the real
  lever, and it is not a KeePassXC question.
- **`moshi-hook` gains a standard-input or environment form for its pairing token.** Today it takes
  `--token` only, so row 10 has no clean fix; upstream adding one would give it the same shape
  `run_before_05` already has, with no boundary change and no patch to a third-party tool.
- **The operator decides the deployed-secret exposure is the primary defect.** Then this whole evaluation
  is subordinate to that work and its ordering changes, as assumption 3 records.

## Open questions for the operator

1. **Which reading of "supported" governs the `kpxc-cli` task?** If a shim behind `keepassxc.command`
   counts as supported, that task's scope changes and this document's rejection of the browser protocol
   weakens to a cost argument. My recommendation: it does not count, and `kpxc-cli` should be closed as
   rejected with this document as the evidence.
1. **Is the YubiKey path worth pricing?** It is the only supported configuration that creates a real
   per-run boundary for chezmoi's rendering. It costs hardware, an experimental chezmoi mode and a
   recovery design, and the touch authorizes a whole run. My recommendation: yes, price it, as its own
   task, after the two questions below.
1. **Should the deployed-secret exposure be fixed before any broker work?** Fourteen files at 0600 in
   `$HOME` plus five age-encrypted targets that need no vault mean an agent on this machine can read
   nearly every credential today. My recommendation: yes, and it reorders the Infisical tasks behind it.
1. **Should the source tree be treated as a trust boundary?** An agent that edits a template or
   `.install-password-manager.sh` reaches the operator's unlocked apply, and nothing in the gates catches
   it. My recommendation: yes, and it belongs in the boundary-definition task at ledger line 2058 rather
   than here, because it needs a mechanism decision, not more research.
1. **Is the half fix to the Moshi pairing token worth making?** `moshi-hook pair` takes its token only as
   `--token`, verified, so the argument-list exposure cannot be removed without patching a third-party
   tool, which is forbidden. The render-time exposure can be removed by fetching at execution time the
   way `run_before_05` does. My recommendation: yes, as a small standalone change, because the token
   currently sits in the source state rather than only in a momentary argument list.
1. **Should `keepassxcAttribute` be verified against a real custom attribute under `mode = "open"`?** One
   unlocked command settles whether the deferred candidate is viable at all.
1. **Should the orphaned `test/fixtures/render-coverage/` fixtures get a test, or be deleted?** The
   vault-exclusion logic in the lint formatter is untested and its fixtures are referenced by nothing. My
   recommendation: one small test, because the logic decides which templates get linted and a silent
   regression there is a lint gap nobody would notice.

## What did not change

No file in the repository was edited. No package was installed. No chezmoi command that writes anything
was run: the only chezmoi invocations against the real configuration were `managed`, `status`, `diff` and
`cat`, all read-only, and `cat` output went straight to `shasum`. The operator's vault was never
unlocked, and its master password was never requested, supplied or cached. The only database created was
the dummy one in the session scratchpad, which the operator steps handed over with this document name for
deletion.
