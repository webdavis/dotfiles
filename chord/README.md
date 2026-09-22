# chord

Readline bindings are written in an escape syntax nobody enjoys reading. A single chord looks like this
in a shell startup file:

```bash
builtin bind -m vi-insert '"\C-gaa": "\C-x0git add "'
builtin bind -m vi-command '"\C-gaa": "i\C-x0git add "'
```

Two lines, six escapes, one readable idea: pressing Ctrl-G then `a` then `a` puts `git add ` on the line.
`chord` lets you write the idea once, in a table a person can read, and compiles it into the shell's own
form:

```toml
[[group.binding]]
key = "ctrl-g a a"
description = "Stage a path."
insert = "git add "
```

The table is the source of truth. The generated file is a build artifact you never edit by hand, and
`chord check` is there to prove you have not.

## Installation

```bash
cargo install --git https://github.com/webdavis/chord chord
```

That repository does not exist yet. `chord` currently lives inside a larger repository and has not been
extracted, so the line above is the installation instruction as it will be, not a URL you can fetch
today.

## The table

A table is one TOML file. It holds groups, and a group holds bindings.

A **group** is a documented section. Its `name` labels the section in the generated file, and its
optional `description` becomes a comment above it. Groups also fix the order of the output, so bindings
appear in the file in the order you wrote them.

A **binding row** is one chord. It needs a `key`, it names exactly one action, and it may carry a
`description` (a comment above the binding) and a `modes` list.

```toml
[[group]]
name = "git"
description = "Staging and committing without leaving the prompt."

[[group.binding]]
key = "ctrl-g a a"
description = "Stage a path."
insert = "git add "
```

### Key notation

A key is space-separated tokens, pressed in sequence. Each token is one of `ctrl-<char>`, `alt-<char>`,
`esc`, `enter`, `tab`, `space`, `backspace`, or a single literal character.

So `ctrl-g a a` means Ctrl-G, then `a`, then `a`, and renders as `\C-gaa`. An unrecognised token is
refused with the row it came from, rather than reaching the generated file as something readline will not
accept.

### Action kinds

A row names exactly one of these. Naming none, or naming two, is refused.

| Kind       | What pressing the chord does                                       |
| ---------- | ------------------------------------------------------------------ |
| `insert`   | Types the text and leaves it on the line for you to edit.          |
| `run`      | Types the text and runs it.                                        |
| `function` | Calls a shell function, through readline's `bind -x`.              |
| `command`  | Runs a readline command, such as `beginning-of-line`.              |
| `macro`    | A readline macro body in readline's own escapes, emitted verbatim. |

`macro` is the escape hatch for chords the other four cannot express. Nothing is escaped or rewritten on
the way out, so what you write is what readline gets.

### Modes

`modes` lists the editing keymaps a row binds in.

- **Absent** binds in both vi keymaps, `vi-insert` and `vi-command`, which is what nearly every chord
  wants. A command-mode binding that types text gets a leading `i` so it enters insert mode first.
- **Present** binds in exactly the keymaps you list, for example `modes = ["emacs"]` or
  `modes = ["vi-insert"]`.
- **Empty**, `modes = []`, binds in whatever keymap is current, with no mode flag at all.

## The `[render]` section

Everything about the generated files themselves lives in one optional top-level section. Every key in it
is optional, and a table with no `[render]` section at all still renders.

```toml
[render]
regenerate = "make bindings"

[render.bash]
output = "bindings.sh"
clear_line = "\\C-x0"
header = '''
# shellcheck shell=bash
#
# GENERATED FILE. Edit bindings.toml and run `make bindings`.
'''

[render.menu]
output = "bindings.tsv"
header = '''
# GENERATED FILE. Edit bindings.toml and run `make bindings`.
#
'''
```

### `regenerate`

The command that rebuilds your generated files. A failed `chord check` names it, so whoever hit the
failure knows how to fix it. Absent, the failure says to render the table again and names no command.

### `output`, per target

The file that target's rendering is written to. **Absent sends the rendering to standard output**, so
`chord render bash --table bindings.toml > bindings.sh` keeps working and you can pipe the rendering
anywhere.

Present, `chord` writes the file itself, and writes it atomically: the text goes to a working file beside
the target and is renamed over it. A render that fails partway leaves the previous file intact rather
than a truncated one.

A relative `output` is read from the **working directory**, the same as the paths you pass to `--table`
and `--against`. Run your renders from one directory and every path in the table means the same thing
every time.

### `header`, per target

The comment block the generated file opens with. Put whatever the readers of that file need there: an
editor modeline, a linter directive, a note saying the file is generated and where to edit instead.

Absent, the target writes a neutral banner naming the generator and saying an edit will be lost.

**Write a header as a literal TOML string, `'''`, never a basic one, `"""`.** A basic string reads `\C`
in `\C-x0` as an escape sequence and rejects the file. This is the first thing that bites anyone whose
header mentions a readline macro.

### `clear_line`, bash only

The readline macro that clears the line before a binding types over it. Without one, a chord pressed on a
line that already has text on it appends to that text instead of replacing it.

Readline has no such macro under a name of its own, so you bind one yourself, in the same file that holds
the shell functions your bindings call. Conventionally it goes on `\C-x0`:

```bash
builtin bind '"\C-x0": kill-whole-line'
```

**`clear_line` is a basic TOML string, and its value needs two backslashes.** `"\\C-x0"` produces the
five characters `\C-x0`, which is what readline wants. A single backslash is an invalid escape and TOML
rejects it. Note that this is the opposite of `header`, which must be a literal string: `header` contains
the escapes and must not have them interpreted, while `clear_line` is written as an escape and must have
it resolved.

**Only `insert` and `run` rows need it.** They are the kinds that type text. A `command`, `function` or
`macro` row never asks for one, so a table using only those needs no `clear_line` and no `[render]`
section.

A row that types text when no `clear_line` is configured is refused, by the key of the row, before
anything is written:

```
binding "ctrl-g a a": types over the line, which needs the clear-line macro no `clear_line` under [render.bash] names
```

That is deliberate. Rendering such a row without the macro produces a binding that silently misbehaves at
the prompt, which is worse than a render that stops and tells you why.

## Targets

`chord render <target>` and `chord check <target>` take one of two targets.

- **`bash`** produces readline `bind` calls, a file your shell startup sources. This is the binding file
  itself.
- **`menu`** produces one tab-separated record per row, `key`, group, kind, action, description. It is
  for a picker: a fuzzy finder over your own chords, so you can search the bindings you forgot you had
  and run one. Every row reaches it whatever keymaps the row binds in, so the picker shows the whole
  surface rather than one keymap's worth.

## Workflow

Render, then check.

```bash
chord render bash --table bindings.toml
chord check bash --table bindings.toml
```

`render` rebuilds the generated file. `check` renders the table again in memory and compares it with the
file on disk, printing a unified diff and exiting 1 when the two disagree.

`check` exists because a generated file is editable and looks editable. Someone fixes a binding in
`bindings.sh`, the next render silently overwrites it, and the fix is gone with no sign it was ever
there. Run `check` in your test suite or a pre-commit hook and that edit fails a gate instead.

With no `--against`, `check` compares the file the table's `output` names, which is the file `render`
writes. Pass `--against <file>` to compare something else.

## Exit codes

| Code | Meaning                                                                                                      |
| ---- | ------------------------------------------------------------------------------------------------------------ |
| 0    | The render or the check succeeded.                                                                           |
| 1    | `check` only: the file on disk is not what the table renders. The diff is on stderr.                         |
| 2    | Refused. Bad arguments, an unknown target, an unreadable or invalid table, or a row the table cannot render. |

A refusal always names what it refused. A row-level refusal names the row's key, because a table of
hundreds of chords is otherwise a long file to search.
