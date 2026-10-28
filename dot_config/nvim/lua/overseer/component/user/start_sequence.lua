-- Stamps each task with a rising number when it starts,
-- because `time_start` counts only whole seconds.
local sequence = 0

---@type overseer.ComponentFileDefinition
return {
  desc = "Record the order in which this task started",
  constructor = function()
    return {
      on_start = function(_, task)
        sequence = sequence + 1
        task.metadata = task.metadata or {}
        task.metadata.start_sequence = sequence
      end,
    }
  end,
}
