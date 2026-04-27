# Isolating GitHub Actions Local Testing on macOS

**Date:** 2026-04-12 **Problem:** `act -P macos-latest=-self-hosted` runs directly on the host Mac with
zero isolation. It can modify files, install packages, and affect system state.

______________________________________________________________________

## Table of Contents

1. [Option 1: Tart VMs (Recommended)](#option-1-tart-vms-recommended)
1. [Option 2: Cirrus CLI + Tart (Alternative to act)](#option-2-cirrus-cli--tart-alternative-to-act)
1. [Option 3: Tartelet (Full ephemeral runner)](#option-3-tartelet-full-ephemeral-runner)
1. [Option 4: sandbox-exec Wrappers](#option-4-sandbox-exec-wrappers)
1. [Option 5: SandVault (User Account Isolation)](#option-5-sandvault-user-account-isolation)
1. [Option 6: Lima VMs](#option-6-lima-vms)
1. [Option 7: Docker-based Approaches](#option-7-docker-based-approaches)
1. [Option 8: Nix Shells](#option-8-nix-shells)
1. [What GitHub Recommends](#what-github-recommends)
1. [What the Community Does](#what-the-community-does)
1. [Comparison Matrix](#comparison-matrix)
1. [Recommended Approach for This Repo](#recommended-approach-for-this-repo)

______________________________________________________________________

## Option 1: Tart VMs (Recommended)

**What it is:** [Tart](https://github.com/cirruslabs/tart) is a virtualization toolset from Cirrus Labs
that manages macOS and Linux VMs on Apple Silicon using Apple's native Virtualization.Framework.

**Isolation level:** Full VM isolation. The guest OS has its own kernel, filesystem, and network stack.
Even if a workflow runs `rm -rf /`, it only affects the disposable VM.

### Setup

```bash
# Install
brew install cirruslabs/cli/tart

# Pull a pre-built macOS image (~25 GB download, one-time)
tart clone ghcr.io/cirruslabs/macos-sequoia-base:latest sequoia-runner

# Clone a disposable copy for each test run
tart clone sequoia-runner test-run-001

# Start the VM (default: 2 CPUs, 4 GB RAM)
tart run test-run-001

# Or customize resources
tart set test-run-001 --cpu 4 --memory 8192
tart run test-run-001
```

### Passing the Repo into the VM

**Directory sharing (recommended):**

```bash
tart run test-run-001 --dir=repo:~/path/to/repo
```

Inside the VM, the repo appears at `/Volumes/My Shared Files/repo`. This is a read-write mount by
default. For read-only:

```bash
tart run test-run-001 --dir=repo:~/path/to/repo:ro
```

**Running commands without SSH (tart exec):**

The Tart Guest Agent (included in all non-vanilla Cirrus images) enables `tart exec`:

```bash
tart exec test-run-001 -- bash -c 'cd "/Volumes/My Shared Files/repo" && act -P macos-latest=-self-hosted'
```

This uses gRPC over virtio-vsock (no SSH, no network needed). Each `tart exec` invocation streams I/O and
returns the exit code.

**SSH approach (fallback):**

```bash
ssh admin@$(tart ip test-run-001)
# Default credentials for Cirrus images: admin/admin
```

### Running act Inside the VM

```bash
# Inside the VM, install act
brew install act

# Run the workflow (self-hosted is fine because the VM IS the sandbox)
cd "/Volumes/My Shared Files/repo"
act -P macos-latest=-self-hosted
```

### Disposable Workflow (Script)

```bash
#!/bin/bash
set -euo pipefail

VM_NAME="act-run-$(date +%s)"
BASE_IMAGE="sequoia-runner"

# Clone a fresh VM
tart clone "$BASE_IMAGE" "$VM_NAME"

# Start in background
tart run "$VM_NAME" --dir=repo:"$PWD" --no-graphics &
VM_PID=$!

# Wait for VM to boot and get IP
sleep 10

# Run the workflow inside the VM
tart exec "$VM_NAME" -- bash -c '
  cd "/Volumes/My Shared Files/repo"
  brew install act 2>/dev/null || true
  act -P macos-latest=-self-hosted
'
EXIT_CODE=$?

# Tear down
tart stop "$VM_NAME" || true
tart delete "$VM_NAME" || true

exit $EXIT_CODE
```

### Performance

- **Near-native** due to Apple's Virtualization.Framework (hardware acceleration)
- VM boot time: ~10-30 seconds
- 12% slower when running 2 VMs in parallel vs 1 (per Shape's benchmarks)
- Cloning a base image is fast (copy-on-write with APFS)

### Resource Requirements

- **Disk:** ~25 GB per macOS base image (stored once). Clones use copy-on-write, so minimal extra space
  until divergence.
- **RAM:** 4 GB default per VM (configurable)
- **CPU:** 2 cores default per VM (configurable)
- **Constraint:** Apple's Virtualization.Framework allows a maximum of 2 concurrent macOS VMs per host.

### Networking

- Default: NAT (VM can reach internet, host can reach VM via `tart ip`)
- Softnet: Strict isolation where the VM cannot access the host at all
- Bridged: VM gets its own IP on the local network

### Limitations

- macOS images are ~25 GB (one-time download)
- Max 2 concurrent macOS VMs per host (Apple licensing restriction)
- `tart cp` (file copy to/from VM) is not yet implemented; use directory sharing or SSH/scp
- Nested virtualization requires M3/M4 chips + macOS 15+

______________________________________________________________________

## Option 2: Cirrus CLI + Tart (Alternative to act)

**What it is:** [Cirrus CLI](https://github.com/cirruslabs/cirrus-cli) is a local CI runner from the same
team that made Tart. Instead of using `act` to run GitHub Actions, you use `cirrus run` to execute tasks
inside Tart VMs. This is a different workflow definition format but provides isolation by default.

### Setup

```bash
brew install cirruslabs/cli/cirrus
brew install cirruslabs/cli/tart

# Pull a base image
tart clone ghcr.io/cirruslabs/macos-sequoia-base:latest sequoia-base
```

### Configuration

Create `.cirrus.yml` in your repo root:

```yaml
task:
  name: lint
  macos_instance:
    image: ghcr.io/cirruslabs/macos-sequoia-base:latest
  install_nix_script:
    - curl -L https://nixos.org/nix/install | sh
  lint_script:
    - nix develop .#run --command ./scripts/lint.sh
```

Run locally:

```bash
cirrus run
```

Cirrus CLI automatically:

1. Starts a Tart VM
1. Copies the working directory into the VM
1. Runs the scripts inside the VM
1. Tears down the VM

### Pros

- Isolation is automatic and built-in (every task runs in a fresh VM)
- No need to manually manage VM lifecycle
- Same config works locally and in CI (Cirrus CI, or via
  [cirrus-action](https://github.com/cirruslabs/cirrus-action) in GitHub Actions)

### Cons

- Requires rewriting workflows from GitHub Actions YAML to `.cirrus.yml` format
- Not 1:1 compatible with GitHub Actions syntax
- An additional config file to maintain alongside `.github/workflows/`

______________________________________________________________________

## Option 3: Tartelet (Full Ephemeral Runner)

**What it is:** [Tartelet](https://github.com/shapehq/tartelet) is a macOS app that manages ephemeral
GitHub Actions runners inside Tart VMs. It is designed for self-hosted runner farms, but works for local
testing.

### How It Works

1. Clones a Tart VM
1. Boots the VM
1. SSHes in and installs the GitHub Actions runner application
1. Registers the runner with your GitHub org/repo
1. Runner picks up and executes one job
1. Deregisters the runner
1. Shuts down and deletes the VM
1. Loops back to step 1

### Setup

1. Create a GitHub App with runner permissions
1. Install Tart and pull a base image
1. Install Tartelet from [releases](https://github.com/shapehq/tartelet/releases)
1. Configure: select VM image, enter SSH credentials (admin/admin for Cirrus images), enter GitHub App
   details

### Pros

- Fully automated ephemeral runner lifecycle
- True GitHub Actions compatibility (runs the real runner, not act)
- Each job gets a pristine VM

### Cons

- Designed for real GitHub Actions jobs (triggered by pushes/PRs), not ad-hoc local testing
- Requires a GitHub App setup
- Max 2 runners (2 VMs) simultaneously
- More infrastructure than needed for "test my workflow locally"

______________________________________________________________________

## Option 4: sandbox-exec Wrappers

**What it is:** macOS includes a built-in sandboxing facility called `sandbox-exec` that restricts what a
process can do at the kernel level. Several tools wrap it for practical use.

### Option 4a: Agent Safehouse

[Agent Safehouse](https://github.com/eugene1g/agent-safehouse) is a single Bash script that applies a
deny-first sandbox profile to any CLI command.

```bash
brew install eugene1g/safehouse/agent-safehouse

# Run act with sandboxing (only allow access to the repo directory)
safehouse --add-dirs="$PWD" act -P macos-latest=-self-hosted
```

**What it blocks by default:**

- All home directory access (except metadata traversal)
- `~/.ssh`, `~/.gnupg`, `~/.docker`, `~/.cargo`, `~/.gradle`
- `~/Documents`, `~/Desktop`, `~/Downloads`
- Password manager app containers
- Sensitive Library subdirectories (Mail, Messages, Photos, Safari, Contacts)

**What it allows:**

- The explicitly granted directory (your repo)
- System binaries and libraries
- Network access (can be restricted with additional profile rules)

**Limitations:**

- macOS only; relies on deprecated (but still functional) `sandbox-exec`
- Not a VM -- process-level isolation only
- A determined attacker could potentially find sandbox escapes
- Cannot restrict CPU/memory usage
- Recursive sandboxing not supported (breaks Swift/xcodebuild)

### Option 4b: bx-mac

[bx-mac](https://github.com/holtwick/bx-mac) takes an allow-first approach (opposite of Safehouse):

```bash
bx exec ~/my-repo -- act -P macos-latest=-self-hosted
```

It scans `$HOME`, blocks sibling directories and sensitive locations, and allows only the specified
working directory.

**Key difference from Safehouse:** bx-mac uses an allow-first / blocklist model (everything accessible by
default, sensitive paths blocked). Safehouse uses deny-first (everything blocked, only specified paths
allowed). Safehouse is more secure; bx-mac is more compatible.

### Option 4c: Raw sandbox-exec

You can write your own profile:

```bash
cat > /tmp/act-sandbox.sb << 'EOF'
(version 1)
(deny default)

; Allow reading system files
(allow file-read* (subpath "/usr"))
(allow file-read* (subpath "/bin"))
(allow file-read* (subpath "/sbin"))
(allow file-read* (subpath "/System"))
(allow file-read* (subpath "/Library"))
(allow file-read* (subpath "/private/etc"))
(allow file-read* (subpath "/private/var"))
(allow file-read* (subpath "/opt/homebrew"))
(allow file-read-metadata)

; Allow read-write only in the repo
(allow file-read* (subpath "/Users/stephen/.local/share/chezmoi"))
(allow file-write* (subpath "/Users/stephen/.local/share/chezmoi"))

; Allow read-write in tmp
(allow file-read* (subpath "/private/tmp"))
(allow file-write* (subpath "/private/tmp"))

; Allow process execution
(allow process-exec*)
(allow process-fork)

; Allow network (needed for act to pull actions)
(allow network*)

; Allow sysctl, mach, ipc
(allow sysctl*)
(allow mach*)
(allow ipc*)
(allow signal)
EOF

sandbox-exec -f /tmp/act-sandbox.sb act -P macos-latest=-self-hosted
```

**Warning:** sandbox-exec is deprecated by Apple. It still works as of macOS 26 (Tahoe), but Apple could
remove it in a future release. The profile syntax is undocumented Scheme/LISP and may change.

______________________________________________________________________

## Option 5: SandVault (User Account Isolation)

**What it is:** [SandVault](https://github.com/webcoyote/sandvault) creates a dedicated limited macOS
user account (`sandvault-$USER`) and runs commands as that user, combined with `sandbox-exec` for
additional restrictions.

### How It Works

Two-layer isolation:

1. **User account separation:** Commands run as a dedicated limited user with no access to your home
   directory
1. **sandbox-exec overlay:** Further restricts filesystem access on top of the user separation

### Setup

```bash
# Install (requires admin for user creation)
brew install webcoyote/sandvault/sandvault

# Run act in the sandbox
sandvault act -P macos-latest=-self-hosted
```

The workspace lives at `/Users/Shared/sv-$USER`. You would need to clone/copy your repo there.

### Pros

- Stronger than sandbox-exec alone (user separation + sandbox)
- No VM overhead
- Near-instant startup

### Cons

- Nested sandboxing fails (Swift/xcodebuild break)
- No GUI application support
- Shared workspace requires file management
- Still shares the same kernel and system packages as the host

______________________________________________________________________

## Option 6: Lima VMs

**What it is:** [Lima](https://github.com/lima-vm/lima) is a CNCF incubating project for running Linux
VMs on macOS. As of v2.1 (March 2026), it experimentally supports macOS guests.

### macOS Guest Support

```bash
brew install lima
limactl start template:macos
```

This creates a macOS VM using the `vz` (Virtualization.Framework) driver. You can then run commands
inside it.

### Pros

- CNCF project with active development
- Supports both Linux and (experimental) macOS guests
- Better documented than raw Tart for some use cases
- Built-in AI agent safety features (v2.1)

### Cons

- macOS guest support is experimental
- Less mature than Tart for macOS-specific workflows
- Fewer pre-built macOS images available
- Primarily designed for Linux VMs

______________________________________________________________________

## Option 7: Docker-based Approaches

### Can act Use a macOS Docker Container?

**No.** There is no macOS Docker image. macOS cannot run in a Docker container because Docker containers
share the host kernel, and macOS requires the XNU kernel which Docker (Linux kernel) cannot provide.

### Linux Containers as Approximation

If your workflow does not require macOS-specific tools (Xcode, macOS SDK), you can run `act` with Linux
containers:

```bash
# Default act behavior -- uses Docker containers
act
```

This provides full container isolation but runs Linux, not macOS. For workflows that only run shellcheck,
shfmt, nix, and similar cross-platform tools (like the lint workflow in this repo), this may be
sufficient.

### The Fundamental Problem

`act` was designed around Docker containers for Linux. macOS support was bolted on via `-self-hosted`
which just means "run on the host." There is an
[open feature request](https://github.com/nektos/act/issues/2105) to integrate Tart as a VM backend for
act, but it has been in the backlog since November 2023 with no implementation.

______________________________________________________________________

## Option 8: Nix Shells

**What it is:** Use `nix develop` to create a reproducible, isolated development environment.

### For This Repo Specifically

The repo already uses Nix:

```bash
nix develop .#run --command ./scripts/lint.sh
```

This provides:

- **Dependency isolation:** Tools come from the Nix store, not the system
- **Reproducibility:** Same tools on any machine

### What It Does NOT Provide

- **Filesystem isolation:** Nix shells do not restrict filesystem access
- **Process isolation:** No sandboxing of what commands can do
- **Network isolation:** No network restrictions

Nix is complementary to the other approaches, not a replacement. It ensures the right tools are available
but does not prevent a malicious workflow from reading `~/.ssh/id_rsa`.

______________________________________________________________________

## What GitHub Recommends

From the
[official documentation](https://docs.github.com/en/actions/reference/runners/self-hosted-runners):

1. **Use the `--ephemeral` flag** when configuring self-hosted runners. This makes the runner accept only
   one job and automatically deregister afterward.

1. **Never use self-hosted runners with public repositories.** Forks could submit PRs that run arbitrary
   code on your runner.

1. **Treat runners as ephemeral.** Use automation to provide a clean environment for each job.

1. **Use Actions Runner Controller (ARC)** on Kubernetes for automatic Pod-per-job isolation.

GitHub provides NO built-in isolation for self-hosted runners. They explicitly state: "Self-hosted
runners for GitHub do not have guarantees around running in ephemeral clean virtual machines, and can be
persistently compromised by untrusted code in a workflow."

______________________________________________________________________

## What the Community Does

Based on GitHub Discussions, issues, and blog posts:

1. **Most common:** Accept the risk and run `act -self-hosted` directly on the host. This is the path of
   least resistance and what most individuals do for local testing.

1. **Infrastructure teams:** Use Tart + Tartelet or similar to run ephemeral VMs per job. This is common
   for companies with Mac Mini farms.

1. **Security-conscious individuals:** Use sandbox-exec wrappers (Safehouse, bx-mac) for lightweight
   process-level isolation.

1. **Linux-only workflows:** Run `act` with default Docker containers, avoiding the macOS problem
   entirely.

1. **Alternative CI systems:** Switch to Cirrus CLI + Tart for local testing, keeping GitHub Actions for
   the actual CI.

The [act issue tracker](https://github.com/nektos/act/issues/2105) shows community demand for Tart
integration, but no implementation exists yet. The act maintainer for Apple platforms acknowledged the
request but cannot develop for Apple M-series platforms personally.

______________________________________________________________________

## Comparison Matrix

| Approach              | Isolation Level   | Setup Effort | Performance        | macOS Compat   | Cost              |
| --------------------- | ----------------- | ------------ | ------------------ | -------------- | ----------------- |
| **Tart VM + act**     | Full VM           | Medium       | Near-native        | Full macOS     | Free + 25GB disk  |
| **Cirrus CLI + Tart** | Full VM           | Medium       | Near-native        | Full macOS     | Free + 25GB disk  |
| **Tartelet**          | Full VM           | High         | Near-native        | Full macOS     | Free + GitHub App |
| **Safehouse**         | Process sandbox   | Low          | Zero overhead      | Most tools     | Free              |
| **SandVault**         | User + sandbox    | Medium       | Zero overhead      | No Swift/Xcode | Free              |
| **bx-mac**            | Process sandbox   | Low          | Zero overhead      | Most tools     | Free              |
| **Raw sandbox-exec**  | Process sandbox   | Medium       | Zero overhead      | Most tools     | Free              |
| **Lima**              | Full VM           | Medium       | Near-native        | Experimental   | Free              |
| **Docker (Linux)**    | Container         | Low          | Container overhead | Linux only     | Free              |
| **Nix shell**         | Dependencies only | Already done | Zero overhead      | N/A            | Free              |

______________________________________________________________________

## Recommended Approach for This Repo

Given that this repo's CI workflow (`.github/workflows/lint.yml`) runs on `macos-latest` and executes
`nix flake check --all-systems` plus the lint suite (shellcheck, shfmt, mdformat, nixfmt), here are two
practical paths:

### Quick Win: Safehouse + act (5 minutes)

For lightweight protection against accidental damage:

```bash
brew install eugene1g/safehouse/agent-safehouse

# Test the lint workflow with filesystem sandboxing
safehouse --add-dirs="$PWD" act -P macos-latest=-self-hosted -W .github/workflows/lint.yml
```

This prevents the workflow from touching anything outside the repo directory. It does NOT provide
VM-level isolation, but it stops the most common accidents (overwriting dotfiles, installing system
packages to unexpected locations, reading secrets).

### Full Isolation: Tart VM + act (30 minutes first time, then seconds)

For true isolation:

```bash
# One-time setup
brew install cirruslabs/cli/tart
tart clone ghcr.io/cirruslabs/macos-sequoia-base:latest act-base

# Install act and nix in the base image (one-time)
tart run act-base  # then inside the VM:
# brew install act
# curl -L https://nixos.org/nix/install | sh
# exit
# tart stop act-base

# Per-test-run (fast -- APFS copy-on-write clone)
tart clone act-base act-run
tart run act-run --dir=repo:"$PWD" --no-graphics &
sleep 15
tart exec act-run -- bash -c '
  source ~/.zprofile
  cd "/Volumes/My Shared Files/repo"
  act -P macos-latest=-self-hosted -W .github/workflows/lint.yml
'
tart stop act-run && tart delete act-run
```

This gives complete isolation. The host is never touched. The VM is destroyed after each run.

### Alternative: Skip act, Use Cirrus CLI

If you are willing to maintain a `.cirrus.yml` alongside your GitHub Actions workflow:

```yaml
# .cirrus.yml
task:
  name: lint
  macos_instance:
    image: ghcr.io/cirruslabs/macos-sequoia-base:latest
  install_script:
    - curl -L https://nixos.org/nix/install | sh
  lint_script:
    - source ~/.nix-profile/etc/profile.d/nix.sh
    - nix develop .#run --command ./scripts/lint.sh
```

```bash
brew install cirruslabs/cli/cirrus
cirrus run
```

______________________________________________________________________

## Sources

- [Tart - GitHub](https://github.com/cirruslabs/tart)
- [Tart Quick Start](https://tart.run/quick-start/)
- [Tart FAQ](https://tart.run/faq/)
- [Tart Guest Agent Blog Post](https://tart.run/blog/2025/06/01/bridging-the-gaps-with-the-tart-guest-agent/)
- [Cirrus CLI - Tart Integration](https://tart.run/integrations/cirrus-cli/)
- [Tartelet - GitHub](https://github.com/shapehq/tartelet)
- [Tartelet Wiki - Setup](https://github.com/shapehq/tartelet/wiki/Setting-Up-a-Host-Machine)
- [Self-hosting macOS GitHub Runners - Joseph Duffy](https://josephduffy.co.uk/posts/self-hosting-macos-github-runners)
- [act - GitHub](https://github.com/nektos/act)
- [act Runners Documentation](https://nektosact.com/usage/runners.html)
- [act Tart Integration Request - Issue #2105](https://github.com/nektos/act/issues/2105)
- [Running act in Tart VMs - Gist](https://gist.github.com/YOU54F/3ac099e54e48a31a69ac2d671aa878f6)
- [Agent Safehouse - GitHub](https://github.com/eugene1g/agent-safehouse)
- [Agent Safehouse - Hacker News](https://news.ycombinator.com/item?id=47301085)
- [SandVault - GitHub](https://github.com/webcoyote/sandvault)
- [bx-mac - GitHub](https://github.com/holtwick/bx-mac)
- [sandbox-exec Guide](https://igorstechnoclub.com/sandbox-exec/)
- [Sandboxing Claude Code on macOS - Infralovers](https://www.infralovers.com/blog/2026-02-15-sandboxing-claude-code-macos/)
- [Lima - GitHub](https://github.com/lima-vm/lima)
- [Lima v2.1 - CNCF Blog](https://www.cncf.io/blog/2026/03/25/lima-v2-1-macos-guests-and-enhanced-ai-agent-safety/)
- [GitHub Self-Hosted Runners Docs](https://docs.github.com/en/actions/reference/runners/self-hosted-runners)
- [GitHub Discussion #180866 - Sandboxing Workflows](https://github.com/orgs/community/discussions/180866)
- [GitHub Ephemeral Runners Changelog](https://github.blog/changelog/2021-09-20-github-actions-ephemeral-self-hosted-runners-new-webhooks-for-auto-scaling/)
- [Safehouse - Tessl Blog](https://tessl.io/blog/safehouse-sandboxes-ai-coding-agents-on-macos/)
