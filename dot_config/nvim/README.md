> [!IMPORTANT]
> moved to `webdavis/dotfiles` under `dot_config/nvim/`.

# Webdavis Neovim Config

[![Lint](https://github.com/webdavis/neovim-config/actions/workflows/lint.yml/badge.svg)](https://github.com/webdavis/neovim-config/actions/workflows/lint.yml)

This is my Neovim config, powered by [lazy.nvim](https://github.com/folke/lazy.nvim).

## Navigating this Repo

The following modules／files run the show:

| Module／File                                 | Description                                                                                  |
| -------------------------------------------- | -------------------------------------------------------------------------------------------- |
| **[init](./init.lua)**                       | Main entry point.                                                                            |
| **[lazy.nvim setup](./lua/config/lazy.lua)** | Initializes `lazy.nvim` plugin manager and loads all plugins from `lua/plugins/`             |
| **[options](./lua/config/options.lua)**      | Global／default Neovim options.                                                              |
| **[autocmds](./lua/config/autocmds.lua)**    | Auto commands for filetypes, events, etc.                                                    |
| **[keymaps](./lua/config/keymaps.lua)**      | Global keyboard shortcuts (note: some plugin-specific mappings are put here for convenience) |
| **[plugins/](./lua/plugins)**                | Individual plugin configurations.                                                            |
| **[custom_api/](./lua/custom_api/)**         | My custom Neovim API functions (mostly `git` and `gh` CLI wrappers)                          |

> [!NOTE]
> Most `plugin-specific-keymaps` are defined in their respective plugin files.

## Profiling Neovim Startup Time

This config can measure how long it takes to start up, helping you identify slow plugins and
configurations. To enable profiling, set the `PROFILE` environment variable before launching Neovim:

#### Linux／macOS

```bash
PROFILE=1 nvim
```

#### Windows PowerShell

```powershell
$env:PROFILE=1; nvim
```

> [!TIP]
> After launching with profiling enabled, Neovim will generate timing information for each loaded plugin
> and configuration file. You can evaluate this output to optimize startup performance. (The profiler is
> set up in [`init.lua`](./init.lua) via `snacks.profiler.startup`.)
