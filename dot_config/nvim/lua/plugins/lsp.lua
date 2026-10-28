return {
  {
    "neovim/nvim-lspconfig",
    event = { "BufReadPre", "BufNewFile" },
    -- Configures servers in `init`, because `automatic_enable` starts a server for a file named
    -- on the command line before the scheduled `config` runs.
    init = function()
      vim.lsp.config("clangd", {
        root_markers = {
          "compile_commands.json",
          "compile_flags.txt",
          "configure.ac",
          "Makefile",
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
      })

      vim.lsp.config("lua_ls", {
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
      })
    end,
    config = vim.schedule_wrap(function(_, _)
      vim.diagnostic.config({
        severity_sort = true,
        signs = {
          text = {
            [vim.diagnostic.severity.ERROR] = "",
            [vim.diagnostic.severity.WARN] = "",
            [vim.diagnostic.severity.INFO] = "",
            [vim.diagnostic.severity.HINT] = "",
          },
        },
        underline = true,
        update_in_insert = false,
        virtual_text = { source = "if_many" },
        virtual_lines = { current_line = true },
      })

      vim.lsp.codelens.enable()
      vim.lsp.inlay_hint.enable()

      -- sourcekit-lsp comes with Xcode, so Mason does not install it.
      if vim.fn.has("mac") == 1 then
        vim.lsp.config("sourcekit", {
          cmd = { "sourcekit-lsp" },
          root_markers = {
            "buildServer.json",
            ".bsp",
            "*.xcodeproj",
            "*.xcworkspace",
            "compile_commands.json",
            "Package.swift",
            ".git",
          },
        })
        vim.lsp.enable("sourcekit")

        -- jdtls needs a newer Java than the system one, and that Java is not on PATH.
        -- Setting JAVA_HOME for jdtls alone leaves every other program's Java unchanged.
        local jdk_home = "/opt/homebrew/opt/openjdk/libexec/openjdk.jdk/Contents/Home"
        if vim.fn.isdirectory(jdk_home) == 1 then
          vim.lsp.config("jdtls", { cmd_env = { JAVA_HOME = jdk_home } })
        end
      end
    end),
  },
  {
    "mason-org/mason.nvim",
    cmd = "Mason",
    -- Adds Mason's tools to PATH at startup, so nvim-treesitter finds Mason's `tree-sitter` when it builds parsers.
    init = function()
      -- Mason's tools come first on PATH, so they win over other copies of `shfmt` and `stylua`.
      -- Rebuilding the list keeps Mason's folder from being added twice.
      local mason_bin = vim.fn.stdpath("data") .. "/mason/bin"
      -- `vim.env.PATH` is nil when the process PATH is unset or empty.
      local path = vim.env.PATH
      if not path then
        vim.env.PATH = mason_bin
        return
      end
      local entries = { mason_bin }
      for _, entry in ipairs(vim.split(path, ":", { plain = true })) do
        -- An empty entry means the current directory, so only Mason's own entry is dropped.
        if entry ~= mason_bin then
          table.insert(entries, entry)
        end
      end
      vim.env.PATH = table.concat(entries, ":")
    end,
    opts = {
      -- Must match `init`, which puts Mason's tools first on PATH.
      PATH = "prepend",
    },
  },
  {
    "mason-org/mason-lspconfig.nvim",
    event = { "BufReadPre", "BufNewFile" },
    dependencies = {
      "neovim/nvim-lspconfig",
      "mason-org/mason.nvim",
    },
    opts = {
      ensure_installed = {
        "ansiblels",
        "basedpyright",
        "bashls",
        "clangd",
        "cssls",
        "docker_compose_language_service",
        "dockerls",
        "elixirls",
        "eslint",
        "gopls",
        "graphql",
        "html",
        "jdtls",
        "lua_ls",
        "marksman",
        "nil_ls",
        "pyright",
        "ruby_lsp",
        "ruff",
        "rust_analyzer",
        "terraformls",
        "tflint",
        "zls",
      },
      -- rustaceanvim starts rust-analyzer itself, so starting it here too would attach a second server.
      automatic_enable = { exclude = { "rust_analyzer" } },
    },
  },
  {
    "WhoIsSethDaniel/mason-tool-installer.nvim",
    -- Loading this plugin loads Mason too, so these triggers keep both out of startup.
    cmd = {
      "MasonToolsClean",
      "MasonToolsInstall",
      "MasonToolsInstallSync",
      "MasonToolsUpdate",
      "MasonToolsUpdateSync",
    },
    event = { "BufReadPre", "BufNewFile" },
    opts = {
      ensure_installed = {
        "ansible-lint",
        "yamllint",
        "jq",
        "yq",
        "json-to-struct",
        "codelldb",
        "dotenv-linter",
        "editorconfig-checker",
        "gofumpt",
        "golines",
        "gomodifytags",
        "gotests",
        "staticcheck",
        "docker-language-server",
        "hadolint",
        "impl",
        "markdown-toc",
        "markdownlint-cli2",
        "nil",
        "prettierd",
        "revive",
        "shellcheck",
        "shfmt",
        "taplo",
        "tree-sitter-cli",
        "lua-language-server",
        "stylua",
        "swiftlint",
        "yamlfmt",
        "vim-language-server",
        "vint",
        "terraform",
      },

      auto_update = false,

      -- A setup script runs `:MasonToolsInstallSync`, so Neovim skips installing at startup.
      run_on_start = false,

      start_delay = 2000,

      debounce_hours = 5,

      integrations = {
        ["mason-lspconfig"] = true,
        ["mason-null-ls"] = true,
        ["mason-nvim-dap"] = true,
      },
    },
  },
  {
    -- Provides code actions only, because conform and nvim-lint already format and lint.
    "nvimtools/none-ls.nvim",
    event = { "BufReadPre", "BufNewFile" },
    dependencies = {
      "nvim-lua/plenary.nvim",
    },
    config = function()
      local null_ls = require("null-ls")
      local code_actions = null_ls.builtins.code_actions

      null_ls.setup({
        sources = {
          code_actions.gitsigns,
          code_actions.refactoring,
        },
      })
    end,
  },
}
