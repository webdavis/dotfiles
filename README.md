<p align="center">
  <img src="./assets/logo.png" alt="Dotfiles Icon" width="200" height="200" />
</p>

# Dotfiles for Webdavis

[![Lint](https://github.com/webdavis/dotfiles/actions/workflows/lint.yml/badge.svg)](https://github.com/webdavis/dotfiles/actions/workflows/lint.yml)

This repository contains the settings/configs for my computers, managed using
[Chezmoi](https://www.chezmoi.io/).

<!-- table-of-contents GFM -->

- [Prerequisites](#prerequisites)
- [Setup](#setup)
- [Managing Files Using Chezmoi](#managing-files-using-chezmoi)
- [Development Environment](#development-environment)
  - [Install](#install)
  - [Usage](#usage)
    - [1. Enter the Dev Shell](#1-enter-the-dev-shell)
    - [2. Run Commands Adhoc](#2-run-commands-adhoc)
  - [Bonus: justfile](#bonus-justfile)

<!-- table-of-contents -->

## Prerequisites

I use Chezmoi's [`keepassxc-cli`](https://www.chezmoi.io/user-guide/password-managers/keepassxc/)
password manager to manage my dotfile secrets, which means this project requires
[KeePassXC](https://keepassxc.org/):

```bash
brew install --cask keepassxc
```

## Setup

To use these dotfiles on your system:

1. **Install Chezmoi**

   Follow the instructions for your platform:
   [https://www.chezmoi.io/install/](https://www.chezmoi.io/install/)

   `macOS` example:

   ```bash
   brew install chezmoi
   ```

1. **Initialize this setup**

   ```bash
   chezmoi init --apply webdavis
   ```

   This will automatically find and clone `webdavis/dotfiles` from GitHub to the local path
   `~/.local/share/chezmoi/`.

## Managing Files Using Chezmoi

These are the bread and butter:

```bash
$ chezmoi status
$ chezmoi diff
$ chezmoi apply
```

Add files like so:

```bash
chezmoi add <FILE>
```

Chezmoi supports templating using Golang templates. Always edit template files using this abstraction:

```bash
chezmoi edit <FILE>
```

## Development Environment

This project's development environment is managed using [Nix Flakes](https://wiki.nixos.org/wiki/Flakes),
and is defined in the [`flake.nix`](./flake.nix) file.

### Install

Install Nix using the
[Nix Installer from Determinate Systems](https://github.com/DeterminateSystems/nix-installer):

```bash
curl -fsSL https://install.determinate.systems/nix | sh -s -- install
```

> [!IMPORTANT]
>
> If you're on macOS and using [nix-darwin](https://github.com/nix-darwin/nix-darwin), when prompted with
> `Install Determinate Nix?`, say `no`
>
> - **Why:** As of `2025-10-07`, Determinate Nix is incompatible with nix-darwin 25.05

### Usage

You have two options for using the flake environment:

#### 1. Enter the Dev Shell

Drop into a persistent development shell with all tools provisioned by the flake:

```bash
nix develop
```

For example, once inside this shell you can lint the project's [`dot_Brewfile`](./dot_Brewfile) with
[RuboCop](https://github.com/rubocop/rubocop) by running Bundler directly:

```bash
bundle exec rubocop dot_Brewfile
```

#### 2. Run Commands Adhoc

Run a single command in a temporary environment without entering the shell:

```bash
nix develop .#adhoc --command ./scripts/lint.sh
```

> [!TIP]
>
> You can replace `./scripts/lint.sh` with any command you want to execute inside the development
> environment (e.g. `bundle exec rubocop dot_Brewfile`).

### Bonus: justfile

This repo provides a [`justfile`](./justfile) for quick command execution. To execute the linter within
the Nix flake shell ad-hoc style simply run:

```bash
just l
```
