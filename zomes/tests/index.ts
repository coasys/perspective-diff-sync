import { Orchestrator, Config, InstallAgentsHapps } from '@holochain/tryorama'
import path from 'path'

//const conductorConfig = Config.gen({network});
const conductorConfig = Config.gen();

const installation: InstallAgentsHapps = [
  // agent 0
  [
    // happ 0
    [path.join("../../workdir/social-context.dna")]
  ]
]

const orchestrator = new Orchestrator()

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

orchestrator.registerScenario("test committing, and updating latest & current revisions", async (s, t) => {
  const [alice] = await s.players([conductorConfig])
  const [[alice_sc_happ]] = await alice.installAgentsHapps(installation)

  let latest_revision = await alice_sc_happ.cells[0].call("social_context", "latest_revision");
  console.warn("latest_revision", latest_revision);

  let current_revision = await alice_sc_happ.cells[0].call("social_context", "current_revision");
  console.warn("current_revision", current_revision);

  let commit = await alice_sc_happ.cells[0].call("social_context", "commit", {additions: [], removals: []});
  console.warn("\ncommit", commit);

  let latest_revision2 = await alice_sc_happ.cells[0].call("social_context", "latest_revision");
  console.warn("latest_revision2", latest_revision2);

  let current_revision2 = await alice_sc_happ.cells[0].call("social_context", "current_revision");
  console.warn("current_revision2", current_revision2);

  let commit2 = await alice_sc_happ.cells[0].call("social_context", "commit", {additions: [], removals: []});
  console.warn("\ncommit2", commit2);

  let latest_revision3 = await alice_sc_happ.cells[0].call("social_context", "latest_revision");
  console.warn("latest_revision3", latest_revision3);

  let current_revision3 = await alice_sc_happ.cells[0].call("social_context", "current_revision");
  console.warn("current_revision3", current_revision3);
})

// Run all registered scenarios as a final step, and gather the report,
// if you set up a reporter
const report = orchestrator.run()

// Note: by default, there will be no report
console.log(report)