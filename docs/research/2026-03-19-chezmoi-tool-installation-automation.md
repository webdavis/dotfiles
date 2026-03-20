# Research: Automating Tool Installation with Chezmoi

**Generated**: 2026-03-19 **Source**: Claude.ai Research Feature (191 sources, 6m 24s) **Chat URL**:
https://claude.ai/chat/94ad67c3-66fe-427c-905e-019f6e82375b

______________________________________________________________________

# Automating tool installers with chezmoi run scripts

The key to reliable dotfile-driven tool installation is choosing `run_onchange_` over `run_once_` as your
default, wrapping every installer in an idempotency guard, and controlling execution order through
numeric-prefixed filenames in `.chezmoiscripts/`. chezmoi's author Tom Payne explicitly recommends
`run_onchange_` as the general-purpose script type because it avoids a subtle but painful edge case where
`run_once_` silently refuses to re-run scripts whose content reverts to a previously-seen hash. This
guide synthesizes the official documentation, chezmoi GitHub discussions, and battle-tested community
patterns into a practical reference for automating `curl | bash`-style tool installers like rustup, nvm,
sdkman, and rbenv.

## `run_once_` tracks content globally; `run_onchange_` tracks it per-name

The distinction between these two script types comes down to how chezmoi records execution state.
`run_once_` computes a SHA256 hash of the script's contents (after template expansion) and stores that
hash as a key in the `scriptState` bucket of `chezmoistate.boltdb`. The hash is content-addressed and
filename-independent -- if you create `run_once_b.sh` with identical contents to a previously-run
`run_once_a.sh`, it will not execute because chezmoi already has that hash recorded.

`run_onchange_` stores the script's target name as the key in the `entryState` bucket, with the contents
hash as the value. It re-runs whenever the current content hash differs from the last-recorded hash for
that specific name. The practical consequence: if you change an `run_onchange_` script from state A -> B
-> A, chezmoi will re-run when reverting to A (content differs from B). A `run_once_` script will not
re-run because hash A is already in the database.

| Behavior                  | `run_once_`                                        | `run_onchange_`                                   |
| ------------------------- | -------------------------------------------------- | ------------------------------------------------- |
| Storage bucket            | `scriptState`                                      | `entryState`                                      |
| Key                       | SHA256 of content                                  | Target name                                       |
| Re-runs on content revert | No                                                 | Yes                                               |
| Re-runs after failure     | Yes (hash not stored)                              | Yes (hash not stored)                             |
| State reset command       | `chezmoi state delete-bucket --bucket=scriptState` | `chezmoi state delete-bucket --bucket=entryState` |

When to use each: Use `run_once_before_` for true one-time bootstrapping -- installing a password manager
needed for template rendering, or installing prerequisites like curl. Use `run_onchange_` for everything
that evolves over time, especially package lists. The declarative package installation pattern (covered
below) relies entirely on `run_onchange_` because adding a new package to a data file changes the
rendered script, triggering re-execution.

## Ordering relies on naming conventions and execution phases

chezmoi has no explicit dependency graph. All ordering is controlled through two mechanisms: the
`before_`/`after_` execution phase attributes and alphabetical sorting within each phase. The full
application order is deterministic:

1. Read source and destination states
1. Compute the target state
1. Run `run_before_` scripts in alphabetical order
1. Update all entries (files, directories, externals, plain `run_` scripts) in ASCII order of target name
1. Run `run_after_` scripts in alphabetical order

The community universally uses zero-padded numeric prefixes to control execution order within a phase.
For a typical development environment setup, the dependency chain looks like this:

```
.chezmoiscripts/
├── run_once_before_00-install-homebrew.sh.tmpl        # package manager first
├── run_once_before_01-install-core-packages.sh.tmpl   # git, curl, build tools
├── run_onchange_before_10-install-packages.sh.tmpl    # evolving package list
├── run_once_before_20-install-rustup.sh.tmpl          # language toolchains
├── run_once_before_21-install-nvm.sh.tmpl
├── run_once_before_22-install-sdkman.sh.tmpl
├── run_after_50-install-cargo-tools.sh.tmpl           # tools depending on rustup
├── run_after_51-install-npm-globals.sh.tmpl           # tools depending on nvm
└── run_after_99-configure-shell.sh.tmpl               # final configuration
```

The `.chezmoiscripts/` directory is essential here -- scripts placed in it execute normally but do not
create a corresponding directory in the target home directory. Without it, each script's parent directory
would be created under `$HOME`.

A critical constraint: chezmoi assumes source and destination states are not modified while it runs. A
`run_before_` script that creates or modifies files in the source or destination directories produces
undefined behavior. Similarly, `run_before_` scripts should not depend on externals (`.chezmoiexternal`),
because externals are applied during the update phase. `run_after_` scripts may safely depend on
externals.

## Error handling should combine `set -euo pipefail` with idempotency guards

The community consensus and codified best-practice guides converge on starting every script with strict
error handling:

```bash
#!/usr/bin/env bash
set -euo pipefail
```

This catches most failures early: `-e` exits on any non-zero return, `-u` catches unset variables, and
`pipefail` propagates failures through pipes rather than silently swallowing them. chezmoi's behavior
reinforces this approach -- when a `run_once_` or `run_onchange_` script exits with a non-zero status,
chezmoi does not record the script's hash as successfully run, so it will be retried on the next
`chezmoi apply`. This built-in retry-on-failure mechanism means `set -e` is safe to use: a transient
network failure during `curl | bash` will cause the script to fail, and chezmoi will retry it
automatically.

For interrupt handling during long-running installations, some community dotfiles add a trap:

```bash
trap 'echo "Installation interrupted"; exit 1' INT TERM
```

There are two important operational constraints to keep in mind. First, never invoke chezmoi from within
a chezmoi script -- chezmoi uses a bbolt database that permits only one writer, and a nested invocation
will deadlock on the lock file. Second, always use non-interactive flags for `curl | bash` installers,
because an installer waiting for input will cause chezmoi to appear stuck with no visible prompt.

| Tool      | Non-interactive invocation                                                         |
| --------- | ---------------------------------------------------------------------------------- |
| rustup    | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh -s -- -y`         |
| nvm       | `curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh \| bash` |
| sdkman    | `curl -s "https://get.sdkman.io" \| bash`                                          |
| Homebrew  | `NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL ...)"`                                |
| oh-my-zsh | `CHSH=no RUNZSH=no KEEP_ZSHRC=yes sh -c "$(curl -fsSL ...)"`                       |

## Idempotency checks should match the tool's installation footprint

Even `run_once_` scripts should be idempotent, because they re-run after failures, after state resets,
and when their content changes. The official documentation is explicit: "All scripts should be
idempotent, including `run_onchange_` and `run_once_` scripts." The right idempotency check depends on
what the installer creates.

For tools that install a binary on `$PATH`, `command -v` is the cleanest check:

```bash
if ! command -v rustup &>/dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
```

For tools that install into a specific directory but may not immediately be on `$PATH` (common with nvm,
sdkman, rbenv), check directory existence:

```bash
if [ ! -d "$HOME/.nvm" ]; then
    curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
fi

if [ ! -d "$HOME/.sdkman" ]; then
    curl -s "https://get.sdkman.io" | bash
fi
```

A more robust pattern used in the community wraps this into a reusable helper function:

```bash
ensure_installed() {
    local tool_name="$1"
    local install_cmd="$2"
    if ! command -v "$tool_name" &>/dev/null; then
        echo "Installing $tool_name..."
        eval "$install_cmd"
    else
        echo "$tool_name already installed"
    fi
}

ensure_installed "rustup" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
ensure_installed "brew"   'NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"'
```

For post-install tool configuration (like installing cargo binaries after rustup), the pattern extends to
checking the specific binary that would be installed:

```bash
source "$HOME/.cargo/env"  # ensure cargo is on PATH
if ! command -v ripgrep &>/dev/null; then
    cargo install ripgrep
fi
```

## Templates make scripts platform-aware and trigger smart re-execution

Adding the `.tmpl` suffix to any script enables chezmoi's full Go template engine, with access to
variables like `.chezmoi.os`, `.chezmoi.arch`, `.chezmoi.osRelease.id` (Linux distro),
`.chezmoi.hostname`, and any custom data from `.chezmoidata/` files. The most powerful feature: if a
template expands to empty or whitespace-only output, the script is not executed. This is the canonical
pattern for platform-conditional scripts:

```bash
# run_once_before_20-install-rustup.sh.tmpl
{{ if ne .chezmoi.os "windows" -}}
#!/usr/bin/env bash
set -euo pipefail
if ! command -v rustup &>/dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
{{ end -}}
```

On Windows, this expands to nothing and is silently skipped. The trailing `-` on template delimiters
(`{{-` and `-}}`) trims whitespace, which matters -- stray newlines count as non-empty output.

For scripts that need different logic per platform rather than simple skip/run, use nested conditionals:

```bash
# run_onchange_before_10-install-packages.sh.tmpl
{{ if eq .chezmoi.os "darwin" -}}
#!/usr/bin/env bash
set -euo pipefail
brew install ripgrep fd bat
{{ else if eq .chezmoi.os "linux" -}}
#!/usr/bin/env bash
set -euo pipefail
  {{ if eq .chezmoi.osRelease.id "debian" -}}
sudo apt install -y ripgrep fd-find bat
  {{ else if eq .chezmoi.osRelease.id "fedora" -}}
sudo dnf install -y ripgrep fd-find bat
  {{ end -}}
{{ end -}}
```

The `lookPath` template function provides another useful check -- it tests whether a binary exists on
`$PATH` at template evaluation time, useful for conditional sections that depend on previously installed
tools.

## Hash computation happens after template expansion, which drives `run_onchange_`

For both `run_once_` and `run_onchange_` scripts, the critical sequencing is: template expansion first,
then SHA256 hash computation on the result. This means any change to template variables -- switching
machines, updating `.chezmoidata/` files, or changing chezmoi config data -- will alter the rendered
output, change the hash, and trigger re-execution.

This is the foundation of the declarative package installation pattern, which is the most sophisticated
community pattern for evolving package lists:

```yaml
# .chezmoidata/packages.yaml
packages:
  darwin:
    brews:
      - ripgrep
      - fd
      - bat
      - jq
```

```bash
# .chezmoiscripts/run_onchange_before_10-install-packages-darwin.sh.tmpl
{{ if eq .chezmoi.os "darwin" -}}
#!/usr/bin/env bash
set -euo pipefail
brew bundle --no-lock --file=/dev/stdin <<EOF
{{ range .packages.darwin.brews -}}
brew {{ . | quote }}
{{ end -}}
EOF
{{ end -}}
```

Adding `fzf` to the YAML changes the rendered script, changes the hash, and `run_onchange_` triggers. The
package manager itself (`brew bundle`) handles idempotency of individual packages.

For scripts that should react to changes in external files not part of the template data, embed the
file's hash in a comment:

```bash
# run_onchange_after_dconf-load.sh.tmpl
#!/bin/bash
# dconf.ini hash: {{ include "dconf.ini" | sha256sum }}
dconf load / < {{ joinPath .chezmoi.sourceDir "dconf.ini" | quote }}
```

When `dconf.ini` changes, the comment changes, the hash changes, and the script re-runs. The referenced
file should also be added to `.chezmoiignore` so chezmoi doesn't try to create it in the home directory.

## Known pitfalls with `run_onchange_` and `run_once_` in practice

**The revert trap with `run_once_`**: If you change a `run_once_` script's content from version A to
version B, then back to A, chezmoi will not re-run it because hash A is already in `scriptState`. This is
the primary reason the chezmoi author recommends `run_onchange_` as the default. `run_onchange_` stores
only the most recent hash per script name, so reverting from B back to A triggers re-execution.

**The `.chezmoiscripts/` ordering surprise**: Because the directory name starts with `.`, scripts inside
it sort before most target entries in ASCII order. A script in `.chezmoiscripts/run_install.sh` that
depends on `~/.Brewfile` being in place will fail if `dot_Brewfile` hasn't been applied yet. The fix is
always to use explicit `before_`/`after_` attributes rather than relying on alphabetical interleaving
with managed files.

**The template prerequisite bootstrapping problem**: If a template needs a tool that isn't installed yet
(e.g., a template uses `output "op" ...` to call 1Password CLI), chezmoi will fail during template
rendering before any scripts execute. The solution is a `run_once_before_` script that is not a template
-- plain `.sh`, no `.tmpl` suffix -- that installs the prerequisite. chezmoi executes `before_` scripts
before rendering templates for the update phase.

**State persistence during development**: Testing `run_once_` scripts during development is frustrating
because chezmoi remembers every hash. Adding a trivial whitespace change creates a new hash, but
`chezmoi state delete-bucket --bucket=scriptState` is the clean reset. For `run_onchange_`, use
`--bucket=entryState`. Inspecting state with `chezmoi state dump` shows exactly what's recorded and when.

**Nested chezmoi invocations deadlock**: chezmoi's bbolt state database allows only one writer. A `run_`
script that invokes chezmoi will deadlock on the database lock. This is a hard constraint -- scripts
cannot call chezmoi commands.

## Conclusion

The most effective pattern for automating development tool installation in chezmoi combines several
reinforcing practices. Place all scripts in `.chezmoiscripts/` with numeric prefixes and explicit
`before_`/`after_` phasing. Default to `run_onchange_` with `.tmpl` templates so package lists can evolve
naturally -- the declarative `.chezmoidata` pattern is the gold standard here. Reserve `run_once_before_`
for true bootstrapping prerequisites that need to exist before template rendering. Wrap every
`curl | bash` installer in a `command -v` or directory-existence guard, use `set -euo pipefail` at the
top of every script, and always pass non-interactive flags to installers. Use the empty-template pattern
for platform skipping rather than putting the conditional inside the script body. These patterns, drawn
from the official documentation and hardened through community use, produce dotfile repositories that
reliably bootstrap a full development environment from a single `chezmoi init && chezmoi apply`.

______________________________________________________________________

## Sources

Research was conducted across 191 sources including:

- chezmoi official documentation (chezmoi.io)
- chezmoi GitHub repository discussions and issues
- Community dotfile repositories on GitHub
- Blog posts and guides (Nathaniel Landau, midogguide, Almaz5200, and others)
- Playbooks and best-practice collections
