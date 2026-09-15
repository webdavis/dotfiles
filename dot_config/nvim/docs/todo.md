# Neovim configuration status

Source status checked on 2026-09-13. The
[acceptance record](../../../docs/research/2026-09-nvim-overhaul-acceptance.md) separates merged
implementation from private checks and operator acceptance.

- [x] Configure auto-save compatibility with language server protocol (LSP) formatting. Automatic writes
  skip formatting; explicit writes retain it. Proposed claudecode diff buffers are excluded. Auto-save
  starts disabled and is enabled with `<leader>uv`. See `lua/plugins/autosave.lua`,
  `lua/custom_api/autosave.lua`, `lua/plugins/lsp.lua` and
  [#337](https://github.com/webdavis/dotfiles/pull/337).

- [x] Configure xcodebuild.nvim. [#292](https://github.com/webdavis/dotfiles/pull/292) added the Swift
  stack; [#363](https://github.com/webdavis/dotfiles/pull/363) completed the configuration against its
  pin. UIKit/Xcode and Vapor workflow acceptance remains open.

- [x] Configure neotest under `<leader>t`. Python and Go landed in
  [#334](https://github.com/webdavis/dotfiles/pull/334), JavaScript/TypeScript, Lua and Swift Testing in
  [#338](https://github.com/webdavis/dotfiles/pull/338), and the published Bash adapter in
  [#376](https://github.com/webdavis/dotfiles/pull/376), updated in
  [#435](https://github.com/webdavis/dotfiles/pull/435). Rust, Zig, Java and Elixir coverage remains
  unresolved in the existing follow-ups; this checkbox covers the configured runner and listed adapters.

- [x] Resolve the cspell work. [#326](https://github.com/webdavis/dotfiles/pull/326) removed its unused
  dependency. Both the codespell install entry and diagnostic source are commented out in
  `lua/plugins/lsp.lua`; this configuration does not enable either spell checker.

- [x] Correct format-on-save. [#335](https://github.com/webdavis/dotfiles/pull/335) removed duplicate
  formatting; [#347](https://github.com/webdavis/dotfiles/pull/347) repaired repeated saves and formatter
  admission. Autosave/format behavior has owned regression coverage in `tests/`.

- [ ] Complete the acceptance record's rendered startup, buffer-local keys, real agent loops, custom
  plugin interactions, Swift workflows, quiescent performance and fresh-user full apply checks. The
  operator owns deployment and session/device checks.
