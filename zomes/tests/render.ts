import { conductorConfig, installation } from './common';
import {generate_link_expression, sleep} from "./utils";

//NOTE; these tests are dependant on the SNAPSHOT_INTERVAL in lib.rs being set to 2
export function render(orchestrator) {
    orchestrator.registerScenario("test simple render", async (s, t) => {
        const [alice, bob] = await s.players([conductorConfig, conductorConfig])
        const [[alice_happ]] = await alice.installAgentsHapps(installation)
        const [[bob_happ]] = await alice.installAgentsHapps(installation)
        await s.shareAllNodes([alice, bob])
        
        let commit = await alice_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("alice1")], removals: []});
        console.warn("\ncommit", commit);
        
        await alice_happ.cells[0].call("social_context", "update_latest_revision", commit);
        await alice_happ.cells[0].call("social_context", "update_current_revision", commit);

        let commit2 = await alice_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("alice2")], removals: []});
        console.warn("\ncommit", commit2);
        
        await alice_happ.cells[0].call("social_context", "update_latest_revision", commit2);
        await alice_happ.cells[0].call("social_context", "update_current_revision", commit2);

        await sleep(1000);
        
        let bob_render = await bob_happ.cells[0].call("social_context", "render");
        console.warn("bob rendered with", bob_render);
        t.deepEqual(bob_render.links.length, 2);

        await bob_happ.cells[0].call("social_context", "update_latest_revision", commit2);
        await bob_happ.cells[0].call("social_context", "update_current_revision", commit2);

        let commit4 = await bob_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("bob3")], removals: []});
        console.warn("\ncommit", commit4);
        
        await bob_happ.cells[0].call("social_context", "update_latest_revision", commit4);
        await bob_happ.cells[0].call("social_context", "update_current_revision", commit4);


        let commit5 = await bob_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("bob4")], removals: []});
        console.warn("\ncommit", commit5);
        
        await bob_happ.cells[0].call("social_context", "update_latest_revision", commit5);
        await bob_happ.cells[0].call("social_context", "update_current_revision", commit5);

        let alice_render = await alice_happ.cells[0].call("social_context", "render");
        console.warn("Alice rendered with", alice_render);
        t.deepEqual(alice_render.links.length, 4);
    })
};
      