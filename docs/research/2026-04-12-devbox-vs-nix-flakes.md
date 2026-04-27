# Devbox Research Report

Research date: 2026-04-12

Evaluating whether [Devbox](https://github.com/jetify-com/devbox) can replace the current Nix flake +
direnv setup used in the chezmoi dotfiles repository.

## 1. Does devbox have a feature that replaces direnv?

**No. Devbox does not replace direnv -- it complements it.**

Devbox has no built-in mechanism for automatically activating a shell environment when you `cd` into a
directory. For automatic activation, Devbox explicitly relies on direnv. The official workflow is:

1. Run `devbox generate direnv` in your project directory.
1. This creates a `.envrc` file containing `eval "$(devbox generate direnv --print-envrc)"`.
1. direnv detects the `.envrc` on directory entry and activates the Devbox environment.

Without direnv, the only options are:

- **`devbox shell`** -- manually spawns an interactive sub-shell with the environment active. You must
  type this every time.
- **`devbox shellenv`** -- prints shell export commands you can `eval` in your current shell or add to
  your `~/.bashrc`. This makes packages globally available but does not provide per-project activation.
- **`devbox global shellenv`** -- same as above but for globally installed Devbox packages.

The Jetify team's own blog post states: "The combination of easy shell environments with Devbox, combined
with convenient environment switching with Direnv makes it simple to manage multiple projects." They are
designed as complementary tools.

**Bottom line:** If you want auto-activation on `cd`, you still need direnv. Devbox replaces the
flake.nix (the environment definition), not direnv (the activation trigger).

### direnv limitation to be aware of

direnv creates a sub-shell with environment variable diffs only. It cannot load shell aliases, functions
sourced in `init_hook`, or modify `$PS1`. Those features only work with `devbox shell`, `devbox run`, or
`devbox services`.

## 2. How does devbox shell activation work?

Devbox provides two activation modes:

### Interactive shell (`devbox shell`)

1. Detects your shell from `$SHELL`.
1. Generates a temporary shellrc file that:
   - Sources your original RC file (preserves existing customizations).
   - Exports all Devbox environment variables.
   - Adds a `(devbox)` prompt prefix (unless `$DEVBOX_NO_PROMPT` is set).
   - Sources init hooks from `.devbox/gen/scripts/.hooks.sh`.
1. Launches a sub-shell using shell-specific mechanisms:
   - bash: `--rcfile` flag
   - zsh: `ZDOTDIR` override
   - fish: `-C ". <shellrc>"`

### Current-shell integration (`devbox shellenv`)

Outputs shell commands that modify `PATH` and set environment variables in the current shell. Used via:

```bash
eval "$(devbox shellenv)"          # per-project
eval "$(devbox global shellenv)"   # global packages
```

**Neither mode auto-activates on directory entry.** That requires direnv.

## 3. Can devbox replicate what the current flake.nix provides?

The current flake provides these packages across `x86_64-linux` and `aarch64-darwin`:

| Package                 | Flake expression                                                  | Devbox equivalent              | Status    |
| ----------------------- | ----------------------------------------------------------------- | ------------------------------ | --------- |
| chezmoi                 | `pkgs.chezmoi`                                                    | `devbox add chezmoi@latest`    | Available |
| shellcheck              | `pkgs.shellcheck`                                                 | `devbox add shellcheck@latest` | Available |
| shfmt                   | `pkgs.shfmt`                                                      | `devbox add shfmt@latest`      | Available |
| nixfmt-tree             | `nixpkgs.legacyPackages.${system}.nixfmt-tree`                    | **Not directly available**     | Problem   |
| mdformat + mdformat-gfm | `pkgs.python312.withPackages (ps: [ps.mdformat ps.mdformat-gfm])` | **Not directly available**     | Problem   |

### Problem 1: nixfmt-tree is not a standard Devbox package

Nixhub.io lists `nixfmt` (the base formatter), but not `nixfmt-tree` (the treefmt-wrapped version) or
`nixfmt-rfc-style`. Running `devbox add nixfmt@latest` would give you plain `nixfmt`, which is
`nixfmt-classic` -- a different formatter with different output.

**Workaround:** Reference the nixpkgs flake attribute directly in `devbox.json`:

```json
{
  "packages": [
    "github:NixOS/nixpkgs/nixos-25.05#nixfmt-tree"
  ]
}
```

This works but defeats the simplicity advantage -- you are writing flake references in JSON instead of
Nix.

### Problem 2: python.withPackages is not supported in devbox.json

Devbox cannot express `python3.withPackages (ps: [ps.mdformat ps.mdformat-gfm])` natively. This is a
known limitation tracked in [GitHub issue #2408](https://github.com/jetify-com/devbox/issues/2408) (filed
November 2024, still open as of April 2026).

The `withPackages` / `withExtensions` pattern is a Nix-specific composition mechanism that Devbox's JSON
configuration cannot represent. Current workarounds:

**Option A -- pip in a virtualenv:**

```json
{
  "packages": ["python@3.12"],
  "shell": {
    "init_hook": [
      "python -m venv .venv",
      "source .venv/bin/activate",
      "pip install mdformat mdformat-gfm"
    ]
  }
}
```

This loses reproducibility (pip resolves at runtime, not from a lockfile) and is slower (runs on every
shell start unless guarded with conditionals).

**Option B -- reference a custom flake:**

Keep a minimal `flake.nix` that builds the Python environment, then reference it in `devbox.json`:

```json
{
  "packages": [
    "path:./nix#mdformat-with-gfm"
  ]
}
```

This defeats the purpose of replacing flake.nix with devbox.

**Option C -- reference individual nixpkgs attributes:**

```json
{
  "packages": [
    "github:NixOS/nixpkgs/nixos-25.05#python312Packages.mdformat",
    "github:NixOS/nixpkgs/nixos-25.05#python312Packages.mdformat-gfm"
  ]
}
```

This may or may not work correctly because mdformat discovers plugins via Python entry points, which
requires them to be in the same Python environment (the whole reason `withPackages` exists). Installing
them as separate Nix packages may result in mdformat not finding the GFM plugin.

### Cross-platform support

Devbox handles cross-platform automatically. Packages from Nixhub resolve to the correct system
architecture without explicit `eachSystem` declarations. This is simpler than the flake approach.

### Shell hooks

The current flake defines a `shfmt` wrapper function (`shfmt -i 2 -ci -s`). Devbox supports this via
`init_hook`:

```json
{
  "shell": {
    "init_hook": [
      "shfmt() { command shfmt -i 2 -ci -s \"$@\"; }"
    ]
  }
}
```

**Note:** This only works with `devbox shell`, not with direnv integration (direnv cannot load shell
functions).

### Two dev shells (default vs run)

The current flake defines two shells: `default` (interactive with colored output) and `run` (headless for
CI/just). Devbox supports only a single shell definition per `devbox.json`. You cannot define multiple
named shells. The CI vs interactive distinction would need to be handled differently (e.g., conditional
logic in `init_hook` or separate scripts).

## 4. Migration path from flake.nix + direnv to devbox

Given the limitations above, a full migration is not clean. Here is what it would look like:

### Step 1: Install devbox

```bash
curl -fsSL https://get.jetify.com/devbox | bash
```

### Step 2: Initialize

```bash
cd ~/.local/share/chezmoi
devbox init
```

### Step 3: Add packages

```bash
devbox add chezmoi@latest
devbox add shellcheck@latest
devbox add shfmt@latest
```

### Step 4: Handle nixfmt-tree (workaround)

Edit `devbox.json` and add the flake reference manually:

```json
"github:NixOS/nixpkgs/nixos-25.05#nixfmt-tree"
```

Or accept `nixfmt` (which is now `nixfmt-rfc-style` as of nixfmt 1.x) instead of `nixfmt-tree`. Since
nixfmt 1.0+, `nixfmt` on nixhub may actually be the RFC-style formatter. Verify with
`devbox add nixfmt@1.2.0` and check `nixfmt --version` output.

### Step 5: Handle mdformat + GFM plugin (hardest part)

**Recommended approach:** Use the pip/virtualenv workaround in `init_hook`:

```json
{
  "shell": {
    "init_hook": [
      "if [ ! -d .venv ]; then python -m venv .venv; fi",
      "source .venv/bin/activate",
      "pip install -q mdformat mdformat-gfm"
    ]
  }
}
```

Or keep a minimal flake.nix just for the Python environment and reference it.

### Step 6: Set up direnv integration

```bash
devbox generate direnv
```

This creates `.envrc`. You still need direnv installed and hooked into your shell.

### Step 7: Update CI

Change `.github/workflows/lint.yml` from `nix develop .#run --command ./scripts/lint.sh` to
`devbox run lint` (after defining a `lint` script in `devbox.json`).

### Step 8: Remove old files

Delete `flake.nix`, `flake.lock`. Keep `.envrc` if using direnv (but it is now generated by devbox).

## 5. Trade-offs: what you lose going from raw Nix flakes to devbox

### What you gain

- **Simpler configuration.** JSON instead of Nix language. No need to understand `mkShell`, `let/in`,
  `inherit`, or `flake-utils`.
- **Easier version pinning.** `package@version` syntax with semver instead of tracking nixpkgs commits.
- **Built-in package search.** `devbox search <name>` and [nixhub.io](https://www.nixhub.io/) for version
  discovery.
- **Cross-platform for free.** No `eachSystem` boilerplate.
- **Faster warm startup.** Devbox uses Nix flakes internally and claims up to 70% faster warm shell
  startup vs nix-shell.
- **Lockfile management.** `devbox.lock` is auto-maintained, similar to package-lock.json.
- **Docker/devcontainer generation.** `devbox generate dockerfile` and `devbox generate devcontainer` for
  portable environments.

### What you lose

- **`withPackages` composition.** Cannot bundle Python/PHP/etc packages with plugins in a single
  environment. This is a hard blocker for your mdformat+GFM setup. (Issue #2408, open since Nov 2024.)
- **Multiple named dev shells.** Only one shell per `devbox.json`. Your `default` vs `run` split has no
  direct equivalent.
- **Full Nix expressiveness.** No overlays, no `override`/`overrideAttrs`, no custom derivations in the
  config file. For anything beyond `packages + env + init_hook`, you fall back to writing a flake.nix
  anyway.
- **Shell function support with direnv.** direnv integration cannot load shell functions or aliases
  defined in `init_hook`. Your `shfmt` wrapper function would not work with direnv -- only with
  `devbox shell`.
- **Package attribute paths.** Some nixpkgs attributes (like `nixfmt-tree`) are not indexed on Nixhub and
  require manual flake references, losing the simplicity advantage.
- **Nix formatter integration.** `nix fmt` integration (the `formatter` output in flake.nix) has no
  devbox equivalent. You would need to run the formatter directly rather than via `nix fmt`.
- **Ecosystem maturity.** Devbox is a younger project with fewer community resources and less
  battle-testing than raw Nix flakes.
- **Another abstraction layer.** When something breaks, you debug both Devbox and Nix rather than just
  Nix. The abstraction can hide the root cause of failures.

## Recommendation

**For this specific repository, devbox is not a clean replacement for the current flake.nix + direnv
setup.** The two blocking issues are:

1. **mdformat + mdformat-gfm** requires `python.withPackages`, which devbox cannot express natively
   (issue #2408). You would need either a pip virtualenv (losing reproducibility) or a companion
   flake.nix (defeating the purpose).

1. **nixfmt-tree** is not indexed on Nixhub and requires a manual flake reference in devbox.json.

If your needs were limited to simple packages (chezmoi, shellcheck, shfmt), devbox would be a clear win
in simplicity. But the Python plugin composition and nixfmt-tree dependency make the flake.nix approach
strictly more capable for this project.

**If you still want to try devbox**, the most practical approach is a hybrid: use devbox for the simple
packages and reference a minimal flake for the Python environment. But at that point, the flake.nix you
already have is doing the job with less indirection.

### Re-evaluate when

- Devbox ships `withPackages` support (issue #2408).
- Nixhub indexes `nixfmt-tree` / `nixfmt-rfc-style` as a first-class package.
- Your project drops the mdformat-gfm dependency.

## Sources

- [Devbox documentation](https://www.jetify.com/docs/devbox/)
- [Devbox GitHub repository](https://github.com/jetify-com/devbox)
- [Devbox + direnv integration docs](https://www.jetify.com/docs/devbox/ide-configuration/direnv)
- [Devbox + direnv blog post](https://www.jetify.com/blog/automated-dev-envs-with-devbox-and-direnv)
- [Using Nix Flakes with Devbox](https://www.jetify.com/blog/using-nix-flakes-with-devbox)
- [devbox.json configuration reference](https://www.jetify.com/docs/devbox/configuration)
- [Devbox shell integration (DeepWiki)](https://deepwiki.com/jetify-com/devbox/4.2-shell-integration)
- [withPackages support request -- issue #2408](https://github.com/jetify-com/devbox/issues/2408)
- [nixfmt on Nixhub](https://www.nixhub.io/packages/nixfmt)
- [Devbox vs plain Nix discussion](https://github.com/orgs/copier-org/discussions/1468)
- [Devbox vs Nix: why we chose simplicity](https://memo.d.foundation/topics/devbox/introduction/why-devbox-but-not-nix)
- [Upgrade your Development Environments with Devbox](https://alan.norbauer.com/articles/devbox-intro/)
- [Devbox Python + pip template](https://www.jetify.com/devbox/templates/python-pip)
