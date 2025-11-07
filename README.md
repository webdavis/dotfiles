<p align="center">
  <img src="./assets/logo.png" alt="Dotfiles Icon" width="200" height="200" />
</p>

# Dotfiles

[![Lint](https://github.com/webdavis/dotfiles/actions/workflows/lint.yml/badge.svg)](https://github.com/webdavis/dotfiles/actions/workflows/lint.yml)

This repository contains the dotfiles for my personal computer, managed with
[Chezmoi](https://www.chezmoi.io/).

<!-- table-of-contents GFM -->

- [Prerequisites](#prerequisites)
- [Setup](#setup)
- [Development Environment](#development-environment)
  - [Install](#install)
  - [Usage](#usage)
    - [1. Enter the Dev Shell](#1-enter-the-dev-shell)
    - [2. Run Commands Adhoc](#2-run-commands-adhoc)

<!-- table-of-contents -->

## Prerequisites

- [KeePassXC](https://keepassxc.org/)

I use Chezmoi's
[`keepassxc-cli` password manager](https://www.chezmoi.io/user-guide/password-managers/keepassxc/) to
manage secrets in my dotfiles.

Before applying them, make sure it's installed:

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

2. **Initialize Chezmoi with this repository**

```bash
chezmoi init --apply webdavis
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
