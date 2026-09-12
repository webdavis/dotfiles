---
name: scalebar
description: Use when logging or reading Stephen's body weight and gym training data through the Scalebar app. Trigger on a weight said out loud ("182 this morning", "log my bedtime weight"), a set finished at the gym, a workout starting or ending, or a question about weight trend, streak, training volume, personal records, training load, or how an exercise has progressed. The mcp__scalebar__* tools are the only way in; never edit the CSV files by hand.
metadata:
  updatedAt: "2026-09-12"
---

# Scalebar

Scalebar is a menu-bar app that owns two records: a body-weight log and a gym log. Its MCP (Model Context Protocol) server
exposes both. Every tool carries its own schema, so this file covers only what the schemas do not
say.

## Fitness reports

When Stephen asks for fitness stats, training stats, advanced stats, or a fitness report, call
`get_fitness_report` and present its output as returned. Do not rebuild the report by combining
the lower-level tools, and do not replace its summary table or labeled explanations with a shorter
summary. The report is the canonical format shared by Claude, Claude Code, Codex, Hermes, and other
harnesses that load this skill.

Pass `days` when Stephen requests a specific reporting period. If no period is requested, use the
tool default. Preserve the report's `Date Range:` and `Period:` lines, its three-column summary
table, explicit labels such as `RIR Definition:`, `Calculation:`, `Meaning:`, `Why It Matters:`,
and `Why Unavailable:`, and the `PERSONAL RECORDS (PRs)` heading.

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

Use `edit_gym_set` to correct an earlier gym set. Obtain its `setId` from `get_workout_review`, then
send only the changed weight, reps, or RIR (reps in reserve). Logging another set adds a record; it
does not correct the earlier one. `list_gym_revisions` and `restore_gym_revision` provide targeted
recovery. A restore preserves unrelated sets and refuses to overwrite a later conflicting edit.

`log_weight`, `log_start_time`, and `log_end_time` overwrite their matching date fields. Their
`clear_*` tools remove the field. Keep correction and removal distinct.

## Logging a set

Read `get_workout_state` and `get_workout_settings` for the selected date and workout. Use the stored
exercise, equipment, weight convention, side mode, and planned round. Do not reinterpret older rows
whose equipment or weight convention is unknown.

Assign a UUID (universally unique identifier) as `setId` before each logging call. Reuse that same
identifier and identical fields when retrying after a timeout. A new identifier means a new set.
Scalebar verifies its local readback before reporting success; that does not confirm Obsidian Sync
has delivered the change to another device.

- Send `weight: 0` for bodyweight. Storage uses `bodyweight`, and views show `BW`. Completed reps
  and reserve ratings count; bodyweight contributes no added-weight volume or estimated one-rep max.
  Bodyweight repetition records remain available.
- Weight follows the exercise's saved `weightBasis`. With `per-hand`, enter each dumbbell's weight.
  `loadMultiplier` indicates whether each completed rep moves one or both weights. With `total`,
  enter the combined added weight and use multiplier 1. Never multiply a per-hand entry yourself.
- Log unilateral reps as explicit `side: left` and `side: right` rows sharing a `roundId` and planned
  `setNumber`. Each side has its own `setId`, reps, and optional reserve rating. Three rounds with
  both sides are three rounds and six side sets. A single-side set records only the performed side.
- With the saved per-hand convention, walking lunges with 30 lb in each hand and five left/four
  right reps produce two rows: weight 30,
  `weightBasis: per-hand`, `loadMultiplier: 2`, reps `"5"` left and `"4"` right. That is four
  complete pairs plus one extra left rep. If the saved convention is total weight, use 60 and
  multiplier 1 instead. A failed attempt is not a completed rep.
- For sequential concentration curls, use per-hand weight and multiplier 1. Log left and right
  separately within the same round. Rest starts after both sides are recorded.
- `rir` is 0 to 4 and applies only to `working`, `rest-pause`, and `drop-set`. Missing is unknown,
  never zero. Warmups, skipped sets, isometric holds, and timed activities have no reserve rating.
- `reps` is a string: completed reps, `skipped`, or elapsed seconds for `isometric` and `timed`.
  For a known unloaded bodyweight hold, send weight 0. Omit weight only when it is unknown or
  does not apply, such as a skipped set. An isometric hold is distinct from a timed run.

The tools enforce these data semantics. The skill describes entry choices; it is not responsible
for guessing resistance type or correcting statistics after the fact.

## Reading it back

- `get_workout_review` returns the stored plan, completion counts, individual sets, and matching
  previous-session comparisons. Historical targets without a saved snapshot are unknown.
- `get_workout_trends` returns equipment- and side-specific records, user milestones, direct and
  secondary muscle sets, and separate morning/bedtime bodyweight means with sample counts.
- `get_exercise_progression` shows recent sets in their recorded weight and equipment context.
- `get_volume` sums entered weight times the recorded load multiplier times completed reps across
  effort sets. Warmups, holds, timed activities, and bodyweight add no external weighted work.
- `get_fitness_report` remains the canonical user-facing report, including definitions and meaning.
- `get_prs` returns Personal Records (PRs). Compare bodyweight by completed reps and isometric holds
  by elapsed seconds at the same load. Keep equipment and side contexts separate.
- `get_training_load` describes recorded workload and effort. Do not present it as an injury
  prediction or proof that a workload is safe.

Use `update_workout_settings` for user-chosen goals, muscle assignments, equipment, notes, and
schedule preferences. Preserve unrelated settings. Never invent a goal, substitute exercise,
missing rep count, or historical plan. A current template cannot establish an older target.

Report what the tools support. Keep morning and bedtime observations separate, state sample counts,
and describe weight and exercise changes alongside each other without claiming one caused the other.
