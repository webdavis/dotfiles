<p align="center">
  <img src="./assets/logo.png" alt="Dotfiles Icon" width="200" height="200" />
</p>

# Dotfiles for Webdavis

[![CI](https://github.com/webdavis/dotfiles/actions/workflows/ci.yml/badge.svg)](https://github.com/webdavis/dotfiles/actions/workflows/ci.yml)

This repository contains the settings/configs for my computers, managed using
[Chezmoi](https://www.chezmoi.io/).

<!-- table-of-contents GFM -->

- [Prerequisites](#prerequisites)
- [Setup](#setup)
- [Managing Files Using Chezmoi](#managing-files-using-chezmoi)
- [Keys](#keys)
- [Development Environment](#development-environment)
  - [Install](#install)
  - [Commands](#commands)

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

   This initializes and applies the dotfiles, including the configured agent skills and harness links. It
   asks once which machine this is (`dresden` or `raith`) and names the Mac to match. To answer up front,
   add `--promptChoice "Which machine is this?=raith"`.

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

Chezmoi supports templating using Golang [text/templates](https://pkg.go.dev/text/template). Always edit
template files using this abstraction:

```bash
chezmoi edit <FILE>
```

## Keys

Every shell key binding is a row in one table,
[`dot_config/chord/bindings.toml`](./dot_config/chord/bindings.toml). Four ways to find one:

- `Ctrl-x v` searches every binding in fzf. Enter puts the chosen command on the line.
- `bindings` prints the table: key, group, description and action. `bindings --group git` prints one
  group; `fzf-pickers` is the group of pickers.
- `Ctrl-x p` lists the fzf pickers, key first. Enter starts the chosen picker, and what you pick in it
  lands on the command line.
- `pickers` does the same from a typed command. What you pick is typed onto the next prompt, and is also
  in history, so Up brings it back if the terminal doesn't type it.

Inside any picker, `Ctrl-/` or `Alt-/` shows its keys.

## Development Environment

This project's contributor toolchain uses Homebrew and uv. There is no Nix development shell.

### Install

From a fresh checkout, install `just` and run the setup recipe:

```bash
brew install just
just setup
```

The recipe installs the contributor tools from [`Brewfile.dev`](./Brewfile.dev) and the pinned Markdown
formatter with its plugins through uv.

### Commands

Run the formatter and lint checks with:

```bash
just l       # format files
just L       # check for formatting drift
just s       # run ShellCheck
just test
just ship
```

See the [`justfile`](./justfile) for the complete command list.
