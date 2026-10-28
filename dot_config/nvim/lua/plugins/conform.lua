-- conform picks formatters by filetype, so it also handles servers like ESLint
-- that register formatting after they attach.
-- Rows agree with this repo's treefmt.toml where it names a formatter.
local log_info = vim.log.levels.INFO
local log_warning = vim.log.levels.WARN

local notify_format_title = { title = "Format" }

return {
  {
    "stevearc/conform.nvim",
    event = { "BufReadPre", "BufNewFile" },
    -- Declared here too so `ZZ`, the format keys and the commands exist before a buffer is read.
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
    -- nvim-ansible sets the `yaml.ansible` filetype that selects `ansible-lint`.
    dependencies = {
      "mfussenegger/nvim-ansible",
    },
    opts = {
      formatters_by_ft = {
        -- prettierd skips `markdown` and `yaml.ansible`, which mdformat and ansible-lint own.
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
        -- Uses treefmt when a treefmt.toml exists upward, so a repo's own Markdown settings win.
        -- Uses mdformat everywhere else.
        markdown = { "treefmt", "mdformat", stop_after_first = true },
        ["markdown.mdx"] = { "prettierd" },
        nix = { "nixfmt" },
        ruby = { "rubocop" },
        scss = { "prettierd" },
        sh = { "shfmt" },
        svelte = { "prettierd" },
        -- swiftlint --fix applies its autocorrections on top of swiftformat's output.
        swift = { "swiftformat", "swiftlint" },
        terraform = { "terraform_fmt" },
        ["terraform-vars"] = { "terraform_fmt" },
        tf = { "terraform_fmt" },
        toml = { "taplo" },
        typescript = { "prettierd" },
        typescriptreact = { "prettierd" },
        vue = { "prettierd" },
        yaml = { "prettierd", "yamlfmt" },
        ["yaml.ansible"] = { "ansible-lint" },
      },
      -- Matches treefmt.toml's shfmt flags for files outside the root .editorconfig's patterns.
      formatters = {
        shfmt = {
          prepend_args = { "-i", "2", "-ci", "-s" },
        },
        -- mdformat over stdin cannot find .mdformat.toml, so the wrap is passed here.
        mdformat = {
          prepend_args = { "--wrap", "120" },
        },
        -- conform formats a uniquely named temp copy, so treefmt's cache never hits.
        treefmt = {
          prepend_args = { "--no-cache" },
        },
      },
      -- Language-server formatting only for filetypes the table leaves out.
      default_format_opts = {
        lsp_format = "fallback",
      },
      -- A function, so both switches are read at save time.
      format_on_save = function(bufnr)
        if not vim.g.autoformat_on_save then
          return nil
        end

        -- auto-save.nvim sets this flag over its own write,
        -- so a timed save leaves the cursor and undo history alone.
        if vim.b[bufnr].autosave_write then
          return nil
        end

        -- `ansible-lint` loads its whole rule set per run, and the write blocks on it.
        return { timeout_ms = 5000 }
      end,
    },
    config = function(_, opts)
      require("conform").setup(opts)

      vim.g.autoformat_on_save = true

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
