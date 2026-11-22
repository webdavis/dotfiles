# Rex: stray `5;1R` typed into my prompt while the phone is attached

- Sent to Moshi support by email: <https://mail.google.com/mail/u/0/#sent/FMfcgzQjgzsLCRdtvSrBtFxXQWncCwxH>
- Date sent: 2026-11-21
- `moshi-hook doctor`: all green

## Versions

- moshi-hook: 0.4.62
- Rex: 0.1.0-snapshot.2506
- macOS: 27.2.1
- mosh: 1.4.0
- bash: 5.3

## Symptom

When I have a Rex session open in the Moshi app and run any moshi-hook command in that pane on my Mac (even
`moshi-hook help`), I get junk like `5;1R` left at my prompt afterwards. It doesn't happen in Ghostty, or in a Rex pane
the phone isn't showing.

## Cause

I dug into it. moshi-hook asks the terminal for the cursor position (ESC\[6n) at startup. Rex answers, but
`moshi-hook rex attach` also forwards the question to the phone's terminal and then types the phone's answer into the
pane as input. By then the command has exited, so the second answer ends up at the shell prompt.

## Evidence

I confirmed it by running my own `moshi-hook rex attach --block <id>` in a pty that answers every ESC\[6n with
ESC\[9;9R. A program in that block then gets Rex's reply plus ESC\[9;9R a millisecond later. Without the attach, it gets
only Rex's reply.

## Related fix

Looks related to the 0.4.52 fix about programs in a Rex block receiving the phone's terminal answers.

## Steps to reproduce

1. Focus a shell pane in Rex on the Mac.
2. Open that session in the Moshi app over Mosh.
3. Run `moshi-hook help` in the pane.
