import { addAllAgentsToAllConductors, AgentApp, cleanAllConductors } from "@holochain/tryorama";
import { call, sleep, generate_link_expression, createConductors, create_link_expression, includedInDiff, includes} from "./utils";
import test from "tape-promise/tape.js";
import ad4m, { LinkExpression, Perspective, PerspectiveDiff } from "@perspect3vism/ad4m";

//NOTE; these tests are dependant on the SNAPSHOT_INTERVAL in lib.rs being set to 2
//@ts-ignore
export async function render(t) {
    let installs = await createConductors(2);
    let aliceHapps = installs[0].agent_happ;
    let conductor1 = installs[0].conductor;
    let bobHapps = installs[1].agent_happ;
    let conductor2 = installs[1].conductor;
    await addAllAgentsToAllConductors([conductor1, conductor2]);
    
    console.log("RENDER 1")
    let commit = await call(aliceHapps, "commit", {
        additions: [generate_link_expression("alice1")], 
        removals: []
    });
    console.warn("\ncommit", commit);
    
    await call(aliceHapps, "update_latest_revision", commit);
    await call(aliceHapps, "update_current_revision", commit);

    let commit2 = await call(aliceHapps, "commit", {
        additions: [generate_link_expression("alice2")], 
        removals: []
    });
    console.warn("\ncommit", commit2);
    
    console.log("RENDER 2")

    await call(aliceHapps, "update_latest_revision", commit2);
    await call(aliceHapps, "update_current_revision", commit2);

    let alice_rendered = await call(aliceHapps, "render");
    //@ts-ignore
    t.equal(alice_rendered.links.length, 2)

    await sleep(5000);
    
    console.log("RENDER 3")

    let bob_latest_revision = await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "latest_revision"
    });

    //@ts-ignore
    t.isEqual(bob_latest_revision.toString(), commit2.toString())

    // Bob hasn't pulled yet, so render on Bob should fail
    let firstRenderFailed = false
    try {
        let bob_render = await call(bobHapps, "render");
    } catch(e) {
        firstRenderFailed = true
    }

    t.assert(firstRenderFailed)

    await call(bobHapps, "pull")
    await call(bobHapps, "pull")

    console.log("Bob has pulled")

    let bob_render = await call(bobHapps, "render");

    
    console.log("RENDER 4")
    console.warn("bob rendered with", bob_render);
    //@ts-ignore
    t.deepEqual(bob_render.links.length, 2);

    await call(bobHapps, "update_latest_revision", commit2);
    await call(bobHapps, "update_current_revision", commit2);

    let commit4 = await call(bobHapps, "commit", {
        additions: [generate_link_expression("bob3")], 
        removals: []
    });
    console.warn("\ncommit", commit4);
    
    await call(bobHapps, "update_latest_revision", commit4);
    await call(bobHapps, "update_current_revision", commit4);


    let commit5 = await call(bobHapps, "commit", {
        additions: [generate_link_expression("bob4")], 
        removals: []
    });
    console.warn("\ncommit", commit5);
    
    await call(bobHapps, "update_latest_revision", commit5);
    await call(bobHapps, "update_current_revision", commit5);

    await sleep(1000);

    await call(aliceHapps, "pull"); 
    let alice_render = await call(aliceHapps, "render");
    console.warn("Alice rendered with", alice_render);
    //@ts-ignore
    t.deepEqual(alice_render.links.length, 4);

    await conductor1.shutDown();
    await conductor2.shutDown();
    await cleanAllConductors();
};

//@ts-ignore
export async function renderMerges(t) {
    let installs = await createConductors(2);
    let aliceHapps = installs[0].agent_happ;
    let conductor1 = installs[0].conductor;
    let bobHapps = installs[1].agent_happ;
    let conductor2 = installs[1].conductor;
    
    console.log("commit1");
    let commit = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "commit", 
        payload: {additions: [generate_link_expression("alice1")], removals: []}
    });
    console.warn("\ncommit", commit);
    
    await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_latest_revision",
        payload: commit
    });
    await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_current_revision", 
        payload: commit
    });

    console.log("commit2");
    let commit2 = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "commit", 
        payload: {additions: [generate_link_expression("alice2")], removals: []}
    });
    console.warn("\ncommit", commit2);
    
    await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_latest_revision",
        payload: commit2
    });
    await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_current_revision", 
        payload: commit2
    });

    console.log("commit3");
    let commit3 = await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "commit", 
        payload: {additions: [generate_link_expression("bob1")], removals: []}
    });
    console.warn("\ncommit", commit3);
    
    await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_latest_revision",
        payload: commit3
    });
    await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_current_revision", 
        payload: commit3
    });

    console.log("commit4");
    let commit4 = await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "commit", 
        payload: {additions: [generate_link_expression("bob2")], removals: []}
    });
    console.warn("\ncommit", commit4);
    
    await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_latest_revision",
        payload: commit4
    });
    await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_current_revision", 
        payload: commit4
    });

    console.log("bob render");
    let bob_render = await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "render"
    });
    console.warn("bob rendered with", bob_render);
    //@ts-ignore
    t.isEqual(bob_render.links.length, 2);

    console.log("alice render");
    let alice_render = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "render"
    });
    console.warn("Alice rendered with", alice_render);
    //@ts-ignore
    t.isEqual(alice_render.links.length, 2);
    
    await addAllAgentsToAllConductors([conductor1, conductor2]);
    await sleep(500);

    //Test getting revision, should return bob's revision since that is the latest entry

    //Alice commit which will create a merge and another entry
    console.log("commit5");
    let commit5 = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "commit", 
        payload: {additions: [generate_link_expression("alice3")], removals: []}
    });
    console.warn("\ncommit5", commit5);
    
    await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_latest_revision",
        payload: commit5
    });
    await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_current_revision", 
        payload: commit5
    });

    //Alice commit which should not create another snapshot
    console.log("commit6");
    let commit6 = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "commit", 
        payload: {additions: [generate_link_expression("alice4")], removals: []}
    });
    console.warn("\ncommit6", commit6);
    
    await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_latest_revision",
        payload: commit6
    });
    await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_current_revision", 
        payload: commit6
    });
    await sleep(2000)

    console.log("bob pull");
    await call(bobHapps, "pull")
    
    console.log("bob render");
    let bob_render2 = await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "render"
    });
    console.warn("bob rendered with", bob_render2);
    //@ts-ignore
    t.isEqual(bob_render2.links.length, 6);

    console.log("alice render");
    let alice_render2 = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "render"
    });
    console.warn("Alice rendered with", alice_render2);
    //@ts-ignore
    t.isEqual(alice_render2.links.length, 4);

    await conductor1.shutDown();
    await conductor2.shutDown();
    await cleanAllConductors();
}

let createdLinks = new Map<string, Array<LinkExpression>>()

async function createLinks(happ: AgentApp, agentName: string, count: number) {
    if(!createdLinks.get(agentName)) createdLinks.set(agentName, [])
    for(let i=0; i < count; i++) {
        let { data } = await create_link_expression(happ.cells[0], agentName);
        createdLinks.get(agentName)!.push(data)
    }
}

//@ts-ignore
async function testSnapshotRenders(t) {
    let installs = await createConductors(2);
    let aliceHapps = installs[0].agent_happ;
    let aliceConductor = installs[0].conductor;
    let bobHapps = installs[1].agent_happ;
    let bobConductor = installs[1].conductor;

    //Create 150 links from alice
    await createLinks(aliceHapps, "alice", 75);

    await addAllAgentsToAllConductors([aliceConductor, bobConductor]);

    await sleep(5000);

    //Pull the 500 links from bobs node
    const bobPull = await call(bobHapps, "pull") as PerspectiveDiff;
    
    //Check that all the created links are in the diff
    for(let link of createdLinks.get("alice")!) {
        t.assert(includedInDiff(bobPull, link))
    }

    for(let link of bobPull.additions) {
        t.assert(createdLinks.get("alice")!.find(aLink => ad4m.linkEqual(aLink, link)))
    }

    //Render the 150 links from bobs node
    const render = await call(bobHapps, "render") as Perspective;

    for(let link of createdLinks.get("alice")!) {
        t.assert(includes(render, link))
    }

    for(let link of render.links) {
        t.assert(createdLinks.get("alice")!.find(aLink => ad4m.linkEqual(aLink, link)))
    }

    ///Test now alice rendering bobs stuff
    ///
    ///

    await createLinks(bobHapps, "bob", 75);

    await sleep(5000);

    const alicePull = await call(aliceHapps, "pull") as PerspectiveDiff;

    for(let link of createdLinks.get("bob")!) {
        t.assert(includedInDiff(alicePull, link))
    }

    for(let link of alicePull.additions) {
        t.assert(createdLinks.get("bob")!.find(aLink => ad4m.linkEqual(aLink, link)))
    }

    const aliceRender = await call(aliceHapps, "render") as Perspective;

    for(let link of createdLinks.get("bob")!) {
        t.assert(includes(aliceRender, link))
    }

    for(let link of aliceRender.links) {
        t.assert(createdLinks.get("bob")!.find(aLink => ad4m.linkEqual(aLink, link)) || createdLinks.get("alice")!.find(aLink => ad4m.linkEqual(aLink, link)))
    }

    await aliceConductor.shutDown();
    await bobConductor.shutDown();
    await cleanAllConductors();
} 

test("render", async (t) => {
    // await render(t)
    // await renderMerges(t)
    await testSnapshotRenders(t)
    t.end()
})