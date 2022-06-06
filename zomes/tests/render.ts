import { conductorConfig, installation } from './common';
import {generate_link_expression, sleep} from "./utils";

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

        let commit3 = await alice_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("alice3")], removals: []});
        console.warn("\ncommit", commit3);
        
        await alice_happ.cells[0].call("social_context", "update_latest_revision", commit3);
        await alice_happ.cells[0].call("social_context", "update_current_revision", commit3);

        await sleep(1000);
        
        let bob_render = await bob_happ.cells[0].call("social_context", "render");
        console.warn("bob rendered with", bob_render);
    })
};
      