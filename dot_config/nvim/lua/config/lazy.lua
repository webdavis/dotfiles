-- ╭────────────────────────────────╮
-- │   Lazy Plugin Manager Config   │
-- ╰────────────────────────────────╯

local lazypath = vim.fn.stdpath("data") .. "/lazy/lazy.nvim"

if not (vim.uv or vim.loop).fs_stat(lazypath) then
  local lazyrepo = "https://github.com/folke/lazy.nvim.git"

  local out = vim.fn.system({
    "git",
    "clone",
    "--filter=blob:none",
    "--branch=stable", -- latest stable release
    lazyrepo,
    lazypath,
  })

  if vim.v.shell_error ~= 0 then
    vim.api.nvim_echo({
      { "Failed to clone lazy.nvim:\n", "ErrorMsg" },
      { out, "WarningMsg" },
      { "\nPress any key to exit..." },
    }, true, {})
    vim.fn.getchar()
    os.exit(1)
  end
end
vim.opt.rtp:prepend(lazypath)

require("lazy").setup({
  spec = {
    -- add LazyVim and import its plugins
    -- { "LazyVim/LazyVim", import = "lazyvim.plugins" },
    -- import/override with your plugins
    { import = "plugins" },
  },
  defaults = {
    -- Lazy by default: a spec with no trigger of its own loads on demand rather
    -- than at startup. lazy.nvim resolves a spec that declares no `lazy` of its own
    -- as `plugin._.dep or defaults.lazy or event or keys or ft or cmd`
    -- (`lazy/core/plugin.lua:233-241` at the pinned version), so an explicit
    -- `lazy = false` is the only thing that keeps a spec eager, and an explicit
    -- `lazy` OVERRIDES a trigger. Write `lazy = false` only onto a spec that has no
    -- `event`, `keys`, `ft` or `cmd` and must be present at startup; writing it onto
    -- a triggered spec makes that plugin eager again.
    lazy = true,
    -- It's recommended to leave version=false for now, since a lot the plugin that support versioning,
    -- have outdated releases, which may break your Neovim install.
    version = false, -- always use the latest git commit
    -- version = "*", -- try installing the latest stable version for plugins that support semver
  },
  -- install = { colorscheme = { "catppuccin-nvim" } },
  checker = {
    enabled = false, -- check for plugin updates periodically
    notify = false, -- notify on update
  },
  performance = {
    rtp = {
      -- disable some rtp plugins
      disabled_plugins = {
        "gzip",
        -- "matchit",
        -- "matchparen",
        -- "netrwPlugin",
        "tarPlugin",
        "tohtml",
        "tutor",
        "zipPlugin",
      },
    },
  },
})
