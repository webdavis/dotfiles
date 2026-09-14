# Quickfix, database and SSH client backlog verdicts, 2026-09-14

## The question

`docs/remaining-work.md` line 2006 asks for a disposition on five still-open task-tracker items that have
sat in the backlog without adjudication:

| Task tracker id    | Title                                                        |
| ------------------ | ------------------------------------------------------------ |
| `6ggcw5hwM9Gp7XQv` | Set up nvim-bqf (Neovim better quickfix)                     |
| `6ggcw5v939gHcFvv` | Set up vim-dadbod (Neovim database interface)                |
| `6ggcw63vFWchgRjv` | Set up vim-dadbod-ui (Neovim database UI)                    |
| `6ggf2WjFmgQqRqRM` | Try dblab (danvergara/dblab) database CLI                    |
| `6gfVJFgXvG9mJ96M` | PostgreSQL workstation setup (daily-use starting 2026-05-16) |
| `6ggcXcJ6jMcm25HM` | Add 'Host \*' SSH hardening block to private_dot_ssh/config  |

Each needs an adopt, defer or close verdict with the current lock state cited. The ledger phrase
"dblab/database-workflow" turned out to name one task, not two: no separate database-workflow item exists
in the tracker. The dblab task carries the whole "how do I work with a database at all" question, which
is the same question vim-dadbod-ui answers from inside Neovim, so the two are adjudicated against each
other below.

No code was written and nothing was installed. Every probe below is read-only.

## Verdict summary

- **nvim-bqf: adopt**, with one operator question open. A real native-quickfix workflow already exists in
  this configuration, and every prerequisite the plugin names is already installed.
- **vim-dadbod and vim-dadbod-ui: defer**, gated on a workload and on a credential decision. It is a
  four-plugin cluster plus a new treesitter parser, serving a two-row database.
- **dblab: defer** the declaration. It handles secrets better than dadbod does, but it faces the same
  missing workload, and the trial the task asks for costs nothing and commits nothing.
- **PostgreSQL client configuration: adopt in part, close the rest.** Two of its four items are real and
  one line each; the other two were already answered or are moot.
- **SSH `Host *` client hardening: close as written, re-file narrowed.** Both of the examples the task
  names are already the measured defaults of the installed client.

One finding belongs to no task and is the most consequential thing in this document: an undeclared
PostgreSQL server runs at every login on this machine with password-free superuser access for every local
process. It has its own section.

## What was checked, and how

Versions measured on dresden on 2026-09-14:

| Component      | Version                         | Measured with                 |
| -------------- | ------------------------------- | ----------------------------- |
| Neovim         | `0.12.5` (LuaJIT 2.1)           | `nvim --version`              |
| fzf            | `0.74.4`                        | `fzf --version`               |
| OpenSSH client | `10.0p2`, `/usr/bin/ssh`        | `ssh -V`                      |
| postgresql@17  | `17.11`, keg-only               | `brew list --versions`        |
| psql           | `17.11 (Homebrew)`              | `psql --version`              |
| pgvector       | `0.8.2`, in `open_brain`        | `psql -d open_brain -c '\dx'` |
| atuin          | `18.22.0`                       | `atuin --version`             |
| dblab          | `0.50.0`, homebrew-core, absent | `brew info dblab`             |

Sources consulted:

- The plugin lock, `dot_config/nvim/lazy-lock.json`, and its deployed copy at
  `~/.config/nvim/lazy-lock.json`. Both hold 93 plugins and are byte-identical (`diff` over `jq -S .`
  output reports no difference).
- A repository-wide `grep` for `bqf`, `dadbod` and `dblab` across every tracked file.
- `dot_config/nvim/lua/config/keymaps.lua`, `dot_config/nvim/lua/plugins/trouble.lua`, `.../hlslens.lua`,
  `.../snacks.lua`, `.../blink-cmp.lua`, `.../treesitter.lua`, `.../lsp.lua`.
- `private_dot_ssh/config` and its deployed copy, `/etc/ssh/ssh_config`, `~/.ssh/known_hosts`.
- `ssh_config(5)` as installed on this machine, for the precedence rule and for the exact semantics of
  `RequiredRSASize`, `IdentitiesOnly` and `HashKnownHosts`.
- `ssh -G` against a name that matches no block, for the effective client defaults, and `ssh -F` against
  a throwaway file, to test precedence by experiment rather than by reading.
- `.chezmoidata/system_packages_autoinstall.yaml`, `dot_bashrc.tmpl`,
  `dot_config/atuin/private_config.toml.tmpl`, `Library/LaunchAgents/`.
- `docs/research/2026-04-12-karl-davis-dotfiles-review.md` section 8, which the PostgreSQL task cites
  (the task's path says `docs/archive/research/...`; the file now lives at `docs/research/...`).
- Upstream `README.md` files for nvim-bqf, vim-dadbod-ui and dblab, and the GitHub API repository records
  for all four projects.

Upstream health, from the GitHub API on 2026-09-14. None is archived:

| Project       | Last push  | Stars | Open issues |
| ------------- | ---------- | ----- | ----------- |
| nvim-bqf      | 2026-04-02 | 2031  | 21          |
| vim-dadbod    | 2026-01-07 | 4444  | 58          |
| vim-dadbod-ui | 2026-06-19 | 2049  | 101         |
| dblab         | 2026-09-09 | 3231  | 12          |

## 1. nvim-bqf: adopt

### Lock state

Absent. `bqf` appears nowhere in the repository except the ledger line that asks this question, and it is
in neither the source lock nor the deployed lock. It has never been adjudicated: the 2026-09 Neovim
overhaul documents do not mention it.

### What is already in place

Three surfaces already read a quickfix list:

- The native window, via `:copen`.
- `trouble.nvim`, via `<leader>Xq` bound to `Trouble qflist toggle`, with `[q` and `]q` routing to
  Trouble when it is open and to `:cprev` / `:cnext` when it is not.
- `snacks.nvim`, via `<leader>sq` bound to `Snacks.picker.qflist()`.

On the face of it that is enough, and the lazy answer is to add nothing. What changes the answer is
`dot_config/nvim/lua/config/keymaps.lua`, which defines a `:ReviewLedger` command: it runs an awk program
over `~/.claude/pipeline/slices/findings-*.md`, loads the result with `vim.fn.setqflist` and ends with
`vim.cmd("copen")`. That is the review pipeline's findings register landing in the **native** quickfix
window, and it is a recurring workflow rather than a one-off.

nvim-bqf improves exactly that window: a persistent preview so scrolling the list previews each location
without leaving it, an fzf filter over the list, and sign-based filtering to cull rows. On a findings
register of a few dozen rows, the filter is the part that earns its place, and `:ReviewLedger!` already
exists precisely because the closed rows are noise.

### Cost

Every prerequisite is already installed. Upstream states "Neovim 0.6.1 or later, fzf (optional, 0.42.0
later), nvim-treesitter (optional)"; this machine has Neovim 0.12.5, fzf 0.74.4 and `nvim-treesitter` in
the lock. `nvim-hlslens` is already in the lock and is by the same author, with documented integration
between the two. So the change is one spec file under `dot_config/nvim/lua/plugins/`, lazy-loaded on
`ft = "qf"`, plus a lock line. No new system package, no new tap, no Mason entry.

Upstream documents no conflict with trouble.nvim, and there is no mechanical one: bqf attaches to the
`qf` filetype, which Trouble's own buffer is not.

### The alternative, stated

The zero-dependency alternative is to change `:ReviewLedger`'s final `vim.cmd("copen")` to
`vim.cmd("Trouble qflist open")` and add nothing at all. Trouble has its own preview. That is a one-word
diff against a new plugin, and under a pure shortest-diff rule it wins.

I am not taking it, for two reasons. Trouble's preview is a different interaction (a separate window
driven by its own list UI, not the native window the operator's command opens), and Trouble has no fuzzy
filter over the list, which is the specific gap on a long findings register. The standing ruling recorded
as "optimal over cheap" (2026-09-05) says not to pick a design because it is less work, so the cheaper
option is named here rather than chosen silently.

**Assumption made in the operator's place:** that the native quickfix window should remain the surface
`:ReviewLedger` opens. If it should not, the Trouble reroute is strictly better and bqf should be closed
rather than adopted. This is open question 1.

## 2. vim-dadbod and vim-dadbod-ui: defer

### Lock state

Both absent, from source and from the deployed lock, and absent from the whole repository.
`vim-dadbod-completion`, which the pair normally comes with, is likewise absent.

### Why this is not one plugin

Adopting dadbod-ui as a working tool means four pieces, not one:

1. `tpope/vim-dadbod`, the engine. Upstream lists it as required by the UI.
1. `kristijanhusak/vim-dadbod-ui`, the drawer.
1. `kristijanhusak/vim-dadbod-completion`, which upstream lists as optional but which is the whole point
   of editing SQL in Neovim rather than in a terminal user interface. It is an nvim-cmp source, and this
   configuration uses blink.cmp, not nvim-cmp. That is survivable: `blink.compat` is already in the lock
   and `dot_config/nvim/lua/plugins/blink-cmp.lua` already has a generic bridge, where a name added to
   `opts.sources.compat` is rewritten into a provider with `module = "blink.compat.source"`. So it is a
   two-line addition, not a rewrite.
1. A `sql` treesitter parser. There is none. `dot_config/nvim/lua/plugins/treesitter.lua` lists 53
   parsers and `sql` is not among them, `dot_config/nvim/lua/plugins/lsp.lua` declares 21 language
   servers and none is a SQL server, and the Mason tool list has no SQL linter or formatter. Neovim on
   this machine has no SQL language support at all today.

### Why it is deferred rather than adopted

The only database on the machine is `open_brain`, which holds one table, `thoughts`, with two rows
(details in the undeclared-server section below). A four-plugin cluster plus a new treesitter parser
serving two rows is scaffolding for later, which this repository's own rules forbid.

There is also a secret-handling decision the operator owes before this could land safely. Upstream
documents three ways to declare connections, and they are not equivalent under this repository's "never a
committed secret value" stance:

- `vim.g.dbs` as a table of literal connection URLs. A URL such as
  `postgres://postgres:mypassword@localhost:5432/my-dev-db` carries the password inline, so this form
  cannot be committed.
- `vim.g.dbs` with a **function** value that resolves the URL dynamically. Upstream's own example uses
  `function('s:resolve_production_url')`. This is the form that fits the existing `keepassxc` model, and
  it is the one I would propose.
- `$DBUI_URL` and `$DBUI_NAME` environment variables (names overridable through
  `g:db_ui_env_variable_url` and `g:db_ui_env_variable_name`).

A hazard to record whichever form is chosen: `:DBUIAddConnection`, and the `A` key in the drawer, write
the entered URL to a connections file under `g:db_ui_save_location`, default `~/.local/share/db_ui`. That
path is outside the repository, so no gitleaks gate sees it, and the URL it stores includes the password.
If this is ever adopted, that file needs a decision of its own.

**Assumption made in the operator's place:** that no database workload is imminent, inferred from the
`open_brain` row count and from the absence of any psql history on the machine. The alternative reading
is that the 2026-05-16 "daily use" date in the PostgreSQL task did arrive and the work simply happened
elsewhere, in which case this is adopt rather than defer. This is open question 2.

## 3. dblab: defer the declaration, and the trial is free

### Lock state

Absent from `.chezmoidata/system_packages_autoinstall.yaml` and not installed. It is in homebrew-core as
`dblab 0.50.0`, not in a third-party tap, so adoption would be one alphabetical line in the formulae list
with no `taps` entry and no `trusted_taps` entry. Homebrew analytics report 95 installs in the last 30
days and 1,074 in the last year, which is modest.

### Why it is the better-aligned of the two candidates

If a database workload does appear, dblab fits this repository's stated positions better than dadbod-ui
does, on three measured points from upstream's README:

- **Secrets go to the Keychain, not to a config file.** Upstream: "The connection parameters are saved to
  `$XDG_CONFIG_HOME/dblab/dblab.json` (excluding passwords), while the database password and SSH password
  (if provided) are stored in the OS keyring." Credentials placed in `.dblab.yaml` by hand are plaintext,
  so the `--save-as` profile path is the one to use and the YAML path is the one to avoid.
- **A read-only mode exists.** `--readonly` "prevent[s] accidental writes by forcing the database session
  into read-only mode", supported for PostgreSQL among others. That matches the position taken on the
  YNAB server in this same backlog, where the read-only default is to be preserved unless writes are
  explicitly requested.
- **Native SSH tunnel support.** `--ssh-host`, `--ssh-port`, `--ssh-user`, `--ssh-key`, `--ssh-key-pass`.
  The PostgreSQL task's closing note says "user mentioned a homelab PostgreSQL is also relevant", and a
  tunnel is how a workstation client reaches it.

### The gotcha to record before anyone runs it

Creating a profile means putting the password on a command line:
`dblab --host localhost --user myuser --db users --pass password ... --save-as myprofile`. This machine
records shell history through atuin 18.22.0 with `secrets_filter = true` and a `history_filter` regex
pulled from KeePassXC. Whether that regex covers a `--pass` argument is not knowable from the repository,
because the pattern lives in the vault. Check it before typing such a command, or type the command with a
leading space if the shell's history control honours that.

### Why defer rather than adopt

Same reason as dadbod: no workload. But the trial the task actually asks for ("Try dblab") costs nothing
and commits nothing: `brew install dblab`, point it at `open_brain`, look at it once, and either add the
formulae line or do not. That trial needs no ledger entry and no declaration, and it is the cheapest way
to close both this task and the dadbod pair for good.

**Assumption made in the operator's place:** that the trial should precede the declaration, rather than
declaring the formula now so the weekly bundle keeps it current. The alternative is to declare it
immediately, which is one line and reversible, at the cost of one more formula the bundle maintains for a
tool that may go unused. This is open question 3.

## 4. PostgreSQL client configuration: adopt in part, close the rest

### What the task asks for

Task `6gfVJFgXvG9mJ96M` records four items, verified against the source as of 2026-05-15. Each is
re-checked here against the source as of 2026-09-14.

**Item 1, confirm psql is on PATH: the concern was correct, and the gap is still open.** The task said
"may need PATH edit in bashrc if keg-only". It is keg-only. `brew --prefix postgresql@17` resolves to
`/opt/homebrew/opt/postgresql@17`, its `bin` holds a `psql` reporting version 17.11, and
`/opt/homebrew/bin/psql` does not exist. `dot_bashrc.tmpl` contains no PostgreSQL PATH entry, and no
shell profile in the repository mentions PostgreSQL. So **`psql` is not invocable by the operator
today**, four months after the stated daily-use date.

The fix follows an existing pattern exactly. `dot_bashrc.tmpl` already prepends three keg-only Homebrew
directories with its own helper:

```bash
path_prepend "${HOMEBREW_PREFIX}/opt/curl/bin"
path_prepend "${HOMEBREW_PREFIX}/opt/trash-cli/bin/"
path_prepend "${HOMEBREW_PREFIX}/opt/gnu-getopt/bin/"
```

One more line in that group is the whole change. Two notes for whoever writes it. First, the major
version is baked into the path, so a move to postgresql@18 means editing both `dot_bashrc.tmpl` and
`.chezmoidata/system_packages_autoinstall.yaml`, the same hand-sync class as the mdformat pin the
repository already documents; say so in a comment next to the line. Second, none of those three lines
sits inside a `{{ if eq .chezmoi.os "darwin" }}` guard, and `path_prepend` prints "Directory ... does not
exist." to stderr and returns 1 when the directory is missing. That block also runs for non-interactive
shells including `ssh host cmd`. A fourth line inherits exactly the behaviour the three existing ones
already have, so this is not a new problem, but do not add the line anywhere that would make it the first
one to fire on a Linux render.

**Item 2, create a psqlrc: still unwritten, and the cited source is thin.** No `dot_psqlrc` exists in the
source, no `~/.psqlrc` exists on the machine, and no `~/.psql_history` exists either, which is
independent evidence that psql has never been run from this home directory. The cited reference,
`docs/research/2026-04-12-karl-davis-dotfiles-review.md` section 8, is nine lines long, is labelled "LOW
VALUE but UNIQUE", and names three settings: `\timing on`, coloured prompts showing timestamp and
database name, and a pager of `less --chop-long-lines`. That is the whole specification. It is one small
file with no template and no secret, so it is cheap, but note that `bat 0.26.1` is installed and `pspg`,
the pager actually built for psql output, is not; the pager choice is a preference, not a finding.

**Item 3, shell setup for `PSQL_EDITOR` and `PSQL_PAGER`: close as redundant for the editor half.** The
installed `psql(1)` page states that `PSQL_EDITOR`, `EDITOR` and `VISUAL` "are examined in the order
listed; the first that is set is used". `dot_bashrc.tmpl` already exports `EDITOR="$(which nvim)"`, so
`PSQL_EDITOR` would be a second copy of the same value.

The pager half is a real gap, and it collapses into item 2 rather than into the shell. The same page
states that `PSQL_PAGER` and `PAGER` are examined in that order and that "if neither of them is set, the
default is to use `more` on most platforms". `dot_bashrc.tmpl` exports neither (it exports `MANPAGER`,
which psql does not read), so psql output on this machine would page through `more`. Fixing that belongs
inside the psqlrc, through `\pset pager` and `\setenv PAGER`, which keeps the setting next to the tool it
configures instead of in a shell file shared by everything.

**Item 4, a templated `~/.pgpass`: close as pointless against the current server.** The task itself
marked it "only if needed". It is not needed and would not work as intended, because
`/opt/homebrew/var/postgresql@17/pg_hba.conf` is Homebrew's default and every line is `trust`. A password
file has nothing to authenticate with while the server asks for no password. If the relevant server is
the homelab one rather than this local one, this item should be re-filed against that server, where
dblab's Keychain-backed profiles are the better answer anyway.

**Assumption made in the operator's place:** that the psqlrc should be a plain `dot_psqlrc` with no
chezmoi template, because it carries no secret. The alternative is a template, which would let it branch
on host or pull a value from the vault later. Plain file recommended; a template can be introduced the
day it has something to interpolate.

## 5. SSH `Host *` client hardening: close as written, re-file narrowed

### Lock state

`private_dot_ssh/config` has no `Host *` block. Its five blocks are `github github.com`,
`unprovisioned_yoshimo`, `yoshimo`, `unprovisioned_bob` and `bob`. The deployed `~/.ssh/config` is
byte-identical to the source. `/etc/ssh/ssh_config` contributes only `Host *` with `SendEnv LANG LC_*`,
and `/etc/ssh/ssh_config.d/` is empty.

### The task's own examples are already the defaults

The task body reads: "Add a 'Host \*' block with hardening defaults (e.g. modern ciphers/KEX, no agent
forwarding by default)". Measured with `ssh -G` against a name matching no block, on OpenSSH 10.0p2:

- `forwardagent no`. Already the default. Also `forwardx11 no` and `forwardx11trusted no`.
- Ciphers are `chacha20-poly1305@openssh.com`, `aes128-gcm@openssh.com`, `aes256-gcm@openssh.com`,
  `aes128-ctr`, `aes192-ctr`, `aes256-ctr`. Every entry is either authenticated encryption or counter
  mode. No cipher-block-chaining modes, no arcfour, no 3DES. Nothing to remove.
- Key exchange leads with `mlkem768x25519-sha256` and `sntrup761x25519-sha512`, both post-quantum
  hybrids, then the curve25519 pair. Restricting this list would remove post-quantum protection, not add
  it.
- Host key and public key algorithm lists contain no `ssh-rsa` and no `ssh-dss`; the RSA entries are
  `rsa-sha2-512` and `rsa-sha2-256` only.

So both examples the task names are satisfied by the software already installed. A `Host *` block written
from a generic hardening guide would restate defaults, which is a file that looks like security work and
is not, and which then has to be re-audited on every OpenSSH release.

### The four settings that would be real changes

Only four commonly recommended settings are genuine deltas here, and three of them break something on
this machine.

**`HashKnownHosts yes` (default `no`): safe, modest, recommended.** `~/.ssh/known_hosts` holds 172 lines
of which 4 are hashed and 168 are plaintext host names, so the file is a readable inventory of every host
reached from this machine. `ssh_config(5)` on this machine warns that the setting is forward-only:
"existing names and addresses in known hosts files will not be converted automatically, but may be
manually hashed using ssh-keygen(1)". So the setting alone fixes nothing already recorded; closing the
existing 168 needs a deliberate `ssh-keygen -H` pass, which rewrites the file and leaves a `.old` copy
that must then be removed, and which makes the file unreadable to a human forever after. Recommended as a
setting; the retro-hash pass is the operator's call.

**`RequiredRSASize 3072` (default `1024`): would silently disable a key in use.** `ssh_config(5)`: "User
authentication keys smaller than this limit will be ignored. Servers that present host keys smaller than
this limit will cause the connection to be terminated." `ssh-add -l` shows five keys in the running agent
and one of them is `2048 SHA256:rSwxde9/... steve@justdavis-ansible-steve.pem (RSA)`. Raising the floor
to 3072 makes ssh ignore that key. Whatever it authenticates to would start failing, and the failure mode
is a key silently not offered, which is the hardest kind to diagnose.

**`IdentitiesOnly yes` (default `no`): would break GitHub and both homelab hosts.** `ssh_config(5)`: ssh
"should only use the configured authentication identity and certificate files (either the default files,
or those explicitly configured in the ssh_config files or passed on the ssh(1) command-line), even if
ssh-agent(1) [...] offers more identities." Two measurements make this fatal here. `ssh -G` lists the
default identity candidates as `~/.ssh/id_rsa`, `id_ecdsa`, `id_ecdsa_sk`, `id_ed25519`, `id_ed25519_sk`
and `id_xmss`. But the keys actually present in `~/.ssh` are `id_ed25519`,
`id_ed25519_webdavis_on_github`, `id_dresden_to_yoshimo` and `homelab_bootstrap_ed25519`, and no block in
`private_dot_ssh/config` declares an `IdentityFile` at all. So GitHub, yoshimo and bob authenticate today
purely because the agent offers those non-default names. `IdentitiesOnly yes` stops that. It becomes safe
only if every block first gains an explicit `IdentityFile`, which is a worthwhile change on its own and
is a different task.

**`PasswordAuthentication no` (default `yes`): would put Pi provisioning at risk, recoverably.** The two
`unprovisioned_*` blocks target `User pi` on port 22, which is the shape of a first-boot login before any
key is in place. Whether password auth is still the live path is not certain from here:
`~/.ssh/homelab_bootstrap_ed25519` exists and the running agent holds a key commented `pi-bootstrap`, so
a fresh image may already be reachable by key. But a genuinely factory-fresh image is not, and a global
`no` removes the fallback.

The recovery is precedence, and it was tested rather than assumed. `ssh_config(5)` states "Since the
first obtained value for each parameter is used, more host-specific declarations should be given near the
beginning of the file, and general defaults at the end." That was then verified by experiment against a
throwaway config file rather than taken on the page's word.

The `earlyhost` block set `PasswordAuthentication yes` and `MACs hmac-sha2-512` and nothing else; the
trailing `Host *` block set `PasswordAuthentication no`, `HashKnownHosts yes` and a different `MACs`
list. `ssh -F <file> -G <name>` then reported:

```
--- earlyhost (specific block first, Host * last):
hashknownhosts yes
passwordauthentication yes
macs hmac-sha2-512
--- otherhost (only Host * applies):
hashknownhosts yes
passwordauthentication no
macs hmac-sha2-256-etm@openssh.com
```

The earlier block won for the two keywords it set, and inherited `HashKnownHosts` from the trailing block
because it set nothing for it. So a `Host *` block is safe **only at the end of the file**, and
`PasswordAuthentication no` there is safe only once both `unprovisioned_*` blocks carry an explicit
`PasswordAuthentication yes`. Placing the block at the top, which is how most hardening snippets are
pasted in, would also shadow the deliberate `ServerAliveInterval 20` on the GitHub block the moment
anyone added a keepalive setting to it, and that block's long comment explains exactly why that value is
load-bearing.

**One more candidate the task did not name.** `MACs` still includes `hmac-sha1-etm@openssh.com` and
`hmac-sha1`, plus the non-encrypt-then-authenticate variants. Dropping the SHA-1 message authentication
codes is a defensible tightening, and it is the only algorithm list on this machine with anything left to
remove. It is also the one change with real interoperability risk against old servers, so it wants
testing against GitHub and both Pis before it ships.

### Verdict

Close `6ggcXcJ6jMcm25HM` as written, because what it asks for is already true, and re-file a narrow
successor: add a trailing `Host *` block carrying `HashKnownHosts yes` and, optionally, a `MACs` list
without the SHA-1 entries; add an explicit `IdentityFile` to each existing block as its own change;
record in a comment why `RequiredRSASize`, `IdentitiesOnly` and `PasswordAuthentication` are absent, so
the next reader with a hardening checklist does not re-add them.

**Assumption made in the operator's place:** that a two-setting block plus a comment explaining four
deliberate omissions is worth having, rather than adding nothing at all. The alternative is to close the
task outright and leave the file as it is, since `HashKnownHosts` is a privacy improvement rather than a
hardening one and it does not touch the 168 entries already recorded. This is open question 4.

## The finding that belongs to no task: an undeclared PostgreSQL server

This was not asked for and is the most consequential thing measured.

`brew services list` reports `postgresql@17` as started for user `stephen` from
`~/Library/LaunchAgents/homebrew.mxcl.postgresql@17.plist`. `plutil -p` on that plist shows
`RunAtLoad => true`, `KeepAlive => true` and
`ProgramArguments => /opt/homebrew/opt/postgresql@17/bin/postgres -D /opt/homebrew/var/postgresql@17`. So
a PostgreSQL server starts at every login and is restarted whenever it dies.

That plist is not tracked. `Library/LaunchAgents/` in this repository holds 13 templates, all named
`com.webdavis.*`. The live `~/Library/LaunchAgents/` holds two additional agents written by
`brew services`: `homebrew.mxcl.postgresql@17.plist` and `homebrew.mxcl.ollama.plist`. The root
`CLAUDE.md` states that "Every scheduled or supervised job on dresden is a chezmoi-tracked plist under
`Library/LaunchAgents/`, bootstrapped by a matching `.chezmoiscripts/run_onchange_after_*` loader". These
two are the exceptions, and nothing in the repository records that they exist.

What the server holds and how it is reachable:

- Databases: `open_brain` plus the three system databases. `open_brain` is 7990 kB and holds one table,
  `thoughts`, with 2 rows. Its columns are `id uuid`, `content text`, `embedding vector(384)`,
  `metadata jsonb`, `created_at`, `updated_at`, with a hierarchical-navigable-small-world index on the
  embedding and a generalized-inverted index on the metadata. The `vector` extension is at 0.8.2. The
  384-dimension embedding is the width of the common small sentence-transformer models, so this reads as
  an abandoned semantic-memory prototype.
- Owner: unknown. `open_brain` appears nowhere in this repository and nowhere in the agent memory index.
  A wider search across `~/workspaces` did not complete inside a 60-second budget, so this is an open
  question rather than a conclusion.
- Network exposure: loopback only. `lsof` shows listeners on `127.0.0.1:5432` and `[::1]:5432` and
  nothing else.
- Authentication: none. Every line of `pg_hba.conf` is `trust`, for `local`, `127.0.0.1/32` and
  `::1/128`, for both `all` and `replication`. That is Homebrew's default from an unconfigured `initdb`,
  and it means any local process can connect as any role, superuser included, with no password.

The last two points together are why this matters beyond tidiness. Claude Code on this machine runs with
`permissions.defaultMode = bypassPermissions`, and the deny list named in `CLAUDE.md` covers six file
paths. A loopback PostgreSQL server with `trust` authentication is not one of them, so any agent session
on this machine can read and write that database, and could create or drop others, without passing any
gate. Today the blast radius is two rows of a prototype. It is worth a deliberate decision rather than
continuing by accident. This is open question 5.

Also worth recording for whoever disposes of it: `postgresql@17` and `pgvector` both show up in
`brew leaves`, meaning neither is required by another installed formula, and both are declared in
`.chezmoidata/system_packages_autoinstall.yaml`. So removing the declaration would put them in scope for
`brew bundle cleanup --force`, which would uninstall the formula while `/opt/homebrew/var/postgresql@17`
and its data keep sitting on disk. Retire the service first, then the declaration, never the other way
round.

## What would change each verdict

- **nvim-bqf, adopt becomes close** if the operator would rather `:ReviewLedger` opened Trouble than the
  native quickfix window. The reroute is a one-word change and makes bqf redundant.
- **dadbod, defer becomes adopt** the moment a database with real tables is in daily use from Neovim, and
  a credential form is chosen. It becomes close if dblab is trialled and found sufficient, since nobody
  needs both a Neovim drawer and a terminal client for the same database.
- **dblab, defer becomes adopt** on a successful trial, or on a homelab PostgreSQL that needs a client;
  the SSH tunnel flags and the Keychain-backed profiles are what would decide it. It becomes close if the
  trial shows psql plus a psqlrc is enough.
- **PostgreSQL client configuration, the two adopted items become moot** if the local server is retired
  and the relevant database turns out to be the homelab one reached through a tunnel. The PATH line is
  still worth having either way, because `psql` is also how you inspect a tunnelled server.
- **SSH, close becomes adopt** if `IdentityFile` declarations are added to every block first, at which
  point `IdentitiesOnly yes` becomes safe and the block is worth more than two settings. It also changes
  if a host outside this machine's current set needs an algorithm restriction the defaults do not give.

## Open questions for the operator

1. **Should `:ReviewLedger` keep opening the native quickfix window?** If yes, nvim-bqf is the adopt
   above. If you would rather it opened Trouble, say so and nvim-bqf closes instead, with a one-word
   change to `dot_config/nvim/lua/config/keymaps.lua`.
1. **Did the 2026-05-16 PostgreSQL daily-use date arrive?** There is no psql history on this machine and
   `psql` is not on `PATH`, which reads as "no". If the work happened somewhere else, the dadbod and
   dblab verdicts flip from defer to a real evaluation.
1. **Trial dblab before declaring it, or declare it now?** The trial is one `brew install` and commits
   nothing. Declaring now is one alphabetical line and keeps the weekly bundle current, at the cost of
   maintaining a formula that may go unused.
1. **Is a two-setting `Host *` block worth adding, or should the task just close?** The honest content is
   `HashKnownHosts yes` plus a comment explaining why four checklist settings are deliberately absent.
   Separately: do you want the existing 168 plaintext `known_hosts` entries retro-hashed with
   `ssh-keygen -H`, which makes the file permanently unreadable to you?
1. **What is `open_brain`, and should its server keep running?** It holds two rows behind `trust`
   authentication on loopback, restarted at every login by an untracked `brew services` LaunchAgent, and
   reachable by any agent session on this machine without passing a permission gate. Three dispositions:
   keep it and record the agent plus the `trust` decision in the repository; keep the formula but stop
   the service (`brew services stop postgresql@17`) until there is a workload; or retire it, which means
   stopping the service, removing the declaration and the data directory, and deciding the same question
   for `homebrew.mxcl.ollama.plist`, which is in exactly the same position.
