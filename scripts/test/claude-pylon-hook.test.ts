import { expect, test } from "bun:test";
import { hookResponse } from "../../dot_config/agents/pylon-guard/claude-hook.ts";

const READ_COMMANDS = ["pylon issues get"];
const bash = (command: string) => ({ tool_name: "Bash", tool_input: { command } });

test("a read-only Pylon command is left to the session's own permissions", () => {
  expect(hookResponse(bash("pylon issues get --id 1"), READ_COMMANDS)).toBeUndefined();
});

test("a command that only mentions Pylon is left to the session's own permissions", () => {
  expect(hookResponse(bash("rg pylon docs"), READ_COMMANDS)).toBeUndefined();
});

test("a command that can change Pylon asks, even in bypass mode", () => {
  expect(hookResponse(bash("env X=1 pylon issues reply --id 1"), READ_COMMANDS)).toEqual({
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "ask",
      permissionDecisionReason: expect.any(String),
    },
  });
});

test("a Pylon call that sends the token elsewhere is denied", () => {
  expect(hookResponse(bash("/opt/homebrew/bin/pylon issues get --base-url http://localhost:1"), READ_COMMANDS)).toEqual({
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: expect.stringContaining("--base-url"),
    },
  });
});

test("tools other than Bash are left alone", () => {
  expect(hookResponse({ tool_name: "Read", tool_input: { file_path: "pylon issues reply" } }, READ_COMMANDS)).toBeUndefined();
});
