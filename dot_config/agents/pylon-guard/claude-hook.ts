import { BASE_URL_BLOCK_REASON, pylonVerdict, readCommandsBesideThisFile } from "./pylon-verdict.ts";

type HookInput = { tool_name: string; tool_input: Record<string, unknown> };

export function hookResponse({ tool_name, tool_input }: HookInput, readCommands: string[]) {
  const command = tool_input.command;
  if (tool_name !== "Bash" || typeof command !== "string") return;
  const verdict = pylonVerdict(command, readCommands);
  if (verdict === "run") return;
  return {
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: verdict === "block" ? "deny" : "ask",
      permissionDecisionReason: verdict === "block" ? BASE_URL_BLOCK_REASON : "This command could change Pylon.",
    },
  };
}

if (import.meta.main) {
  const response = hookResponse(JSON.parse(await Bun.stdin.text()), readCommandsBesideThisFile());
  if (response) console.log(JSON.stringify(response));
}
