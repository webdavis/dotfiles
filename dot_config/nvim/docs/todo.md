# Neovim Config Task List

- [ ] Figure out fallback LSP formatting compatibility with https://github.com/okuuva/auto-save.nvim:
  - Ref: https://github.com/nvimtools/none-ls.nvim/wiki/Compatibility-with-other-plugins
  - Note: this configures https://github.com/Pocco81/auto-save.nvim, which may is a little
    different. So it's not a one-to-one drop-in and some adjustments will need to be made
- [ ] Setup [wojciech-kulik/xcodebuild.nvim](https://github.com/wojciech-kulik/xcodebuild.nvim)
- [ ] Setup [nvim-neotest/neotest](https://github.com/nvim-neotest/neotest)
- [x] Figure out what the difference between `cspell` and `codespell` (if there is one), and
  then configure Neovim (via Mason or none-ls) to handle its installation and integration
  automatically <!-- completed: 2025-11-04 -->
- [x] Fix Neovim `lsp-format` config (currently, it's mucking up my code every time the
  auto-formatter runs, which is every time I save) <!-- completed: 2025-11-04 -->
