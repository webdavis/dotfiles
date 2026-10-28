# Fresh-machine recovery and the two current secret exposure paths

Status: design, written 2026-09-14 with the operator asleep. Nothing here is approved. No code was
written or changed, and no live credential was read or printed. Every choice made in the operator's
place is listed under "Assumptions made in your place" with its alternative; the decisions that are
genuinely theirs are in "Open questions".

Ledger task: `docs/remaining-work.md`, section "Safe agent credentials and Infisical client
integration", the bullet beginning "Include fresh-machine recovery and current secret exposure paths
in that design" (line 2148 at the time of writing).

Second pass: the load-bearing claims below were re-derived from primary sources (chezmoi's source at
its installed tag, chezmoi's reference documentation, the installed `moshi-hook` binary, and the
repository itself) rather than by re-running the same experiments. They hold. Three findings were
added, and the fresh-machine table gained a row; all of it is in "Second pass" near the end, so the
original text is still readable as it was written.

Chain: this continues the credential access boundary design written earlier tonight, which measured a
live path from an agent-written source file to the whole KeePass database and answered it with two
tiers (Tier 1 brokered by the homelab Agent Proxy, Tier 2 rendered locally and openly agent-readable).
That document deliberately stopped short of three things and named them as this one's work: the
age-key restore call site, the moshi pairing token placement, and fresh-machine bootstrap. All three
are below, plus one correction to that document's headline recommendation.

## The correction, first, because it moves the floor

The boundary design called `keepassxc.mode = "builtin"` "the single highest-value change in the
document and it is one line". Measured tonight: it is not one line, and applied on its own it breaks
every vault lookup in the repository.

chezmoi's builtin mode does not resolve entries the way `keepassxc-cli` does. It walks the database
and builds keys as `<group>/<title>`, starting from the subgroups of the root group, so an entry that
sits directly in the root group is never reachable at all, and an entry inside a group is reachable
only by its full path. `keepassxc-cli show`, by contrast, resolves a bare title from anywhere in the
database. All 43 call sites in this repository (16 rendered files, counted excluding documentation,
tests, fixtures, the pns crates and the one inert commented reference) pass a bare title with no
slash in it, so the call sites carry no information about where the entries actually live, and under
builtin mode they are either wrong (entry in a group, wrong key) or unresolvable (entry at the root).

So the builtin switch is a three-part change: confirm where each entry lives, move any root-level
entries into a group, and rewrite all 43 call sites to the full path. It stays the right destination,
because it is the change that stops chezmoi from ever handing the master password to a child process.
It is not the free win the earlier document described, and it should not be done in a hurry.

## What was measured, and how

Environment: dresden, macOS 26.2 (Darwin 25.2.0), chezmoi v2.72.1 from Homebrew, keepassxc-cli 2.7.12,
moshi-hook 0.3.16, atuin 18.22.0, age and age-keygen from Homebrew. Every chezmoi experiment ran
against a throwaway source directory, a throwaway home directory, and a dummy KeePass database holding
dummy values, with its own throwaway configuration file, so none of it touched the live configuration,
the live vault, or the live home directory. The exact commands are in the appendix.

1. **M1, builtin mode keys entries by group path.** In chezmoi v2.72.1,
   `internal/cmd/keepassxctemplatefuncs.go` builds the builtin lookup from
   `db.Content.Root.Groups[0].Groups`, joining `<group>/<title>`; the root group's own entries are
   never visited. Confirmed on a dummy database: a root-level entry returned
   `map has no entry for key "Password"`, the same entry inside a group returned its value when asked
   for as `Grp/<title>`. Under `cache-password`, both forms returned the value.
1. **M2, `keepassxc-cli` resolves a bare title anywhere in the database.** On the same dummy database,
   `keepassxc-cli show -s -a Password <db> "<title>"` returned the value for an entry sitting inside a
   group. The secret-free way to list the real paths is `keepassxc-cli ls -R -f <db>`, which prints
   entry paths and no values; it is an operator step because it needs the master password.
1. **M3, source attribute order.** `create_private_dot_alpha` produces target `.alpha`;
   `private_create_dot_beta` is not parsed as an attribute at all and produces a target literally named
   `create_dot_beta`. `create_` comes first.
1. **M4, a fresh machine restores the age identity and decrypts in one apply.** With
   `dot_config/chezmoi/create_private_key.txt.tmpl` reading the identity from the vault and an
   age-encrypted target present, one apply against an empty home directory wrote the identity at mode
   0600 and then decrypted the encrypted target in the same run. chezmoi applies targets in
   lexicographic order of target name, so `.config/chezmoi/key.txt` lands before anything under
   `.hermes/`. No `keepassxc-cli` process was involved (the experiment ran in builtin mode).
1. **M5, `create_` never overwrites an existing regular file.** A key file holding two identities (the
   overlap window the rotation runbook creates) survived a re-apply byte for byte. The apply did not
   prompt for the master password at all, so with the identity in place the vault entry is not read.
1. **M6, the ordering invariant is load-bearing.** With an encrypted target named to sort *before*
   `.config/`, the apply aborted at that target, never wrote the identity, and a second apply failed
   identically. That is a deadlock, not a retry: recovery is the runbook's manual copy step. Today all
   five encrypted source files live under `private_dot_hermes/`, and no other `encrypted_` file exists
   in the repository.
1. **M7, a wrong identity fails loudly on every apply**: `age: error: no identity matched any of the
   recipients`, followed by `chezmoi: <target>: exit status 1`.
1. **M8, symlink behavior at the identity path.** With prior chezmoi state for that target, a symlink
   raises chezmoi's "has changed since chezmoi last wrote it" prompt, and a non-interactive apply dies
   on it. With no prior state, chezmoi replaces the link with a regular 0600 file holding the link
   target's content and never consults the vault. In neither case did chezmoi write through the link:
   the decoy file the link pointed at was unchanged both times.
1. **M9, script ordering.** A `run_before_` script printed its marker before the vault password
   prompt for the first secret template; the `run_after_` marker printed after. So every
   `run_before_` script, including the package installer, runs ahead of the first vault read.
1. **M10, the KeePassXC cask ships the command.** `brew info --cask keepassxc` lists
   `/Applications/KeePassXC.app/Contents/MacOS/keepassxc-cli (Binary)`, and `keepassxc` is already
   declared under `packages.macos.homebrew.casks` in `.chezmoidata/system_packages_autoinstall.yaml`.
1. **M11, moshi-hook supports an environment variable for the pairing token.** With
   `MOSHI_PAIRING_TOKEN` set and no flag, `moshi-hook pair` posted to `/hosts/register` (proved by
   pointing `--base-url` at a dead port and reading the dial error). With neither, it refused:
   `no pairing token provided (use --token or set MOSHI_PAIRING_TOKEN)`. Tested in an isolated
   environment with its own config, state and home directories, against an unroutable base URL, so the
   live pairing was never touched.
1. **M12, an environment variable is not a hiding place.** `ps eww -p <pid>` printed a marker
   variable from a separate same-user process, reproducing the boundary document's measurement
   independently.
1. **M13, the command-line form has already leaked into history.** `atuin search` reports 12 entries
   matching `moshi-hook pair --token` (counted, never printed). atuin's search is fuzzy, so treat 12 as
   an upper bound. `auto_sync` is false in `dot_config/atuin/private_config.toml.tmpl`, so those rows
   are local, and local means agent-readable. Whether the vault-held `history_filter` regex would have
   caught the pattern cannot be checked without reading that value, so it is an operator check.

## Constraints this design has to hold

Carried from `CLAUDE.md`, the ledger and recorded rulings; only the ones that bind the choices below.

- The operator runs applies, interactively, with the vault unlocked. Agents do not.
- Third-party tools are configured, never patched or forked. moshi-hook, chezmoi and KeePassXC are all
  third party here, so every alternative below uses a documented input of the tool as shipped.
- No removal mechanisms are built in this repository. A retired script leaves no cleanup machinery.
- Never tell the operator to run an apply without having verified it will pass. The measurements above
  exist for that reason.
- No steady state may depend on the operator remembering a recurring manual step. A once-per-machine
  step in the fresh-machine runbook is not a recurring step; the runbook already carries a dozen.
- Tests cover the behavior of tools this repository wrote, not chezmoi's, not Homebrew's, and not
  deployment.
- Demonstrate any proposed chezmoi flow with dummy credentials first, including locked or denied
  access, secret-free output, and a complete render and deployment cycle.

## Deliverable 1: the age-key restore call site

Today `.chezmoiscripts/run_before_05-restore-age-key.sh.tmpl` runs on every apply. It is careful in the
ways that matter: the secret is never rendered into the script body, it never reaches a command line,
it is streamed into a 0600 temporary file and moved into place, an existing key is validated rather
than clobbered, and a symlink at the path is refused. Its one problem is structural. It calls
`keepassxc-cli` itself, so it is a second vault client invocation that `keepassxc.mode` does not
govern, and it keeps the substituted-client path open even after chezmoi stops spawning the client.

### Option A1: leave it as it is

Costs nothing, changes nothing. The script remains the one place in the repository that spawns a vault
client, and the builtin switch then buys less than it should, because a shim on `PATH` still gets a
turn on every fresh-machine apply. Rejected as the end state, acceptable as the state until the
operator approves a replacement.

### Option A2: make the identity a `create_` target and delete the script (recommended)

Add `dot_config/chezmoi/create_private_key.txt.tmpl` holding exactly the template expression that
reads the identity out of the vault, and delete the script. chezmoi's `create_` attribute means
"create it if it is missing, never touch it if it is there", which is the same validate-never-clobber
intent the script implements, enforced by chezmoi instead of by a script of our own.

What the measurements say it gives:

- Fresh machine: one apply writes the identity at 0600 and then decrypts every encrypted target in the
  same run (M4).
- Steady state: the file is present, so chezmoi does not render the template and does not read the
  entry at all (M5). The apply still prompts once, for the other secret targets.
- Rotation: the overlap window, where the key file holds the old and new identity while the vault entry
  holds only the new one, is preserved untouched (M5). Under any design that rewrites the
  file from the vault on every apply, that window would be clobbered and the runbook's rollback would
  stop working.
- Wrong key: loud failure on every apply, naming the target (M7).
- Symlink: chezmoi refuses or replaces the link, and never writes through it (M8).
- No vault client process, ever, for this value.

What it gives up: the script's warnings, which name `docs/runbooks/age-key.md`. The replacement for
those is one line in that runbook mapping `no identity matched any of the recipients` to the recovery
steps that already exist there. A validator script is not worth writing: chezmoi decrypts the five
encrypted sources on every apply in order to compare them, so a wrong or missing key surfaces on the
next apply without anything of ours running.

What it depends on: **no `encrypted_` target may ever sort before `.config/chezmoi/key.txt`**
(M6). Today that holds with room, since the only encrypted targets are under `.hermes/`.
It is a real constraint and it needs to be written where someone adding an encrypted target will see
it, which is the age runbook and a comment in the new template.

Two details worth fixing at the same time: the directory stays `dot_config/chezmoi/`, not
`dot_config/private_chezmoi/`, so the apply does not change the mode of chezmoi's own configuration
directory, and it must never become `exact_`, which would make chezmoi delete `chezmoi.toml` and the
state database sitting beside the key.

### Option A3: call `chezmoi execute-template` from inside the script

Rejected. A nested chezmoi process has its own password cache, so it prompts the operator a second
time, and running chezmoi inside its own apply is not a supported arrangement.

## Deliverable 2: the moshi pairing token in command arguments

Today `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl` renders the token into the script
body and passes it as `moshi-hook pair --token <value>`. The boundary document measured the rendered
script's own exposure (a 0700 file in the per-user temporary directory, which is same-user readable);
the argument adds `ps` visibility for the length of the call. The runbook and the script's comments
also tell the operator to re-pair by hand with `moshi-hook pair --token <token>`, and M13
says that has already put the value into the local history store a dozen times.

The value is not going away: the same vault entry is rendered into `~/.config/pns/config.toml`, where
pns reads it as the token it posts with. So this is a placement question, not a removal question.

### Option P1: keep the value in the script, pass it in the environment

`MOSHI_PAIRING_TOKEN={{ $token | quote }} /opt/homebrew/bin/moshi-hook pair` is a one-line change and
is supported by the installed version (M11). It removes the `ps` window and nothing else:
the rendered body still holds the value, and a child environment is readable by any same-user process
(M12). Worth having as an interim, not worth calling a fix.

### Option P2: take pairing out of the apply entirely (recommended)

Pairing is once per machine. It is also the one step in that script that consumes a credential, that
can fail the whole apply when a consumed token is replayed (the script's comments record that
happening live), and that has an interactive source of truth anyway, since the token comes off the
iPhone app's Integrations screen.

Move it to `docs/runbooks/macos-fresh-machine-quickstart.md` as an operator step, in this exact shape,
which keeps the value out of the command line and out of history:

    read -rsp 'moshi pairing token: ' token; echo
    MOSHI_PAIRING_TOKEN="$token" moshi-hook pair
    unset token
    brew services restart moshi-hook

The script keeps everything else it does, all of which is non-secret and idempotent: the Codex
uninstall, the install for the other harnesses, the tap re-trust, and the service start. With the
credential gone it no longer needs `run_once_` semantics to protect a single-use token, so it can
become a `run_onchange_` and stop carrying the replay hazard.

What this costs: one more step on a fresh machine, in the runbook that already carries the privacy
permission grants, the LuLu approvals and the Bluetooth pairing. What it buys: the apply path stops
touching this credential in any form, so there is no rendered body, no argument, and no environment
to reason about.

### Option P3: read the token out of the already-rendered pns configuration

The pns config is written before `run_after_` scripts run, so the script could lift the value from
`~/.config/pns/config.toml` and export it. That adds no new exposure, since the file exists for other
reasons, and it keeps pairing automatic. It also puts fragile TOML parsing in bash and couples the
moshi setup to pns's file layout. Recorded because it is the only fully automatic option that adds no
exposure; not recommended, on the grounds that the coupling is worse than a runbook line.

### What to do about the history already holding it

Two operator steps, both cheap: rotate the moshi token at its source and update the vault entry (the
pns config picks the new value up on the next apply), then delete the history rows with
`atuin search --delete "moshi-hook pair --token"`, which atuin performs without printing matches.
Rotate first; deleting history on a live credential protects nothing.

## Deliverable 3: fresh-machine recovery

Two independent chains have to come up on a new machine. The Tier 2 chain is what exists today, minus
the pieces this document retires. The Tier 1 chain does not exist yet and is gated on the homelab
deployment.

### Tier 2, the local chain

| Step | Needs | State after this design |
| --- | --- | --- |
| Xcode command line tools, Apple ID, iCloud sign-in | operator | unchanged |
| The vault database appears at its configured path | iCloud sync | its own row now, see Addition 1 |
| `brew install chezmoi`, `chezmoi init <repo>` | network | unchanged; the config reads no secret |
| Apply, `run_once_before_00` | network | installs Homebrew |
| Apply, `run_onchange_before_10` | Homebrew | installs the cask, which ships the client (M10) |
| Apply, first secret target | master password, once | first vault read, after every `run_before_` (M9) |
| Apply, `.config/chezmoi/key.txt` | vault entry | created by the `create_` target at 0600 (M4) |
| Apply, `.hermes/*` | the identity above | decrypted in the same run (M4) |
| Apply, `run_after_` scripts | nothing secret | moshi pairing no longer among them |
| After the apply | operator, once | pair moshi by hand, restart its service |

The `read-source-state` pre hook can go, and this document answers the boundary design's open question
about it with evidence rather than preference. Its only job is installing KeePassXC when it is missing.
The cask is declared in the package data and the package script is a `run_before_` script, so it
already installs before the first vault read (M9 and M10). On the truly fresh machine the
hook is useless anyway, because Homebrew does not exist yet when it runs, which is exactly what its own
comments describe. Removing it deletes `test/unit/install-password-manager-hook.sh` with it, which is
the correct treatment of a test for code that no longer exists.

Once the builtin switch lands, the chain gets shorter again: chezmoi needs no vault client at all, so a
fresh machine needs the database file and the master password and nothing else. The KeePassXC cask
stays declared because the operator wants the application.

### Tier 1, the brokered chain

Verified against Infisical's own documentation for the standalone Agent Proxy: under `connect`, each
agent authenticates as its own machine identity through Universal Auth, the wrapper downloads the root
certificate authority to `~/.infisical/agent-proxy/mitm-ca.pem` and points `SSL_CERT_FILE`,
`NODE_EXTRA_CA_CERTS`, `REQUESTS_CA_BUNDLE`, `CURL_CA_BUNDLE`, `GIT_SSL_CAINFO` and `DENO_CERT` at it,
sets `HTTPS_PROXY` and `HTTP_PROXY` to the proxy, and always keeps `localhost,127.0.0.1` in `NO_PROXY`.
The root private key never leaves the Infisical server; the proxy holds a short-lived intermediate.

Three consequences for fresh-machine recovery:

1. **Nothing is added to the system trust store.** The trust is per-process environment variables
   pointing at a file, which is the narrower grant and the better one. Keep it that way: a certificate
   authority
   trusted for arbitrary hosts, installed into the login keychain, would be trusted by every
   application on the machine, including the ones that never go near the proxy.
1. **The machine identity credential is Tier 2 by construction.** Universal Auth is a client identifier
   and a client secret, and they live on the laptop. The honest statement is the boundary design's:
   there is no arrangement here where an agent can start a brokered session and cannot read the
   credential that starts it, so that credential must be worth only brokered use. Recovery is therefore
   re-issue, never copy: one identity per machine, a lost machine's identity revoked server-side.
1. **Reachability is the failure mode, and it must stay loud.** With the home server unreachable, a
   brokered operation fails and no local credential copy is created. Ordinary local development, which
   is everything Tier 2, keeps working.

The unresolved piece is which process gets wrapped. `connect` sets those variables for the process tree
it launches, so the practical question is whether the harness itself is launched under the wrapper (one
wrap, everything inherits, including things that should not be proxied) or whether individual tools are
wrapped at their call sites (precise, and easy to forget). That is an open question below rather than a
decision made in the operator's sleep.

## Recommended design, consolidated

Three changes now, which need no vault reorganization and no homelab dependency:

1. The age identity becomes `dot_config/chezmoi/create_private_key.txt.tmpl`, and
   `.chezmoiscripts/run_before_05-restore-age-key.sh.tmpl` is deleted. The age runbook gains the
   ordering invariant and the failure-message mapping.
1. moshi pairing leaves the apply. The script drops the token and becomes `run_onchange_`; the
   fresh-machine runbook gains the four-line pairing step. The moshi token is rotated and the history
   rows are deleted.
1. The `read-source-state` pre hook is removed from `.chezmoi.toml.tmpl`, along with
   `.install-password-manager.sh` and its unit test.

After those three the repository contains no direct `keepassxc-cli` invocation and no credential in any
script body or argument. chezmoi itself still spawns the client under `cache-password`, which is what
the fourth change fixes:

4. The builtin switch, as a sequenced change of its own: list the real entry paths with
   `keepassxc-cli ls -R -f`, move any root-level entries into a group, rewrite all 43 call sites in the
   16 rendered files to `<group>/<title>`, verify each renders, then set `keepassxc.mode = "builtin"`.
   Not one line, and not to be attempted in the same change as anything above.

### Behaviors, and which of them are tests

The scope ruling matters here, so each behavior says what checks it. Most of this is chezmoi's
behavior, which this repository does not test.

Rehearsed tonight with dummy credentials, recorded in the appendix, and re-runnable by hand:

1. A fresh home plus a vault entry yields the identity at 0600 and a decrypted age target in one apply.
1. An existing identity file survives an apply byte for byte, and the vault is not read.
1. An identity that does not match the recipient fails every apply with a message naming the target.
1. A symlink at the identity path is never written through.
1. `run_before_` scripts run ahead of the first vault read.
1. `moshi-hook pair` takes the token from the environment and refuses when it has neither source.

Ours, and therefore suite tests:

1. The moshi setup script skips pairing when the host is already paired, and contains no credential.
   (unit, with a stubbed `moshi-hook` on `PATH`; the existing `moshi-hook-bounce-on-upgrade.sh` test is
   the shape to follow)
1. `test/unit/install-password-manager-hook.sh` is deleted with the hook it tests.

Not built, deliberately: a drill script that rehearses the fresh-machine apply end to end. The
rehearsal above is the evidence, `test/e2e/age-rotation-drill.sh` is the precedent for the one case
worth automating, and a slow apply-driven test would be deleted under the speed rule within a month.

### Failure modes

- **Encrypted target sorts before the identity.** Hard deadlock on a fresh machine: the apply aborts,
  the identity is never written, and repeating the apply repeats the failure. Recovery is the runbook's
  manual copy of the vault entry into `~/.config/chezmoi/key.txt`, followed by `chmod 600`. Prevention
  is the invariant, written in the runbook and in the template's own comment.
- **Wrong identity in place.** Every apply fails at the first encrypted target with
  `no identity matched any of the recipients`. The file is never overwritten, which is intentional, so
  the operator decides whether the file or the vault entry is wrong.
- **Symlink at the identity path.** Either a visible prompt or a silent conversion to a regular file
  holding the link target's content, depending on whether chezmoi has written that target before.
  Neither writes through the link. The silent case is the weaker one and is the price of dropping the
  script; it surfaces immediately as the wrong-identity failure above.
- **Vault locked or the operator cancels.** The render fails and the apply aborts. This must never be
  softened into a skip: a skipped secret target renders an empty credential into a configuration file.
- **Pairing token already consumed.** Under the recommendation this cannot fail an apply any more,
  because pairing is not in the apply. By hand it fails at the moshi API, which is the right place.
- **moshi token rotated but not re-applied.** pns keeps posting the old value and its pushes fail. The
  boundary's rotation record has to name both consumers of that entry, the pairing step and the pns
  configuration.

### Security notes

- Nothing in this document creates isolation, and none of it should be described as though it does.
  Every value discussed is Tier 2, readable by any process running as the operator. What changes is the
  number of places each value is written, how long it lives there, and whether it lands in a durable
  store that outlives the operation.
- Ranking the three placements by durability, since they are equal on access control: a rendered script
  body lives for the length of the apply, a child environment for the length of one call, and a command
  line for the length of one call plus however long the history store keeps it. The history residue is
  the only one of the three that is still readable months later, which is why the command-line form is
  the one worth removing first.
- `chezmoi apply -v` and `chezmoi diff` print rendered secret values for any target that differs. This
  is not a new capability, since those files are readable anyway, but it does mean a review command
  puts every changed secret into terminal scrollback and into any transcript capturing it.
- The measurements above used a dummy database and dummy values throughout. The one operator step that
  needs the real database, `keepassxc-cli ls -R -f`, prints entry paths and no values.

## Second pass: independent verification, three additions, and a note

### Verified against primary sources

1. **M1 holds, from chezmoi's own source.** At tag `v2.72.1`,
   `internal/cmd/keepassxctemplatefuncs.go` seeds the builtin cache from
   `db.Content.Root.Groups[0].Groups` and builds each key by appending `group.Name` and then
   `groupEntry.GetTitle()` to the accumulated path. Entries sitting directly in the root group are
   never visited, so in builtin mode they are unreachable, and an entry inside a group is reachable
   only as `<group>/<title>`. The cache-password and open modes build no map at all: they hand the
   entry string to `keepassxc-cli` and let it resolve. The correction at the top of this document is
   therefore documentation of chezmoi's behavior, not an artifact of the dummy database.
1. **The call-site count is exactly 43 across 16 files, and not one entry name carries a slash.**
   Recounted independently: 35 calls in 13 template files, plus 8 in three `modify_` templates
   (`modify_private_dot_claude.json`, `private_dot_codex/modify_private_config.toml`, and
   `Library/Application Support/Claude/modify_private_claude_desktop_config.json`). Those three have
   no `.tmpl` suffix, so a search restricted to `*.tmpl` misses them, which is worth knowing before
   anyone sizes the builtin switch from a quick grep. Three of the 16 files being the modify-templates
   that reconcile live configuration is also the part of that change to schedule most carefully.
1. **`create_` is spelled before `private_`.** chezmoi's source state attribute reference gives the
   create-file order as `create_`, `encrypted_`, `private_`, `readonly_`, `empty_`, `executable_`,
   `dot_`. So `create_private_key.txt.tmpl` is the correct source name for a 0600
   `~/.config/chezmoi/key.txt`, and M3's measurement and the documentation agree.
1. **The repository is in cache-password mode by omission.** `.chezmoi.toml.tmpl` sets
   `keepassxc.database` and no `keepassxc.mode`, and the documented default for that variable is
   `cache-password`. The builtin switch is an added line rather than a changed one, and nobody ever
   chose the current mode.
1. **`MOSHI_PAIRING_TOKEN` is real in the installed binary.** `moshi-hook pair --help` on 0.3.16 does
   not mention it, but the binary carries both the variable name and the refusal string
   `no pairing token provided (use --token or set MOSHI_PAIRING_TOKEN)`. So the environment path is
   supported input of the tool as shipped. Being absent from `--help` is worth a sentence wherever the
   runbook uses it, so that a later reader does not "simplify" it back to the flag.
1. **The cask does ship the client.** `brew info --cask keepassxc` lists
   `/Applications/KeePassXC.app/Contents/MacOS/keepassxc-cli` as a binary, and `keepassxc` is declared
   in the cask list of `.chezmoidata/system_packages_autoinstall.yaml`. M10 holds, and with it the
   argument for deleting the `read-source-state` hook.

Not re-verified, and flagged as such: M6, the ordering deadlock, rests on a measurement of how chezmoi
aborts an apply rather than on documented behavior. The invariant it argues for does not depend on the
abort semantics, though. An `encrypted_` target that sorts before the identity cannot decrypt on a
fresh machine either way; the only thing the abort changes is whether the rest of that first apply
also stops. Documenting the invariant is the right response to both readings.

### Addition 1: the vault database is itself a fresh-machine dependency, and it arrives over iCloud

`[keepassxc].database` in `.chezmoi.toml.tmpl` points at
`~/Library/Mobile Documents/iCloud~com~strongbox/Documents/keepass.kdbx`. That is inside Strongbox's
iCloud container, so on a fresh machine the file does not exist until the operator signs into iCloud
and that container syncs. The original table folded this into one operator line; it deserves its own
row, because it is the single prerequisite that waits on a third party's sync rather than on a command
the operator can run.

Measured: the path lists today as a 2153231-byte 0600 regular file. That does not prove it is
materialized, because `ls` reports the logical size of an evicted iCloud file too, and materialization
was not checked. From training and not verified here: macOS materializes an evicted iCloud file on
first read, so a first apply on a freshly synced machine can block on that download and will fail
rather than wait if the network is gone. The
safe fresh-machine sequence is to confirm the path reads before starting the first apply. The fallback
worth one line in the quickstart is to copy the database from another machine or from a backup and let
iCloud reconcile afterwards, which also removes the sync wait from the critical path entirely.

This does not change any recommendation above. It closes a gap in the recovery chain: every other step
in that table was verified to work, and this one was assumed.

### Addition 2: a stale database artifact sits beside the live one

The same directory holds `keepass.yKLDVO`, 1834507 bytes, mode 0600, last modified 2025-06-26. It was
not opened, read, or copied. From its name it is a Strongbox working file. Whatever it is, it is a
year-old artifact that is either a full copy of the vault or a fragment of one, and it is 85 percent of
the live database's size.

Two consequences, both belonging to "current exposure paths", which is this task. A fresh-machine
restore done by hand could pick the wrong file and silently run on a year-old vault. And any credential
rotated since June 2025 may still be recoverable from that file by anything that can read the
directory, which is every process running as the operator. One operator check, not a design change:
identify it, then keep it deliberately or move it to the backups location with the dated naming
convention.

### Addition 3: one entry, two roles, and the rotation hazard that follows

Both consumers read the SAME entry name. `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl`
and `dot_config/pns/private_config.toml.tmpl` both name `moshi-hook :: Device Token`. The setup
script's comment says the value "is the credential `moshi-hook pair` consumes, and it is NOT the
webhook secret the mobile plugin posts with, which is a separate entry and a separate rotation". The
pns template's comment says "Pair with moshi and put the webhook secret it issues here". Those cannot
both describe one entry.

Four facts settle most of it. `moshi-hook pair --help` says pairing consumes "a pairing token from the
Moshi iPhone app (Settings -> Integrations)" and stores "the token, host ID, and host secret" in the
configured secret store, so pairing has three outputs and its input is only one of them. pns posts its
value repeatedly, as `{"token": ..., "title": ..., "message": ...}` to
`https://api.getmoshi.app/api/webhook` (`pns/crates/pns-adapters/src/destinations/moshi.rs`). The
pairing token is single-use for registration: the setup script carries a live observation of a replayed
one failing with "Invalid pairing token". And both paths work today on that one entry.

The reading those four support is one value with two roles: the app's device token registers a host
exactly once and remains a valid webhook credential indefinitely. That makes the setup script's
"separate entry and a separate rotation" comment the stale half, and it makes rotation a coupled
two-step rather than one: rotating the device token at the app invalidates pns's pushes AND requires
re-pairing every host. The recommendation above already rotates this value, so this matters to it
directly.

Cheap confirmation that reads no value: re-pair one host, then fire one test push and see whether it
lands. Whichever way it comes out, one of the two comments gets corrected in the same change.

### A closing note: where pairing writes its outputs

`pair` stores into the macOS login Keychain by default, with `--store file` documented "for headless
sessions where Keychain is unavailable". Two consequences for recovery. The pairing result is not
restorable from the vault or from this repository, which is the independent reason re-pairing is the
only fresh-machine answer, and it holds whether or not pairing stays in the apply. And a first setup
driven over SSH, where no login keychain is unlocked, needs `--store file` explicitly, which is worth
naming in the quickstart step rather than discovering at 2am on a new machine.

## Out of scope

- The boundary itself, the tier assignment of the 24 entries, and the register. That is the previous
  document.
- Deploying, licensing or networking Infisical. That is homelab A6.
- The builtin switch as an implementation. This document sizes it and sequences it; the call-site
  rewrite is its own change with its own verification.
- kpxc-cli, Passage, and the NetBird cutover.
- Rotating anything other than the moshi token, whose exposure is measured here rather than assumed.
- Any change to the operator-runs-applies rule.
- Patching or forking chezmoi, KeePassXC or moshi-hook. Every mechanism above is a documented input of
  the tool as shipped.

## Assumptions made in your place

1. **The `create_` attribute is the right expression of validate-never-clobber.** *Alternative:* keep
   the script and accept the second vault client call site, which is the only way to keep its loud
   warnings and its explicit symlink refusal.
1. **The validator is not worth rebuilding.** Detection is delegated to age's own failure on the next
   apply. *Alternative:* a small secret-free `run_after_` script that derives the recipient with
   `age-keygen -y` and warns with a runbook pointer, at the cost of a script that exists only to
   improve one error message.
1. **Pairing moves to the runbook.** *Alternative:* option P1, one line, keeps pairing automatic and
   keeps the value in the rendered script body; or option P3, automatic with no new exposure and a
   coupling to pns's file layout.
1. **The moshi token is treated as exposed and rotated.** *Alternative:* leave it, on the grounds that
   everything in Tier 2 is exposed anyway and one more store changes nothing.
1. **The `read-source-state` hook is removed rather than repointed.** *Alternative:* move it outside
   the source tree and keep a best-effort installer that the measurements say is already redundant.
1. **The ordering invariant is documented, not enforced.** *Alternative:* a check that fails when an
   `encrypted_` source file would sort before the identity target, which is a tool this repository
   would own and could test, at the cost of another gate to maintain.
1. **Tier 1's certificate authority trust stays per-process.** *Alternative:* install it into the login
   keychain, which is more convenient and trusts it for every application on the machine.
1. **The Infisical machine identity is a per-machine vault entry rendered to a 0600 file.**
   *Alternative:* the operator supplies it interactively at each session start, which is stronger and
   collides with unattended overnight work.
1. **`moshi-hook :: Device Token` is one value with two roles**, so the rotation step in the
   recommendation is written as a coupled two-step (rotate at the app, re-pair, re-apply). This is an
   inference from four measured facts, not a confirmation. *Alternative:* they really are two values
   and one of the two consumers has been reading the wrong entry, in which case the rotation step
   splits and one of the two comments is a correctness fix rather than a documentation fix. Addition 3
   gives the one-command confirmation.
1. **The stale `keepass.yKLDVO` artifact is surfaced, not touched.** Nothing in this design moves,
   opens or deletes it. *Alternative:* treat it as in-scope cleanup now, which means deciding what it
   is, and deletion of a possible vault copy is an operator action under the destructive-action rule
   regardless.

## Open questions

1. **Do you approve the three floor changes** (age identity as a `create_` target with the script
   deleted, pairing out of the apply, hook removed)? They are independent of the homelab and of the
   builtin switch, and each is small enough to be its own pull request.
1. **Pairing in the runbook, or one line in the script?** P2 is the recommendation; P1 is the answer
   if you want the fresh machine to stay one command.
1. **Rotate the moshi token now?** The history rows are measured, not hypothetical. Rotation touches the
   vault entry, the pns configuration on the next apply, and the pairing itself.
1. **Is `moshi-hook :: Device Token` one credential or two?** Measured in the second pass: both
   consumers read that one entry name, and both paths work today, so the likely answer is one value
   with two roles and the setup script's "separate entry" comment is the stale half. Confirm it, then
   fix whichever comment is wrong, because the rotation step in this document depends on knowing
   whether rotating that value also breaks pns's pushes. See Addition 3.
1. **Where does the builtin switch sit in the queue?** It is the change that closes the substituted
   client path, and it needs a vault reorganization you have to do by hand.
1. **Which process is wrapped by `connect`?** The harness itself, so everything inherits proxy routing
   and certificate trust, or individual tools at their call sites.
1. **Do you want the ordering invariant enforced by a check, or documented?** Documented is the
   recommendation while all encrypted targets live under one directory.
1. **What is `keepass.yKLDVO`, and does it stay?** A year-old, 0600, 1.8 MB file beside the live
   database. Keeping it deliberately is a fine answer; not knowing what it is, is not. If it goes, it
   goes to `~/workspaces/backups/` under the dated naming convention, and the move is yours.
1. **Should the quickstart carry the copy-the-database-by-hand fallback?** It removes an iCloud sync
   wait from the critical path of a fresh machine, at the cost of one more way to end up running on a
   stale vault copy, which Addition 2 shows is not hypothetical here.

## Appendix: the commands behind each measurement

All of these ran in a scratch directory with dummy values. They are here so the measurements can be
re-run rather than trusted.

    # M1, M2: builtin versus cache-password entry resolution, on a dummy database
    keepassxc-cli db-create -p dummy.kdbx
    keepassxc-cli mkdir dummy.kdbx Grp
    keepassxc-cli add -p dummy.kdbx "Grp/Dummy :: Grouped Entry"
    chezmoi --config <throwaway>.toml --no-tty execute-template '{{ (keepassxc "<name>").Password }}'
    keepassxc-cli ls -R -f dummy.kdbx          # the secret-free path listing

    # M3: attribute order
    # source files create_private_dot_alpha and private_create_dot_beta, then:
    chezmoi --config <throwaway>.toml --destination <home> managed

    # M4 through M8: the bootstrap rehearsals
    age-keygen -o id.txt                        # dummy identity, stored in the dummy database
    age -r <recipient> -o src/encrypted_dot_zsecret.age   # dummy payload
    chezmoi --config <throwaway>.toml --destination <home> --no-tty apply

    # M9: script ordering
    # .chezmoiscripts/run_before_01-marker.sh and run_after_99-marker.sh printing markers

    # M11: the pairing token environment variable, against an unroutable base URL
    env -i ... MOSHI_PAIRING_TOKEN=dummy moshi-hook pair --base-url http://127.0.0.1:1
    env -i ... moshi-hook pair --base-url http://127.0.0.1:1

    # M13: history residue, counted and never printed
    atuin search --limit 1000 "moshi-hook pair --token" | wc -l

Second pass, the verification commands. All read-only, no vault opened, no value printed.

    # chezmoi's builtin lookup, at the installed tag
    # https://raw.githubusercontent.com/twpayne/chezmoi/v2.72.1/internal/cmd/keepassxctemplatefuncs.go
    # attribute order and the keepassxc.mode default
    # https://www.chezmoi.io/reference/source-state-attributes/
    # https://www.chezmoi.io/reference/configuration-file/variables/

    # the 43 call sites in 16 files: 35 in templates, 8 in the three modify_ files
    grep -rn 'keepassxc\(Attribute\)\? \{1,\}"' --include='*.tmpl' . | grep -v -e '^docs/' -e '^test/'
    grep -rIln keepassxc . | grep -v -e '^docs/' -e '^test/' -e '^\.git/'

    # the environment variable, without running pair
    strings "$(command -v moshi-hook)" | grep -o 'MOSHI_[A-Z_]*' | sort -u
    strings "$(command -v moshi-hook)" | grep 'no pairing token provided'
    moshi-hook pair --help

    # the cask ships the client
    brew info --cask keepassxc | grep Binary

    # the database path and the stale artifact beside it, listed and never opened
    ls -l "$HOME/Library/Mobile Documents/iCloud~com~strongbox/Documents/"

    # one entry, two consumers
    grep -o 'keepassxc "[^"]*"' .chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl
    grep -n 'keepassxc' dot_config/pns/private_config.toml.tmpl
