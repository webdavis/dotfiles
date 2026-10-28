return {
  "nvim-treesitter/nvim-treesitter",
  dependencies = {
    {
      "nvim-treesitter/nvim-treesitter-context",
      opts = {
        max_lines = 4,
        multiline_threshold = 2,
      },
      config = function(_, opts)
        local treesitter_context = require("treesitter-context")
        treesitter_context.setup(opts)

        map({
          mode = "n",
          lhs = "[r",
          rhs = function()
            treesitter_context.go_to_context(vim.v.count1)
          end,
          silent = true,
          desc = "nvim-treesitter-context: jump up to context",
        })
      end,
    },
    {
      "nvim-treesitter/nvim-treesitter-textobjects",
      branch = "main",
      init = function()
        -- Turns off the mappings that built-in ftplugins add, so they cannot clash with these.
        vim.g.no_plugin_maps = true
      end,
      config = function()
        require("nvim-treesitter-textobjects").setup({
          select = {
            -- Jumps forward to the next text object when the cursor is not on one, as targets.vim does.
            lookahead = true,
            selection_modes = {
              ["@parameter.outer"] = "v",
              ["@function.outer"] = "V",
            },
            include_surrounding_whitespace = false,
          },
          move = {
            set_jumps = true,
          },
        })

        local select = require("nvim-treesitter-textobjects.select")

        -- stylua: ignore start
        map({ mode = { "x", "o" }, lhs = "ac", rhs = function() select.select_textobject("@class.outer", "textobjects") end, desc = "outer class" })
        map({ mode = { "x", "o" }, lhs = "ic", rhs = function() select.select_textobject("@class.inner", "textobjects") end, desc = "inner class" })

        map({ mode = { "x", "o" }, lhs = "al", rhs = function() select.select_textobject("@loop.outer", "textobjects") end, desc = "outer loop" })
        map({ mode = { "x", "o" }, lhs = "il", rhs = function() select.select_textobject("@loop.inner", "textobjects") end, desc = "inner loop" })

        map({ mode = { "x", "o" }, lhs = "as", rhs = function() select.select_textobject("@local.scope", "locals") end, desc = "local scope" })

        local move = require("nvim-treesitter-textobjects.move")

        map({ mode = { "n", "x", "o" }, lhs = "[c", rhs = function() move.goto_previous_start({ "@class.outer", "@class.inner" }, "textobjects") end, desc = "previous class start" })
        map({ mode = { "n", "x", "o" }, lhs = "]c", rhs = function() move.goto_next_start({ "@class.outer", "@class.inner" }, "textobjects") end, desc = "next class start" })
        map({ mode = { "n", "x", "o" }, lhs = "[C", rhs = function() move.goto_previous_end({ "@class.outer", "@class.inner" }, "textobjects") end, desc = "previous class end" })
        map({ mode = { "n", "x", "o" }, lhs = "]C", rhs = function() move.goto_next_end({ "@class.outer", "@class.inner" }, "textobjects") end, desc = "next class end"})

        map({ mode = { "n", "x", "o" }, lhs = "[m", rhs = function() move.goto_previous_start({ "@function.outer", "@function.inner" }, "textobjects") end, desc = "previous function start" })
        map({ mode = { "n", "x", "o" }, lhs = "]m", rhs = function() move.goto_next_start({ "@function.outer", "@function.inner" }, "textobjects") end, desc = "next function start" })
        map({ mode = { "n", "x", "o" }, lhs = "[M", rhs = function() move.goto_previous_end({ "@function.outer", "@function.inner" }, "textobjects") end, desc = "previous function end" })
        map({ mode = { "n", "x", "o" }, lhs = "]M", rhs = function() move.goto_next_end({ "@function.outer", "@function.inner" }, "textobjects") end, desc = "next function end" })

        map({ mode = { "n", "x", "o" }, lhs = "[l", rhs = function() move.goto_previous_start({ "@loop.outer", "@loop.inner" }, "textobjects") end, desc = "previous loop start" })
        map({ mode = { "n", "x", "o" }, lhs = "]l", rhs = function() move.goto_next_start({ "@loop.outer", "@loop.inner" }, "textobjects") end, desc = "next loop start" })
        map({ mode = { "n", "x", "o" }, lhs = "[L", rhs = function() move.goto_previous_end({ "@loop.outer", "@loop.inner" }, "textobjects") end, desc = "previous loop end" })
        map({ mode = { "n", "x", "o" }, lhs = "]L", rhs = function() move.goto_next_end({ "@loop.outer", "@loop.inner" }, "textobjects") end, desc = "next loop end" })

        map({ mode = { "n", "x", "o" }, lhs = "]s", rhs = function() move.goto_next("@local.scope", "locals") end, desc = "next local scope" })
        map({ mode = { "n", "x", "o" }, lhs = "[s", rhs = function() move.goto_previous("@local.scope", "locals") end, desc = "previous local scope" })

        map({ mode = { "n", "x", "o" }, lhs = "]z", rhs = function() move.goto_next("@fold", "folds") end, desc = "next fold" })
        map({ mode = { "n", "x", "o" }, lhs = "[z", rhs = function() move.goto_previous("@fold", "folds") end, desc = "previous fold" })

        map({ mode = { "n", "x", "o" }, lhs = "]k", rhs = function() move.goto_next({ "@conditional.outer", "@conditional.inner" }, "textobjects") end, desc = "next conditional" })
        map({ mode = { "n", "x", "o" }, lhs = "[k", rhs = function() move.goto_previous({ "@conditional.outer", "@conditional.inner" }, "textobjects") end, desc = "previous conditional" })

        local ts_repeat_move = require("nvim-treesitter-textobjects.repeatable_move")

        -- `;` repeats in the direction of the last move.
        map({ mode = { "n", "x", "o" }, lhs = ";", rhs = ts_repeat_move.repeat_last_move, desc = "treesitter-textobjects: repeat last move" })
        map({ mode = { "n", "x", "o" }, lhs = ",", rhs = ts_repeat_move.repeat_last_move_opposite, desc = "treesitter-textobjects: repeat last move (opposite)" })

        -- Make f, F, t and T repeatable with ; and ,.
        map({ mode = { "n", "x", "o" }, lhs = "f", rhs = ts_repeat_move.builtin_f_expr, expr = true, desc = "treesitter-textobjects: f (with repeat)" })
        map({ mode = { "n", "x", "o" }, lhs = "F", rhs = ts_repeat_move.builtin_F_expr, expr = true, desc = "treesitter-textobjects: F (with repeat)" })
        map({ mode = { "n", "x", "o" }, lhs = "t", rhs = ts_repeat_move.builtin_t_expr, expr = true, desc = "treesitter-textobjects: t (with repeat)" })
        map({ mode = { "n", "x", "o" }, lhs = "T", rhs = ts_repeat_move.builtin_T_expr, expr = true, desc = "treesitter-textobjects: T (with repeat)" })
      end,
    },
  },
  lazy = false,
  branch = "main",
  build = ":TSUpdate",
  opts = {
    ensure_installed = {
      "bash",
      "c",
      "clojure",
      "cpp",
      "css",
      "diff",
      "dockerfile",
      "ecma",
      "eex",
      "elixir",
      "git_config",
      "git_rebase",
      "gitattributes",
      "gitcommit",
      "gitignore",
      -- Installed up front, because neotest-golang finds Go tests with this grammar
      -- and the bootstrap waits for this list.
      "go",
      "gpg",
      "graphql",
      "haskell",
      "hcl",
      "heex",
      "html",
      "html_tags",
      "http",
      "ini",
      "java",
      "javascript",
      "jq",
      "json",
      "jsx",
      "just",
      "latex",
      "lua",
      "markdown",
      "markdown_inline",
      "ninja",
      "nix",
      "nu",
      "python",
      "query",
      "regex",
      "ron",
      "rst",
      "rust",
      "swift",
      "terraform",
      "toml",
      "tsx",
      "typescript",
      "typst",
      "vim",
      "vimdoc",
      "xml",
      "yaml",
      "zig",
    },
  },
  config = function(plugin, opts)
    local treesitter = require("nvim-treesitter")

    -- Buffers waiting for a parser, keyed by language and then by buffer.
    local waiting_buffers = {}

    -- Languages being installed, so each installs once.
    local installing_languages = {}

    local treesitter_setup_group = vim.api.nvim_create_augroup("TreesitterSetup", { clear = true })

    local function enable_treesitter(buffer, language)
      if not vim.api.nvim_buf_is_valid(buffer) then
        return false
      end

      local ok = pcall(vim.treesitter.start, buffer, language)
      if ok then
        vim.api.nvim_set_option_value("indentexpr", "v:lua.require'nvim-treesitter'.indentexpr()", { buf = buffer })

        for _, win in ipairs(vim.api.nvim_list_wins()) do
          if vim.api.nvim_win_get_buf(win) == buffer and vim.api.nvim_win_is_valid(win) then
            vim.api.nvim_set_option_value("foldmethod", "expr", { win = win })
            vim.api.nvim_set_option_value("foldexpr", "v:lua.vim.treesitter.foldexpr()", { win = win })
          end
        end
      end
      return ok
    end

    vim.api.nvim_create_autocmd("User", {
      group = treesitter_setup_group,
      pattern = "LazyDone",
      once = true,
      desc = "Install core treesitter parsers",
      callback = function()
        plugin._.core_parser_install = treesitter.install(opts.ensure_installed)
      end,
    })

    -- `checkhealth` resolves to the `vimdoc` grammar, which would highlight a health report as help.
    local ignore_filetypes = {
      checkhealth = true,
    }

    -- nvim-treesitter skips a language missing from its parser table, so the poll below would wait
    -- for nothing. The table is checked on every call, because `:TSUpdate` and `:TSInstall` change it.
    ---@param language string
    ---@return boolean
    local function is_installable(language)
      return vim.list_contains(require("nvim-treesitter.config").get_available(), language)
    end

    vim.api.nvim_create_autocmd("FileType", {
      group = treesitter_setup_group,
      desc = "Enable treesitter highlighting and indentation",
      callback = function(event)
        local filetype = event.match

        if ignore_filetypes[filetype] then
          return
        end

        local language = vim.treesitter.language.get_lang(filetype) or filetype
        local buffer = event.buf

        if not enable_treesitter(buffer, language) then
          -- Checked after the highlight attempt, so a filetype that already has a parser
          -- is always highlighted.
          if not is_installable(language) then
            return
          end

          waiting_buffers[language] = waiting_buffers[language] or {}
          waiting_buffers[language][buffer] = true

          if not installing_languages[language] then
            installing_languages[language] = true
            treesitter.install({ language })

            local attempts = 0
            local max_attempts = 60 -- polls every half second, so this gives up after about 30 seconds
            local function poll_parser()
              attempts = attempts + 1
              local ok, result = pcall(vim.treesitter.language.add, language)

              if ok and result then
                installing_languages[language] = nil
                local buffers = waiting_buffers[language]
                if buffers then
                  for b in pairs(buffers) do
                    enable_treesitter(b, language)
                  end
                  waiting_buffers[language] = nil
                end
              elseif attempts < max_attempts then
                vim.defer_fn(poll_parser, 500)
              else
                installing_languages[language] = nil
                waiting_buffers[language] = nil
              end
            end

            vim.defer_fn(poll_parser, 1000)
          end
        end
      end,
    })

    vim.api.nvim_create_autocmd("BufDelete", {
      group = treesitter_setup_group,
      desc = "Clean up treesitter waiting buffers",
      callback = function(event)
        for lang, buffers in pairs(waiting_buffers) do
          buffers[event.buf] = nil
          if next(buffers) == nil then
            waiting_buffers[lang] = nil
          end
        end
      end,
    })
  end,
}
