import { AgentHapp, addAllAgentsToAllConductors, cleanAllConductors } from "@holochain/tryorama";
import { call, sleep, createConductors, create_link_expression, generate_link_expression} from "./utils";
import ad4m, { LinkExpression, Perspective } from "@perspect3vism/ad4m"
import test from "tape-promise/tape.js";

let createdLinks = new Map<string, Array<LinkExpression>>()

async function createLinks(happ: AgentHapp, agentName: string, count: number) {
    if(!createdLinks.get(agentName)) createdLinks.set(agentName, [])
    for(let i=0; i < count; i++) {
        let { data } = await create_link_expression(happ.cells[0], agentName, true, true);
        createdLinks.get(agentName)!.push(data)
    }
}

//@ts-ignore
export async function latestRevisionStress(t) {
    let installs = await createConductors(2);
    let aliceHapps = installs[0].agent_happ;
    let aliceConductor = installs[0].conductor;
    let bobHapps = installs[1].agent_happ;
    let bobConductor = installs[1].conductor;

    await addAllAgentsToAllConductors([aliceConductor, bobConductor]);

    let link_data = generate_link_expression("alice");
    let commit = await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "commit", 
        payload: {additions: [link_data], removals: []}
    });

    for (let i = 0; i < 1000; i++) {
        console.log("Latest update revision", i);
        let now = performance.now();
        let create = await aliceHapps.cells[0].callZome({zome_name: "perspective_diff_sync", fn_name: "update_latest_revision", payload: commit});
        let after = performance.now();
        console.log(" Create execution took: ", after - now);
        let fetch = await aliceHapps.cells[0].callZome({zome_name: "perspective_diff_sync", fn_name: "latest_revision"});
        let after2 = performance.now();
        console.log("Fetch execution took: ", after2 - after);
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
    for(let i=0; i < 3; i++) {
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

        await sleep(1000)

        console.log("-------------------------");
        console.log("Now pulling on both agents...");
        console.log("-------------------------");

        let pullSuccessful = false
        while(!pullSuccessful) {
            try {
                await call(aliceHapps, "pull");
                await call(bobHapps, "pull");

                await call(aliceHapps, "pull");
                await call(bobHapps, "pull");
                pullSuccessful = true
            } catch(e) {
                console.error("Pulling failed with error:", e)
                await sleep(2000)
            }
        }
        
        await sleep(5000)
        

        let alice_latest_revision = await call(aliceHapps, "latest_revision")
        let bob_latest_revision = await call(bobHapps, "latest_revision")
        let alice_current_revision = await call(aliceHapps, "current_revision")
        let bob_current_revision = await call(bobHapps, "current_revision")

        //@ts-ignore
        t.isEqual(alice_latest_revision.toString("base64"), bob_latest_revision.toString("base64"))
        //@ts-ignore
        t.isEqual(alice_current_revision.toString("base64"), bob_current_revision.toString("base64"))
        //@ts-ignore
        t.isEqual(alice_latest_revision.toString("base64"), alice_current_revision.toString("base64"))

        console.log("-------------------------");
        console.log("All good :)))))))))))))))");
        console.log("-------------------------");

    }


    let alice_rendered = await call(aliceHapps, "render") as Perspective
    let bob_rendered = await call(bobHapps, "render") as Perspective

    t.isEqual(alice_rendered.links.length, bob_rendered.links.length)

    function includes(perspective: Perspective, link: LinkExpression) {
        return perspective.links.find(l => ad4m.linkEqual(l,link))
    }

    for(let link of createdLinks.get("alice")!) {
        t.assert(includes(alice_rendered, link))
        t.assert(includes(bob_rendered, link))
    }

    await aliceConductor.shutDown();
    await bobConductor.shutDown();
    await cleanAllConductors();
}

test("stress", async (t) => {
    await stressTest(t);
})
