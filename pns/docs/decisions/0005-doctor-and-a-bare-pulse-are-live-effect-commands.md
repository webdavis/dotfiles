# 0005: `doctor` and a bare `pulse` change the real world, so no test or harness may run them

Status: accepted, after an incident.

## What happened

On 2026-09-02 a verification harness ran the built binary across its command-line surface to compare
behavior before and after a change. It included `doctor` and a bare `pulse`. The run posted two real
banners to the operator's desktop and drove the operator's lamps.

## Why those two and not the others

`doctor` is not a read-only report. Its own opening line says what it does:

```
pns doctor: sending one test to every enabled channel.
```

It delivers a real notification through every configured destination and fires a real pulse at the lamps.
A bare `pulse` is the operator's manual lamp check, so it drives the lamps by design.

Every other subcommand either refuses, prints, or acts on state inside a sandboxed HOME.

## The rule

1. No test, differential, or verification step runs `pns doctor` or a bare `pns pulse` against a real
   configuration.
1. No test, differential, or verification step reads `~/.config/pns/config.toml` or touches the
   operator's real state directory. Every run uses a sandbox HOME and scripted transports.
1. Nothing in a test reaches a real Hue bridge, moshi-hook, hermes gateway, or the macOS banner.
1. The argv differential at `~/.claude/pipeline/extraction-verify.sh` excludes both commands for this
   reason. Extend that harness rather than writing a new one; a new one will not carry the exclusion.

`pulse` with an argument that cannot be a valid exit code is safe and is exercised: the differential
covers `pulse notanumber`, `pulse -1` and `pulse 99999999999999999999`, all of which refuse before
reaching a lamp.

## Consequence for the refactor

`doctor` becomes a set of diagnostic checks behind an application port, and the destinations it exercises
are the same trait objects the delivery path uses. That makes a fully offline `doctor` test possible for
the first time, against in-memory destinations. It does not make the real command safe to run in a test,
and this record stays in force.
