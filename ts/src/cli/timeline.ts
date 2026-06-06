/**
 * timeline.ts — print a match trace as a tick timeline in the spec's worked-example table style [edge].
 *
 * Renders the integers-only trace (decision STATE snapshots + COMMIT/CONTACT/CANCEL/KO events) into a
 * `T | <fighter0> | <fighter1> | Engine` table, with a closing HP/resource summary. cli/ may use
 * `toNumber` for display.
 */
import { type MatchResult, type TraceEvent, type EntityIndex } from "../core/engine";
import { assertNever } from "../core/assert-never";

export interface TimelineOptions {
  readonly title: string;
  /** moveId → display name. */
  readonly names: Readonly<Record<string, string>>;
  /** display names for entity 0 and entity 1. */
  readonly fighters: readonly [string, string];
}

interface Row {
  readonly t: string;
  readonly c0: string;
  readonly c1: string;
  readonly eng: string;
}

function buildRows(result: MatchResult, opts: TimelineOptions): readonly Row[] {
  const moveName = (id: string): string => opts.names[id] ?? id;
  const stateAt = new Map<number, Extract<TraceEvent, { kind: "STATE" }>>();
  for (const e of result.trace) if (e.kind === "STATE") stateAt.set(e.t, e);

  const ticks = [...new Set(result.trace.filter((e) => e.kind !== "STATE").map((e) => e.t))].sort(
    (a, b) => a - b,
  );

  const rows: Row[] = [];
  for (const t of ticks) {
    const st = stateAt.get(t);
    let c0 = "";
    let c1 = "";
    let eng = st ? `[${st.regime}]` : "";
    let meaningful = false; // skip pure idle/wait ticks to keep the timeline readable
    const put = (i: EntityIndex, text: string): void => {
      if (i === 0) c0 = text;
      else c1 = text;
    };
    for (const ev of result.trace.filter((e) => e.t === t)) {
      switch (ev.kind) {
        case "STATE":
          break;
        case "COMMIT": {
          const ap = st ? st.entities[ev.entity].ap : null;
          put(ev.entity, `▶ ${moveName(ev.moveId)}${ap !== null ? ` (ap ${ap})` : ""}`);
          meaningful = true;
          break;
        }
        case "WAIT":
          put(ev.entity, "· wait");
          break;
        case "DENIED":
          put(ev.entity, `✗ ${moveName(ev.moveId)} (can't afford)`);
          meaningful = true;
          break;
        case "CANCEL":
          put(ev.entity, `↳ cancel → ${moveName(ev.into)}`);
          meaningful = true;
          break;
        case "CONTACT":
          eng += ` ${opts.fighters[ev.attacker]} ${ev.result}${ev.counter ? " (CH)" : ""} → ${opts.fighters[ev.defender]}`;
          meaningful = true;
          break;
        case "KO":
          eng += ` ✖ KO ${opts.fighters[ev.loser]}`;
          meaningful = true;
          break;
        default:
          assertNever(ev);
      }
    }
    if (meaningful) rows.push({ t: String(t), c0, c1, eng: eng.trim() });
  }
  return rows;
}

function pad(s: string, w: number): string {
  return s + " ".repeat(Math.max(0, w - s.length));
}

export function printTimeline(result: MatchResult, opts: TimelineOptions): void {
  const rows = buildRows(result, opts);
  const [f0, f1] = opts.fighters;
  const wT = Math.max(1, ...rows.map((r) => r.t.length));
  const w0 = Math.max(f0.length, ...rows.map((r) => r.c0.length));
  const w1 = Math.max(f1.length, ...rows.map((r) => r.c1.length));

  const log = (s: string): void => console.log(s);
  log("");
  log(`  ── ${opts.title} ──`);
  log(`  ${pad("T", wT)} │ ${pad(f0, w0)} │ ${pad(f1, w1)} │ Engine`);
  log(`  ${"─".repeat(wT)}─┼─${"─".repeat(w0)}─┼─${"─".repeat(w1)}─┼─${"─".repeat(28)}`);
  for (const r of rows) {
    log(`  ${pad(r.t, wT)} │ ${pad(r.c0, w0)} │ ${pad(r.c1, w1)} │ ${r.eng}`);
  }

  const [e0, e1] = result.finalState.entities;
  const res = (e: typeof e0): string =>
    `HP ${e.resources.hp} · ST ${e.resources.stamina} · FO ${e.resources.focus} · AP ${e.resources.ap}`;
  log(`  ${"─".repeat(wT + w0 + w1 + 36)}`);
  log(`  ${pad(f0, w0)} : ${res(e0)}`);
  log(`  ${pad(f1, w1)} : ${res(e1)}`);
  const winner = result.winner === null ? "— (no KO within the scenario)" : opts.fighters[result.winner];
  log(`  winner: ${winner}`);
}
