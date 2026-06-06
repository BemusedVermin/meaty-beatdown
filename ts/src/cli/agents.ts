/**
 * agents.ts — the three Agent implementations (spec "Determinism & the Agent interface") [edge].
 *
 * The engine is pure/synchronous; agents are where decisions (and, for Interactive, I/O) enter. This
 * is a cli/ edge module, so async/I/O are permitted — but the Agent methods themselves return values
 * synchronously, as the engine requires.
 */
import { readSync } from "node:fs";
import {
  type Action,
  type Agent,
  type CancelView,
  type Decision,
  type DecisionResult,
  type PlayerView,
} from "../core/engine";

export { type Decision } from "../core/engine";

/** ScriptedAgent — fixed action/cancel lists → deterministic tests & scenarios. */
export class ScriptedAgent implements Agent {
  private ai = 0;
  private ci = 0;
  constructor(
    private readonly actions: readonly Action[],
    private readonly cancels: readonly DecisionResult[] = [],
  ) {}
  chooseAction(_view: PlayerView): Action {
    return this.actions[this.ai++] ?? { kind: "WAIT" };
  }
  chooseCancel(_view: CancelView): DecisionResult {
    return this.cancels[this.ci++] ?? { kind: "DECLINE" };
  }
}

/** RecordingAgent — wraps another agent and records its decision stream (for golden-vector emit). */
export class RecordingAgent implements Agent {
  readonly decisions: Decision[] = [];
  constructor(private readonly inner: Agent) {}
  chooseAction(view: PlayerView): Action {
    const action = this.inner.chooseAction(view);
    this.decisions.push({ kind: "action", action });
    return action;
  }
  chooseCancel(view: CancelView): DecisionResult {
    const result = this.inner.chooseCancel(view);
    this.decisions.push({ kind: "cancel", result });
    return result;
  }
}

/** ReplayAgent — replays a recorded decision stream → the golden-vector verification harness. */
export class ReplayAgent implements Agent {
  private i = 0;
  constructor(private readonly decisions: readonly Decision[]) {}
  chooseAction(_view: PlayerView): Action {
    const d = this.decisions[this.i++];
    if (!d || d.kind !== "action") throw new Error("replay desync: expected an action decision");
    return d.action;
  }
  chooseCancel(_view: CancelView): DecisionResult {
    const d = this.decisions[this.i++];
    if (!d || d.kind !== "cancel") throw new Error("replay desync: expected a cancel decision");
    return d.result;
  }
}

/** Read one line from stdin synchronously (the engine asks synchronously). */
function readLineSync(): string {
  const buf = Buffer.alloc(1);
  let line = "";
  for (;;) {
    let n = 0;
    try {
      n = readSync(0, buf, 0, 1, null);
    } catch {
      break; // EOF / non-interactive
    }
    if (n === 0) break;
    const ch = buf.toString("utf8");
    if (ch === "\n") break;
    if (ch !== "\r") line += ch;
  }
  return line.trim();
}

/**
 * InteractiveAgent — synchronous stdin play. Secondary to scripted scenarios; prints the view and
 * reads a move id (or "wait"); at a cancel checkpoint reads a move id (or blank to decline).
 */
export class InteractiveAgent implements Agent {
  constructor(private readonly label: string) {}
  chooseAction(view: PlayerView): Action {
    process.stdout.write(
      `\n[${this.label}] T=${view.t} ${view.regime.kind} — moves: ${view.availableMoves.join(", ")}\n> `,
    );
    const input = readLineSync();
    if (input === "" || input === "wait") return { kind: "WAIT" };
    return { kind: "MOVE", moveId: input };
  }
  chooseCancel(view: CancelView): DecisionResult {
    process.stdout.write(
      `\n[${this.label}] T=${view.t} cancel (${view.contact}) into: ${view.cancelInto.join(", ")} (blank = decline)\n> `,
    );
    const input = readLineSync();
    if (input === "") return { kind: "DECLINE" };
    return { kind: "MOVE", moveId: input };
  }
}
