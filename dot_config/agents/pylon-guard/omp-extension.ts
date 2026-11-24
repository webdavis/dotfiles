import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";
import { BASE_URL_BLOCK_REASON, pylonVerdict, readCommandsBesideThisFile } from "./pylon-verdict.ts";

export default function pylonApproval(pi: ExtensionAPI, readCommands: string[] = readCommandsBesideThisFile()) {
  pi.on("tool_call", async (event, ctx) => {
    const command = event.input.command;
    if (event.toolName !== "bash" || typeof command !== "string") return;
    const verdict = pylonVerdict(command, readCommands);
    if (verdict === "run") return;
    if (verdict === "block") return { block: true, reason: BASE_URL_BLOCK_REASON };
    if (ctx.hasUI && (await ctx.ui.confirm("Run this Pylon command?", command))) return;
    return { block: true, reason: "The Pylon command was not approved." };
  });
}
