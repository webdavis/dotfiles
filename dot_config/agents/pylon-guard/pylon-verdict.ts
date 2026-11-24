import { readFileSync } from "node:fs";
import { join } from "node:path";
import { simpleCommandsIn } from "./shell-commands.ts";

export type PylonVerdict = "run" | "ask" | "block";

export const BASE_URL_BLOCK_REASON = "A pylon call with --base-url would send the Pylon token to another server.";

const TEXT_TOOL_RUNS_A_PROGRAM_WHEN: Record<string, (args: string[]) => boolean> = {
  cat: () => false,
  echo: () => false,
  fd: (args) => args.some((arg) => /^--exec(-batch)?(=|$)/.test(arg) || /^-[^-]*[xX]/.test(arg)),
  grep: () => false,
  head: () => false,
  jq: () => false,
  ls: () => false,
  printf: () => false,
  rg: (args) => args.some((arg) => /^--(pre|hostname-bin)(=|$)/.test(arg)),
  tail: () => false,
  wc: () => false,
  which: () => false,
};

export function readCommandsBesideThisFile(): string[] {
  try {
    const parsed: unknown = JSON.parse(readFileSync(join(import.meta.dir, "pylon-read-commands.json"), "utf8"));
    return Array.isArray(parsed) ? parsed.filter((command): command is string => typeof command === "string") : [];
  } catch {
    return [];
  }
}

export function pylonVerdict(script: string, readCommands: string[]): PylonVerdict {
  const readSubcommands = readCommands.map((command) => command.split(" ").slice(1));
  let verdict: "run" | "ask" = "run";
  let mentionsPylon = false;
  let runsAnotherProgram = false;
  for (const { assignments, program, args, hereDocuments } of simpleCommandsIn(script)) {
    const programIsNamedAtRunTime = program !== undefined && !program.isLiteral;
    if (programIsNamedAtRunTime) {
      verdict = "ask";
      continue;
    }
    const name = program?.text.slice(program.text.lastIndexOf("/") + 1).toLowerCase();
    if (name === "pylon") {
      const sendsTheTokenElsewhere = args.some((arg) => arg.text.startsWith("--base-url"));
      if (sendsTheTokenElsewhere) return "block";
      const isRead = readSubcommands.some((subcommand) =>
        subcommand.every((part, index) => args[index]?.isLiteral && args[index].text === part),
      );
      const setsItsOwnEnvironment = assignments.length > 0;
      if (!isRead || setsItsOwnEnvironment) verdict = "ask";
      continue;
    }
    const words = [...assignments, ...(program ? [program] : []), ...args].map((word) => word.text);
    mentionsPylon ||= [...words, ...hereDocuments].some((text) => /pylon/i.test(text));
    const textToolRunsAProgram = name === undefined ? () => false : TEXT_TOOL_RUNS_A_PROGRAM_WHEN[name];
    runsAnotherProgram ||= textToolRunsAProgram === undefined || textToolRunsAProgram(args.map((arg) => arg.text));
  }
  const anotherProgramCouldRunPylon = mentionsPylon && runsAnotherProgram;
  return anotherProgramCouldRunPylon ? "ask" : verdict;
}
