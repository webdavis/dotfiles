-- herdr-nvim-annotate-extension.nvim: the line annotator (spec 7.7).
--
-- It used to be `custom_api/annotate.lua`. It is a published plugin now
-- (operator ruling 2026-09-05): what the annotator is coupled to is
-- `herdr-nvim`, whose annotation store it writes into, and nothing in this
-- config, so the dependency is declared here rather than hidden in a private
-- module. The plugin carries its own git edge and string helpers;
-- `custom_api.git` and `custom_api.util` keep theirs for their other consumers.
--
-- `<leader>Cx` is its only entry point, and this `keys` row IS the mapping: no
-- `map()` call declares it anywhere else. It lives here rather than on
-- `claudecode.lua` because the annotator is not a claudecode.nvim command and
-- needs no WebSocket server; the `<leader>C` prefix stays because the
-- annotation is written FOR an agent to read.
--
-- The plugin ships `:HerdrAnnotateLine`, which calls `line()` and notifies the
-- refusal itself. This config binds `line()` directly instead, so the refusal
-- and any raise out of herdr-nvim go through the keymap-layer error boundary
-- (spec 6.1) like every other keymap here rather than through a second
-- notifier.
return {
  "webdavis/herdr-nvim-annotate-extension.nvim",
  commit = "a7713d5e857c8c8c41ea6cd707eda07c2d082c9c",
  dependencies = { "ChmaraX/herdr-nvim" },
  keys = {
    {
      "<leader>Cx",
      function()
        require("custom_api.try")(function()
          return require("herdr-nvim-annotate-extension").line()
        end, { label = "herdr-nvim-annotate-extension.line" })
      end,
      desc = "Claude: annotate line with diagnostic and blame",
    },
  },
}
