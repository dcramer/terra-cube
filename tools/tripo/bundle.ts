#!/usr/bin/env node
// Reads the Studio web app's own JavaScript and compares it with this CLI: which endpoints and
// model versions the app has today, and which of ours it no longer has. See MAINTAINING.md.
//
//   node tools/tripo/bundle.ts ENTRY                  the drift report; exits 1 if we use what the app dropped
//   node tools/tripo/bundle.ts ENTRY --grep REGEX     the code around each match, to read a request's body
//
// ENTRY is the app's entry script, `<script type="module" src=".../_nuxt/XXXX.js">` in the page
// at https://studio.tripo3d.ai (its name or URL). The page sits behind Cloudflare's challenge, so
// it is read in a browser once; the scripts themselves come from an open CDN.

import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseArgs } from "node:util";
import * as versions from "./versions.ts";

const CDN = "https://tripo-webapp-assets.tripo3d.ai/studio-prod/_nuxt";
const HERE = import.meta.dirname;
// What a model version looks like: v3.1-20260211, Nexus-v2.0-20260801.
const VERSION = /^(?:[A-Za-z]+-)?v\d+\.\d+-\d{8}$/;

// Every chunk the app can load, found by following the chunk names each one mentions.
async function crawl(entry: string, dir: string): Promise<string[]> {
  mkdirSync(dir, { recursive: true });
  const queue = [entry.split("/").pop()!];
  const seen = new Set(queue);
  while (queue.length) {
    const name = queue.shift()!;
    const path = join(dir, name);
    if (!existsSync(path)) {
      const res = await fetch(`${CDN}/${name}`);
      if (!res.ok) {
        if (name === entry.split("/").pop()) throw new Error(`${name}: ${res.status} from the CDN; is it the entry script of today's build?`);
        continue; // a name that looked like a chunk but is not one
      }
      writeFileSync(path, await res.text());
    }
    for (const m of readFileSync(path, "utf8").matchAll(/[A-Za-z0-9_.-]{6,40}\.js\b/g)) {
      if (!seen.has(m[0])) {
        seen.add(m[0]);
        queue.push(m[0]);
      }
    }
  }
  return readdirSync(dir).filter((f) => f.endsWith(".js"));
}

// The minified enums whose values are model versions: `function(e){return e.v3_0=`v3.0-…`,e}({})`.
export function versionEnums(js: string): string[][] {
  const found: string[][] = [];
  for (const m of js.matchAll(/function\(e\)\{return ((?:e\.[A-Za-z0-9_$]+=`[^`]*`,)+)e\}\(\{\}\)/g)) {
    const values = [...m[1].matchAll(/=`([^`]*)`/g)].map((v) => v[1]);
    if (values.some((v) => VERSION.test(v))) found.push(values);
  }
  return found;
}

// What this CLI sends from versions.ts: every model version, image model, skeleton and format.
export function ourNames(): Set<string> {
  return new Set(Object.values(versions).flatMap((v) => (typeof v === "string" ? [v] : Array.isArray(v) ? v : Object.values(v))));
}

// The endpoints this CLI calls, read from its source: paths written out, and operations named
// where they are started.
export function ourEndpoints(source: string): Set<string> {
  const endpoints = new Set<string>();
  for (const m of source.matchAll(/\/v2\/studio\/[A-Za-z0-9_/]+/g)) endpoints.add(m[0].replace(/\/$/, ""));
  for (const m of source.matchAll(/(?:start\(|apply\(client, a, pid, |endpoint = )"([a-z_/]+)"/g)) endpoints.add(`/v2/studio/operation/${m[1]}`);
  endpoints.delete("/v2/studio/operation");
  return endpoints;
}

// The CLI's own source, without its tests and this file.
function source(dir = HERE): string {
  return readdirSync(dir, { withFileTypes: true })
    .flatMap((e) => (e.isDirectory() ? [source(join(dir, e.name))] : e.name.endsWith(".ts") && !e.name.endsWith(".test.ts") && e.name !== "bundle.ts" ? [readFileSync(join(dir, e.name), "utf8")] : []))
    .join("\n");
}

async function main(): Promise<void> {
  const { values, positionals } = parseArgs({
    options: { grep: { type: "string" }, around: { type: "string", default: "400" }, dir: { type: "string" } },
    allowPositionals: true,
  });
  const entry = positionals[0];
  if (!entry) {
    console.error("give the app's entry script: node tools/tripo/bundle.ts XXXX.js (see MAINTAINING.md)");
    process.exit(2);
  }
  const build = await (await fetch(`${CDN}/builds/latest.json`)).json();
  const dir = values.dir ?? join(tmpdir(), `tripo-bundle-${build.id}`);
  const files = await crawl(entry, dir);
  const js = files.map((f) => ({ f, s: readFileSync(join(dir, f), "utf8") }));

  if (values.grep) {
    const around = Number(values.around);
    for (const { f, s } of js) {
      for (const m of s.matchAll(new RegExp(values.grep, "g"))) {
        console.log(`${f}: ${s.slice(Math.max(0, m.index - around), m.index + m[0].length + around)}\n`);
      }
    }
    return;
  }

  const appEndpoints = new Set(js.flatMap(({ s }) => [...s.matchAll(/\/v2\/studio\/[A-Za-z0-9_/]+/g)].map((m) => m[0].replace(/\/$/, ""))));
  const enums = js.flatMap(({ s }) => versionEnums(s));
  const appVersions = new Set(enums.flat().filter((v) => VERSION.test(v)));
  const cli = { endpoints: ourEndpoints(source()), names: ourNames() };
  // A name the app still sends is written in its code as a string.
  const quoted = new Set(js.flatMap(({ s }) => [...s.matchAll(/[`"]([^`"\n]{1,60})[`"]/g)].map((m) => m[1])));
  // Endpoints built from a prefix in the app (`uv_edit/${action}`) count as present.
  const inApp = (e: string) => appEndpoints.has(e) || [...appEndpoints].some((a) => e.startsWith(`${a}/`));
  const stale = [...cli.endpoints].filter((e) => !inApp(e));
  const staleNames = [...cli.names].filter((v) => !(appVersions.has(v) || quoted.has(v)));

  console.log(`build ${build.id}, ${new Date(build.timestamp).toISOString()}: ${files.length} scripts in ${dir}\n`);
  console.log("Model versions the app knows, by enum:");
  for (const e of enums) console.log(`  ${e.join("  ")}`);
  console.log(`\nVersions and names we send that the app no longer has: ${staleNames.join(", ") || "none"}`);
  console.log(`Endpoints we call that the app no longer has: ${stale.join(", ") || "none"}`);
  const unused = [...appEndpoints].filter((e) => ![...cli.endpoints].some((c) => c === e || c.startsWith(`${e}/`))).sort();
  console.log(`\nEndpoints the app has that we do not call (${unused.length}):`);
  for (const e of unused) console.log(`  ${e}`);
  if (stale.length || staleNames.length) process.exitCode = 1;
}

if (import.meta.main) await main();
