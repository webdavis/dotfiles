// Rex has no omp hook, so this tells an omp session inside Rex which Rex session and pane it runs in.
import { join } from "node:path";
import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";

const REX_SAYS_WHERE_THE_AGENT_RUNS = ["__internal", "session-start", "--agent", "omp", "--feature-session-naming=false"];
const REX_GETS_AT_MOST_MS = 5000;

export default function rexSessionStart(pi: ExtensionAPI) {
  let whereTheAgentRuns: string | undefined;

  const askRexWhereTheAgentRuns = async () => {
    // Rex names its own folder in each pane, so rex is found even when PATH lacks it.
    const rex = process.env.REX_BIN_DIR ? join(process.env.REX_BIN_DIR, "rex") : "rex";
    try {
      const answer = await pi.exec(rex, REX_SAYS_WHERE_THE_AGENT_RUNS, { timeout: REX_GETS_AT_MOST_MS });
      return answer.code === 0 && answer.stdout.trim() ? answer.stdout : undefined;
    } catch {
      return undefined;
    }
  };

  // The system prompt carries the answer, so it is never stored in the session:
  // a resumed or compacted session gets it again, once, for the pane it now runs in.
  pi.on("before_agent_start", async (event, ctx) => {
    if (!process.env.REX_SESSION || ctx.agent?.kind !== "main") return;
    whereTheAgentRuns ??= await askRexWhereTheAgentRuns();
    if (!whereTheAgentRuns) return;
    // Another extension may have replaced the base prompt with one string.
    return { systemPrompt: [event.systemPrompt, whereTheAgentRuns].flat() };
  });
}
