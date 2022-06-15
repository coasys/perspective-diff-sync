import { conductorConfig, installation } from './common';
import {generate_link_expression, sleep} from "./utils";

//NOTE; these tests are dependant on the SNAPSHOT_INTERVAL in lib.rs being set to 2
export function render(orchestrator) {
    orchestrator.registerScenario("test simple render", async (s, t) => {
        const [alice, bob] = await s.players([conductorConfig, conductorConfig])
        const [[alice_happ]] = await alice.installAgentsHapps(installation)
        const [[bob_happ]] = await bob.installAgentsHapps(installation)
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

export function renderMerges(orchestrator) {
    orchestrator.registerScenario("test complex render", async (s, t) => {
        const [alice, bob] = await s.players([conductorConfig, conductorConfig])
        const [[alice_happ]] = await alice.installAgentsHapps(installation)
        const [[bob_happ]] = await bob.installAgentsHapps(installation)
        
        console.log("commit1");
        let commit = await alice_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("alice1")], removals: []});
        console.warn("\ncommit", commit);
        
        await alice_happ.cells[0].call("social_context", "update_latest_revision", commit);
        await alice_happ.cells[0].call("social_context", "update_current_revision", commit);

        console.log("commit2");
        let commit2 = await alice_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("alice2")], removals: []});
        console.warn("\ncommit", commit2);
        
        await alice_happ.cells[0].call("social_context", "update_latest_revision", commit2);
        await alice_happ.cells[0].call("social_context", "update_current_revision", commit2);

        console.log("commit3");
        let commit3 = await bob_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("bob1")], removals: []});
        console.warn("\ncommit", commit3);
        
        await bob_happ.cells[0].call("social_context", "update_latest_revision", commit3);
        await bob_happ.cells[0].call("social_context", "update_current_revision", commit3);

        console.log("commit4");
        let commit4 = await bob_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("bob2")], removals: []});
        console.warn("\ncommit", commit4);
        
        await bob_happ.cells[0].call("social_context", "update_latest_revision", commit4);
        await bob_happ.cells[0].call("social_context", "update_current_revision", commit4);

        console.log("bob render");
        let bob_render = await bob_happ.cells[0].call("social_context", "render");
        console.warn("bob rendered with", bob_render);
        t.isEqual(bob_render.links.length, 2);

        console.log("alice render");
        let alice_render = await alice_happ.cells[0].call("social_context", "render");
        console.warn("Alice rendered with", alice_render);
        t.isEqual(alice_render.links.length, 2);
        
        await s.shareAllNodes([alice, bob])
        await sleep(500);

        //Test getting revision, should return bob's revision since that is the latest entry

        //Alice commit which will create a merge and another entry
        console.log("commit5");
        let commit5 = await alice_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("alice3")], removals: []});
        console.warn("\ncommit5", commit5);
        
        await alice_happ.cells[0].call("social_context", "update_latest_revision", commit5);
        await alice_happ.cells[0].call("social_context", "update_current_revision", commit5);

        //Alice commit which should not create another snapshot
        console.log("commit6");
        let commit6 = await alice_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("alice4")], removals: []});
        console.warn("\ncommit6", commit6);
        
        await alice_happ.cells[0].call("social_context", "update_latest_revision", commit6);
        await alice_happ.cells[0].call("social_context", "update_current_revision", commit6);
        await sleep(500)

        console.log("bob render");
        let bob_render2 = await bob_happ.cells[0].call("social_context", "render");
        console.warn("bob rendered with", bob_render2);
        t.isEqual(bob_render2.links.length, 6);

        console.log("alice render");
        let alice_render2 = await alice_happ.cells[0].call("social_context", "render");
        console.warn("Alice rendered with", alice_render2);
        t.isEqual(alice_render2.links.length, 6);
    })
}