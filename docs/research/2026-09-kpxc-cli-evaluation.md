# kpxc-cli evaluation, 2026-09-14

Verdict document for the ledger item "Evaluate kpxc-cli as an unadopted third-party candidate" under
**Safe agent credentials and Infisical client integration** in `docs/remaining-work.md`.

## Verdict

**Reject as a replacement for the current chezmoi KeePassXC integration. Defer as an optional operator
convenience, pending the first two open questions below.**

It cannot replace the integration, for a reason that is structural rather than a rough edge: the
KeePassXC browser protocol looks entries up by uniform resource locator (URL) host only, and all 23 entry
titles this repository reads are naming-convention titles (`Anthropic :: Auth Token`,
`OpenHue :: API Key (hue-bridge-pro)`), not hostnames. There is no protocol field for a title, in either
direction, so the migration is not a config change but a re-keying of the vault plus a rewrite of 43
template actions, and the re-keying itself degrades the approval granularity described below.

Separately, and more importantly for the section this task sits in: kpxc-cli provides **no isolation
whatsoever** from an agent running as the operator. Its authentication material is a plaintext 0600 file
in `$HOME`, its transport is a socket in the user's own temporary directory, and its approval state is a
one-time remembered grant per entry. Every one of those is reachable by any process running as the
operator, which is exactly the trust boundary an agent already sits inside. On the isolation question it
is neutral at best, and it adds a second credential-bearing path to defend.

## Assumptions made in the operator's absence

Each of these was a choice I made in your place. The alternative is named so you can overrule it.

1. **I did not install kpxc-cli, and I did not run `kpxc-cli setup` against the live vault.** Every
   finding below is read from the upstream sources of kpxc-cli, of its `keepassxc-browser-api`
   dependency, and of KeePassXC 2.7.12 itself, cross-checked against the installed KeePassXC command-line
   interface. The reason: `setup` performs a permanent named key exchange that writes a new association
   key into the `keepass.kdbx` your iPhone also syncs, and the whole point of the surrounding section is
   that agents do not get to create credential paths on their own. *Alternative:* you run the five
   command sequence under "What would change the verdict" yourself against a throwaway database, which
   converts findings 1 through 4 from source-read to measured.
1. **I read "can it replace the current integration" as all 43 call sites, not a subset.** A partial
   answer (kpxc-cli for a handful of new agent-scoped secrets, `keepassxc-cli` for the existing ones) is
   technically possible through a different chezmoi function and is written up below, but I did not treat
   it as satisfying the task. *Alternative:* you accept a two-store split as the target, in which case
   the verdict on the narrow question becomes "adopt for new agent secrets only" and the open questions
   change.
1. **I treated the pain kpxc-cli would relieve as "one master-password typing per `chezmoi apply`".** I
   could not find a recorded statement of what problem it was being considered for, so I inferred it from
   what the tool uniquely offers (Touch ID unlock) against the current integration's cost. *Alternative:*
   if the real motive was unattended agent applies, say so, because kpxc-cli does not deliver that and
   the verdict below is then not even close.

## What was checked, and how

Versions, measured on dresden on 2026-09-13:

| Thing                    | Version                      | How                       |
| ------------------------ | ---------------------------- | ------------------------- |
| chezmoi                  | `v2.72.1` (Homebrew)         | `chezmoi --version`       |
| official `keepassxc-cli` | `2.7.12`, at `/opt/homebrew` | `keepassxc-cli --version` |
| KeePassXC application    | at `/Applications`           | `ls /Applications`        |
| kpxc-cli                 | **not installed**            | `which kpxc kpxc-cli`     |

Upstream sources read (fetched raw, not summarized):

- `mietzen/keepassxc-cli` at `main`: `README.md`, `pyproject.toml`, `keepassxc_cli/commands/setup.py`,
  `show.py`, `exit_codes.py`.
- `mietzen/keepassxc-browser-api` at `main`: `PROTOCOL.md`, `keepassxc_browser_api/config.py`,
  `keepassxc_browser_api/client.py`.
- `keepassxreboot/keepassxc` at tag `2.7.12`: `src/browser/BrowserService.cpp`,
  `src/browser/BrowserSettings.h`, `src/browser/BrowserSettings.cpp`,
  `src/browser/BrowserSettingsWidget.ui`, `src/gui/dbsettings/DatabaseSettingsWidgetBrowser.ui` and
  `.cpp`, `src/touchid/TouchID.h`.
- `twpayne/chezmoi` at tag `v2.72.1`: `internal/cmd/keepassxctemplatefuncs.go`, `internal/cmd/config.go`,
  and the shipped documentation for the `keepassxc*` and `secret*` template functions.
- GitHub application programming interface for repository and release metadata on both `mietzen` repos.

This repository's own surface, counted rather than estimated:

```
16 source files hold a keepassxc template action
43 template actions total: 29 .Password, 11 .UserName, 3 keepassxcAttribute
23 distinct entry titles, of which 0 are a hostname or a URL
 1 further entry read outside template lookup, by keepassxc-cli directly:
   "chezmoi :: Private Key :: age" in .chezmoiscripts/run_before_05-restore-age-key.sh.tmpl
```

`.chezmoi.toml.tmpl` sets only `[keepassxc] database`, so the rest of the block takes chezmoi's defaults,
set at `internal/cmd/config.go:3245`: the command is `keepassxc-cli`, `prompt` is true, and `mode` is
`cache-password`. That default means one master-password prompt per chezmoi process, cached in memory for
that process's lifetime.

## Findings

### 1. Entry approval: a one-time remembered grant, not a per-read approval

This is the finding most likely to be misread, so it is first.

`kpxc-cli show <url>` issues the protocol action `get-logins`. In KeePassXC 2.7.12, `findEntries` walks
every entry whose URL matches and calls `checkAccess`, which returns one of `Allowed`, `Denied` or
`Unknown` (`BrowserService.cpp:1304`). The state lives in the entry's own custom data, keyed by the
association name plus the site host (`allowEntry` and `denyEntry`, `BrowserService.cpp:1220` onward).

- `Unknown` and the global setting off: a graphical user interface (GUI) dialog appears
  (`confirmEntries`, `BrowserService.cpp:461`). The dialog carries a **"Remember this decision"**
  checkbox. If it is checked and the operator accepts, `allowEntry` persists `Allowed` for that entry and
  host pair, and **every later read of that entry is served silently, with no prompt**.
- `Unknown` and the global setting on: served silently, no dialog. The setting is **"Never ask before
  accessing credentials"** (`alwaysAllowAccess`, `BrowserSettingsWidget.ui:283`), under Settings, Browser
  Integration, Advanced.
- `Allowed`: served silently.
- `Denied`: skipped.

Three consequences worth stating plainly:

- **The prompt is a pairing-time ceremony, not a runtime gate.** An agent that can run `kpxc-cli` gets
  the secret with no human in the loop for every entry already remembered. "It prompts" is true exactly
  once per entry.
- **The global escape hatch is not scoped to one client.** `alwaysAllowAccess` is a single application
  preference read from `BrowserSettings`, with no per-association variant. Turning it on so a headless
  caller is not blocked also removes the prompt for the actual browser extension.
- **There is no concurrency queue.** `confirmEntries` returns empty immediately when `m_dialogActive` is
  already set, so a second caller arriving while a dialog is open gets nothing rather than a second
  dialog. Two agents and a browser racing produce silent empty results, not errors.

The one case that always confirms is HTTP Basic Auth (`if (!ignoreHttpAuth && entryParameters.httpAuth)`,
`BrowserService.cpp:405`). kpxc-cli's `show` calls `client.get_logins(url)` with no `http_auth` argument
(`show.py:34`), so that path never applies to it.

### 2. Association key protection: filesystem permissions only, no isolation

`kpxc-cli setup` generates an X25519 keypair for message encryption plus a second identity keypair, gets
the association approved and named in the GUI, and writes all of it to `~/.keepassxc/browser-api.json`.
The dataclass is explicit about what lands there (`keepassxc_browser_api/config.py`):

```python
class Association:
    id: str
    id_key: str  # base64-encoded identity public key
    key: str     # base64-encoded identity secret key
```

plus `client_public_key` and `client_secret_key` on the enclosing `BrowserConfig`. So the file holds
**both secret keys in base64 plaintext**. `save()` writes through a 0600 temporary file and renames;
`load()` warns, but proceeds, when the mode is looser than 0600.

That is the whole of the protection. Specifically:

- No macOS keychain, no Secure Enclave, no hardware binding, no passphrase on the key file.
- Any process running as the operator can read the file and replay the association. That includes every
  agent harness, every `run_` script, and anything an agent writes and executes.
- The transport is no stronger. The socket path on macOS is
  `tempfile.gettempdir() + "/org.keepassxc.KeePassXC.BrowserServer"` (`client.py:28`), which resolves
  through `TMPDIR` to the operator's private per-user temporary directory. Private to the user, and the
  agent is the user.
- The same file is shared with `keepassxc-ssh-agent` by design (`config.py` docstring, and the kpxc-cli
  README: "shared with `keepassxc-ssh-agent` if installed"). One stolen association covers both tools.
- The association authorizes **writes**. `set-login` creates and updates entries and `delete-entry`
  removes them, and kpxc-cli exposes both as `add`, `edit` and `rm`. An association key an agent can read
  is an association key an agent can use to rewrite or delete vault entries, subject only to
  `alwaysAllowUpdate` ("Never ask before updating credentials") for the update path.

Compare the current integration honestly, because it is not obviously better: chezmoi's default mode
prompts for the master password and holds it in memory for the process. A process running as the operator
could read that memory too. The difference is duration and blast radius, not kind. The current path holds
a password for the seconds of one apply; a kpxc-cli association is a durable on-disk credential that
survives reboots and grants read and write until revoked by hand.

### 3. Revocation: available, GUI only, and it does not disarm the client

Revocation is real and it works at the database level. KeePassXC 2.7.12's Database Settings, Browser
Integration page (`DatabaseSettingsWidgetBrowser.ui` and `.cpp`) offers exactly three levers, wired at
`.cpp:50-55`:

- **"Remove selected key"**, from the "Stored browser keys" list (`removeSelectedKey()`): drops one
  association's key from the database metadata custom data.
- **"Disconnect all browsers"** (`removeSharedEncryptionKeys()`): drops every stored association key.
- **"Forget all site-specific settings on entries"** (`removeStoredPermissions()`): clears the per-entry
  allow and deny state written by `allowEntry` and `denyEntry`.

After a key removal, `test-associate` fails and the client reports that all stored associations are
invalid (`client.py:198`). Properties that matter for a security design:

- **It is a database edit.** It has to be saved, and it propagates to your other devices only through the
  iCloud and Strongbox sync of the `.kdbx`. Revocation is not instant across machines.
- **It is GUI only.** There is no command for it in the official command-line interface, and none in
  kpxc-cli. A revocation cannot be scripted, staged in this repository, or included in a fresh-machine
  runbook as a command.
- **It does not disarm the client.** `~/.keepassxc/browser-api.json` still holds its keys after
  revocation; nothing wipes it. Re-associating is one `kpxc-cli setup` and one GUI approval away, which
  is the correct security posture (a human at the dialog) but means revocation is not durable against a
  compromised agent that can also nag the operator into clicking Allow.
- **Association identity is operator-chosen at the dialog, not client-declared.** The `associate` action
  sends only the two public keys; KeePassXC returns the `id` the operator typed (`client.py:345-395`,
  `PROTOCOL.md` under `associate`). So per-tool identities are possible, but only by giving each tool its
  own config file through `--browser-api-config` and running a separate key exchange per tool, and
  nothing stops a tool from pointing at another tool's config file.

### 4. Locked-database behavior: clean, well-signposted, and requires a running application

This is the part of kpxc-cli that is genuinely well built.

- KeePassXC must be **running** with the database **open**. The README lists this first under Known
  Limitations.
- With the database locked, the client sends `get-databasehash` with `triggerUnlock` set, which raises
  KeePassXC's unlock dialog, then polls until `unlock_timeout` (default **30 seconds**,
  `config.py: DEFAULT_UNLOCK_TIMEOUT = 30`) before raising `DatabaseLockedError` (`client.py:460-490`).
- Exit codes are stable and documented, and they distinguish the three failure modes a caller cares
  about: `2` KeePassXC not running or socket missing, `3` unlock timed out, `4` access denied by the
  operator (`commands/exit_codes.py`). `1` is a generic error including "entry not found".
- The unlock dialog is where Touch ID applies, through KeePassXC's own Quick Unlock, not through anything
  kpxc-cli implements. `src/touchid/TouchID.h` declares `storeKey(databasePath, passwordKey)` and
  `getKey(...)`: the master key is stored after a successful full unlock and retrieved under biometric
  authentication afterwards. **A prior full password unlock in the session is a precondition.** Touch ID
  never replaces the first unlock.

There is one note in the other direction. `findEntries` re-checks `isDatabaseOpened()` after the dialog
closes and returns empty if the database locked while the prompt was up (`BrowserService.cpp:438`), which
surfaces as exit `1`, "entry not found", rather than as a lock error. A caller that treats `1` as
"absent" and provisions a default would be wrong in that window.

### 5. Why it is not a drop-in for the current chezmoi integration

Four independent blockers. Any one of them is sufficient.

**a. chezmoi cannot drive it.** `keepassxc.command` is configurable, but the argument vector is not.
chezmoi builds `<command> show <keepassxc.args...> <database> --quiet --show-protected <entry>` for the
`keepassxc` function and `<command> show <database> <entry> --attributes <attr> --quiet --show-protected`
for `keepassxcAttribute` (`keepassxctemplatefuncs.go:111`, `:147`, `:176-180`), then parses the output as
key-value pairs. kpxc-cli takes no database argument (it talks to whatever database the running
application has open), has no `--quiet` or `--show-protected`, and takes a URL where chezmoi puts a
title. Pointing `keepassxc.command` at it produces an argument error, not a lookup.

**b. Lookup is by URL host, and nothing here is a URL.** `get-logins` matches on `QUrl(siteUrl).host()`
and `PROTOCOL.md` states it directly: "Only URL/hostname-based matching is supported, there is no way to
search by entry title or other fields through this action." Zero of the 23 titles in use is a hostname.
Note the asymmetry that makes this worse rather than better: `prepareEntry` *does* return the title as
`name` (`BrowserService.cpp:1265`), so titles come back in the response, but they cannot be searched on,
and kpxc-cli offers no filter over them. The official command-line interface, by contrast, has both
`show` by title and a `search` subcommand.

**c. Re-keying to URLs collapses approval granularity.** The natural workaround is a synthetic host per
secret (`https://anthropic-auth-token.local`). That is 23 new URL fields and a rewrite of 43 template
actions, and every write through the protocol re-derives the title from the hostname, so the
`Vendor :: Kind :: Detail` convention cannot survive a kpxc-cli `add` or `edit` at all. The cheaper
workaround, one shared host with client-side filtering on `name`, is worse: approval is stored per entry
*and host*, so a single shared host means the first Allow grants the agent every secret filed under it.
All-or-nothing, by construction.

**d. Custom attributes need a global setting and a vault-wide rename.** The three `keepassxcAttribute`
call sites (`org_id` and `test_user_id` on the Composio entry, `Public Signing Subkey ID` on the GitHub
signing key) would each need the attribute renamed to `KPH: <name>` and the application setting **Return
advanced string fields which start with "KPH: "** (`supportKphFields`) turned on. That is enforced server
side in `prepareEntry` (`BrowserService.cpp:1287-1298`); the client cannot override it. The official
command-line interface reads any attribute with no prefix and no setting.

Two further paths the ledger already flags stay unaddressed either way, because neither goes through
template lookup:

- `run_before_05-restore-age-key.sh.tmpl` calls
  `keepassxc-cli show -s -a Password "$keepass_db" "chezmoi :: Private Key :: age"` at execution time and
  streams it into a 0600 file. It does not use the template function, so replacing the template function
  does nothing here.
- `run_once_after_60-moshi-hook-setup.sh.tmpl` renders the moshi device token into
  `moshi-hook pair --token {{ $token | quote }}`, which puts the token in an argument vector visible in
  `ps` for the life of that process, and in the rendered script body on disk. Also untouched by a
  template-function swap.

**The supported bridge, for the record.** chezmoi's generic `secret` and `secretJSON` functions run
`secret.command` with `secret.args` and are independent of the `keepassxc*` functions, so
`secretJSON "show" "-j" "-p" "https://..."` against kpxc-cli would work, alongside the existing 43 calls,
with no argument-vector conflict. That is the only supported way to reach kpxc-cli from chezmoi. It does
not remove blockers (b), (c) or (d), and it adds a second unlock path per apply. It is listed as the
mechanism, not as a recommendation.

### 6. Touch ID and masked output are not evidence of isolation

The ledger item says not to treat these as proof of isolation. Both claims check out as written, and both
are cosmetic with respect to an agent.

**Masked output is a print-time choice over an already-fetched secret.** `show` calls
`client.get_logins(url)`, which returns the full entry including `password` and the time-based one-time
password, and only then does `print_entry_detail(entry, fmt, show_password=args.show_password)` decide
what to render (`show.py:34-39`). The secret is in the process's memory either way, and `-p` or
`show ... -j` prints it. An agent that can run the command can pass the flag.

**Touch ID authenticates the human at the unlock, once per unlocked session.** Per finding 4 it requires
a prior full password unlock, and after that the stored key answers biometric requests. It gates the
*unlock*, not the *read*. Once the database is unlocked and the entry remembered, reads are silent and
unauthenticated in every sense an agent cares about.

Set against the section's stated target, this is the crux. Track A6 in `webdavis/homelab`'s
`docs/plans/PLAN-v12.md` asks for agent identities that may "use the proxy without permission to read
secret values", with the proxy identity kept "outside the agent's execution boundary", and states
explicitly that "Runtime environment injection alone does not meet this requirement". kpxc-cli is the
opposite shape: it hands the plaintext value to a local process running as the operator, keeps its own
durable credential inside that same boundary, and offers no delegated-use mode at all.

One more gap relevant to the sibling ledger item on "audit records without secret values": there is no
audit trail. The setting **"Show a notification when credentials are requested"** exists
(`showNotification`, `BrowserSettingsWidget.ui:190`, default checked) and is read and written in
`BrowserSettings.cpp:48-56`, but no browser-path source I checked consumes it (`BrowserService.cpp`,
`BrowserAction.cpp`, `BrowserAccessControlDialog.cpp`, `NativeMessageInstaller.cpp` all have zero
references), and its declaration in `BrowserSettings.h:34` carries a `// TODO!!` comment. So in 2.7.12 a
silent, remembered read appears to leave no record at all. Treat that as "no consumer found in the four
files read", not as an exhaustive proof over the whole tree.

### 7. Maturity and supply chain

For something that would sit in the path of every secret in the vault, this is not a footnote.

| Signal         | kpxc-cli             | keepassxc-browser-api          |
| -------------- | -------------------- | ------------------------------ |
| Created        | 2026-04-12           | 2026-04-12                     |
| Last push      | 2026-05-22           | 2026-05-20                     |
| Latest release | `v3.0.0`, 2026-05-20 | (dependency, pinned `==1.5.0`) |
| Stars, forks   | 12, 0                | 1, 0                           |
| License        | MIT                  | MIT                            |
| Maintainers    | one                  | one, the same person           |

Roughly four months without a commit, one maintainer across both halves, twelve stars, three major
version bumps in five weeks. Installation would be `uv tool install keepassxc-cli` on this repository's
existing uv lane, which is the cheap path and needs no new Homebrew tap trust (the Homebrew route,
`mietzen/tap/keepassxc-cli`, would need an entry under `trusted_taps` in
`.chezmoidata/system_packages_autoinstall.yaml`). Note the PyPI distribution name is `keepassxc-cli`,
colliding with the official tool's binary name, with `kpxc-cli` as the entry point. It also hard-depends
on `pyperclip==1.8.0` for the clipboard command alone, so that pin arrives whether or not `clip` is ever
used.

## What would change the verdict

The structural finding, URL-only lookup, is upstream protocol behavior and will not change through
configuration. Concretely, the verdict flips only on one of these:

1. **KeePassXC adds title-based lookup to the browser protocol.** Watch `get-database-entries`, which
   returns every entry and exists only on `develop` (absent from `BrowserSettings.h` at `2.7.12`,
   confirmed: no `allowGetDatabaseEntriesRequest` in that file). If it ships behind a per-client
   permission, title filtering becomes possible client side, and blockers (b) and (c) weaken. It would
   still hand plaintext to a local process, so it does not touch the isolation finding.

1. **You accept URL-keyed entries as the vault's naming convention.** That is your call about your own
   vault, not a technical question. It costs 23 URL fields, a rewrite of 43 template actions, the loss of
   the `Vendor :: Kind :: Detail` titles on any protocol write, and the `KPH: ` renames.

1. **The motive turns out to be Touch ID convenience only, with isolation explicitly out of scope.** Then
   the honest framing is a supplementary path through `secret.command`, and the question becomes whether
   one fewer password typing per apply is worth a durable on-disk read-write vault credential. My
   recommendation stays no, on blast radius.

1. **The four behaviors measure differently than the sources read.** Everything above is source reading;
   I did not run the tool. If you want it measured, this is the sequence, against a **throwaway**
   database, never `keepass.kdbx`:

   ```bash
   uv tool install keepassxc-cli          # provides kpxc-cli, not keepassxc-cli
   # open the throwaway .kdbx in the KeePassXC application, then:
   kpxc-cli setup                          # name the association, then inspect the key file:
   ls -l ~/.keepassxc/browser-api.json && jq 'keys' ~/.keepassxc/browser-api.json
   kpxc-cli show https://test.example.com  # first read: expect the Allow dialog
   kpxc-cli show https://test.example.com  # second read: expect silence if Remember was checked
   kpxc-cli lock && kpxc-cli show https://test.example.com; echo "exit: $?"   # expect 3 after ~30s
   # revoke in Database Settings, Browser Integration, "Remove selected key", save, then:
   kpxc-cli status; echo "exit: $?"        # expect an invalid-association report
   ```

   A few minutes of work, and it converts findings 1 through 4 from source-read to measured.

## Open questions for the operator

1. **What problem was kpxc-cli under consideration for?** Touch ID convenience during applies, or agent
   credential isolation? The verdict is "reject" either way, but the follow-up differs completely: the
   first leads to the `secret.command` bridge question below, the second leads nowhere and the item
   should be closed against the A6 Infisical direction instead.
1. **Do you want a supplementary kpxc-cli path at all, through `secret.command`, for new agent-scoped
   secrets only?** This is the one decision I deliberately did not make for you. It is the only supported
   way to reach the tool from chezmoi, it leaves the existing 43 call sites untouched, and its cost is a
   durable on-disk read-write vault credential plus a second unlock path per apply. My recommendation is
   no.
1. **Should this item be ticked closed, or kept open as a watch on `get-database-entries`?** The
   evaluation is complete and the answer will not change without an upstream protocol change. I have not
   ticked it, because closing it is a disposition call and the item sits inside a section you are
   actively designing.
1. **Do the two non-template paths get their own ledger items now?** The age-key script's direct
   `keepassxc-cli` call and the moshi token in an argument vector are both named in the section's later
   items. Neither is affected by any decision about kpxc-cli, and both are real exposure today. They read
   like separate work rather than part of this evaluation.
1. **Is "no audit record of a remembered credential read" a finding you want pursued?** Finding 6 is
   based on four files; proving it needs either a read of the whole 2.7.12 tree or one live observation.
   It bears directly on the section's "audit records without secret values" requirement, and it applies
   to the browser extension you already use, not only to a hypothetical kpxc-cli.
