# Research: What is bash-preexec and how does atuin use it for shell history on bash?

**Generated**: 2026-03-19 **Source**: Claude.ai Research Feature (via web research)

______________________________________________________________________

## What is bash-preexec?

bash-preexec is a library by Ryan Caloras that brings Zsh-style `preexec` and `precmd` hook functions to
Bash 3.1+. Bash, unlike Zsh and Fish, does not natively provide lifecycle hooks that fire before and
after command execution. bash-preexec fills this gap by leveraging two Bash mechanisms -- the `DEBUG`
trap and `PROMPT_COMMAND` -- to simulate these hooks.

The project is used in production by several notable tools including Bashhub, iTerm2, and Ghostty.

### The Two Hooks

- **`preexec`**: Invoked just after a command has been read from the terminal and is about to be
  executed. The command string typed by the user is passed as the first argument.
- **`precmd`**: Invoked just before each prompt is displayed. Functionally similar to `PROMPT_COMMAND`,
  but designed to be more flexible and resilient when multiple tools need to hook into the same lifecycle
  point.

### Technical Implementation

bash-preexec works by combining two Bash primitives:

1. **`DEBUG` trap**: Bash's `trap ... DEBUG` mechanism fires before every simple command. bash-preexec
   installs a function as the DEBUG trap handler. This handler inspects the current environment to
   determine whether the command is being run interactively (as opposed to internally by PROMPT_COMMAND
   or other shell infrastructure), and fires the `preexec` hook if appropriate.

1. **`PROMPT_COMMAND`**: This built-in Bash variable specifies a command to run after the previous
   command line completes and before the next prompt is displayed. bash-preexec uses this to fire the
   `precmd` hook.

A critical implementation challenge is that the DEBUG trap fires not just before user commands, but
before every simple command -- including commands within `PROMPT_COMMAND` itself, inside shell functions,
and within control structures. The library uses careful flag management (such as an `AT_PROMPT` flag) to
ensure that `preexec` fires exactly once per interactive command line, not once per sub-command.

For a compound command like `echo 1; echo 2 && echo 3`, the flow is:

1. `preexec` fires once (before all commands begin)
1. Each sub-command executes
1. `precmd` fires once (after all commands complete, before the next prompt)

### Installation

```bash
curl https://raw.githubusercontent.com/rcaloras/bash-preexec/master/bash-preexec.sh -o ~/.bash-preexec.sh
echo '[[ -f ~/.bash-preexec.sh ]] && source ~/.bash-preexec.sh' >> ~/.bashrc
```

A critical requirement: bash-preexec must be the last thing sourced in your bash profile to ensure it
properly wraps all existing `PROMPT_COMMAND` and `DEBUG` trap handlers.

### Usage

Users can define `preexec` and `precmd` functions directly, or append functions to the
`$preexec_functions` and `$precmd_functions` arrays when multiple hooks are needed. Library authors can
detect bash-preexec's presence via the `bash_preexec_imported` variable.

### Subshell Support

Subshell support is disabled by default due to known issues with `functrace` and Bash's DEBUG trap. It
can be enabled by setting `__bp_enable_subshells="true"`.

### Bash 5.3: Native Alternative

Bash 5.3 introduced native preexec-like functionality via the `PS0` prompt parameter. `PS0` is expanded
and displayed after a command is read but before it executes, providing a clean alternative to the DEBUG
trap approach:

```bash
PS0='${ preexec;}'
PROMPT_COMMAND="precmd"
```

This avoids the complexity and edge cases of the DEBUG trap approach but requires Bash 5.3+, which is not
yet widely deployed.

______________________________________________________________________

## What is Atuin?

Atuin is a tool that replaces the default shell history with a SQLite database, providing enhanced
search, sync across machines, and richer metadata (timestamps, exit codes, duration, working directory,
session tracking). It supports Bash, Zsh, Fish, and Nushell.

______________________________________________________________________

## How Atuin Uses bash-preexec for Shell History on Bash

### The Core Problem

Atuin needs to intercept two moments in the shell command lifecycle:

1. **Before a command runs**: to record the command text, timestamp, working directory, and generate a
   history ID.
1. **After a command completes**: to record the exit code and duration.

Zsh and Fish provide these hooks natively. Bash does not. This is why Atuin depends on bash-preexec (or
ble.sh) when running under Bash.

### Hook Registration

When a user runs `eval "$(atuin init bash)"`, Atuin generates a bash script that registers two functions
with bash-preexec's hook arrays:

```bash
precmd_functions+=(__atuin_precmd)
preexec_functions+=(__atuin_preexec)
```

### The `__atuin_preexec` Function

This function runs before each command via bash-preexec's preexec mechanism. It:

1. **Detects the active backend**: Determines whether ble.sh or bash-preexec is providing the hooks via
   `__atuin_update_preexec_backend`.
1. **Starts a history entry**: Calls `atuin history start` with the command text, which creates a new
   record in the SQLite database and returns a history ID.
1. **Captures the start time**: Records `EPOCHREALTIME` (on Bash 5.0+) for later duration calculation.
1. **Stores the history ID**: Saves it in `ATUIN_HISTORY_ID` for correlation with the subsequent precmd
   call.

### The `__atuin_precmd` Function

This function runs after each command completes, via bash-preexec's precmd mechanism. It:

1. **Captures exit status immediately**: Records `$?` before any other operation can alter it.
1. **Calculates duration**: Computes elapsed time using `EPOCHREALTIME` (Bash 5.0+) or ble.sh's
   `_ble_exec_time_ata`.
1. **Finalizes the history entry**: Calls
   `atuin history end --exit "$EXIT" --duration="$duration" -- "$ATUIN_HISTORY_ID"` to update the
   database record with the exit code and duration.
1. **Runs asynchronously**: Executes in the background to avoid blocking prompt display.

### Session and Environment Management

Atuin maintains several environment variables for tracking:

- **`ATUIN_SESSION`**: A UUID generated once per shell session.
- **`ATUIN_SHLVL`**: Tracks shell nesting depth.
- **`ATUIN_HISTORY_ID`**: Correlates preexec and precmd calls for the same command.
- **`ATUIN_STTY`**: Preserves terminal settings during command execution.

The integration also checks for interactive mode (`[[ $- == *i* ]]`) and only activates in interactive
shells.

### Two Backends: bash-preexec vs. ble.sh

Atuin supports two preexec backends for Bash:

| Feature                 | bash-preexec                            | ble.sh (>= 0.4)                   |
| ----------------------- | --------------------------------------- | --------------------------------- |
| Timing accuracy         | May have minor inaccuracies             | Accurate via `_ble_exec_time_ata` |
| `ignorespace` support   | Broken (silently removed from HISTOPTS) | Properly honored                  |
| Installation complexity | Simple (single script)                  | Heavier (full line editor)        |
| Recommendation          | Functional but limited                  | Recommended by Atuin docs         |

______________________________________________________________________

## Known Issues and Caveats

### The `ignorespace` Problem

bash-preexec unconditionally removes the `ignorespace` setting from `HISTOPTS` during initialization.
This means that commands prefixed with a space -- which users expect to be excluded from history -- will
appear in Bash's native history file even though Atuin itself correctly respects the setting. Users are
often unaware of this silent behavior change.

This was documented in [GitHub Issue #752](https://github.com/atuinsh/atuin/issues/752). Atuin itself
handles space-prefixed commands correctly (since an early PR), but the bash-preexec layer underneath
breaks the protection at the Bash level.

### Duration and Exit Code Accuracy

When using bash-preexec (not ble.sh), there can be minor inaccuracies in recorded command duration and
exit status for certain command types.

### Missing Commands

bash-preexec cannot properly invoke the preexec hook for subshell commands, function definitions, and
empty for-in statements, meaning these may not be recorded in Atuin's history.

### Ordering Sensitivity

bash-preexec must be sourced before `eval "$(atuin init bash)"`, and ideally as the last import in the
bash profile. Incorrect ordering can cause hooks to fail silently.

### Compatibility with Other Tools

Multiple tools competing for the DEBUG trap and PROMPT_COMMAND can cause conflicts. VS Code's shell
integration, for example, has documented issues coexisting with bash-preexec.

______________________________________________________________________

## Summary

bash-preexec is a compatibility shim that brings Zsh-style `preexec` and `precmd` hooks to Bash by
cleverly combining the `DEBUG` trap and `PROMPT_COMMAND`. Atuin relies on it (or alternatively ble.sh) to
intercept the command lifecycle on Bash -- recording commands before execution and capturing results
after completion. While functional, this approach has known limitations around `ignorespace` handling,
timing accuracy, and edge-case command types. For Bash users who need the most reliable experience, Atuin
recommends ble.sh over bash-preexec. Looking forward, Bash 5.3's native `PS0`-based preexec support may
eventually eliminate the need for either shim.

______________________________________________________________________

## Sources

- [rcaloras/bash-preexec - GitHub](https://github.com/rcaloras/bash-preexec)
- [bash-preexec README](https://github.com/rcaloras/bash-preexec/blob/master/README.md)
- [Atuin Shell Integration Documentation](https://docs.atuin.sh/cli/guide/shell-integration/)
- [atuinsh/atuin - GitHub](https://github.com/atuinsh/atuin)
- [Alert bash users to the ramifications of relying on bash-preexec - Issue #752](https://github.com/atuinsh/atuin/issues/752)
- [Tracking bash-preexec issue - Issue #2059](https://github.com/atuinsh/atuin/issues/2059)
- [DEBUG trap and PROMPT_COMMAND in Bash - Chuan Ji](https://jichu4n.com/posts/debug-trap-and-prompt_command-in-bash/)
- [Preexec hooks are finally trivial in bash 5.3 - Terminal](https://posix.nexus/posts/native-bash-preexec/)
- [bash-preexec - Homebrew Formulae](https://formulae.brew.sh/formula/bash-preexec)
- [Atuin Installation Documentation](https://docs.atuin.sh/cli/guide/installation/)
