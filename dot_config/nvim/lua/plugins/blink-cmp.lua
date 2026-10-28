return {
  {
    "xzbdmw/colorful-menu.nvim",
    config = function()
      require("colorful-menu").setup({
        ls = {
          lua_ls = {
            arguments_hl = "@comment",
          },
          gopls = {
            align_type_to_right = true,
            add_colon_before_type = false,
            preserve_type_when_truncate = true,
          },
          ts_ls = {
            extra_info_hl = "@comment",
          },
          vtsls = {
            extra_info_hl = "@comment",
          },
          ["rust-analyzer"] = {
            extra_info_hl = "@comment",
            align_type_to_right = true,
            preserve_type_when_truncate = true,
          },
          clangd = {
            extra_info_hl = "@comment",
            align_type_to_right = true,
            import_dot_hl = "@comment",
            preserve_type_when_truncate = true,
          },
          zls = {
            align_type_to_right = true,
          },
          roslyn = {
            extra_info_hl = "@comment",
          },
          dartls = {
            extra_info_hl = "@comment",
          },
          basedpyright = {
            extra_info_hl = "@comment",
          },
          pylsp = {
            extra_info_hl = "@comment",
            arguments_hl = "@comment",
          },
          fallback = true,
          fallback_extra_info_hl = "@comment",
        },
        fallback_highlight = "@variable",
        max_width = 60,
      })
    end,
  },
  {
    "saghen/blink.cmp",
    version = "1.*",
    event = { "InsertEnter", "CmdlineEnter" },
    opts_extend = {
      "sources.completion.enabled_providers",
      "sources.compat",
      "sources.default",
    },
    dependencies = {
      -- `draw.components` calls it, so it must be set up before the menu first draws.
      "xzbdmw/colorful-menu.nvim",
      "folke/lazydev.nvim",
      "rafamadriz/friendly-snippets",
      "bydlw98/blink-cmp-env",
      "Kaiser-Yang/blink-cmp-git",
      "moyiz/blink-emoji.nvim",
      "joelazar/blink-calc",
      {
        "saghen/blink.compat",
        version = "2.*",
      },
      {
        "mikavilpas/blink-ripgrep.nvim",
        version = "*",
      },
    },

    ---@module 'blink.cmp'
    ---@type blink.cmp.Config
    opts = {
      keymap = { preset = "default" },

      appearance = {
        nerd_font_variant = "mono",
      },

      completion = {
        accept = {
          auto_brackets = {
            enabled = true,
          },
        },
        documentation = {
          auto_show = true,
        },
        menu = {
          draw = {
            columns = { { "kind_icon" }, { "label", gap = 1 } },
            components = {
              label = {
                text = function(ctx)
                  return require("colorful-menu").blink_components_text(ctx)
                end,
                highlight = function(ctx)
                  return require("colorful-menu").blink_components_highlight(ctx)
                end,
              },
            },
          },
        },
        ghost_text = {
          enabled = true,
        },
      },

      sources = {
        compat = {},
        providers = {
          lazydev = {
            name = "LazyDev",
            module = "lazydev.integrations.blink",
            score_offset = 100,
          },
          lsp = {
            name = "LSP",
            enabled = true,
            module = "blink.cmp.sources.lsp",
            score_offset = 1,
            fallbacks = {},
          },
          snippets = {
            name = "Snippets",
            module = "blink.cmp.sources.snippets",
            score_offset = -1,
          },
          ripgrep = {
            module = "blink-ripgrep",
            name = "Ripgrep",
            ---@module "blink-ripgrep"
            ---@type blink-ripgrep.Options
            opts = {},
          },
          env = {
            name = "Env",
            module = "blink-cmp-env",
            --- @type blink-cmp-env.Options
            opts = {
              show_braces = false,
              show_documentation_window = true,
            },
          },
          git = {
            module = "blink-cmp-git",
            name = "Git",
            enabled = function()
              return vim.tbl_contains({ "octo", "gitcommit", "markdown" }, vim.bo.filetype)
            end,
            --- @module 'blink-cmp-git'
            --- @type blink-cmp-git.Options
            opts = {},
          },
          calc = {
            name = "Calc",
            module = "blink-calc",
          },
          emoji = {
            module = "blink-emoji",
            name = "Emoji",
            score_offset = 15,
            opts = {
              insert = true,
              ---@type string|table|fun():table
              trigger = function()
                return { ":" }
              end,
            },
          },
        },
        default = {
          "lazydev",
          "snippets",
          "buffer",
          "lsp",
          "env",
          "git",
          "path",
          "ripgrep",
          "emoji",
          "calc",
        },
      },

      cmdline = {
        enabled = true,
        keymap = {
          preset = "cmdline",
          ["<Right>"] = false,
          ["<Left>"] = false,
        },
        completion = {
          list = { selection = { preselect = false } },
          menu = {
            auto_show = function(_ctx)
              return vim.fn.getcmdtype() == ":"
            end,
          },
          ghost_text = { enabled = true },
        },
      },

      fuzzy = {
        implementation = "prefer_rust",
        sorts = {
          "exact",
          "score",
          "sort_text",
          "label",
        },
      },

      signature = {
        enabled = true,
      },
    },
    config = function(_, opts)
      local enabled = opts.sources.default
      for _, source in ipairs(opts.sources.compat or {}) do
        opts.sources.providers[source] = vim.tbl_deep_extend(
          "force",
          { name = source, module = "blink.compat.source" },
          opts.sources.providers[source] or {}
        )
        if type(enabled) == "table" and not vim.tbl_contains(enabled, source) then
          table.insert(enabled, source)
        end
      end

      -- blink.cmp's validation rejects this custom key.
      opts.sources.compat = nil

      require("blink.cmp").setup(opts)
    end,
  },
}
