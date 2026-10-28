-- Stamp each task with a monotonically increasing number every time it STARTS.
--
-- Overseer records `time_start` with `os.time()`, one-second resolution, so two
-- tasks started inside the same second compare equal and "the newest task" is
-- undecidable. A creation id does not help either: an older task restarted last
-- is the newest run, and its id is still the lower one.
--
-- The counter is a file-local, and `require` returns one module table per
-- process, so every task stamped here shares the same sequence.
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
