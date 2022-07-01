import { addAllAgentsToAllConductors, cleanAllConductors } from "@holochain/tryorama";
import { sleep, createConductors} from "./utils";

//@ts-ignore
export async function testRevisionUpdates(t) {
    let installs = await createConductors(2);
    let aliceHapps = installs[0].agent_happ;
    let aliceConductor = installs[0].conductor;
    let bobHapps = installs[1].agent_happ;
    let bobConductor = installs[1].conductor;

    await addAllAgentsToAllConductors([aliceConductor, bobConductor]);

    let latest_revision = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "latest_revision"
    });
    console.warn("latest_revision", latest_revision);

    let current_revision = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "current_revision"
    });
    console.warn("current_revision", current_revision);

    let commit = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "commit", 
        payload: {additions: [], removals: []}
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

    let latest_revision2 = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "latest_revision"
    });
    console.warn("latest_revision2", latest_revision2);
    //@ts-ignore
    t.isEqual(commit.toString(), latest_revision2.toString())

    let current_revision2 = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "current_revision"
    });
    console.warn("current_revision2", current_revision2);
    //@ts-ignore
    t.isEqual(commit.toString(), current_revision2.toString())

    await sleep(1000)

    //test bobs latest revision is updated
    let bob_latest_revision = await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "latest_revision"
    });
    console.warn("bob_latest_revision", bob_latest_revision);
    //@ts-ignore
    t.isEqual(commit.toString(), bob_latest_revision.toString())

    //test bobs current revision is not updated
    let bob_current_revision = await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "current_revision"
    });
    console.warn("bob_current_revision", bob_current_revision);
    //@ts-ignore
    t.isEqual(null, bob_current_revision);

    let commit2 = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "commit", 
        payload: {additions: [], removals: []}
    });
    //@ts-ignore
    console.warn("\ncommit2", commit2);

    await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_current_revision", 
        payload: commit2
    });

    let current_revision3 = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "current_revision"
    });
    console.warn("current_revision3", current_revision3);
    //@ts-ignore
    t.isEqual(current_revision3.toString(), commit.toString());

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

    let latest_revision3 = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "latest_revision"
    });
    console.warn("latest_revision3", latest_revision3);
    //@ts-ignore
    t.isEqual(commit2.toString(), latest_revision3.toString())

    let current_revision4 = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "current_revision"
    });
    console.warn("current_revision4", current_revision4);
    //@ts-ignore
    t.isEqual(commit2.toString(), current_revision4.toString())

    await aliceConductor.shutDown();
    await bobConductor.shutDown();
    await cleanAllConductors();
}