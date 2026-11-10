// Tripo Studio's private backend, so our tools spend the subscription's credits, not the API's.
//
// Not a public API: these are the requests studio.tripo3d.ai makes, captured from the web app
// and read from its JavaScript. Tripo changes them when it ships the app; MAINTAINING.md says
// how to see what the app sends now.
//
// The login is TRIPO_STUDIO_COOKIE: the ory_kratos_session cookie of a signed-in browser, or
// the whole `cookie` header of any request the web app makes to api.tripo3d.ai. It lasts about
// 30 days; each run trades it for a bearer token that lasts 15 minutes.

import { createHash, createHmac } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const API = "https://api.tripo3d.ai";
const BROWSER = {
  Accept: "application/json",
  Origin: "https://studio.tripo3d.ai",
  Referer: "https://studio.tripo3d.ai/",
  "User-Agent":
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36",
};
// What progress reports for an operation that will never finish.
const FAILED = ["failed", "error", "banned", "cancelled", "expired", "partial"];
// Charges that were, or will be, taken; the rest were given back.
const CHARGED = ["fulfilled", "pending"];
// Requests that only read or check, so a hiccup (a 502, a dropped connection) can be tried
// again. Anything that starts an operation is sent once: sending it again could pay twice.
const READS = [
  "/v2/studio/progress",
  "/v2/studio/image/get_image_asset",
  "/v2/studio/motion/get_task",
  "/v2/studio/storage/temporary_token",
  "/v2/studio/audit/image",
  "/v2/studio/operation/symmetry_check",
  "/v2/studio/operation/pre_rig_check",
  "/v2/studio/operation/uv_edit/context",
];

export class StudioError extends Error {}

// The variables of a .env file.
export function parseEnv(text: string): Record<string, string> {
  const vars: Record<string, string> = {};
  for (const raw of text.split(/\r?\n/)) {
    const m = raw.match(/^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$/);
    if (!m) continue;
    const quoted = m[2].match(/^(["'])(.*)\1$/);
    vars[m[1]] = quoted ? quoted[2] : m[2].replace(/\s+#.*$/, "");
  }
  return vars;
}

// A file in Tripo's bucket, and an image there as requests that take one want it.
export type Place = { bucket: string; key: string };
export type ImageRef = Place & { image_source: "upload" | "generate"; image_audit_result: string };
type Reply = { code?: number; message?: string; data?: any; tokenized?: string };

// A project: its latest model and the operation that made it (or the one asked for).
export type Project = {
  id: string;
  project_name?: string;
  model_url: string;
  operator: Operator;
  running_operator?: { operator_id: string; type?: string } | null;
};
export type Operator = {
  operator_id: string;
  model_version?: string;
  rigging?: { status: string; type: string };
  retarget?: Retarget[];
  // What has been done to the model: is_textured, is_hd_textured, is_rigged, is_nexus_mesh…
  [flag: `is_${string}`]: boolean | undefined;
};
export type Retarget = { name: string; status: string; operator_id: string; motion_asset_id?: string };

export function log(message: string): void {
  console.error(message);
}

export function sleep(ms: number): Promise<void> {
  return new Promise((done) => setTimeout(done, ms));
}

// A setting from the environment, else from .env in this checkout or, in a worktree, the main one.
export function env(name: string): string {
  if (process.env[name]) return process.env[name];
  const roots = [ROOT];
  const marker = `${sep}.claude${sep}worktrees${sep}`;
  if (ROOT.includes(marker)) roots.push(ROOT.split(marker)[0]);
  for (const root of roots) {
    const file = join(root, ".env");
    const value = existsSync(file) ? parseEnv(readFileSync(file, "utf8"))[name] : undefined;
    if (value) return value;
  }
  throw new StudioError(`${name} is not set: put it in the environment or in .env (see tools/tripo/README.md)`);
}

export class Studio {
  cookie: string;
  device: string;
  csrf: string;
  jwt = "";
  expires = 0;
  // What this run has started that may be charged for, not yet counted by charged().
  unbilled = new Set<string>();
  since = Date.now();

  constructor(cookie = env("TRIPO_STUDIO_COOKIE")) {
    cookie = cookie.trim();
    // The bare session value, which may end in base64 padding, or a whole cookie header.
    if (!cookie.includes(";") && !cookie.startsWith("ory_kratos_session=")) cookie = `ory_kratos_session=${cookie}`;
    const jar = new Map(
      cookie
        .split(";")
        .filter((p) => p.includes("="))
        .map((p) => {
          const at = p.indexOf("=");
          return [p.slice(0, at).trim(), p.slice(at + 1)] as [string, string];
        }),
    );
    if (!jar.has("ory_kratos_session")) {
      throw new StudioError("TRIPO_STUDIO_COOKIE has no ory_kratos_session: copy it from the browser again");
    }
    this.cookie = cookie;
    this.device = jar.get("tripo_device_id") ?? "";
    this.csrf = [...jar].find(([k]) => k.startsWith("csrf_token_"))?.[1] ?? "";
  }

  // One request; returns the HTTP status and the decoded reply. Reads are tried up to five times.
  async send(method: string, path: string, body?: unknown, query?: Record<string, unknown>, auth = true): Promise<[number, Reply]> {
    const tries = method === "GET" || READS.includes(path) ? 5 : 1;
    for (let attempt = 0; ; attempt++) {
      const [status, reply] = await this.sendOnce(method, path, body, query, auth);
      if (status < 500 || attempt === tries - 1) return [status, reply];
      await sleep(2000 * (attempt + 1));
    }
  }

  async sendOnce(method: string, path: string, body: unknown, query: Record<string, unknown> | undefined, auth: boolean): Promise<[number, Reply]> {
    const url = new URL(API + path);
    for (const [k, v] of Object.entries(query ?? {})) url.searchParams.set(k, String(v));
    const headers: Record<string, string> = { ...BROWSER, Cookie: this.cookie };
    if (auth) headers.Authorization = `Bearer ${await this.token()}`;
    if (this.device) headers["x-tripo-device-id"] = this.device;
    if (this.csrf) headers.csrf_token = this.csrf;
    if (body !== undefined) headers["Content-Type"] = "application/json";
    try {
      const res = await fetch(url, {
        method,
        headers,
        body: body === undefined ? undefined : JSON.stringify(body),
        signal: AbortSignal.timeout(120_000),
      });
      const text = await res.text();
      try {
        return [res.status, text ? JSON.parse(text) : {}];
      } catch {
        return [res.status, {}];
      }
    } catch (err) {
      return [599, { message: `no answer from Tripo: ${(err as Error).message}` }];
    }
  }

  // One request that must succeed; returns its data.
  async call(method: string, path: string, body?: unknown, query?: Record<string, unknown>): Promise<any> {
    const [status, reply] = await this.send(method, path, body, query);
    if (reply.code !== 0) throw new StudioError(`${method} ${path} failed (${status}): ${reply.message || JSON.stringify(reply)}`);
    return reply.data ?? {};
  }

  // A bearer token from the session cookie, renewed shortly before it runs out.
  async token(): Promise<string> {
    if (Date.now() / 1000 < this.expires - 120) return this.jwt;
    const [status, reply] = await this.send("GET", "/v2/studio/studio/whoami", undefined, { tokenizeAs: "default_jwt" }, false);
    if (status !== 200 || !reply.tokenized) {
      throw new StudioError(`Studio login failed (${status}): the session cookie has probably expired; copy it again`);
    }
    this.jwt = reply.tokenized;
    this.expires = JSON.parse(Buffer.from(this.jwt.split(".")[1], "base64url").toString()).exp;
    return this.jwt;
  }

  async credits(): Promise<number> {
    return (await this.call("GET", "/v2/studio/user/profile/payment")).wallet?.total_credit;
  }

  // Files in and out.

  // Puts a file in Tripo's bucket with the temporary keys the web app gets; returns its place.
  async put(path: string, format: string): Promise<Place> {
    if (!existsSync(path)) throw new StudioError(`${path}: no such file`);
    const sts = await this.call("POST", "/v2/studio/storage/temporary_token", { client: "aws", format });
    const bucket: string = sts.resource_bucket;
    const key: string = sts.resource_uri;
    const region = (sts.host ?? "").split("s3.").at(-1).split(".amazonaws")[0] || "us-west-2";
    const url = presignPut(bucket, key, region, sts.sts_ak, sts.sts_sk, sts.session_token);
    const res = await fetch(url, {
      method: "PUT",
      headers: { "Content-Type": "application/octet-stream" },
      body: readFileSync(path),
      signal: AbortSignal.timeout(300_000),
    });
    if (!res.ok) throw new StudioError(`uploading ${path} failed (${res.status}): ${(await res.text()).slice(0, 300)}`);
    return { bucket, key };
  }

  // An image uploaded and passed by Tripo's content check, as every request that takes one wants it.
  async uploadImage(path: string): Promise<ImageRef> {
    const ext = path.split(".").at(-1)!.toLowerCase();
    const format = { jpg: "jpeg", jpeg: "jpeg", png: "png", webp: "webp" }[ext];
    if (!format) throw new StudioError(`${path}: Studio takes PNG, JPEG or WebP images`);
    const place = await this.put(path, format);
    const result: string = (await this.call("POST", "/v2/studio/audit/image", { image: place })).result ?? "pass";
    if (result === "reject" || result === "nsfw") throw new StudioError(`${path}: Tripo's content check refused the image (${result})`);
    return { ...place, image_source: "upload", image_audit_result: result };
  }

  async download(url: string, path: string): Promise<string> {
    const res = await fetch(url, { signal: AbortSignal.timeout(600_000) });
    if (!res.ok) throw new StudioError(`downloading ${path} failed (${res.status})`);
    mkdirSync(dirname(resolve(path)), { recursive: true });
    writeFileSync(path, Buffer.from(await res.arrayBuffer()));
    return path;
  }

  // Waiting.

  // Until an operation succeeds. Progress can sit at 99% for a minute or two while it bakes.
  async wait(operator: string, label = operator): Promise<any> {
    let last = "";
    for (;;) {
      const item = ((await this.call("POST", "/v2/studio/progress", { ids: [operator] })) ?? [])[0] ?? {};
      if (item.status === "success") return item;
      if (FAILED.includes(item.status)) throw new StudioError(`${label} ${operator} ${item.status}: ${item.reason ?? JSON.stringify(item)}`);
      const line = `${item.status} ${item.progress ?? 0}%`;
      if (line !== last) log(`  ${label}: ${line}`);
      last = line;
      await sleep(4000);
    }
  }

  // Until an image asset (generated images, views) is ready.
  async waitImage(assetId: string, label = assetId): Promise<any> {
    let last = "";
    for (;;) {
      const asset = await this.call("POST", "/v2/studio/image/get_image_asset", { asset_id: assetId });
      if (asset.status === "success") return asset;
      if (FAILED.includes(asset.status)) throw new StudioError(`${label} ${assetId} ${asset.status}`);
      if (asset.status !== last) log(`  ${label}: ${asset.status}`);
      last = asset.status;
      await sleep(4000);
    }
  }

  // Projects: a model and the operations done to it.

  // Starts work: an operation by name (POST /v2/studio/operation/<name>), or another endpoint
  // by its path. Returns the receipt, and keeps its ids to find what it was charged.
  async start(name: string, body: unknown): Promise<any> {
    const receipt = await this.call("POST", name.startsWith("/") ? name : `/v2/studio/operation/${name}`, body);
    this.bill(receipt.operator_id, receipt.task_id, receipt.asset_id, ...(receipt.variations ?? []).map((v: any) => v.operator_id));
    return receipt;
  }

  // Notes ids Tripo may charge under: an operation's, or a motion's once it is made.
  bill(...ids: (string | undefined)[]): void {
    for (const id of ids) if (id) this.unbilled.add(id);
  }

  // What Tripo charged for the work started since the last call, read from its own history,
  // so it is exact even while other runs spend from the same account.
  async charged(): Promise<number> {
    let total = 0;
    for (let page = 1; this.unbilled.size && page <= 5; page++) {
      const data = await this.call("GET", "/v2/studio/txn/records", undefined, { page_num: page, page_size: 100 });
      const records: any[] = data.records ?? [];
      for (const r of records) {
        if (r.txn_type === "charge" && CHARGED.includes(r.status) && this.unbilled.has(r.operator_id)) total += Number(r.amount) || 0;
      }
      // Newest first: stop at charges from before this run began.
      const oldest = Date.parse(records.at(-1)?.create_time ?? "");
      if (!data.have_more || oldest < this.since - 600_000) break;
    }
    this.unbilled.clear();
    return total;
  }

  // A project's detail, at one operation's result when given, else at its latest.
  async project(projectId: string, operator?: string): Promise<Project> {
    return this.call("GET", `/v2/studio/project/detail/v3/${projectId}`, undefined, operator ? { operator_id: operator } : undefined);
  }
}

// RFC 3986 encoding, as S3 signs it.
function encode(s: string): string {
  return encodeURIComponent(s).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
}

// A presigned S3 PUT URL (SigV4), from the temporary credentials the upload token gives.
export function presignPut(bucket: string, key: string, region: string, ak: string, sk: string, sessionToken?: string, now = new Date(), expires = 900): string {
  const stamp = now.toISOString().replace(/[-:]/g, "").replace(/\.\d+/, "");
  const day = stamp.slice(0, 8);
  const scope = `${day}/${region}/s3/aws4_request`;
  const host = `${bucket}.s3.${region}.amazonaws.com`;
  const uri = "/" + key.replace(/^\/+/, "").split("/").map(encode).join("/");
  const query: Record<string, string> = {
    "X-Amz-Algorithm": "AWS4-HMAC-SHA256",
    "X-Amz-Credential": `${ak}/${scope}`,
    "X-Amz-Date": stamp,
    "X-Amz-Expires": String(expires),
    "X-Amz-SignedHeaders": "host",
  };
  if (sessionToken) query["X-Amz-Security-Token"] = sessionToken;
  const qs = Object.keys(query)
    .sort()
    .map((k) => `${k}=${encode(query[k])}`)
    .join("&");
  const canonical = ["PUT", uri, qs, `host:${host}\n`, "host", "UNSIGNED-PAYLOAD"].join("\n");
  const toSign = ["AWS4-HMAC-SHA256", stamp, scope, createHash("sha256").update(canonical).digest("hex")].join("\n");
  let k: Buffer = Buffer.from("AWS4" + sk);
  for (const part of [day, region, "s3", "aws4_request"]) k = createHmac("sha256", k).update(part).digest();
  const signature = createHmac("sha256", k).update(toSign).digest("hex");
  return `https://${host}${uri}?${qs}&X-Amz-Signature=${signature}`;
}
