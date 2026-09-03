# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Personal Neovim configuration powered by lazy.nvim (standalone, not the LazyVim framework). All configuration is in Lua targeting LuaJIT 5.1 (Neovim's runtime).

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

Formatting rules are in `stylua.toml` (2-space indent, 120 column width, double quotes). Linting rules are in `.luacheckrc`.

## Architecture

**Loading order** (`init.lua`):
1. Optional profiling setup (`PROFILE=1 nvim`)
2. `custom_api` loaded; `map()` set as global (`_G.map`)
3. `config/options.lua` → `config/keymaps.lua` → `config/autocmds.lua` → `config/lazy.lua`

**Plugin system**: lazy.nvim loads all specs from `lua/plugins/`. Each plugin file returns a lazy.nvim spec table. Plugins are **not** lazy-loaded by default (`lazy = false`).

### Key directories

- `lua/config/` — Core config: options, keymaps, autocmds, lazy.nvim setup
- `lua/plugins/` — One file per plugin (or plugin group), each returning a lazy.nvim spec
- `lua/custom_api/` — Custom utility modules exported via `custom_api/init.lua`
- `lua/overseer/template/user/` — Custom Overseer task templates

### Custom API (`lua/custom_api/`)

Modules are accessed via `require("custom_api")`:
- `util` — `map()` (global keymap helper), `trim()`, `overseer_runner()`, shell command execution
- `git` — Git CLI wrappers (branch parsing, URL generation, protocol conversion)
- `github` — GitHub CLI (`gh`) integration for account/repo info
- `delegate` — Tmux delegate window management (send commands/selections to a tmux pane)
- `try`: the keymap-layer error boundary; turns a raise or a `nil, message` result into one notification

### The `map()` function

The global `map()` is the primary way keymaps are defined outside of plugin `keys` specs. It wraps `vim.keymap.set` with:
- Auto `noremap = true`
- Auto `silent` unless RHS starts with `:`
- Accepts `lhs` as a string or table of strings (multiple keys)
- Merges extra options (`expr`, `nowait`, `remap`, etc.)

```lua
map({ mode = "n", lhs = "<leader>x", rhs = function() ... end, desc = "Do thing" })
```

## Conventions

- **Keymaps**: Global keymaps in `config/keymaps.lua`; plugin-specific keymaps in their respective plugin file's `keys` field
- **Leader**: `<Space>` (leader), `\` (localleader)
- **Which-key groups** are defined in `lua/plugins/which-key.lua`
- **LSP servers** are configured in `lua/plugins/lsp.lua` via mason + lspconfig
- **Completion** uses blink.cmp (not nvim-cmp)
- **Colorscheme**: catppuccin (macchiato)
- **Globals**: `vim`, `map`, `Snacks`, `_` are declared as globals in `.luacheckrc`
