return {
  {
    "neovim/nvim-lspconfig",
    event = { "BufReadPre", "BufNewFile" },
    -- `init`, not `config`: mason-lspconfig's `automatic_enable` starts a server on the
    -- FileType of a file named on the command line, which happens before a scheduled
    -- `config` callback runs. A server table registered there would lose the race and the
    -- client would start on nvim-lspconfig's defaults.
    init = function()
      vim.lsp.config("clangd", {
        root_markers = {
          "compile_commands.json",
          "compile_flags.txt",
          "configure.ac", -- AutoTools
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
        severity_sort = true, -- most severe sign wins the gutter and sorts first in lists
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
        virtual_text = { source = "if_many" }, -- name the source only when several report
        virtual_lines = { current_line = true }, -- the full message under the cursor line
      })

      -- Global switches: every client that supports the method gets them as it attaches.
      vim.lsp.codelens.enable()
      vim.lsp.inlay_hint.enable()

      -- sourcekit-lsp: the Xcode toolchain's own LSP for Swift, ObjC and C/C++ in a Swift
      -- project. Not a Mason package; the binary ships with Xcode/the Swift toolchain.
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

        -- jdtls requires a Java Development Kit (JDK) 21 or newer to run itself (it is a Java
        -- program), which is separate from any JDK a project under edit targets. Homebrew's
        -- `openjdk` formula is keg-only and not linked onto PATH, so without this jdtls falls
        -- back to macOS's `/usr/bin/java` stub, which prompts to install a JDK rather than
        -- running one. Mason's `jdtls` wrapper script reads `JAVA_HOME` itself when set, ahead
        -- of the bare `java` it would otherwise resolve from PATH. Scoped to jdtls's own
        -- `cmd_env` rather than `vim.env`, so it does not leak into every `:terminal`, `:!`, or
        -- other LSP client in the process and override a project's own JDK; guarded so a
        -- machine where `openjdk` is not installed yet (a fresh apply, before `brew bundle`)
        -- falls back to PATH resolution instead of pointing jdtls at a directory that does not
        -- exist. lspconfig's default jdtls `cmd` (root markers, per-project `-data` workspace)
        -- is left untouched; `cmd_env` merges into it.
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
    -- `init` runs at startup even though the plugin itself waits for `:Mason`, and putting
    -- Mason's bin on PATH is the one part of `mason.setup()` that cannot wait for that. Mason
    -- is the only source of `tree-sitter` on this machine, and nvim-treesitter builds parsers
    -- from two places that run before any buffer trigger fires: the `LazyDone` core-parser
    -- install in plugins/treesitter.lua, and a fileless `:TSUpdate`. While this group was
    -- eager, mason-tool-installer pulled Mason in at startup and `setup()` did the prepend as
    -- a side effect; with every spec here lazy that side effect is gone, and `tree-sitter
    -- build` fails with ENOENT until something types `:Mason`.
    init = function()
      -- The same directory and the same order `mason.setup()` uses: it prepends
      -- `install_root_dir .. "/bin"` under its default `PATH = "prepend"`, pinned in `opts`
      -- below so the two cannot drift apart. Every existing occurrence is dropped and one is
      -- put back at the front, because FIRST is the whole point and merely being present is
      -- not enough: with Homebrew ahead of it, `shfmt` and `stylua` resolve to Homebrew's
      -- copies while Mason's sit further down the list. Rebuilding the list this way is also
      -- what keeps a second call idempotent.
      local mason_bin = vim.fn.stdpath("data") .. "/mason/bin"
      -- `vim.env.PATH` is nil when the process PATH is unset or empty (measured on 0.12.5),
      -- and there is nothing to keep in either case.
      local path = vim.env.PATH
      if not path then
        vim.env.PATH = mason_bin
        return
      end
      local entries = { mason_bin }
      for _, entry in ipairs(vim.split(path, ":", { plain = true })) do
        -- Only Mason's own entry is filtered out. An empty entry means the working directory
        -- (`/usr/bin::/bin` looks in `.` between the two) and it stays where it is: it is the
        -- operator's configuration, and the plain prepend `mason.setup()` does keeps it too.
        if entry ~= mason_bin then
          table.insert(entries, entry)
        end
      end
      vim.env.PATH = table.concat(entries, ":")
    end,
    opts = {
      -- Mason's own default, named because the `init` above hard-codes the same prepend.
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
        "clangd", -- c/cpp
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
        "nil_ls", -- nix
        "pyright",
        "ruby_lsp",
        "ruff",
        "rust_analyzer",
        "terraformls",
        "tflint",
        "zls",
      },
      -- rustaceanvim manages rust-analyzer itself, attaching a client named `rust-analyzer`
      -- rather than mason-lspconfig's `rust_analyzer`, and its README warns against also
      -- calling lspconfig's own setup for it: doing so starts a second server on the same
      -- buffer, which is what dot_config/nvim's own probe reproduced (see
      -- docs/research/2026-09-rust-neotest-disposition.md, finding 5). Mason still installs
      -- the `rust_analyzer` binary above; only the automatic client start is excluded.
      automatic_enable = { exclude = { "rust_analyzer" } },
    },
  },
  {
    "WhoIsSethDaniel/mason-tool-installer.nvim",
    -- Its `init.lua` requires `mason-registry`, `mason.version` and
    -- `mason-lspconfig` the moment it loads, so an untriggered spec here drags
    -- the whole Mason side of the group into startup and makes their triggers
    -- inert. `run_on_start` is false, so the commands are what it exists for.
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

      -- if set to true this will check each tool for updates. If updates
      -- are available the tool will be updated. This setting does not
      -- affect :MasonToolsUpdate or :MasonToolsInstall.
      -- Default: false
      auto_update = false,

      -- automatically install / update on startup. If set to false nothing
      -- will happen on startup. You can use :MasonToolsInstall or
      -- :MasonToolsUpdate to install tools and check for updates.
      -- Default: true
      -- Off here so the apply-time `:MasonToolsInstallSync` owns installation and
      -- cannot race an autostart run of the same queue.
      run_on_start = false,

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
    -- none-ls is here for its CODE ACTIONS and nothing else. Formatting moved to
    -- conform.nvim (plugins/conform.lua) and diagnostics to nvim-lint
    -- (plugins/nvim-lint.lua); neither of those replaces a code-action source, so
    -- the plugin stays, narrowed to the two sources that provide one.
    --
    -- Do not add a formatter or a diagnostic back here. A none-ls source is a
    -- language server as far as Neovim is concerned, so a formatter registered
    -- here would compete with conform for the buffer and a diagnostic would
    -- publish into the shared LSP namespace instead of nvim-lint's own.
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
          code_actions.refactoring, -- Filetypes: go, javascript, lua, python, typescript.
        },
      })
    end,
  },
}
