# Running GitHub Actions Locally with macOS Runners

Deep research report -- April 2026

______________________________________________________________________

## Table of Contents

1. [Can `act` Run macOS Workflows?](#1-can-act-run-macos-workflows)
1. [The `-self-hosted` Flag](#2-the--self-hosted-flag)
1. [Tart / Orchard / macOS VMs](#3-tart--orchard--macos-vms)
1. [GitHub's Own Local Runner Options](#4-githubs-own-local-runner-options)
1. [Alternative Tools to `act`](#5-alternative-tools-to-act)
1. [`act` Issues/Discussions -- State of macOS Support](#6-act-issuesdiscussions--state-of-macos-support)
1. [Best Way to Test CI/CD Workflows Locally in 2026](#7-best-way-to-test-cicd-workflows-locally-in-2026)
1. [Recommendations for This Dotfiles Repo](#8-recommendations-for-this-dotfiles-repo)

______________________________________________________________________

## 1. Can `act` Run macOS Workflows?

**Yes, with caveats.** `act` (nektos/act, currently at v0.2.86 as of March 2026) does not natively
emulate macOS runners in containers the way it emulates Ubuntu runners. macOS cannot run in Docker
containers. However, `act` supports a **host execution mode** that bypasses Docker entirely and runs
workflow steps directly on your local machine.

### How It Works

The `--no-container` feature (implemented via [PR #1293](https://github.com/nektos/act/pull/1293),
closing [Issue #97](https://github.com/nektos/act/issues/97)) allows `act` to execute workflow steps
directly on your host OS rather than inside Docker. This is exposed through the `-self-hosted` platform
designation.

### The Command

```bash
act -P macos-latest=-self-hosted
```

This tells `act`: "When you see `runs-on: macos-latest`, don't look for a Docker image -- run steps
directly on the host machine."

### What Works

- Shell steps (`run:` commands) execute natively on your macOS host
- Actions that don't depend on the GitHub-hosted runner's preinstalled toolchain
- Xcode, Homebrew, and other macOS-native tools (since they're on your real machine)
- Basic workflow orchestration (job ordering, step sequencing, conditionals)

### What Does NOT Work (or Has Issues)

- **Service containers**: These require Docker; they do not function in `-self-hosted` mode
- **`actions/cache`**: Requires additional setup (a local cache server); does not work out of the box
- **`RUNNER_TOOL_CACHE` / `AGENT_TOOLSDIRECTORY`**: Known issues where setup actions (e.g.,
  `setup-python`) try to write to `/Users/runner` and fail with permission errors
  ([Issue #5974](https://github.com/nektos/act/issues/5974))
- **`RUNNER_OS` and `RUNNER_ARCH`**: These environment variables may report incorrect values
  ([Issue #1509](https://github.com/nektos/act/issues/1509),
  [Issue #2579](https://github.com/nektos/act/issues/2579))
- **Annotations and problem matchers**: Not supported in self-hosted mode
- **GitHub API interactions**: No native mocking; real API calls will execute

______________________________________________________________________

## 2. The `-self-hosted` Flag

### Syntax

```bash
# Single platform override
act -P macos-latest=-self-hosted

# Multiple platforms (if workflow has both Ubuntu and macOS jobs)
act -P ubuntu-latest=catthehacker/ubuntu:act-latest -P macos-latest=-self-hosted

# Can also be placed in .actrc for persistence
```

### `.actrc` Configuration

Create `~/.actrc` or `.actrc` in your project root (one argument per line, no comments):

```
-P macos-latest=-self-hosted
-P macos-14=-self-hosted
-P macos-15=-self-hosted
--artifact-server-path=/tmp/act-artifacts
```

Arguments are loaded in order: XDG `.actrc` -> `~/.actrc` -> project `.actrc` -> CLI flags.

### Limitations of `-self-hosted`

| Feature            | Works?  | Notes                                  |
| ------------------ | ------- | -------------------------------------- |
| Shell `run:` steps | Yes     | Executes natively on host              |
| JavaScript actions | Yes     | If Node.js is installed on host        |
| Docker actions     | No      | No Docker involved in self-hosted mode |
| Service containers | No      | Requires Docker                        |
| `actions/cache`    | Partial | Needs local cache server setup         |
| `actions/setup-*`  | Partial | May fail due to path/permission issues |
| Matrix builds      | Yes     | But all run on same host               |
| Secrets            | Yes     | Via `--secret-file` or `--secret`      |
| Artifacts          | Partial | Via `--artifact-server-path`           |

### Key Environment Variable Issues

When using `-self-hosted` on macOS:

- `RUNNER_TOOL_CACHE` may conflict with a previous GitHub Actions runner installation's
  `AGENT_TOOLSDIRECTORY`. A [fix is in progress](https://github.com/nektos/act/pull/6052)
- `RUNNER_OS` should return `macOS` but may return `darwin`
- `RUNNER_ARCH` may not be set correctly on Apple Silicon

______________________________________________________________________

## 3. Tart / Orchard / macOS VMs

For teams needing true macOS VM isolation (not just host execution), several tools exist.

### Tart (cirruslabs/tart)

**What it is**: An open-source CLI tool for building, running, and managing macOS and Linux VMs on Apple
Silicon. Built on Apple's Virtualization.framework for near-native performance.

**Repository**: <https://github.com/cirruslabs/tart>

**Key features**:

- Near-native performance via Apple Virtualization.framework
- OCI-compatible registry support (push/pull VM images like containers)
- Packer plugin for infrastructure-as-code VM provisioning
- Pre-built images with Xcode versions (updated within 24 hours of new Xcode releases)
- ~25 GB initial base image download
- Requires macOS 13+ (Ventura) on Apple Silicon

**Quick start**:

```bash
brew install cirruslabs/cli/tart
tart clone ghcr.io/cirruslabs/macos-sequoia-xcode:latest runner
tart run runner
```

**Used by**: Atlassian, Figma, Mullvad, Krisp, TestingBot, and others.

### Orchard (cirruslabs/orchard)

**What it is**: An orchestration layer on top of Tart for managing clusters of Apple Silicon machines
running macOS VMs.

**Repository**: <https://github.com/cirruslabs/orchard>

**Use case**: When you have multiple Mac minis or Mac Studios and want to distribute CI jobs across them.
Supports local development mode for single-machine testing.

### Tartelet (shapehq/tartelet)

**What it is**: A macOS app that manages up to two GitHub Actions runners in ephemeral Tart VMs on a
single host.

**Repository**: <https://github.com/shapehq/tartelet>

**Key features**:

- Parallel runner execution
- Job isolation (each job gets a fresh VM)
- Heavily inspired by Cilicon
- On a Mac mini M1 (16 GB), jobs run 3-4x faster than GitHub-hosted runners

### Cilicon (traderepublic/Cilicon)

**What it is**: A macOS app by Trade Republic for self-hosted ephemeral CI on Apple Silicon, using
Virtualization.framework.

**Repository**: <https://github.com/traderepublic/Cilicon>

**Key features**:

- Uses Tart container format with built-in OCI client
- Supports GitHub Actions, Buildkite, GitLab Runner, and arbitrary scripts
- GUI app with SSH client for direct command execution on VMs
- Installable via Homebrew: `brew install --cask cilicon`

### Tart + act Combo (Community Script)

A community-maintained approach combines Tart VMs with `act`:

1. Clone a Tart macOS image
1. Boot the VM, SSH in
1. Install `act` inside the VM
1. Run `act -P macos-latest=-self-hosted` inside the VM

This gives you an isolated macOS environment running `act`, closer to what GitHub's hosted runners
provide. See: <https://gist.github.com/YOU54F/3ac099e54e48a31a69ac2d671aa878f6>

______________________________________________________________________

## 4. GitHub's Own Local Runner Options

### `github/local-action` (Official)

**Repository**: <https://github.com/github/local-action>

**What it is**: GitHub's official CLI utility for testing *custom actions* locally (not full workflows).

**Severe limitations**:

- **JavaScript/TypeScript actions only** -- no Docker or composite actions
- **Non-transpiled code only** -- won't work with bundled/built distributions
- **Tests individual actions**, not complete workflows
- **Not a replacement for `act`** -- fundamentally different scope
- Latest release: v7.0.1 (February 2026), actively maintained

**Install**: `npm install -g @github/local-action`

### GitHub Self-Hosted Runners (Official)

GitHub's official `actions/runner` can be installed on your Mac to receive jobs from GitHub. This is not
"local testing" per se -- your Mac becomes a real self-hosted runner connected to GitHub's
infrastructure. Jobs still flow through GitHub's API.

**2026 enforcement update**: Starting March 16, 2026, GitHub blocks runner registration for versions
older than v2.329.0.

### `github-nix-ci` (Community, Nix-based)

**Repository**: <https://github.com/juspay/github-nix-ci>

A NixOS and nix-darwin module for self-hosting GitHub runners declaratively using Nix. Useful if your CI
is already Nix-based (like this dotfiles repo).

______________________________________________________________________

## 5. Alternative Tools to `act`

### ChristopherHX/github-act-runner

**Repository**: <https://github.com/ChristopherHX/github-act-runner>

**What it is**: A reverse-engineered GitHub Actions runner that implements GitHub's runner protocol using
a modified version of nektos/act to execute steps.

**Key differences from nektos/act**:

- Registers as a real self-hosted runner with GitHub (not purely local)
- Supports Linux, Windows, macOS, **and FreeBSD**
- No Docker required for native execution
- Companion project `runner.server` enables fully local testing without GitHub connectivity
- Latest release: v0.13.0 (January 2026)

**Limitations**:

- Annotations and problem matchers not supported
- May leak more secrets than official runner
- Node.js 20 must be manually installed
- Manual version upgrades required

### act-js (kiegroup/act-js)

**Repository**: <https://github.com/kiegroup/act-js>

A Node.js wrapper around nektos/act for programmatic testing. Pairs with
[mock-github](https://github.com/redhat-developer/mock-github) for mocking Git repos and GitHub API
calls. Enables test-driven workflow development with Jest.

### actionlint (rhysd/actionlint)

**Repository**: <https://github.com/rhysd/actionlint>

**What it is**: A static analysis linter for GitHub Actions workflow files. Does not execute workflows
but catches errors before you push.

**Checks**: Expression type-checking, action input validation, shell script analysis (integrates with
shellcheck and pyflakes), security checks for script injection, cron syntax, runner label validation,
`needs:` dependency verification.

**Install**: `brew install actionlint`

**Best used alongside** `act`, not as a replacement. Catches syntax/logic errors instantly without
needing Docker.

### zizmor (zizmorcore/zizmor)

**Repository**: <https://github.com/zizmorcore/zizmor>

**What it is**: A security-focused static analysis tool for GitHub Actions workflows. ~24 audit rules
covering injection vulnerabilities, permission issues, mutable tags, and more.

**Integration**: Can publish findings to GitHub's Advanced Security tab via `zizmor-action`.

### Nix Flake Checks (for Nix-based projects)

For projects using Nix flakes (like this dotfiles repo), `nix flake check` provides a way to run all CI
checks locally using the exact same environment as CI:

```bash
# Run all checks locally
nix flake check --all-systems

# Enter dev shell with all tools
nix develop

# Run specific check
nix develop .#run --command ./scripts/lint.sh
```

This is already the approach used by this dotfiles repo. The key advantage: **perfect environment parity
between local and CI** -- no Docker images that differ from GitHub's runners.

______________________________________________________________________

## 6. `act` Issues/Discussions -- State of macOS Support

### Open Issues (as of April 2026)

| Issue                                              | Title                                                                  | Status                                                             |
| -------------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------ |
| [#2445](https://github.com/nektos/act/issues/2445) | Autoconfigure for running on Mac platform                              | Open                                                               |
| [#5974](https://github.com/nektos/act/issues/5974) | self-hosted on macOS is trying to create directory under /Users/runner | Open (PR [#6052](https://github.com/nektos/act/pull/6052) pending) |
| [#5946](https://github.com/nektos/act/issues/5946) | Why is there no documentation on using act under MacOS?                | Open                                                               |
| [#2579](https://github.com/nektos/act/issues/2579) | `RUNNER_ARCH` and `-self-hosted`                                       | Open                                                               |
| [#1509](https://github.com/nektos/act/issues/1509) | RUNNER_OS should return macOS instead of darwin                        | Open                                                               |

### Closed/Resolved Issues

| Issue                                            | Title                                                 | Status                            |
| ------------------------------------------------ | ----------------------------------------------------- | --------------------------------- |
| [#97](https://github.com/nektos/act/issues/97)   | Add support for `--no-container` to run windows/macos | Closed (implemented via PR #1293) |
| [#475](https://github.com/nektos/act/issues/475) | Skipping unsupported platform 'macos-latest'          | Closed (duplicate of #97)         |

### Summary of macOS Support State

The core functionality works (`-P macos-latest=-self-hosted`), but the experience is rough:

- Error messages when misconfigured are cryptic and unhelpful
- No autoconfiguration for macOS (you must know the right flags)
- Documentation for macOS usage is sparse/scattered
- Several environment variable bugs remain open
- `setup-*` actions frequently break due to path assumptions

______________________________________________________________________

## 7. Best Way to Test CI/CD Workflows Locally in 2026

### Tiered Approach (Recommended)

**Tier 1 -- Static Analysis (instant, no execution)**:

- `actionlint` -- catches syntax errors, type mismatches, invalid runner labels, bad cron expressions
- `zizmor` -- catches security vulnerabilities in workflows
- Cost: zero; runs in milliseconds
- Install: `brew install actionlint` + `pip install zizmor`

**Tier 2 -- Local Execution with `act` (seconds to minutes)**:

- Best for: smoke-testing workflow logic, debugging step ordering, verifying conditionals
- For Ubuntu workflows: `act` with Docker images (default mode)
- For macOS workflows: `act -P macos-latest=-self-hosted` (host execution mode)
- Use `.actrc` to persist platform configuration
- Limitations: imperfect fidelity to GitHub's environment; some actions won't work

**Tier 3 -- Nix-based Environment Parity (for Nix projects)**:

- Define your CI environment in a Nix flake
- Use `nix flake check` locally -- runs the exact same checks as CI
- Use `nix develop` to enter the same shell environment
- **Best fidelity** for projects already using Nix (like this dotfiles repo)

**Tier 4 -- macOS VM Isolation (minutes to set up, high fidelity)**:

- Use Tart to create ephemeral macOS VMs
- Install a self-hosted runner or `act` inside the VM
- Best for: Xcode builds, macOS-specific testing, security-sensitive CI
- Tools: Tart, Tartelet, Cilicon, Orchard

**Tier 5 -- Real Self-Hosted Runner (full fidelity)**:

- Register your Mac as a GitHub self-hosted runner
- Jobs run exactly as they would on GitHub, on your hardware
- Best for: validating exact CI behavior, performance testing
- Downside: requires GitHub connectivity; your machine is a real runner

### General Best Practice

```
Write/edit workflow YAML
         |
         v
  actionlint + zizmor    <-- catches 80% of issues instantly
         |
         v
  act (local execution)  <-- catches logic/ordering issues
         |
         v
  Push to GitHub         <-- final validation on real runners
```

______________________________________________________________________

## 8. Recommendations for This Dotfiles Repo

This repo's CI (`macos-latest` runner, Nix flake) is already well-structured for local testing:

### What You Already Have

- `nix flake check --all-systems` runs all checks locally
- `nix develop .#run --command ./scripts/lint.sh` executes the exact CI lint suite
- `just l` shortcuts for individual linters

### What You Could Add

1. **actionlint** -- add to flake devShell or install via Homebrew:

   ```bash
   actionlint .github/workflows/lint.yml
   ```

   This would catch workflow YAML errors without pushing.

1. **`act` with `.actrc`** -- for testing the full workflow orchestration locally:

   ```
   # .actrc (project root, gitignored or chezmoi-ignored)
   -P macos-latest=-self-hosted
   --artifact-server-path=/tmp/act-artifacts
   ```

   Then run:

   ```bash
   act -j lint
   ```

   This would execute the lint job directly on your Mac, using your real Nix installation.

1. **Consider NOT using `act`** -- given that your CI is purely `nix flake check` + `lint.sh`, you
   already have perfect local/CI parity through Nix. Adding `act` would add complexity without much
   benefit beyond validating the workflow YAML structure (which `actionlint` handles better).

### Bottom Line for This Repo

Your current `nix develop .#run --command ./scripts/lint.sh` approach is already the gold standard for
local CI testing. The main gap is workflow YAML validation, which `actionlint` fills perfectly without
the overhead of `act`.

______________________________________________________________________

## Sources

- [act Runners Documentation](https://nektosact.com/usage/runners.html)
- [nektos/act GitHub Repository](https://github.com/nektos/act)
- [act Issue #97: --no-container for Windows/macOS](https://github.com/nektos/act/issues/97)
- [act Issue #475: Skipping unsupported platform macos-latest](https://github.com/nektos/act/issues/475)
- [act Issue #2445: Autoconfigure for Mac platform](https://github.com/nektos/act/issues/2445)
- [act Issue #5974: self-hosted macOS /Users/runner directory](https://github.com/nektos/act/issues/5974)
- [act Issue #5946: No macOS documentation](https://github.com/nektos/act/issues/5946)
- [act Issue #2579: RUNNER_ARCH and -self-hosted](https://github.com/nektos/act/issues/2579)
- [act Issue #1509: RUNNER_OS returns darwin](https://github.com/nektos/act/issues/1509)
- [Tart -- macOS VMs on Apple Silicon](https://github.com/cirruslabs/tart)
- [Orchard -- Tart VM Orchestrator](https://github.com/cirruslabs/orchard)
- [Tartelet -- GitHub Actions in Tart VMs](https://github.com/shapehq/tartelet)
- [Cilicon -- Ephemeral macOS CI](https://github.com/traderepublic/Cilicon)
- [ChristopherHX/github-act-runner](https://github.com/ChristopherHX/github-act-runner)
- [github/local-action -- Official GitHub Tool](https://github.com/github/local-action)
- [actionlint -- Workflow Linter](https://github.com/rhysd/actionlint)
- [zizmor -- Security Linter](https://github.com/zizmorcore/zizmor)
- [act-js -- Programmatic Testing](https://github.com/kiegroup/act-js)
- [mock-github -- Git/GitHub API Mocking](https://github.com/redhat-developer/mock-github)
- [Tart + act Gist](https://gist.github.com/YOU54F/3ac099e54e48a31a69ac2d671aa878f6)
- [github-nix-ci -- Nix-based Self-Hosted Runners](https://github.com/juspay/github-nix-ci)
- [Nix GitHub Actions Integration (Determinate Systems)](https://determinate.systems/blog/nix-github-actions/)
- [GitHub Actions 2026 Runner Updates](https://github.blog/changelog/2026-02-05-github-actions-early-february-2026-updates/)
- [Self-hosting macOS GitHub Runners (Joseph Duffy)](https://josephduffy.co.uk/posts/self-hosting-macos-github-runners)
- [macOS CI/CD with Tart (Snowflake Blog)](https://medium.com/snowflake/macos-ci-cd-with-tart-d3c0e511f3c9)
