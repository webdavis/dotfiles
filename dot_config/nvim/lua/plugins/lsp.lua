return {
  {
    "neovim/nvim-lspconfig",
    config = vim.schedule_wrap(function(_, _)
      -- Enable virtual diagnostics.
      -- stylua: ignore start
      vim.diagnostic.config({
        virtual_text = true,      -- show inline errors/warnings
        signs = true,             -- show symbols in the gutter
        underline = true,         -- underline problematic code
        update_in_insert = false, -- don't update while typing
      })
      -- stylua: ignore end
    end),
  },
  {
    "mason-org/mason.nvim",
    opts = {},
  },
  {
    "mason-org/mason-lspconfig.nvim",
    dependencies = {
      "neovim/nvim-lspconfig",
      "mason-org/mason.nvim",
    },
    opts = {
      ensure_installed = {
        "ansiblels",
        "basedpyright",
        "bashls",
        "clangd", -- c/cpp
        "cssls",
        "docker_compose_language_service",
        "dockerls",
        "eslint",
        "graphql",
        "html",
        "lua_ls",
        "marksman",
        "nil_ls", -- nix
        "pyright",
        "ruby_lsp",
        "ruff",
        "rust_analyzer",
        "terraformls",
        "tflint",
        "zls",
      },
      automatic_enable = true,
      servers = {
        stylua = { enabled = false },
        lua_ls = {
          -- mason = false, -- set to false if you don't want this server to be installed with mason
          -- Use this to add any additional keymaps
          -- for specific lsp servers
          -- keys = {},
          settings = {
            Lua = {
              workspace = {
                checkThirdParty = false,
              },
              codeLens = {
                enable = true,
              },
              completion = {
                callSnippet = "Replace",
              },
              doc = {
                privateName = { "^_" },
              },
              hint = {
                enable = true,
                setType = false,
                paramType = true,
                paramName = "Disable",
                semicolon = "Disable",
                arrayIndex = "Disable",
              },
            },
          },
        },
        ansiblels = {},
        clangd = {
          keys = {
            { "<leader>ch", "<cmd>ClangdSwitchSourceHeader<cr>", desc = "Switch Source/Header (C/C++)" },
          },
          root_markers = {
            "compile_commands.json",
            "compile_flags.txt",
            "configure.ac", -- AutoTools
            "Makefile",
            "configure.ac",
            "configure.in",
            "config.h.in",
            "meson.build",
            "meson_options.txt",
            "build.ninja",
            ".git",
          },
          capabilities = {
            offsetEncoding = { "utf-16" },
          },
          cmd = {
            "clangd",
            "--background-index",
            "--clang-tidy",
            "--header-insertion=iwyu",
            "--completion-style=detailed",
            "--function-arg-placeholders",
            "--fallback-style=llvm",
          },
          init_options = {
            usePlaceholders = true,
            completeUnimported = true,
            clangdFileStatus = true,
          },
        },
        dockerls = {},
        docker_compose_language_service = {},
        -- elixirls = {
        --   keys = {
        --     {
        --       "<leader>cp",
        --       function()
        --         local params = vim.lsp.util.make_position_params()
        --         LazyVim.lsp.execute({
        --           command = "manipulatePipes:serverid",
        --           arguments = { "toPipe", params.textDocument.uri, params.position.line, params.position.character },
        --         })
        --       end,
        --       desc = "To Pipe",
        --     },
        --     {
        --       "<leader>cP",
        --       function()
        --         local params = vim.lsp.util.make_position_params()
        --         LazyVim.lsp.execute({
        --           command = "manipulatePipes:serverid",
        --           arguments = { "fromPipe", params.textDocument.uri, params.position.line, params.position.character },
        --         })
        --       end,
        --       desc = "From Pipe",
        --     },
        --   },
        -- },
        marksman = {},
        nil_ls = {},
      },
    },
  },
  {
    "WhoIsSethDaniel/mason-tool-installer.nvim",
    opts = {
      ensure_installed = {

        -- Pin version example:
        -- { 'golangci-lint', version = 'v1.47.0' },

        -- Turn off/on auto_update example:
        -- { 'bash-language-server', auto_update = true },

        -- Conditional installation example:
        -- { 'gopls', condition = function() return vim.fn.executable('go') == 1  end },

        "ansible-lint",
        "yamllint",
        "jq",
        "yq",
        "json-to-struct",
        "codelldb", -- c, cpp, rust
        -- "codespell",
        "dotenv-linter",
        "editorconfig-checker",
        "gofumpt",
        "golines",
        "gomodifytags",
        "gotests",
        "staticcheck", -- go
        "docker-language-server",
        "hadolint", -- Dockerfiles.
        -- "kubescape", -- Kubernetes security scanner. (ISSUE: Currently disabled because the download URL in the Mason registry (https://mason-registry.dev/) is broken.)
        "impl",
        "markdown-toc",
        "markdownlint-cli2",
        -- "misspell",
        "nil",
        -- "nixfmt", -- (ISSUE: currently disabled because macOS platform is not supported by Mason. Downloaded using Hombrew, instead.)
        "prettierd",
        "revive",
        "shellcheck",
        "shfmt",
        "tree-sitter-cli",
        "lua-language-server",
        "stylua",
        "swiftlint",
        "yamlfmt",
        "vim-language-server",
        "vint",
        "terraform",
      },

      -- if set to true this will check each tool for updates. If updates
      -- are available the tool will be updated. This setting does not
      -- affect :MasonToolsUpdate or :MasonToolsInstall.
      -- Default: false
      auto_update = false,

      -- automatically install / update on startup. If set to false nothing
      -- will happen on startup. You can use :MasonToolsInstall or
      -- :MasonToolsUpdate to install tools and check for updates.
      -- Default: true
      run_on_start = true,

      -- set a delay (in ms) before the installation starts. This is only
      -- effective if run_on_start is set to true.
      -- e.g.: 5000 = 5 second delay, 10000 = 10 second delay, etc...
      -- Default: 0
      start_delay = 2000, -- 3 second delay

      -- Only attempt to install if 'debounce_hours' number of hours has
      -- elapsed since the last time Neovim was started. This stores a
      -- timestamp in a file named stdpath('data')/mason-tool-installer-debounce.
      -- This is only relevant when you are using 'run_on_start'. It has no
      -- effect when running manually via ':MasonToolsInstall' etc....
      -- Default: nil
      debounce_hours = 5, -- at least 5 hours between attempts to install/update

      -- By default all integrations are enabled. If you turn on an integration
      -- and you have the required module(s) installed this means you can use
      -- alternative names, supplied by the modules, for the thing that you want
      -- to install. If you turn off the integration (by setting it to false) you
      -- cannot use these alternative names. It also suppresses loading of those
      -- module(s) (assuming any are installed) which is sometimes wanted when
      -- doing lazy loading.
      integrations = {
        ["mason-lspconfig"] = true,
        ["mason-null-ls"] = true,
        ["mason-nvim-dap"] = true,
      },
    },
  },
  {
    "nvimtools/none-ls.nvim",
    dependencies = {
      "nvim-lua/plenary.nvim",
      "nvimtools/none-ls-extras.nvim",
      "davidmh/cspell.nvim",
    },
    config = function()
      local null_ls = require("null-ls")
      local diagnostics = null_ls.builtins.diagnostics
      local formatting = null_ls.builtins.formatting
      local code_actions = null_ls.builtins.code_actions
      local completion = null_ls.builtins.completion

      -- For configuring sources by filetype, see:
      -- https://github.com/nvimtools/null-ls.nvim/blob/main/doc/BUILTIN_CONFIG.md#filetypes
      null_ls.setup({
        sources = {
          diagnostics.actionlint.with({
            disabled_filetypes = { "yaml.ansible" },
          }),
          diagnostics.ansiblelint,
          -- diagnostics.codespell,
          diagnostics.dotenv_linter.with({
            disabled_filetypes = { "sh", "bash" },
          }),
          diagnostics.hadolint, -- Filetypes: Dockerfile.

          formatting.shfmt.with({
            extra_args = { "-i", "2", "-ci", "-s" },
          }),
          formatting.mdformat.with({
            extra_args = { "--number", "--wrap", "105" },
          }),
          formatting.nixfmt, -- Filetypes: .nix config files, specifically.
          formatting.nix_flake_fmt.with({ -- Filetypes: flake.nix files, specifically.
            filetypes = { "nix" },
          }),
          formatting.prettierd.with({
            disabled_filetypes = { "markdown", "yaml.ansible" },
          }),
          formatting.rubocop.with({ -- Filetypes: Ruby (supports linting & formatting).
            extra_args = { "--display-time", "--extra-details", "--autocorrect", "--fail-level autocorrect" },
          }),
          formatting.stylua,
          formatting.swiftformat,
          formatting.swiftlint,
          formatting.terraform_fmt,
          formatting.treefmt, -- A polyglot formatter/linter orchestration tool.
          formatting.yamlfmt.with({
            disabled_filetypes = { "yaml.ansible" },
          }),

          completion.spell,

          code_actions.gitsigns,
          code_actions.refactoring, -- Filetypes: go, javascript, lua, python, typescript.

          -- The following require none-ls-extras.nvim:
          require("none-ls.formatting.ansiblelint"),
          require("none-ls.diagnostics.eslint"),
        },
      })
    end,
  },
  {
    "lukas-reineke/lsp-format.nvim",
    dependencies = {
      "mfussenegger/nvim-ansible",
    },
    config = function()
      local lsp_format = require("lsp-format")

      lsp_format.setup({
        lua = {
          exclude = { "lua_ls" },
        },
        sh = {
          exclude = { "bashls" },
        },
        markdown = {
          exclude = {
            "prettierd",
            "nix flake fmt",
          },
        },
        ["yaml.ansible"] = {
          exclude = {
            "prettierd",
            "action-lint",
            "yamlfmt",
            "nix flake fmt",
          },
        },
      })

      vim.g.autoformat_on_save = true

      -- ╭─────────────╮
      -- │   Helpers   │
      -- ╰─────────────╯
      local log_info = vim.log.levels.INFO
      local log_warning = vim.log.levels.WARN
      local log_error = vim.log.levels.ERROR

      local notify_lsp_format_title = { title = "LSP Format" }

      local function safe_format()
        if not vim.g.autoformat_on_save then
          return true
        end

        local ok, err = pcall(function()
          vim.cmd("Format sync")
        end)

        if not ok then
          vim.notify("Autoformat failed: " .. err, log_error, notify_lsp_format_title)
        end
        return ok
      end

      local format_group = vim.api.nvim_create_augroup("AutoformatGroup", { clear = true })

      -- ╭──────────────╮
      -- │   Autocmds   │
      -- ╰──────────────╯
      vim.api.nvim_create_autocmd("LspAttach", {
        group = format_group,
        callback = function(args)
          local client = assert(vim.lsp.get_client_by_id(args.data.client_id))
          lsp_format.on_attach(client, args.buf)
        end,
      })

      vim.api.nvim_create_autocmd("BufWritePre", {
        group = format_group,
        pattern = "*",
        callback = safe_format,
      })

      -- ╭──────────────╮
      -- │   Commands   │
      -- ╰──────────────╯
      vim.api.nvim_create_user_command("CustomFormatEnable", function()
        local ok, err = pcall(function()
          vim.cmd("FormatEnable")
        end)
        if not ok then
          vim.notify("CustomFormatEnable failed: " .. err, log_error, notify_lsp_format_title)
          return
        end

        vim.g.autoformat_on_save = true

        vim.notify("Enabled **Autoformat on Save**", log_info, notify_lsp_format_title)
      end, { desc = "LSP Format: custom FormatEnable" })

      vim.api.nvim_create_user_command("CustomFormatDisable", function()
        local ok, err = pcall(function()
          vim.cmd("FormatDisable")
        end)
        if not ok then
          vim.notify("CustomFormatDisable failed: " .. err, log_error, notify_lsp_format_title)
          return
        end

        vim.g.autoformat_on_save = false

        vim.notify("Disabled **Autoformat on Save**", log_warning, notify_lsp_format_title)
      end, { desc = "LSP Format: custom FormatDisable" })

      -- ╭──────────────╮
      -- │   Mappings   │
      -- ╰──────────────╯
      map({
        mode = "n",
        lhs = "ZZ",
        rhs = function()
          safe_format() -- runs formatting and logs errors
          if vim.bo.modified then
            vim.cmd("update")
          end
          vim.cmd("quit")
        end,
        desc = "Custom ZZ with safe formatting before closing the file",
      })

      map({
        mode = "n",
        lhs = { "<leader>uf", "<leader>cc" },
        rhs = function()
          if vim.g.autoformat_on_save then
            vim.cmd("CustomFormatDisable")
          else
            vim.cmd("CustomFormatEnable")
          end
        end,
        desc = "Format: toggle autoformat-on-save",
      })

      map({
        mode = "n",
        lhs = "<leader>ce",
        rhs = function()
          vim.cmd("CustomFormatEnable")
        end,
        desc = "Format: enable autoformat-on-save",
      })

      map({
        mode = "n",
        lhs = "<leader>cd",
        rhs = function()
          vim.cmd("CustomFormatDisable")
        end,
        desc = "Format: disable autoformat-on-save",
      })

      map({
        mode = "n",
        lhs = "<leader>cf",
        rhs = function()
          vim.lsp.buf.format({
            filter = function(client)
              -- apply whatever logic you want (in this example, we'll only use null-ls)
              return client.name == "null-ls"
            end,
            bufnr = vim.api.nvim_get_current_buf(),
          })
        end,
        desc = "Format: default",
      })
    end,
  },
}
