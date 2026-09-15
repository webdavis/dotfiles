-- Formatting. conform.nvim picks the formatter for a buffer out of the table
-- below, by filetype, rather than out of the language servers' capabilities.
--
-- That is the whole reason this file exists. The ESLint server registers
-- `textDocument/formatting` AFTER it attaches and unregisters it again whenever
-- its configuration changes, which an attach-time capability snapshot cannot
-- see; the previous setup carried a hundred-line admission rule under
-- `lua/custom_api/`, deleted with this migration, to keep its queue in step
-- with those late registrations. A filetype table has nothing to keep in step.
--
-- The mapping is the same set of tools the none-ls sources used to register, so
-- every filetype that had a formatter still has one. Two behaviors of conform
-- replace configuration that used to be written out by hand:
--
--   * A formatter whose command is missing is simply not run, so the `nixfmt`
--     and `rubocop` executable guards are gone. Those two, plus `mdformat`
--     and `swiftformat`, come from a project's toolchain or Homebrew rather
--     than from Mason.
--   * `lsp_format = "fallback"` (below) formats through the language server
--     only for a filetype NOTHING here names, which is what the old per-filetype
--     `exclude` lists were for: `lua` excluded `lua_ls` because stylua owns Lua,
--     `sh` excluded `bashls` because shfmt owns shell. Go, Python, Java, Zig, C,
--     C++, Elixir and Rust name no formatter here and keep formatting through
--     their servers exactly as before.
--
-- Where this repository's own treefmt.toml configures a formatter for a
-- filetype, the row agrees with it: shfmt for shell, mdformat for markdown,
-- stylua for Lua, taplo for TOML.

local log_info = vim.log.levels.INFO
local log_warning = vim.log.levels.WARN

local notify_format_title = { title = "Format" }

return {
  {
    "stevearc/conform.nvim",
    event = { "BufReadPre", "BufNewFile" },
    -- The mappings and the two commands live in `config`, which only runs once a
    -- buffer is read, so they are named here too. Without them a fresh instance
    -- with no file has no `ZZ` and no `<leader>c` formatting keys at all. The rows
    -- are the mappings themselves; lazy.nvim installs the placeholder and sets the
    -- real mapping from the same row on first press.
    cmd = { "ConformInfo", "CustomFormatDisable", "CustomFormatEnable" },
    keys = {
      {
        "ZZ",
        function()
          if vim.g.autoformat_on_save then
            require("conform").format({ async = false })
          end
          if vim.bo.modified then
            vim.cmd("update")
          end
          vim.cmd("quit")
        end,
        desc = "Custom ZZ with safe formatting before closing the file",
        silent = true,
      },
      {
        "<leader>uf",
        function()
          if vim.g.autoformat_on_save then
            vim.cmd("CustomFormatDisable")
          else
            vim.cmd("CustomFormatEnable")
          end
        end,
        desc = "Format: toggle autoformat-on-save (alias of <leader>cc)",
        silent = true,
      },
      {
        "<leader>cc",
        function()
          if vim.g.autoformat_on_save then
            vim.cmd("CustomFormatDisable")
          else
            vim.cmd("CustomFormatEnable")
          end
        end,
        desc = "Format: toggle autoformat-on-save",
        silent = true,
      },
      {
        "<leader>ce",
        function()
          vim.cmd("CustomFormatEnable")
        end,
        desc = "Format: enable autoformat-on-save",
        silent = true,
      },
      {
        "<leader>cd",
        function()
          vim.cmd("CustomFormatDisable")
        end,
        desc = "Format: disable autoformat-on-save",
        silent = true,
      },
      {
        "<leader>cf",
        function()
          require("conform").format({ async = false })
        end,
        desc = "Format: default",
        silent = true,
      },
    },
    -- nvim-ansible is what gives an Ansible playbook the `yaml.ansible` filetype,
    -- and that filetype is what selects `ansible-lint` below instead of the plain
    -- yaml row. Its own spec declares no trigger of its own.
    dependencies = {
      "mfussenegger/nvim-ansible",
    },
    opts = {
      formatters_by_ft = {
        -- One row per filetype the none-ls sources formatted. `prettierd` rows are
        -- its own upstream filetype list minus the two the old configuration
        -- excluded by hand: `markdown`, which mdformat owns, and `yaml.ansible`,
        -- which ansible-lint owns.
        astro = { "prettierd" },
        css = { "prettierd" },
        graphql = { "prettierd" },
        handlebars = { "prettierd" },
        html = { "prettierd" },
        htmlangular = { "prettierd" },
        javascript = { "prettierd" },
        javascriptreact = { "prettierd" },
        json = { "prettierd" },
        json5 = { "prettierd" },
        jsonc = { "prettierd" },
        less = { "prettierd" },
        lua = { "stylua" },
        luau = { "stylua" },
        markdown = { "mdformat" },
        ["markdown.mdx"] = { "prettierd" },
        nix = { "nixfmt" },
        ruby = { "rubocop" },
        scss = { "prettierd" },
        sh = { "shfmt" },
        svelte = { "prettierd" },
        -- Both, in this order, as the two none-ls sources did: swiftformat
        -- reshapes the file and `swiftlint --fix` applies the lint autocorrections
        -- on top of the result.
        swift = { "swiftformat", "swiftlint" },
        terraform = { "terraform_fmt" },
        ["terraform-vars"] = { "terraform_fmt" },
        tf = { "terraform_fmt" },
        -- New name for a filetype that used to be reached only through the
        -- repository-wide `treefmt` source; taplo is what treefmt.toml runs on
        -- TOML, so the row agrees with it.
        toml = { "taplo" },
        typescript = { "prettierd" },
        typescriptreact = { "prettierd" },
        vue = { "prettierd" },
        yaml = { "prettierd", "yamlfmt" },
        ["yaml.ansible"] = { "ansible-lint" },
      },
      -- conform's shfmt builtin only adds an indent flag when no .editorconfig
      -- is found upward from the buffer; this repo's root .editorconfig covers
      -- only dot_fzf* and dot_bash*, so scripts/**/*.sh would otherwise format
      -- with shfmt's own defaults instead of the flags treefmt.toml runs
      -- (`-i 2 -ci -s`), fighting the drift gate on every save.
      formatters = {
        shfmt = {
          prepend_args = { "-i", "2", "-ci", "-s" },
        },
        -- mdformat runs over stdin, so it has no file path to resolve
        -- .mdformat.toml from and falls back to Neovim's cwd. Editing a repo
        -- markdown file while cwd is elsewhere would format with mdformat's
        -- own default wrap instead of the 105 columns treefmt.toml runs, so
        -- the wrap is passed explicitly rather than left to config discovery.
        mdformat = {
          prepend_args = { "--wrap", "105" },
        },
      },
      -- Formatting through a language server for any filetype the table does not
      -- name, and never for one it does.
      default_format_opts = {
        lsp_format = "fallback",
      },
      -- A function, so both switches are read at save time rather than once at
      -- setup: returning nil skips the write's formatting entirely.
      format_on_save = function(bufnr)
        if not vim.g.autoformat_on_save then
          return nil
        end

        -- auto-save.nvim raises this flag over its own write (plugins/autosave.lua).
        -- Reformatting a buffer the operator did not ask to write moves their cursor
        -- and their undo history on a timer; an explicit `:w` still formats.
        if vim.b[bufnr].autosave_write then
          return nil
        end

        -- Above conform's 1000 ms default because `ansible-lint` is a Python
        -- program that loads its whole rule set per run, and the write blocks on it.
        return { timeout_ms = 5000 }
      end,
    },
    config = function(_, opts)
      require("conform").setup(opts)

      vim.g.autoformat_on_save = true

      -- ╭──────────────╮
      -- │   Commands   │
      -- ╰──────────────╯
      vim.api.nvim_create_user_command("CustomFormatEnable", function()
        vim.g.autoformat_on_save = true
        vim.notify("Enabled **Autoformat on Save**", log_info, notify_format_title)
      end, { desc = "Format: enable autoformat on save" })

      vim.api.nvim_create_user_command("CustomFormatDisable", function()
        vim.g.autoformat_on_save = false
        vim.notify("Disabled **Autoformat on Save**", log_warning, notify_format_title)
      end, { desc = "Format: disable autoformat on save" })
    end,
  },
}
