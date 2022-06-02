import { conductorConfig, installation } from './common'

export function testRevisionUpdates(orchestrator) {
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
}