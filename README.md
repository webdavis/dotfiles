<p align="center">
  <img src="./assets/logo.png" />
</p>

# Dotfiles

[![Lint](https://github.com/webdavis/dotfiles/actions/workflows/lint.yml/badge.svg)](https://github.com/webdavis/dotfiles/actions/workflows/lint.yml)

This repository contains my personal configuration files, managed with [Chezmoi](https://www.chezmoi.io/).

## Prerequisites

- [KeePassXC](https://keepassxc.org/)

I use Chezmoi's [`keepassxc-cli` password manager](https://www.chezmoi.io/user-guide/password-managers/keepassxc/)
to manage secrets in my dotfiles.

Before applying them, make sure it's installed:

```bash
brew install --cask keepassxc
```

## Setup

To use these dotfiles on your system:

1. **Install Chezmoi**

Follow the instructions for your platform: [https://www.chezmoi.io/install/](https://www.chezmoi.io/install/)

macOS example:

```bash
brew install chezmoi
```

2. **Initialize Chezmoi with this repository**

```bash
sh -c "$(curl -fsLS get.chezmoi.io)" -- init --apply webdavis
```
