import { beforeEach, describe, expect, test } from "bun:test";
import pylonApproval from "../../dot_config/agents/pylon-guard/omp-extension.ts";

const READ_COMMANDS = ["pylon issues get", "pylon issues messages list"];

type Verdict = { block: true; reason: string } | undefined;
type Context = { hasUI: boolean; ui: { confirm: (title: string, message: string) => Promise<boolean> } };

let toolIsAboutToRun: (toolName: string, input: object, context: Context) => Promise<Verdict>;
let questions: string[];

const youAnswer = (approves: boolean): Context => ({
  hasUI: true,
  ui: {
    confirm: async (_title, message) => {
      questions.push(message);
      return approves;
    },
  },
});
const NOBODY_TO_ASK: Context = { hasUI: false, ui: { confirm: async () => true } };

const run = (command: string, context = youAnswer(false)) => toolIsAboutToRun("bash", { command }, context);

beforeEach(() => {
  questions = [];
  const handlers: Record<string, (event: object, context: Context) => Promise<Verdict>> = {};
  pylonApproval(
    {
      on: (name: string, handler: (event: object, context: Context) => Promise<Verdict>) => {
        handlers[name] = handler;
      },
    },
    READ_COMMANDS,
  );
  toolIsAboutToRun = (toolName, input, context) => handlers.tool_call({ toolName, input }, context);
});

describe("a read-only Pylon command runs without asking", () => {
  test.each([
    "pylon issues get --id 1",
    "pylon issues messages list --id 1",
    "/opt/homebrew/bin/pylon issues get --id 1",
    "pylon issues get --id 1 | jq .data",
    "x=$(pylon issues get --id 1) && echo \"$x\"",
  ])("%s", async (command) => {
    expect(await run(command)).toBeUndefined();
    expect(questions).toEqual([]);
  });
});

describe("a command that only mentions Pylon runs without asking", () => {
  test.each([
    "rg pylon docs",
    "grep -rn 'pylon issues reply' .",
    "cat pylon-notes.md | head -5",
    "fd pylon ~/.config",
    "echo pylon",
    "ls ~/.config/agents/broccoli-tech-support/.agents/skills | rg pylon",
  ])("%s", async (command) => {
    expect(await run(command)).toBeUndefined();
    expect(questions).toEqual([]);
  });
});

describe("a command that can change Pylon asks first", () => {
  test.each([
    "pylon issues reply --id 1",
    "PYLON issues reply --id 1",
    "/opt/homebrew/bin/pylon issues reply --id 1",
    "'py''lon' issues reply --id 1",
    "py\\lon issues reply --id 1",
    "env X=1 pylon issues reply --id 1",
    "command pylon issues reply --id 1",
    "bash -c 'pylon issues reply --id 1'",
    "echo hi && pylon issues reply --id 1",
    "rg x \"$(pylon issues reply --id 1)\"",
    "echo `pylon issues reply --id 1`",
    "cat <(pylon issues reply --id 1)",
    "cat <<EOF\n$(pylon issues reply --id 1)\nEOF",
    "cat <<'EOF' | sh\npylon issues reply --id 1\nEOF",
    "echo pylon issues reply --id 1 > reply.sh; sh reply.sh",
    "rg --pre 'pylon issues reply' x",
    "fd -x pylon issues reply --id",
    "xargs pylon issues reply < ids",
    "pylon issues \"$verb\" --id 1",
    "pylon issues get --id 1 | sh -c 'pylon issues reply --id 1'",
    "CMD=pylon env sh -c '$CMD issues reply --id 1'",
  ])("%s", async (command) => {
    await run(command);
    expect(questions).toEqual([command]);
  });
});

test("a program named at run time asks first", async () => {
  await run("p=pyl; ${p}on issues reply --id 1");
  expect(questions).toHaveLength(1);
});

describe("a Pylon read run with its own environment asks first, since the environment can redirect it", () => {
  test.each([
    "PYLON_BASE_URL=http://localhost:1 pylon issues get --id 1",
    "HTTPS_PROXY=http://localhost:1 pylon issues get --id 1",
  ])("%s", async (command) => {
    await run(command);
    expect(questions).toEqual([command]);
  });
});

describe("a Pylon call that sends the token elsewhere is blocked without asking", () => {
  test.each([
    "pylon issues get --id 1 --base-url http://localhost:1",
    "/opt/homebrew/bin/pylon issues get --base-url=http://localhost:1 --id 1",
    "PYLON --base-url http://localhost:1 me get",
  ])("%s", async (command) => {
    expect(await run(command, youAnswer(true))).toEqual({ block: true, reason: expect.stringContaining("--base-url") });
    expect(questions).toEqual([]);
  });
});

test("an approved command runs", async () => {
  expect(await run("pylon issues reply --id 1", youAnswer(true))).toBeUndefined();
});

test("a declined command is blocked", async () => {
  expect(await run("pylon issues reply --id 1", youAnswer(false))).toEqual({
    block: true,
    reason: expect.stringContaining("not approved"),
  });
});

test("a command that needs approval is blocked when nobody can answer", async () => {
  expect(await run("pylon issues reply --id 1", NOBODY_TO_ASK)).toEqual({
    block: true,
    reason: expect.stringContaining("not approved"),
  });
});

test("tools other than bash are left alone", async () => {
  expect(await toolIsAboutToRun("eval", { code: "pylon issues reply" }, youAnswer(false))).toBeUndefined();
  expect(questions).toEqual([]);
});
