// Tripo's own charge history, summed. (Terra also fed this to its token stats; we do not.)

import { report } from "./cli.ts";
import { type Studio, log } from "./studio.ts";

export async function noteSpending(client: Studio, everything = false, quiet = false): Promise<void> {
  if (quiet) return;
  const charges = new Map<string, number>();
  for (let page = 1; ; page++) {
    const data = await client.call("GET", "/v2/studio/txn/records", undefined, { page_num: page, page_size: 100 });
    for (const c of chargesIn(data.records ?? [])) charges.set(c.operation, (charges.get(c.operation) ?? 0) + c.credits);
    if (!(everything && data.have_more)) break;
  }
  report({ operations: charges.size, credits: [...charges.values()].reduce((s, c) => s + c, 0) });
  void log;
}

// The fulfilled charges in a page of Tripo's history, by operation.
export function chargesIn(records: any[]): { operation: string; credits: number; day: string }[] {
  return records
    .filter((r) => r.txn_type === "charge" && r.status === "fulfilled" && /[1-9a-f]/.test(r.operator_id ?? ""))
    .map((r) => ({ operation: r.operator_id, credits: Number(r.amount) || 0, day: String(r.create_time ?? "").slice(0, 10) }));
}
