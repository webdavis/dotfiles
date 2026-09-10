---
name: scalebar
description: Use when logging or reading Stephen's body weight and gym training data through the Scalebar app. Trigger on a weight said out loud ("182 this morning", "log my bedtime weight"), a set finished at the gym, a workout starting or ending, or a question about weight trend, streak, training volume, personal records, training load, or how an exercise has progressed. The mcp__scalebar__* tools are the only way in; never edit the CSV files by hand.
metadata:
  updatedAt: "2026-09-10"
---

# Scalebar

Scalebar is a menu-bar app that owns two records: a body-weight log and a gym log. Its MCP server
exposes both. Every tool carries its own schema, so this file covers only what the schemas do not
say.

## Never write the files directly

The app holds these records and writes them itself. Reaching around it with an editor or a shell
redirect races the app and loses whichever side saves last. The `mcp__scalebar__*` tools are the
whole interface.

## Ask rather than guess

A logged number is a record Stephen will read back weeks later and trust. If a value needed to log
something is missing, ask for it. Do not infer a weight from a previous day, an exercise from a
similar name, or a workout from what was logged yesterday.

The two identifiers, `exerciseId` and `workoutId`, come from Stephen's own naming. Read them back
out of `get_gym_history` before inventing one, and ask when nothing matches.

## Correcting versus removing

Both records separate the two, and the wrong choice loses data or leaves a phantom entry:

- To FIX a wrong number, log it again. `log_weight`, `log_start_time` and `log_end_time` overwrite.
- To DELETE an entry that should not exist at all, use the matching `clear_*` tool.

## Logging a set

`log_gym_set` derives the set number from what is already logged today for that exercise, so passing
one is not possible and counting sets yourself is not needed.

Two fields are conditional rather than optional:

- `weight` is omitted for a bodyweight or unweighted set, not sent as zero.
- `rir` (reps in reserve, 0 to 4, where 0 is failure) is only valid on `working`, `rest-pause` and
  `drop-set`. Sending it on a warmup or an isometric is wrong.

`reps` is a string, because a skipped set is logged as the word `skipped` rather than a count.

## Reading it back

Four tools answer four different questions, and reaching for the wrong one gives a true answer to
something nobody asked:

- `get_exercise_progression` shows what was lifted, set by set, over recent sessions. It makes no
  judgment about whether a target was hit, because planned reps live outside the gym log.
- `get_volume` sums weight times reps per session. Warmups and isometrics are excluded.
- `get_prs` answers "what is the most I have ever done", including the reps at one exact weight.
- `get_training_load` answers whether the recent load is sustainable, not what was lifted.

Report what a tool returns. These are Stephen's own numbers, so framing them as encouragement or
concern is noise on top of the answer.
