rex.bind("ctrl+d>ctrl+d", "pane.send_key", { key = "ctrl+d" })
rex.bind("ctrl+d>ctrl+l", "pane.send_key", { key = "ctrl+l" })

rex.bind("ctrl+d>1", "client.tab.goto", { index = 1 })
rex.bind("ctrl+d>2", "client.tab.goto", { index = 2 })
rex.bind("ctrl+d>3", "client.tab.goto", { index = 3 })
rex.bind("ctrl+d>4", "client.tab.goto", { index = 4 })
rex.bind("ctrl+d>5", "client.tab.goto", { index = 5 })
rex.bind("ctrl+d>6", "client.tab.goto", { index = 6 })
rex.bind("ctrl+d>7", "client.tab.goto", { index = 7 })
rex.bind("ctrl+d>8", "client.tab.goto", { index = 8 })
rex.bind("ctrl+d>9", "client.tab.goto", { index = 9 })
rex.bind("ctrl+d>shift+n", "client.tab.move", { direction = "forward" })
rex.bind("ctrl+d>shift+p", "client.tab.move", { direction = "backward" })
rex.bind("ctrl+d>alt+n", "client.tab.rename")
rex.bind("ctrl+d>,", "client.tab.rename")
rex.bind("ctrl+d>ctrl+n", "client.tab.new")

rex.bind("ctrl+d>]", "session.next")
rex.bind("ctrl+d>[", "session.previous")
rex.bind("ctrl+d>alt+[", "session.rename")
rex.bind("ctrl+d>ctrl+,", "session.rename")
rex.bind("ctrl+d>ctrl+[", "session.new")

rex.bind("ctrl+d>shift+h", "pane.move", { direction = "left" })
rex.bind("ctrl+d>shift+j", "pane.move", { direction = "down" })
rex.bind("ctrl+d>shift+k", "pane.move", { direction = "up" })
rex.bind("ctrl+d>shift+l", "pane.move", { direction = "right" })
rex.bind("ctrl+d>alt+h", "pane.rename")
rex.bind("ctrl+d>shift+,", "pane.rename")
rex.bind("ctrl+d>shift+'", "pane.split", { direction = "down" })
rex.bind("ctrl+d>shift+5", "pane.split", { direction = "right" })
rex.bind("ctrl+d>=", "pane.zoom")

rex.bind("ctrl+d>alt+x", "session.close")
rex.bind("ctrl+d>g", "session.switch")
rex.bind("ctrl+d>shift+o", "session.switch")
rex.bind("ctrl+d>c", "pane.copy_mode.enter")
rex.bind("ctrl+d>shift+r", "client.config.reload")

rex.bind("cmd+b", "client.sidebar.toggle")
rex.bind("cmd+/", "pane.view_activity")

local pwe_loaded, pwe = pcall(require, "pwe")
if pwe_loaded then
  pwe.setup()
else
  print("pwe: its pane keys were not loaded: " .. tostring(pwe))
end
