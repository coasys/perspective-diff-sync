import { AgentHapp, addAllAgentsToAllConductors, cleanAllConductors } from "@holochain/tryorama";
import { sleep, createConductors, create_link_expression} from "./utils";

async function call(happ: AgentHapp, fn_name: string, payload?: any) {
    return await happ.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name,
        payload
    });
}

async function createLinks(happ: AgentHapp, agentName: string, count: number) {
    for(let i=0; i < count; i++) {
        await create_link_expression(happ.cells[0], agentName, true, true);
    }
}

//@ts-ignore
export async function stressTest(t) {
    let installs = await createConductors(2);
    let aliceHapps = installs[0].agent_happ;
    let aliceConductor = installs[0].conductor;
    let bobHapps = installs[1].agent_happ;
    let bobConductor = installs[1].conductor;

    await addAllAgentsToAllConductors([aliceConductor, bobConductor]);

    console.log("==============================================")
    console.log("=================START========================")
    console.log("==============================================")
    for(let i=0; i < 20; i++) {
        console.log("-------------------------");
        console.log("Iteration: ", i)
        console.log("-------------------------");
        await Promise.all([
            createLinks(aliceHapps, "alice", 20),
            createLinks(bobHapps, "bob", 20)
        ])

        console.log("-------------------------");
        console.log("Created 20 links each (Alice and Bob)");
        console.log("waiting a second");
        console.log("-------------------------");

        sleep(1000)

        console.log("-------------------------");
        console.log("Now pulling on both agens...");
        console.log("-------------------------");
        await Promise.all([
            call(aliceHapps, "pull"),
            call(bobHapps, "pull")
        ])

        let alice_latest_revision = await call(aliceHapps, "latest_revision")
        let bob_latest_revision = await call(bobHapps, "latest_revision")
        let alice_current_revision = await call(aliceHapps, "current_revision")
        let bob_current_revision = await call(bobHapps, "current_revision")

        t.isEqual(alice_latest_revision, bob_latest_revision)
        t.isEqual(alice_current_revision, bob_current_revision)
        t.isEqual(alice_latest_revision, alice_current_revision)

        console.log("-------------------------");
        console.log("All good :)))))))))))))))");
        console.log("-------------------------");

    }

    await aliceConductor.shutDown();
    await bobConductor.shutDown();
    await cleanAllConductors();
}