// The command line: how a command is described, its arguments read, its help and its output.

import { basename, extname } from "node:path";
import { parseArgs } from "node:util";
import { type Studio, StudioError } from "./studio.ts";

export type Opt = { type: "string" | "boolean"; short?: string; multiple?: boolean; default?: string | boolean; choices?: string[]; help: string };
export type Args = { values: Record<string, any>; positionals: string[] };
// A command: its help, its options, what it does and whether it can spend credits.
export type Command = { usage: string; help: string; options?: Record<string, Opt>; free?: boolean; run: (client: Studio, a: Args) => Promise<void> };

export const out: Record<string, Opt> = { out: { type: "string", short: "o", help: "output file" } };

// A command's arguments. A boolean option is turned off with --no-NAME.
export function parse(command: Command, argv: string[]): Args {
  const options = command.options ?? {};
  const defined = (o: object) => Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined));
  const spec = Object.fromEntries(Object.entries(options).map(([k, o]) => [k, defined({ type: o.type, short: o.short, multiple: o.multiple, default: o.default })]));
  const { values, positionals } = parseArgs({ args: argv, options: spec as any, allowPositionals: true, allowNegative: true, strict: true });
  for (const [k, o] of Object.entries(options)) {
    const v = values[k];
    if (o.choices && typeof v === "string" && !o.choices.includes(v)) throw new Error(`--${k} must be one of ${o.choices.join(", ")}`);
  }
  return { values, positionals };
}

export function overview(commands: Record<string, Command>): string {
  const width = Math.max(...Object.keys(commands).map((n) => n.length));
  const lines = Object.entries(commands).map(([n, c]) => `  ${n.padEnd(width)}  ${c.help}`);
  return `Make, rig and animate 3D models with Tripo, on our Studio subscription's credits.

  node tools/tripo/tripo.ts <command> [options]

${lines.join("\n")}

Every command that makes or changes a model prints one line of JSON with the project it worked
on, so the next can take it. PROJECT is that id or the project's Studio URL. See README.md, and
\`node tools/tripo/tripo.ts <command> --help\` for a command's options.`;
}

export function commandHelp(name: string, c: Command): string {
  const opts = Object.entries(c.options ?? {}).map(([k, o]) => {
    const flag = `${o.short ? `-${o.short}, ` : ""}--${k}${o.type === "string" ? " VALUE" : ""}`;
    const extra = [o.choices ? `one of ${o.choices.join(", ")}` : "", o.default !== undefined && o.type === "string" ? `default ${o.default}` : ""].filter(Boolean);
    return `  ${flag.padEnd(26)} ${o.help}${extra.length ? ` (${extra.join("; ")})` : ""}`;
  });
  return `${c.help}\n\n  node tools/tripo/tripo.ts ${c.usage}${opts.length ? `\n\n${opts.join("\n")}` : ""}`;
}

// A project id from the first argument, an id or any Studio URL with one in it.
export function projectIdOf(ref: string | undefined): string {
  if (ref === undefined) throw new StudioError("give a project: its id, or its Studio URL");
  const found = ref.match(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g);
  if (!found) throw new StudioError(`${JSON.stringify(ref)} is not a project id or a Studio project URL`);
  return found.at(-1)!;
}

export function projectId(a: Args): string {
  return projectIdOf(a.positionals[0]);
}

// The one positional argument a command takes.
export function one(a: Args, what: string): string {
  if (a.positionals.length !== 1) throw new StudioError(`give ${what}`);
  return a.positionals[0];
}

export function int(value: unknown, flag: string): number {
  const n = Number(value);
  if (!Number.isInteger(n) || n <= 0) throw new StudioError(`${flag} must be a whole number`);
  return n;
}

// Where a command writes: -o, else a name from the project.
export function output(a: Args, pid: string, suffix: string, ext = ".glb"): string {
  return a.values.out ?? `${pid.slice(0, 8)}-${suffix}${ext}`;
}

// A command's result: one line of JSON, for the next command or a script to read.
export function report(fields: Record<string, unknown>): void {
  console.log(JSON.stringify(Object.fromEntries(Object.entries(fields).filter(([, v]) => v !== undefined))));
}

export function slug(text: string): string {
  return (text.toLowerCase().match(/[a-z0-9]+/g) ?? []).slice(0, 6).join("-") || "tripo";
}

export function stem(path: string): string {
  return basename(path, extname(path));
}

export function numbered(path: string, n: number): string {
  const ext = extname(path);
  return `${path.slice(0, path.length - ext.length)}-${n}${ext}`;
}
