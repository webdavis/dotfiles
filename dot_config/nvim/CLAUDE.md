# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Personal Neovim configuration powered by lazy.nvim (standalone, not the LazyVim framework). All
configuration is in Lua targeting LuaJIT 5.1 (Neovim's runtime).

## Lint and Format Commands

```bash
# Format check (CI runs these)
stylua --check init.lua
stylua --check lua/

# Format in-place
stylua init.lua lua/

# Lint
luacheck init.lua lua/
```

Formatting rules are in `stylua.toml` (2-space indent, 120 column width, double quotes). Linting rules
are in `.luacheckrc`.

## Architecture

**Loading order** (`init.lua`):

1. Optional profiling setup (`PROFILE=1 nvim`)
1. `custom_api.keymap` loaded; `map()` set as global (`_G.map`)
1. `config/options.lua` → `config/keymaps.lua` → `config/autocmds.lua` → `config/lazy.lua`

**Plugin system**: lazy.nvim loads all specs from `lua/plugins/`. Each plugin file returns a lazy.nvim
spec table. Plugins are **not** lazy-loaded by default (`lazy = false`).

### Key directories

- `lua/config/` — Core config: options, keymaps, autocmds, lazy.nvim setup
- `lua/plugins/` — One file per plugin (or plugin group), each returning a lazy.nvim spec
- `lua/custom_api/` — Custom utility modules, each required by its own name
- `lua/overseer/template/user/` — Custom Overseer task templates

### Custom API (`lua/custom_api/`)

Each module is required by its own name, `require("custom_api.git")`. There is no umbrella module:
`init.lua` requires the one module it needs before `config.options` sets the leader key, so an umbrella
that pulled in a module with a side effect at load ran that side effect too early. Requiring a
`custom_api` module must do nothing but return it.

- `util` — `trim()`, `normalize()`, and `run_shell_command()`, whose `cmd` is argv words as a table (no
  shell) or a string for a deliberate pipeline
- `keymap` — `map()`, the global keymap helper
- `git` — Git CLI wrappers (branch parsing, URL generation, protocol conversion)
- `github` — GitHub CLI (`gh`) integration for account/repo info
- `auto_reload`: `watch()` and `unwatch()`, the `vim.uv` file-watch handles behind the
  reload-on-external-write autocmd in `config/autocmds.lua`
- `overseer`: `overseer_runner()`, which chains commands into one Overseer orchestrator task
- `try`: the keymap-layer error boundary; turns a raise or a `nil, message` result into one notification
- `preview_host`: `resolve()`, the address markdown-preview advertises; takes the hostname and the
  suffix as arguments rather than reading `vim.env` itself, so it can be tested

### The `map()` function

The global `map()` is the primary way keymaps are defined outside of plugin `keys` specs. It wraps
`vim.keymap.set` with:

- Auto `noremap = true`
- Auto `silent` unless RHS starts with `:`
- Accepts `lhs` as a string or table of strings (multiple keys)
- Merges extra options (`expr`, `nowait`, `remap`, etc.)

```lua
map({ mode = "n", lhs = "<leader>x", rhs = function() ... end, desc = "Do thing" })
```

## Conventions

- **Keymaps**: Global keymaps in `config/keymaps.lua`; plugin-specific keymaps in their respective plugin
  file's `keys` field
- **Leader**: `<Space>` (leader), `\` (localleader)
- **Which-key groups** are defined in `lua/plugins/which-key.lua`
- **LSP servers** are configured in `lua/plugins/lsp.lua` via mason + lspconfig
- **Completion** uses blink.cmp (not nvim-cmp)
- **Colorscheme**: catppuccin (macchiato)
- **Globals**: `vim`, `map`, `Snacks`, `_` are declared as globals in `.luacheckrc`
