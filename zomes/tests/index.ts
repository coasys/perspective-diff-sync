import { Orchestrator, Config, InstallAgentsHapps } from '@holochain/tryorama'
import path from 'path'
import faker from 'faker'

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

  await alice_sc_happ.cells[0].call("social_context", "update_latest_revision", commit);
  await alice_sc_happ.cells[0].call("social_context", "update_current_revision", commit);

  let latest_revision2 = await alice_sc_happ.cells[0].call("social_context", "latest_revision");
  console.warn("latest_revision2", latest_revision2);
  t.isEqual(commit.toString(), latest_revision2.toString())

  let current_revision2 = await alice_sc_happ.cells[0].call("social_context", "current_revision");
  console.warn("current_revision2", current_revision2);
  t.isEqual(commit.toString(), current_revision2.toString())

  let commit2 = await alice_sc_happ.cells[0].call("social_context", "commit", {additions: [], removals: []});
  console.warn("\ncommit2", commit2);

  await alice_sc_happ.cells[0].call("social_context", "update_latest_revision", commit2);
  await alice_sc_happ.cells[0].call("social_context", "update_current_revision", commit2);

  let latest_revision3 = await alice_sc_happ.cells[0].call("social_context", "latest_revision");
  console.warn("latest_revision3", latest_revision3);
  t.isEqual(commit2.toString(), latest_revision3.toString())

  let current_revision3 = await alice_sc_happ.cells[0].call("social_context", "current_revision");
  console.warn("current_revision3", current_revision3);
  t.isEqual(commit2.toString(), current_revision3.toString())
})

orchestrator.registerScenario("test un-synced fetch", async (s, t) => {
  const [alice, bob] = await s.players([conductorConfig, conductorConfig])
  const [[alice_happ]] = await alice.installAgentsHapps(installation)
  const [[bob_happ]] = await alice.installAgentsHapps(installation)
  await s.shareAllNodes([alice, bob])

  let commit = await alice_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression()], removals: []});
  console.warn("\ncommit", commit);

  await alice_happ.cells[0].call("social_context", "update_latest_revision", commit);
  await alice_happ.cells[0].call("social_context", "update_current_revision", commit);

  let pull_alice = await alice_happ.cells[0].call("social_context", "pull");
  console.warn("\npull alice", pull_alice);

  let pull_bob = await bob_happ.cells[0].call("social_context", "pull");
  console.warn("\npull bob", pull_bob);
  t.isEqual(pull_bob.length, 1);
  t.end()
})

function generate_link_expression() {
  return {
    data: {source: faker.name.findName(), target: faker.name.findName(), predicate: faker.name.findName()},
    author: "test1", 
    timestamp: new Date().toISOString(), 
    proof: {signature: "sig", key: "key"},
 }
}

// Run all registered scenarios as a final step, and gather the report,
// if you set up a reporter
const report = orchestrator.run()

// Note: by default, there will be no report
console.log(report)