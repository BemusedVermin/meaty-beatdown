/**
 * fight.ts — `npm run fight` entry point [edge].
 *
 * Drives a scripted scenario (the default) and prints its tick timeline. With no argument it runs all
 * built-in scenarios; pass a scenario id to run just one. Interactive play is available via the
 * InteractiveAgent (cli/agents.ts) but the primary path is scripted scenarios.
 */
import { runMatch } from "../core/engine";
import { printTimeline } from "./timeline";
import { allScenarios, scenarioById, SCENARIO_IDS, type Scenario } from "./scenarios";

function run(scenario: Scenario): void {
  const result = runMatch(scenario.initial, scenario.tables, scenario.agents, scenario.options);
  printTimeline(result, { title: scenario.title, names: scenario.names, fighters: scenario.fighters });
}

function main(): void {
  const id = process.argv[2];
  if (id) {
    const scenario = scenarioById(id);
    if (!scenario) {
      console.error(`unknown scenario "${id}". available: ${SCENARIO_IDS.join(", ")}`);
      process.exitCode = 1;
      return;
    }
    run(scenario);
    return;
  }
  console.log("\n  TICK — fight-runner (scripted scenarios)");
  for (const scenario of allScenarios()) run(scenario);
  console.log("");
}

main();
