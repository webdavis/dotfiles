import { afterEach, beforeEach, expect, test } from "bun:test";
import rexSessionStart from "../../private_dot_omp/private_agent/private_extensions/rex-session-start.ts";

const BASE_PROMPT = ["base instructions", "environment"];
const WHERE_THE_AGENT_RUNS = "You're running in a Rex terminal.\nREX_BLOCK=block:1\n";
const MAIN_AGENT = { agent: { kind: "main" } };
const SUBAGENT = { agent: { kind: "sub" } };

type PromptOverride = { systemPrompt: string[] } | undefined;

let rexAnswers: { code: number; stdout: string }[];
let rexCalls: { command: string; args: string[] }[];
let promptIsAboutToRun: (context: object, basePrompt?: string | string[]) => Promise<PromptOverride>;
const environmentBefore = { ...process.env };

beforeEach(() => {
  rexAnswers = [];
  rexCalls = [];
  process.env.REX_SESSION = "session:1";
  delete process.env.REX_BIN_DIR;
  const handlers: Record<string, (event: object, context: object) => Promise<unknown>> = {};
  rexSessionStart({
    on: (name, handler) => {
      handlers[name] = handler;
    },
    exec: async (command, args) => {
      rexCalls.push({ command, args });
      const answer = rexAnswers.shift();
      if (!answer) throw new Error("rex is not installed");
      return answer;
    },
  });
  promptIsAboutToRun = (context, basePrompt = BASE_PROMPT) =>
    handlers.before_agent_start({ systemPrompt: basePrompt }, context) as Promise<PromptOverride>;
});

afterEach(() => {
  process.env = { ...environmentBefore };
});

test("the main agent inside Rex is told where it runs, after the base prompt", async () => {
  rexAnswers = [{ code: 0, stdout: WHERE_THE_AGENT_RUNS }];

  const result = await promptIsAboutToRun(MAIN_AGENT);

  expect(result).toEqual({ systemPrompt: [...BASE_PROMPT, WHERE_THE_AGENT_RUNS] });
  expect(rexCalls).toEqual([
    { command: "rex", args: ["__internal", "session-start", "--agent", "omp", "--feature-session-naming=false"] },
  ]);
});

test("a base prompt another extension replaced with one string is kept whole", async () => {
  rexAnswers = [{ code: 0, stdout: WHERE_THE_AGENT_RUNS }];

  const result = await promptIsAboutToRun(MAIN_AGENT, "one string");

  expect(result).toEqual({ systemPrompt: ["one string", WHERE_THE_AGENT_RUNS] });
});

test("a context that names no agent is left alone", async () => {
  expect(await promptIsAboutToRun({})).toBeUndefined();
  expect(rexCalls).toHaveLength(0);
});

test("rex is found in the folder Rex names when PATH lacks it", async () => {
  process.env.REX_BIN_DIR = "/rex/bin";
  rexAnswers = [{ code: 0, stdout: WHERE_THE_AGENT_RUNS }];

  await promptIsAboutToRun(MAIN_AGENT);

  expect(rexCalls[0].command).toBe("/rex/bin/rex");
});

test("rex is asked once for a whole session", async () => {
  rexAnswers = [{ code: 0, stdout: WHERE_THE_AGENT_RUNS }];

  await promptIsAboutToRun(MAIN_AGENT);
  const second = await promptIsAboutToRun(MAIN_AGENT);

  expect(second).toEqual({ systemPrompt: [...BASE_PROMPT, WHERE_THE_AGENT_RUNS] });
  expect(rexCalls).toHaveLength(1);
});

test("the prompt is left alone outside Rex", async () => {
  delete process.env.REX_SESSION;

  expect(await promptIsAboutToRun(MAIN_AGENT)).toBeUndefined();
  expect(rexCalls).toHaveLength(0);
});

test("a subagent's prompt is left alone", async () => {
  expect(await promptIsAboutToRun(SUBAGENT)).toBeUndefined();
  expect(rexCalls).toHaveLength(0);
});

test("a failing rex leaves the prompt alone and is asked again at the next prompt", async () => {
  rexAnswers = [
    { code: 1, stdout: "" },
    { code: 0, stdout: WHERE_THE_AGENT_RUNS },
  ];

  expect(await promptIsAboutToRun(MAIN_AGENT)).toBeUndefined();
  expect(await promptIsAboutToRun(MAIN_AGENT)).toEqual({ systemPrompt: [...BASE_PROMPT, WHERE_THE_AGENT_RUNS] });
});

test("a missing rex leaves the prompt alone", async () => {
  expect(await promptIsAboutToRun(MAIN_AGENT)).toBeUndefined();
});
