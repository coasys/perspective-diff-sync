import { conductorConfig, installation } from './common';
import {sleep, generate_link_expression} from "./utils";

export function unSyncFetch(orchestrator) {
    orchestrator.registerScenario("test un-synced fetch", async (s, t) => {
    const [alice, bob] = await s.players([conductorConfig, conductorConfig])
    const [[alice_happ]] = await alice.installAgentsHapps(installation)
    const [[bob_happ]] = await bob.installAgentsHapps(installation)
    await s.shareAllNodes([alice, bob])
    
    let commit = await alice_happ.cells[0].call("social_context", "commit", {additions: [generate_link_expression("alice")], removals: []});
    console.warn("\ncommit", commit);
    
    await alice_happ.cells[0].call("social_context", "update_latest_revision", commit);
    await alice_happ.cells[0].call("social_context", "update_current_revision", commit);
    
    let pull_alice = await alice_happ.cells[0].call("social_context", "pull");
    console.warn("\npull alice", pull_alice);
    
    let pull_bob = await bob_happ.cells[0].call("social_context", "pull");
    console.warn("\npull bob", pull_bob);
    t.isEqual(pull_bob.additions.length, 1);
    })
};
      
export function mergeFetch(orchestrator) {
    orchestrator.registerScenario("test merge fetch", async (s, t) => {
        const [alice, bob] = await s.players([conductorConfig, conductorConfig])
        const [[alice_happ]] = await alice.installAgentsHapps(installation)
        const [[bob_happ]] = await bob.installAgentsHapps(installation)
      
        //Create new commit whilst bob is not connected
        let link_data = generate_link_expression("alice");
        console.log("Alice posting link data", link_data);
        let commit = await alice_happ.cells[0].call("social_context", "commit", {additions: [link_data], removals: []});
        console.warn("\ncommit", commit.toString("base64"));
        await alice_happ.cells[0].call("social_context", "update_latest_revision", commit);
        await alice_happ.cells[0].call("social_context", "update_current_revision", commit);
      
        //Pull from bob and make sure he does not have the latest state
        let pull_bob = await bob_happ.cells[0].call("social_context", "pull");
        t.isEqual(pull_bob.additions.length, 0);
      
        //Bob to commit his data, and update the latest revision, causing a fork
        let bob_link_data = generate_link_expression("bob");
        console.log("Bob posting link data", bob_link_data);
        let commit_bob = await bob_happ.cells[0].call("social_context", "commit", {additions: [bob_link_data], removals: []});
        console.warn("\ncommit_bob", commit_bob.toString("base64"));
        await bob_happ.cells[0].call("social_context", "update_latest_revision", commit_bob);
        await bob_happ.cells[0].call("social_context", "update_current_revision", commit_bob);
      
        let pull_bob2 = await bob_happ.cells[0].call("social_context", "pull");
        t.isEqual(pull_bob2.additions.length, 0);
      
        //Connect nodes togther
        await s.shareAllNodes([alice, bob])
        //note; running this test on some machines may require more than 200ms wait
        await sleep(200)
      
        //Alice tries to merge
        let merge_alice = await alice_happ.cells[0].call("social_context", "pull");
        t.isEqual(merge_alice.additions.length, 1);
        t.isEqual(JSON.stringify(merge_alice.additions[0].data), JSON.stringify(bob_link_data.data));
      
        //note; running this test on some machines may require more than 200ms wait
        await sleep(200)
      
        let pull_bob3 = await bob_happ.cells[0].call("social_context", "pull");
        t.isEqual(pull_bob3.additions.length, 1);
        console.log(pull_bob3.additions[0].data);
        t.isEqual(JSON.stringify(pull_bob3.additions[0].data), JSON.stringify(link_data.data));
      })
}

export function complexMerge(orchestrator) {
    orchestrator.registerScenario("test complex merge", async (s, t) => {
        const [alice, bob, eric] = await s.players([conductorConfig, conductorConfig, conductorConfig])
        const [[alice_happ]] = await alice.installAgentsHapps(installation)
        const [[bob_happ]] = await bob.installAgentsHapps(installation)
        const [[eric_happ]] = await eric.installAgentsHapps(installation)
      
        // 1 -> alice_link (2)
        //Create new commit whilst bob is not connected
        let link_data = generate_link_expression("alice1");
        console.log("Alice posting link data", link_data);
        let commit = await alice_happ.cells[0].call("social_context", "commit", {additions: [link_data], removals: []});
        console.warn("\ncommit", commit.toString("base64"));
        await alice_happ.cells[0].call("social_context", "update_latest_revision", commit);
        await alice_happ.cells[0].call("social_context", "update_current_revision", commit);
      
        //1 -> bob_link (3)
        //Bob to commit his data, and update the latest revision, causing a fork
        let bob_link_data = generate_link_expression("bob1");
        console.log("Bob posting link data", bob_link_data);
        let commit_bob = await bob_happ.cells[0].call("social_context", "commit", {additions: [bob_link_data], removals: []});
        console.warn("\ncommit_bob", commit_bob.toString("base64"));
        await bob_happ.cells[0].call("social_context", "update_latest_revision", commit_bob);
        await bob_happ.cells[0].call("social_context", "update_current_revision", commit_bob);
      
        //Update bob to use latest revision as created by bob; bob and eric now in their own forked state
        //await eric_happ.cells[0].call("social_context", "update_latest_revision", commit_bob);
        await eric_happ.cells[0].call("social_context", "update_current_revision", commit_bob);
      
        //1 -> bob_link(3) -> eric_link(4)
        //Eric to commit his data, and update the latest revision, causing another fork on a fork
        let eric_link_data = generate_link_expression("eric1");
        console.log("eric posting link data, child of bob commit", eric_link_data);
        let commit_eric = await eric_happ.cells[0].call("social_context", "commit", {additions: [eric_link_data], removals: []});
        console.warn("\ncommit_eric", commit_eric.toString("base64"));
        await eric_happ.cells[0].call("social_context", "update_latest_revision", commit_eric);
        await eric_happ.cells[0].call("social_context", "update_current_revision", commit_eric);
      
        // let eric_pull = await eric_happ.cells[0].call("social_context", "pull");
        // console.log("eric pull result", eric_pull);
      
        //1 -> bob_link(3) -> eric_link(4)
        //                 -> bob_link(5)
        //1 -> alice_link(2) 
        let bob_link_data2 = generate_link_expression("bob2");
        console.log("Bob posting link data, child of bob last commit", bob_link_data2);
        let commit_bob2 = await bob_happ.cells[0].call("social_context", "commit", {additions: [bob_link_data2], removals: []});
        console.warn("\ncommit_bob2", commit_bob2.toString("base64"));
        await bob_happ.cells[0].call("social_context", "update_latest_revision", commit_bob2);
        await bob_happ.cells[0].call("social_context", "update_current_revision", commit_bob2);
        
        //Connect nodes togther
        await s.shareAllNodes([alice, bob, eric])
        //note; running this test on some machines may require more than 500ms wait
        await sleep(1000)

        //1 -> bob_link(3) -> eric_link(4) -> merge(6) -> eric_link(7)
        //                 -> bob_link(5) -> merge(6)
        //1 -> alice_link(2) 
        //Eric to commit his data, and update the latest revision, causing another fork on a fork
        let eric_link_data2 = generate_link_expression("eric2");
        console.log("eric posting link data, will merge bob second and eric first entry", eric_link_data2);
        let commit_eric2 = await eric_happ.cells[0].call("social_context", "commit", {additions: [eric_link_data2], removals: []});
        console.warn("\ncommit_eric2", commit_eric2.toString("base64"));
        await eric_happ.cells[0].call("social_context", "update_latest_revision", commit_eric2);
        await eric_happ.cells[0].call("social_context", "update_current_revision", commit_eric2);

        await sleep(1000)

        //1 -> bob_link(3) -> eric_link(4) -> merge(6) -> eric_link(7)
        //                 -> bob_link(5) -> merge(6)
        //1 -> alice_link(2) 
        let bob_pull = await bob_happ.cells[0].call("social_context", "pull");
        console.log("Bob pull result", bob_pull);
        //Should get two entries from Eric
        t.isEqual(bob_pull.additions.length, 2);
        await sleep(200)
      
        //1 -> bob_link(3) -> eric_link(4) -> merge(6) -> eric_link(7) -> merge(8)
        //                 -> bob_link(5) -> merge(6)
        //1 -> alice_link(2)                                           -> merge(8)
        let alice_merge = await alice_happ.cells[0].call("social_context", "pull");
        console.log("Alice merge result", alice_merge);
        //should get whole side of bob/eric graph
        t.isEqual(alice_merge.additions.length, 4);
        await sleep(200)
        
        //1 -> bob_link(3) -> eric_link(4) -> merge(6) -> eric_link(7) -> merge(8)
        //                 -> bob_link(5) -> merge(6)
        //1 -> alice_link(2)                                           -> merge(8)
        //Should get one entry from alice
        let eric_pull = await eric_happ.cells[0].call("social_context", "pull");
        console.log("Eric pull result", eric_pull);
        t.isEqual(eric_pull.additions.length, 1);
    })
}
