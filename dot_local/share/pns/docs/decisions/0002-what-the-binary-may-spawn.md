# 0002: The binary's spawn roster is a closed, operator-approved list

Status: accepted. The roster is recorded in the header comment of `dot_local/share/pns/Cargo.toml`, and
this record exists so the list survives the crate being split into a workspace.

## The list

Operator-approved, and extended in place as the hooks converted on 2026-08-13:

| Command                | Why it is on the list                                                 |
| ---------------------- | --------------------------------------------------------------------- |
| `terminal-notifier`    | A banner needs a signed application bundle                            |
| `moshi-hook`           | Third party, and it owns the approval socket                          |
| `herdr`                | Its command-line interface is the supported way to ask about panes    |
| `ioreg`, `pgrep`, `ps` | No public application programming interface exists for those readings |
| `codex`                | The reply condenser, which arrived with the hooks                     |
| `git`                  | The branch lookup, which arrived with the hooks                       |
| `gh`                   | The recap's merged pull request section                               |

`gh` is the first and only entry gated by a configuration key: nothing runs it unless `[recap] repos`
names a repository. The call is one read-only listing, bounded in count, in time and in bytes. No token
is read and no credential is passed, because `gh` carries its own authentication and pns never touches
it.

## What is deliberately absent

`gtimeout` and `timeout(1)`. Every deadline in this crate is a named constant in Rust. macOS ships no
`timeout(1)`, and shelling out for one would be a spawn taken in order to bound a spawn.

`nettop` left the roster on 2026-08-15, and `ps` arrived in the same ruling. The phone's presence used to
be a one second `nettop` sample of bytes moving over moshi, which passive viewing could not move. It is
now the access time of the mosh client's pseudoterminal, and `pgrep -P` plus `ps -o tty=` are how that
pseudoterminal is found.

## The two spawns the roster cannot cover

1. An **executable channel**, run by name out of the channels directory. The roster is what this binary
   chooses to run; an executable channel is what an operator dropped in for it to run.
1. The recap's **summarizer**, where the operator names the command word by word and pns runs exactly
   what they wrote.

Both are operator-supplied by construction, so they are governed by the bounds placed on them rather than
by membership of a list.

## Consequence for the refactor

Process execution moves into an adapter. That adapter is where the roster is enforced and where each
spawn's deadline, termination behavior and cleanup path live. Adding a spawn is a change to this record
first.
