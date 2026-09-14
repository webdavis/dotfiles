# SP7 tool and quick-win verdicts, 2026-09-14

Ledger entry: `docs/remaining-work.md`, section "SP7 scope recovered from the roadmap and Todoist", line
1904: "Revisit the roadmap's bandwhich/doggo/ouch evaluation, remaining shell quick wins and optional
Tart clean-machine environment. `MANPAGER` and Git's `autocorrect = prompt` already exist. VM creation
remains operator-gated. Re-rule the old documentation/archive tasks S1/S2/S4 against current files."

Nothing was installed, removed, edited or applied for this record. Every measurement below is a read or a
benchmark against binaries already on the machine, plus one shallow clone of the doggo source into the
session scratchpad.

## The question

Four separate questions travel under one ledger bullet:

1. Does each of `bandwhich`, `doggo` and `ouch` earn a line in
   `.chezmoidata/system_packages_autoinstall.yaml`? (Roadmap row P6, Todoist `6gfVJCvxV34W3hgM`.)
1. What is actually left of the "remaining quick wins" list? (Roadmap row P8, Todoist `6gfVJ8Rfh8ppwpqv`,
   sourced from `docs/research/2026-04-14-dotfiles-improvements.md`.)
1. Is the optional Tart clean-machine virtual machine worth building? (Roadmap row P13, Todoist
   `6gfVJ8v6pjjF5Qwv`.)
1. Do setup steps S1, S2 and S4 from `docs/superpowers/specs/2026-05-15-dotfiles-tasks-design.md` still
   apply to the files as they stand? (S3 is not in scope, but one of its two issues is noted below.)

## Verdicts

| Subject                                    | Verdict                                                    |
| ------------------------------------------ | ---------------------------------------------------------- |
| `doggo`                                    | **Adopt**                                                  |
| `bandwhich`                                | **Decline**, macOS ships the capability unprivileged       |
| `ouch`                                     | **Decline**, every format it covers is already covered     |
| Quick win 15, hidden files in the picker   | **Adopt a one-word fix**, not the tool swap that was filed |
| Quick win 13, `batman` as the man pager    | **Decline**, keep `nvim +Man!`                             |
| Quick wins 1 to 12, 14, 16 to 28           | **Already shipped, or dead with tmux and the flake**       |
| Tart clean-machine environment             | **Defer**, and retire P13 as written                       |
| S1, archive completed docs                 | **Decline as written**, the premise inverted               |
| S2, the skip-the-archive rule              | **Moot**, it only exists to serve S1                       |
| S4, add `Closes #17` to an unpushed commit | **Superseded**, and #17 must stay open                     |

One line: one tool in, two out, the quick-wins list is down to a single one-word bug fix and two matters
of taste, and all three old setup steps are dead in their filed form.

## What was checked and how

Host: dresden, macOS 26.2 (build 25C56), arm64, `Darwin 25.2.0`.

Installed versions read directly: `chezmoi v2.72.1`, `fzf 0.74.4 (Homebrew)`, `ripgrep 15.2.0`,
`fd 8.4.0` at `~/.cargo/bin/fd` with `fd 10.5.0` at `/opt/homebrew/bin/fd`, `hyperfine 1.20.0`,
`bash 5.3.15`, `NVIM v0.12.5`, `bsdtar 3.5.3` with `libarchive 3.7.4`, `7-Zip (z) 26.03`, `tart 2.36.0`.

Homebrew formula metadata read from the local application programming interface cache with
`brew info --json=v2`: `bandwhich 0.23.1`, `doggo 1.4.0`, `ouch 0.8.3`, `tart 2.37.0` with license
`FSL-1.1-ALv2`. All three candidates are bottled for `arm64_tahoe`, so none would build from source on
this machine.

Repository state read from the checkout at `f24aba51` on `main`:
`.chezmoidata/system_packages_autoinstall.yaml` (17 taps, 17 trusted taps, 132 formulae, 55 casks, 5 App
Store entries), `dot_bashrc.tmpl`, `dot_fzf_bindings`, `dot_gitconfig.tmpl`, `dot_inputrc`,
`dot_config/ghostty/config`, `dot_config/starship.toml`, `dot_config/bat/config`, `dot_aerospace.toml`,
`pns/crates/pns-domain/src/shell.rs`, `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl`,
`treefmt.toml`, `docs/runbooks/macos-fresh-machine-quickstart.md`.

Upstream sources fetched: the bandwhich and ouch project readme files, `tart.run/licensing`,
`tart.run/quick-start`, `tart.run/faq`, chezmoi's "use scripts to perform actions" guide, and a shallow
clone of `github.com/mr-karan/doggo` (tip `7f6b105`, 2026-09-01) unshallowed to confirm which release
carries the macOS resolver work.

Issue state read with `gh-axi issue list --state all --limit 60`: #5 closed, #13 open, #17 open.

Benchmarks: `hyperfine --warmup 2 --runs 8` inside the checkout, plus five hand-timed runs of fzf's
built-in directory walker over a pseudo-terminal (`script -q /dev/null`) because fzf's filter mode reads
standard input when standard input is not a terminal, which makes a plain hyperfine run measure nothing.
The pseudo-terminal overhead was measured separately at 1 to 2 milliseconds and is therefore ignorable.

One claim below is from training and was not verified against a primary source; it is labelled where it
appears.

## Findings

### doggo: adopt, for a reason this machine specifically has

The filed rationale for doggo was cosmetic ("replaces `dig` with colored, human-readable output"). That
rationale does not justify a formula. A different one does, and it took reading the source to find.

macOS does not resolve names the way `dig` queries them. `/etc/resolv.conf` on this machine is a symlink
to `/var/run/resolv.conf`, whose own header says so:

```
# This file is not consulted for DNS hostname resolution, address
# resolution, or the DNS query routing mechanism used by most
# processes on this system.
#
# To view the DNS configuration used by this system, use:
#   scutil --dns
```

Its only nameserver line is `nameserver 192.168.1.1`. Meanwhile `scutil --dns` reports 76 resolvers,
including a supplemental entry that routes the Domain Name System (DNS) suffix `ts.net` to
`100.100.100.100`, which is Tailscale's resolver. The consequence, measured:

```
$ dig +short dresden.tail2f2430.ts.net
                                          # empty, exit 0
$ dscacheutil -q host -a name dresden.tail2f2430.ts.net
name: dresden.tail2f2430.ts.net
ip_address: 100.77.192.92
```

`dig` reads `/var/run/resolv.conf`, asks the home router, and returns nothing with a success exit code.
That is the worst possible failure mode for the one DNS question this repository actually owns: whether a
MagicDNS name is being answered by MagicDNS or by the `/etc/hosts` registration-layer pin that
`.chezmoidata/macos_system_setup.yaml` declares and `~/.local/libexec/tailscale/reconcile-hosts-pin.sh`
converges.

doggo, from version 1.4.0, is macOS-resolver-aware. `pkg/config/config_darwin.go` parses `scutil --dns`
rather than `/etc/resolv.conf`, and `internal/app/nameservers.go` calls `config.MatchDomainNameservers`
on the default query path before falling back to the general-purpose list. The source comments name the
exact case:

- "GetAllServers is like GetDefaultServers but also includes Supplemental and domain-scoped resolvers
  (e.g. the resolvers a VPN or Tailscale installs for split-DNS)."
- "MatchDomainNameservers selects nameservers for query names that fall under a macOS
  Supplemental/domain-specific scutil resolver. Longest domain match wins per query name. ok is true only
  when every query name falls under such a resolver."
- "Prefer those nameservers when a query falls under such a domain so the NAMESERVER column reports the
  resolver that was actually used (issue #49)."

So `doggo dresden.tail2f2430.ts.net` with no flags routes to `100.100.100.100` and prints which resolver
answered. `dig` cannot do that without the operator already knowing the answer and typing
`dig @100.100.100.100`.

Release coverage was checked, because this is new code. `MatchDomainNameservers` was added on 2026-08-31
in commit `3111835`, "fix(darwin): use and report domain-specific scutil resolvers (#253)", and tag
`v1.4.0` (`7647a7e`, 2026-09-01) contains it. Homebrew ships 1.4.0, so the installable version has the
capability. It is two weeks old, which is the one real risk in adopting it.

What doggo does not replace: `dscacheutil -q host` still answers "what does the system resolver return,
including `/etc/hosts`", because doggo is a DNS client and never reads a hosts file. The two are
complementary, and the hosts-pin question needs `dscacheutil`. doggo needs no elevated privileges, has no
runtime dependencies in its formula, and adds machine-readable output plus a delegation trace.

Placement if adopted: one line in `.chezmoidata/system_packages_autoinstall.yaml` under
`packages.macos.homebrew.formulae`, between `direnv` and `dust`.

### bandwhich: decline, macOS ships the capability without sudo

Two facts kill this one.

First, privileges. The upstream readme states: "Since `bandwhich` sniffs network packets, it requires
elevated privileges." It offers a `setcap` route to avoid repeated escalation, but `setcap` is Linux
only; the readme's macOS content is limited to noting that bandwhich uses `lsof` there. So on dresden
bandwhich means `sudo bandwhich`, every time.

Second, the platform already does it. `/usr/bin/nettop` is system-shipped, runs unprivileged, and reports
per-process and per-connection byte counts with remote addresses. Measured:

```
$ nettop -P -L 1 -J bytes_in,bytes_out
,bytes_in,bytes_out,
apsd.456,3006727,7544211,
...
$ nettop -m tcp -L 1
tcp4 192.168.1.26:60086<->17.57.144.104:5223,en0,Established,3006727,7544211,...,67.06 ms,...
```

That covers process attribution, connection attribution, remote address, state, round-trip time and a
comma-separated logging mode suitable for scripting. bandwhich's remaining advantages over it are reverse
Domain Name System hostnames instead of bare addresses, transfer rates instead of cumulative counts, and
a nicer ranked full-screen display. That is a presentation delta, bought with a sudo prompt on every
invocation.

There is a third cost specific to this repository. `pns/crates/pns-domain/src/shell.rs` decides which
commands the long-running-command notifier stays quiet for, and it is a hardcoded prefix list:

```rust
pub fn shell_is_interactive(command: &str) -> bool {
    [
        "vim", "nvim", "less", "man", "top", "btop", "ssh", "herdr", "claude", "hermes", "codex",
        "fzf",
    ]
```

A full-screen monitor left open for a few minutes is exactly what that list exists to suppress, and
`bandwhich` is not on it. Worse, the prefix match is against the whole command line, so adding
`"bandwhich"` would still not match `sudo bandwhich`. Adopting bandwhich therefore means either accepting
a spurious notification on every session or changing Rust source and teaching the matcher about `sudo`.
That is not a p4 quick win.

The filed dependency is also gone. The 2026-05-15 spec ordered P6 before P7 because "Task 12's
ignored-tools audit explicitly references `bandwhich`, which P6 installs. Without P6, P7 references a
missing binary." The skip list has since moved from bash into pns and still holds the same twelve names,
so P7's audit sub-task was never executed. Nothing is now blocked on installing bandwhich.

### ouch: decline, with the taste caveat stated

ouch is a good program. The question is whether it covers anything this machine cannot already do.

Its readme lists 14 formats: tar, zip, 7z, gz, sz, zst, xz, lzma, lz, bz/bz2, bz3, lz4, rar and br, with
rar decompression-only because of licensing. Against that, the machine already has: `bsdtar 3.5.3` with
`libarchive 3.7.4` (tar, zip, 7z read, gz, bz2, xz), `/usr/bin/unzip` and `/usr/bin/zip`,
`/usr/bin/bzip2`, and Homebrew's `gzip`, `xz`, `zstd`, `lz4` and `brotli`. All of those except the system
ones are already declared.

The rar gap closes too. `7zz` is installed (declared as `sevenzip`, with `p7zip` alongside) and `7zz i`
lists both `Rar` and `Rar5` among its readable formats. So `ouch` would add no format this machine cannot
open today.

What it would add is one verb instead of three: `ouch decompress whatever.ext` rather than remembering
`tar xf`, `unzip` or `7zz x`. That is a genuine ergonomic win and a matter of taste, which is why this is
a decline rather than a rejection. Two constraints bound it if the operator overrules:

- It is interactive-only. `CLAUDE.md` is explicit that scripts prefer stable, system-shipped tools over
  newer alternatives, so no script in this repository would be allowed to call it, and the bash-to-Rust
  and converge tooling all shell out to system binaries today.
- Its format inference prompts for confirmation when a filename carries no extension, which is correct
  behavior interactively and unusable in automation.

Placement if overruled: between `opus` and `oven-sh/bun/bun` in the formulae list.

### Quick win 15: the file picker cannot see any hidden file, and the fix is one word

The filed item was "Use fd as FZF_DEFAULT_COMMAND instead of rg", justified in the research as "faster
than rg for file listing". The speed claim does not survive measurement, and the tool swap is the wrong
fix for the defect sitting underneath it.

`dot_fzf_bindings` line 53 sets the picker's command:

```bash
FZF_DEFAULT_COMMAND='rg --files --no-ignore -L -g "!.git/" 2>/dev/null'
```

`--no-ignore` disables ignore files. It does **not** enable hidden files; ripgrep needs `--hidden`
separately. So in this checkout, measured:

```
$ rg --files --no-ignore -L -g '!.git/' | grep -c '^\.chezmoiscripts/'
0
$ fd --type f --type l --hidden --no-ignore --follow --exclude .git | grep -c '^\.chezmoiscripts/'
53
```

Ctrl-T Ctrl-T in the dotfiles repository cannot reach a single file under `.chezmoiscripts/`,
`.chezmoidata/`, `.chezmoitemplates/` or `.github/`. That is 8,712 files missing out of 116,165, and they
include most of what this repository is. The binding is real: `dot_fzf_bindings` binds `\C-t\C-t` to
fzf's `fzf-file-widget`, which reads `FZF_CTRL_T_COMMAND`, which line 59 assigns from
`FZF_DEFAULT_COMMAND`. Nothing else in the repository reads either variable.

The speed claim does not survive either. With the two commands listing different sets, ripgrep wins; with
the sets matched at 116,165 entries each, the difference is inside the noise:

```
rg --files --no-ignore --hidden -L -g "!.git/"                     648.8 ms ± 130.3 ms
fd --type f --type l --hidden --no-ignore --follow --exclude .git   555.9 ms ± 105.5 ms
Summary: fd ran 1.17 ± 0.32 times faster
```

A confidence interval spanning 1.0 is not a reason to change tools. The recommendation is therefore the
one-word fix, `--hidden` added to the existing ripgrep line, which closes the blind spot, keeps the
measured performance, keeps the existing three-tier fallback (ripgrep, then the silver searcher, then
`find`), and adds no dependency.

Three things make that the right rung rather than the lazy one:

- **Switching to fd would bind the picker to a binary the repository does not control.** `command -v fd`
  resolves to `~/.cargo/bin/fd`, version **8.4.0**, an undeclared cargo install that shadows the declared
  Homebrew `fd 10.5.0` by PATH order. A recommendation built on fd would run on a binary six major
  versions stale that no declaration governs.
- **Deleting the override entirely is tempting and measurably worse.** fzf 0.74.4's built-in walker
  defaults to `file,follow,hidden` skipping `.git,node_modules`, and it produces the identical 116,165
  entries including all 53 `.chezmoiscripts/` files, so it is semantically correct with seven fewer lines
  of shell. But it is slower and far more variable: five hand-timed runs came in at 0.936, 1.260, 1.528,
  2.486 and 5.241 seconds against ripgrep's 649 to 831 millisecond mean. ripgrep parallelizes the walk
  and fzf's walker does not.
- **Alt-C is already fine and needs nothing.** `FZF_ALT_C_COMMAND` is unset, and fzf's `__fzf_cd__`
  passes `--walker=dir,follow,hidden` explicitly, so directory selection has never had the blind spot.
  Setting `FZF_ALT_C_COMMAND` would be a change with no defect behind it.

One consequence to name honestly: `--hidden` makes the walk larger in `$HOME`. It is already unusable
there, so this changes nothing. Both commands were run with a 60-second cap in `$HOME` and both hit it
(exit 124), because `--no-ignore` already removes every bound on that walk. fzf streams results as they
arrive, so in practice the picker fills and stays useful; it simply never finishes.

Two hygiene notes fall out of this, neither in scope: `dot_fzf_bindings` is not covered by treefmt's
shellcheck or shfmt formatters, whose includes are `["*.sh", "*.bash", "dot_bash*", "dot_profile"]`, so
an edit there is unlinted by the gate despite the file carrying its own `# shellcheck shell=bash`
directive. And `nu` plus `nu_plugin_core_match` sit in `~/.cargo/bin` despite the ratified nushell no-go
of 2026-07-09.

### Quick win 13: keep `nvim +Man!`, decline `batman`

The ledger's note that "`MANPAGER` ... already exist" is true but ambiguous, so state it exactly:
`dot_bashrc.tmpl` line 50 sets `export MANPAGER="nvim +Man!"`, and the quick win that was filed asked for
something different, the bat-extras `batman` pager. `bat-extras` is already declared and installed, so
`/opt/homebrew/bin/batman` is on the machine; only the `MANPAGER` assignment is unchanged.

The filed justification was that `batman` is "a faster, syntax-highlighted pager that does not require
loading neovim". The size of that win, measured: `nvim --headless +q` with the full user configuration
runs in 209.8 ms ± 123.5 ms. An interactive man view costs somewhat more than headless, so call it a
quarter of a second per `man` invocation.

A quarter second buys, today: the operator's own Neovim keymaps in every man page, `:Man` cross-reference
following, `gO` outline navigation, and one editor rather than two. Neovim and vim mode are both locked
toolchain choices. Trading that for 0.2 s is a regression in consistency, so the verdict is to keep
`nvim +Man!` and leave `batman` available as an explicit command for anyone who wants colored output
once. Neither choice interacts with the notifier, since `man` is already on the pns skip list.

### The rest of the quick-wins list, re-ruled against current files

Every item in the "Summary: Priority-Ranked Action Items" list of
`docs/research/2026-04-14-dotfiles-improvements.md`, checked against the files as they stand:

| Item                                         | State now                                                                   |
| -------------------------------------------- | --------------------------------------------------------------------------- |
| 1, git config modernization (seven keys)     | **Shipped**, `dot_gitconfig.tmpl` lines 92 to 131                           |
| 2, consolidate on delta                      | **Shipped**, `pager = delta` line 29, `[delta]` line 221                    |
| 3, 4, 7, 16, 18 to 20, every tmux item       | **Dead**, tmux retired for herdr                                            |
| 5, bracketed paste                           | **Shipped**, `dot_inputrc` line 30                                          |
| 6, Ghostty `clipboard-read = ask`            | **Shipped**, ghostty config line 62                                         |
| 8, `--force-with-lease` in the `acp` alias   | **Moot**, no `acp` alias survives                                           |
| 9, starship `nix_shell` and `direnv` modules | **Shipped**, starship config lines 9, 10, 218, 224                          |
| 10, bat config overhaul                      | **Mostly shipped**, the `*.tmpl` syntax map is in place; the theme is taste |
| 11, atuin daemon                             | **Shipped**, `com.webdavis.atuin-daemon` supervises it                      |
| 12, gitleaks                                 | **Shipped**, pre-commit hook                                                |
| 13, bat-extras `batman` as the man pager     | **Declined above**, `MANPAGER` stays `nvim`                                 |
| 14, hyperfine                                | **Shipped**, 1.20.0 installed and declared                                  |
| 15, fd as the picker command                 | **Replaced above** by the one-word `--hidden` fix                           |
| 17, Ghostty `shell-integration-features`     | **Shipped** (line 64); `quick-terminal-size` unset, taste                   |
| 21, bash 5.3 forkless substitution           | **Available**, bash 5.3.15 is the managed shell; a style note               |
| 22, cache shell init evals                   | **Shipped in part**, the brew shellenv cache refresher                      |
| 23, 24, AeroSpace monitor and app rules      | **Open**, taste, depends on monitor count                                   |
| 25, chezmoi apply notifications              | **Superseded**, pns owns notifications; see the quiet-apply ruling          |
| 26, age encryption                           | **Shipped**, identity restored from the vault                               |
| 27, `git maintenance start`                  | **Decline**, it schedules a job chezmoi does not declare                    |
| 28, Ghostty background blur                  | **Open**, pure aesthetics                                                   |

So the list is down to: one bug fix (adopt), one pager decision (declined), and four taste items the
operator may take or leave (Catppuccin for bat, `quick-terminal-size`, background blur, AeroSpace
assignments). Item 27 gets an actual decline rather than a shrug, because a `git maintenance` schedule
would be the one background job on this machine that chezmoi does not declare.

### Tart: defer, and retire P13 as written

P13 asked for one thing: pull a ~25 GB base image and have `tart list` show `sequoia-runner`. As written
it is dead three times over.

**The image name and version are stale.** Tart publishes `vanilla`, `base` and `xcode` variants per
release at `ghcr.io/cirruslabs/macos-<release>-<variant>`. The current pairing for this machine is
`macos-tahoe-*`, since dresden runs macOS 26.2; `sequoia` is two majors behind, and `sequoia-runner` was
only ever the local clone name from the 2026-04-14 research snippet.

**Its original rationale is gone.** P13 came out of the act-runner work, and
`docs/research/2026-04-14-act-macos-runners.md` already reached the opposite conclusion in its own
recommendations: "Consider NOT using `act` ... Adding `act` would add complexity without much benefit
beyond validating the workflow YAML structure (which `actionlint` handles better)", with a bottom line of
"The main gap is workflow YAML validation, which `actionlint` fills perfectly without the overhead of
`act`." actionlint has since been adopted (`just lint-actions`, and `just lint-actions-security` is one
of the three gates continuous integration (CI) runs). The gap that motivated a macOS runner image is
closed by a linter.

**Licensing is fine, and is not the blocker.** `tart.run/licensing` states "Usage on personal computers
including personal workstations is royalty-free", with paid tiers starting above 100 central processing
unit cores; the Homebrew formula's license field reads `FSL-1.1-ALv2`. Apple's own macOS software license
agreement, section 2.B(iii), permits up to two additional virtual instances per owned Mac for software
development, testing during development, macOS Server, or personal non-commercial use, and that
two-instance ceiling is enforced in the Virtualization framework rather than merely stated. Both permit
exactly what a clean-machine rehearsal would be.

That leaves the only live rationale, the one the ledger actually names: a clean machine on which to
rehearse a fresh `chezmoi init` plus full apply. Here is what such a rehearsal could and could not cover.

It **could** cover the part nothing currently tests: bootstrap ordering on a machine with nothing
installed. `run_once_before_00-install-homebrew.sh.tmpl`, then the package bundle, then the age-key
restore, the four cargo builders, the osquery converge, and the LaunchAgent loaders, in that order, with
no prior state to hide a missing prerequisite. One friction point widely assumed to be fatal is not:
`.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` splits the App Store lines out of the
Brewfile and publishes them for the weekly job, with the comment "AND THE APPLY NEVER TALKS TO THE APP
STORE AT ALL", so the five declared App Store entries would not block an apply inside a virtual machine
even though Apple still does not support Mac App Store sign-in in one. Apple Account and iCloud sign-in
itself does work in a virtual machine from macOS 15 onward when both host and guest are 15 or later,
which dresden and a Tahoe guest both satisfy, so the KeePassXC database could reach the guest by its
normal iCloud Drive route.

It **could not** cover most of `docs/runbooks/macos-fresh-machine-quickstart.md`, which is the document a
rehearsal would exist to validate. That runbook is dominated by steps with no command-line surface at
all: the privacy consent grants for Ghostty, Karabiner-Elements and Hammerspoon (the runbook's own words:
"Each grant requires opening the Privacy sheet and dragging the app into the listed sheet. There's no CLI
surface"), LuLu's network system extension approval and its separate filter consent, OverSight's
notification authorization, the two Mission Control settings deliberately left manual because the
`defaults` key name drifts across releases, and Touch ID. Whether a network system extension can even
activate inside a Virtualization framework guest was not tested here and is the open technical question.

And the cheap substitutes already exist. chezmoi v2.72.1 has both `-D/--destination` and `-n/--dry-run`,
and chezmoi's own guide states plainly that "In dry-run mode, scripts are not executed", so
`chezmoi apply --dry-run --destination <scratch>` renders every template against a throwaway root and
runs nothing, catching template errors, missing data and unresolvable vault entries at zero cost. For the
scripts themselves, the 2026-09-08 operator ruling already prescribes the cheaper loop: render with
`CI=1 chezmoi --source "$PWD" execute-template --no-tty < .chezmoiscripts/<name>` and run the result.
Those two together cover the apply-will-fail question that an agent is actually asked to answer.

So: **defer**, and close P13 in its filed form. If the operator wants the clean-machine rehearsal, it is
a real project with a stated scope ("does a fresh init plus full apply complete unattended on a machine
with nothing installed"), an honest ceiling (it cannot rehearse the interactive majority of the runbook),
and a maintenance cost (a 25 GB pull now, redone at each macOS major, plus vault and iCloud setup inside
the guest). It is not a p4 quick win, and it was never going to be one.

A side observation while checking this: `tart` is declared at line 157 of the package file and installed
at 2.36.0, and `tart list` is empty, so the tool has been carried with zero use since it was declared.
`act` is declared at line 48 and installed, with no `.actrc`, no justfile recipe, no workflow reference,
and its own research doc recommending against it. Both are candidates for removal from the declaration if
the operator closes P13, which is an operator decision rather than a verdict this record can make.

### S1, S2 and S4, re-ruled

**S1, move 15 research files and 5 superpowers artifacts into `docs/archive/`. Decline as written.**
Verified never executed: `docs/archive` does not exist, and `find docs -name .gitkeep` returns nothing,
so none of the eight planned placeholder files was added either. All 15 named research files are still in
`docs/research/`. The premise has inverted in the four months since. S1 would move 20 of the files that
were then the bulk of `docs/`; `docs/research/` alone now holds 24 entries, and the repository has since
grown two conventions that do the job S1 was for. `CLAUDE.md`'s runbook table routes conditional detail
to `docs/runbooks/` and says it is "read on demand, not carried in this file". `docs/remaining-work.md`
is the live backlog, which memory records as "the live 51-task list". The audit-noise problem S1 solved
is now solved by the ledger naming what is live, and moving two dozen files would break the paths in the
eleven files under `docs/` that carry a `docs/research/` reference today (the ledger, the roadmap, three
specs, three plans and one audit among them). Recommendation: leave the files where they are, and if
archival is still wanted, do it as a deliberate reorganization with a link sweep, not as a setup step for
a plan that has otherwise been superseded.

**S2, add a "skip `docs/archive/` when auditing" rule to `CLAUDE.md`. Moot.** Verified absent: neither
`CLAUDE.md` nor `.chezmoitemplates/global-agent-rules.md` mentions `docs/archive` or an "Auditing docs"
section. S2 exists only to make S1 safe. With S1 declined there is nothing to skip, and adding a rule
about a directory that does not exist would be the kind of stale instruction the rule was meant to
prevent. If S1 is ever executed, S2 comes back with it, in the same change.

**S4, amend the unpushed 2026-05-05 macos-defaults commit to add `Closes #17`. Superseded, and do not
close #17.** The precondition is dead: the spec required `git log origin/main..HEAD` to contain the
commit, and `git merge-base --is-ancestor 409dd2a origin/main` confirms `409dd2a` ("feat(chezmoiscripts):
add tier 1 macos defaults runner", 2026-05-05) is on `origin/main`, with nothing unpushed on `main` at
all. No `Closes #17` trailer exists anywhere in history.

The spec's own fallback was `gh issue close 17`. **That fallback must not be taken.** Issue #17,
"Automate macOS system settings via chezmoi run scripts", is still open, and the first bullet of the same
SP7 section of the ledger lists "remaining macOS settings (#17)" as live work to reconcile. Closing it
would delete a tracked backlog item to satisfy a bookkeeping step. The macOS defaults work that #17 asked
for is largely shipped (two runners, `.chezmoidata/macos_defaults.yaml` and
`.chezmoidata/macos_system_setup.yaml`, plus the capture and drift recipes), which is why the trailer was
once appropriate, but "largely" is the operative word and the ledger says so. Recommendation: retire S4,
and let the SP7 package-and-settings bullet decide #17's fate on its own evidence.

**S3 is out of scope, and half of it is already owned elsewhere.** #5 (Bash to Nu Shell) is closed, per
the ratified no-go, so that half of S3 is done. #13 ("Exploration: try switching from Tmux to Zellij
(again)") is still open, and the ledger bullet at line 2022 already owns its disposition, so this record
only records the state: open, against a multiplexer this machine no longer runs.

## Assumptions made in the operator's place

Each of these is a choice this record made because the operator was asleep. Each names its alternative.

1. **doggo is judged on its macOS resolver awareness, not its output.** The filed task asked for
   readability. This record adopts it for split-Domain-Name-System correctness instead, which means the
   value claim rests on code that shipped two weeks ago. *Alternative:* judge it on the original cosmetic
   grounds, in which case it is a decline like the other two, because `dig` already prints answers.
1. **The picker fix is `--hidden` on the existing ripgrep line, not a tool swap and not a deletion.**
   *Alternatives:* swap to fd (rejected because PATH resolves an undeclared, stale fd), or delete the
   override and use fzf's built-in walker (correct and shorter, but measured at 0.9 to 5.2 seconds
   against ripgrep's 0.65 to 0.83).
1. **The picker fix is recommended for now rather than folded into SP4.** SP4 is chartered to rewrite
   exactly this area ("one binding table that drives both key bindings and an fzf menu"), so there is an
   argument for waiting. This record treats a one-word correctness fix as too small to hold hostage to a
   subproject that has not started. *Alternative:* fold it into SP4's diff and leave the blind spot for
   now.
1. **ouch is declined rather than left undecided.** Its only delta is ergonomic and the operator may
   simply want it. *Alternative:* adopt it as an interactive convenience, accepting that no script may
   call it.
1. **Tart is deferred rather than rejected.** The clean-machine idea has real value that nothing else
   covers, but not at p4 and not as filed. *Alternative:* reject outright and remove `tart` from the
   declaration, or accept the project now with the stated ceiling.
1. **S1 is declined rather than rescheduled.** *Alternative:* execute the archive move as specified,
   accepting the path-reference sweep across all eleven referring files.

## What would change these verdicts

- **doggo, to a decline:** if `scutil --dns` parsing proves fragile in practice (a resolver shape it
  mishandles, a wrong answer on a `.ts.net` name), or if a 1.4.x regression appears, since the feature is
  two weeks old. The test is one command: `doggo dresden.tail2f2430.ts.net` must return `100.77.192.92`
  and name `100.100.100.100` as the resolver.
- **bandwhich, to an adopt:** if reverse-hostname attribution or live transfer rates become something the
  operator needs regularly, or if `nettop` is found to under-report for a case that matters. The sudo
  requirement and the pns prefix-list interaction would still need answering.
- **ouch, to an adopt:** operator taste, no evidence needed.
- **The picker fix, to a deletion of the override:** if fzf's walker timing is re-measured on an idle
  machine and lands near ripgrep's. This machine had agents running throughout, which is why every
  benchmark here carries a wide standard deviation.
- **`batman`, to an adopt:** if `nvim +Man!` startup ever becomes noticeable, or if the operator wants
  colored man output as the default rather than on demand.
- **Tart, to an adopt:** if the fresh-machine runbook needs validating for a reason that outweighs its
  interactive ceiling, for example an imminent second machine. It would also change if a network system
  extension turns out to activate inside a Virtualization framework guest, which would close the largest
  single gap in what a rehearsal can cover.
- **S1, to an execute:** if `docs/` audit noise becomes a measured problem rather than a predicted one.

## Open questions for the operator

1. **doggo: adopt?** It is the one recommended addition. One line between `direnv` and `dust`, installed
   first per the Homebrew agent workflow, then declared.
1. **bandwhich and ouch: accept the declines, or overrule either on taste?** If bandwhich is wanted, the
   pns skip-list question needs an answer first: extend `shell_is_interactive` and teach it about `sudo`,
   or accept a notification after every session?
1. **The picker fix: apply `--hidden` now, or hold it for SP4?** And should `dot_fzf*` be added to
   treefmt's shellcheck and shfmt includes, so an edit there is linted at all?
1. **Tart: retire P13, or charter the clean-machine rehearsal as its own project?** If retired, should
   `tart` and `act` come out of the formulae declaration, given that both are installed, unused, and in
   `act`'s case recommended against by its own research?
1. **Issue #17: confirm it stays open.** This record recommends against the S4 fallback that would close
   it, because the SP7 section lists it as live. Please confirm, since the spec's instruction says
   otherwise.
1. **Issue #13 (Zellij): no decision needed here.** The separate ledger bullet at line 2022 already owns
   it ("#13 (Zellij predates the Herdr decision)"), so this record only confirms the state it found: #13
   open, #5 closed. Decide it there, not here.
1. **The four taste items:** Catppuccin for bat, Ghostty `quick-terminal-size` and background blur, and
   AeroSpace workspace-to-monitor assignment. Each is a small diff, none has a correctness argument.
1. **Out of scope, found while probing the network tooling, and untracked anywhere:** `nettop` reports
   `tcp4 *:445 Listen`, and `launchctl print-disabled system` shows `"com.apple.smbd" => enabled`, so
   Server Message Block file sharing is listening on this machine. Nothing in `docs/remaining-work.md` or
   `.chezmoidata/macos_posture_controls.yaml` mentions port 445 or file sharing, and the ledger's
   existing exposure section covers SSH only. Should file sharing be off, or a posture control that
   asserts its state? Filing this as its own item rather than folding it into a quick-wins bullet.
1. **Also out of scope:** `~/.cargo/bin/fd` (8.4.0) shadows the declared Homebrew `fd` (10.5.0), and `nu`
   plus `nu_plugin_core_match` are installed there despite the ratified nushell no-go. Both are
   undeclared cargo installs that contradict a declaration. Clean up, or leave?

## Sources

- Bandwhich readme: <https://raw.githubusercontent.com/imsnif/bandwhich/main/README.md>
- ouch readme: <https://raw.githubusercontent.com/ouch-org/ouch/main/README.md>
- doggo readme: <https://raw.githubusercontent.com/mr-karan/doggo/main/README.md>
- doggo source, `pkg/config/config_darwin.go` and `internal/app/nameservers.go`, tag `v1.4.0`
  (`7647a7e`), feature commit `3111835` (#253): <https://github.com/mr-karan/doggo>
- Tart licensing: <https://tart.run/licensing/>
- Tart quick start and image list: <https://tart.run/quick-start/>
- Tart frequently asked questions: <https://tart.run/faq/>
- macOS software license agreement, Tahoe 26: <https://www.apple.com/legal/sla/docs/macOSTahoe.pdf>
- The two-virtual-machine ceiling and its enforcement:
  <https://eclecticlight.co/2022/08/04/virtualisation-on-apple-silicon-macs-8-how-apple-limits-vms/> and
  <https://developer.apple.com/forums/thread/729580>
- Apple Account and iCloud in a virtual machine, and the standing Mac App Store restriction:
  <https://support.apple.com/en-us/120468> and
  <https://www.macrumors.com/2024/06/20/macos-sequoia-adds-icloud-support-vms/>
- chezmoi, scripts and dry-run mode: <https://www.chezmoi.io/user-guide/use-scripts-to-perform-actions/>
- chezmoi global flags: <https://www.chezmoi.io/reference/command-line-flags/global/>

The Apple software license agreement section number (2.B(iii)) and the Virtualization framework error
name (`VZError.Code.virtualMachineLimitExceeded`) come from the secondary sources above rather than from
the agreement text itself, which was not opened. From training, not verified: that the two-instance
clause dates to Mac OS X Lion.
