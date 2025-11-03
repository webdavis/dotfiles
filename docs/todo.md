# Task List

- [ ] Refactor `dot_fzf_bindings` and `dot_bash_bindings` and solve all Shellcheck
  diagnostics
- [ ] Figure out what the difference between `cspell` and `codespell` (if there is one), and
  then configure Neovim (via Mason or none-ls) to handle its installation and integration
  automatically
- [ ] Fix Neovim `lsp-format` config (currently, it's mucking up my code every time the
  auto-formatter runs, which is every time I save)
- [ ] Huge migration! Replace Bash with Nu Shell. This requires rewrites of the following
  configs:
  - [ ] `dot_bashrc`
  - [ ] `dot_bashrc_local`
  - [ ] `inputrc`
  - [ ] `dot_profile`
  - [ ] `dot_bash_profile`
  - [ ] `dot_bash_bindings`
  - [ ] `dot_fzf_bindings`
  - [ ] `dot_bash_secrets`
- [x] Install [Yazi](https://yazi-rs.github.io/) with Homebrew, and if there's a Neovim plugin
  for it, integrate that too <!-- completed: 2025-11-03 -->
- [ ] Homebrew auto-installs! Configure Chezmoi to run `brew bundle <subcommand>` (or something
  like that) when it detects changes to `dot_Brewfile`
- [ ] Turn off **Automatically rearrange Spaces based on most recent use** using
  `defaults write com.apple.dock workspaces-auto-swoosh -bool NO && killall dock`
- [ ] Automate installation of [claude-code](https://www.anthropic.com/claude-code) via
  `npm install -g @anthropic-ai/claude-code`
- [ ] Automate installation of [kulala-ls](https://github.com/mistweaverco/kulala-ls) via
  `npm install -g @mistweaverco/kulala-ls`
- [ ] Automate installation of [meiji163/gh-notify](https://github.com/meiji163/gh-notify)
  (Version: `556df2e`) extension for `gh`
- [ ] Replace [Nix Installer from Determinate Systems](https://github.com/DeterminateSystems/nix-installer)
  with [DeterminateSystems/nix-installer-action](https://github.com/DeterminateSystems/nix-installer-action)
  (The default Determinate Nix branch no longer supports upstream installs)
