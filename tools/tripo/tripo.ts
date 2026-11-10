#!/usr/bin/env node
// Make, rig and animate 3D models with Tripo, on our Studio subscription's credits. See README.md.
// Node 22.18 or later, which runs TypeScript directly; no dependencies.
//
// The commands are in commands/, grouped by what they do; the steps they share in steps.ts;
// Studio's requests in studio.ts; the versions the app sends in versions.ts.

import { type Args, type Command, commandHelp, overview, parse } from "./cli.ts";
import { editing } from "./commands/edit.ts";
import { make } from "./commands/make.ts";
import { projects } from "./commands/project.ts";
import { rigging } from "./commands/rig.ts";
import { noteSpending } from "./spending.ts";
import { Studio, StudioError } from "./studio.ts";

// The account first, then in the order a model is made, then the rest of a project's life.
export const COMMANDS: Record<string, Command> = {
  balance: projects.balance,
  list: projects.list,
  show: projects.show,
  ...make,
  ...editing,
  ...rigging,
  ...projects,
};

// Runs one command line. `connect` makes the client, so tests can give a fake one.
export async function main(argv: string[], connect = () => new Studio()): Promise<void> {
  const name = argv[0];
  const command = COMMANDS[name];
  if (!command || name === "help" || argv.includes("--help") || argv.includes("-h")) {
    console.log(command ? commandHelp(name, command) : overview(COMMANDS));
    if (!command && name && !["help", "--help", "-h"].includes(name)) process.exitCode = 2;
    return;
  }
  let a: Args;
  try {
    a = parse(command, argv.slice(1));
  } catch (err) {
    console.error(`tripo ${name}: ${(err as Error).message}\n\n${commandHelp(name, command)}`);
    process.exitCode = 2;
    return;
  }
  let client: Studio | undefined;
  try {
    client = connect();
    await command.run(client, a);
  } catch (err) {
    // Tripo's answers are StudioErrors; anything else is a bug here, so keep its stack.
    console.error(err instanceof StudioError ? `tripo: ${err.message}` : err);
    process.exitCode = 1;
  } finally {
    // Every command that can spend notes the account's charges for the token stats.
    if (client && !command.free) await noteSpending(client, false, true);
  }
}

if (import.meta.main) await main(process.argv.slice(2));
