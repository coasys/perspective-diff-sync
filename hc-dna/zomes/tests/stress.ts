import { AgentApp, addAllAgentsToAllConductors, cleanAllConductors, Conductor, Scenario } from "@holochain/tryorama";
import { call, sleep, createConductors, create_link_expression, generate_link_expression} from "./utils";
import ad4m, { LinkExpression, Perspective } from "@perspect3vism/ad4m"
import test from "tape-promise/tape.js";
import { hrtime } from 'node:process';
//@ts-ignore
import divide from 'divide-bigint'
import { AsyncQueue } from "./queue";
import { resolve } from "path";
import { dnas } from "./common";

let createdLinks = new Map<string, Array<LinkExpression>>()

async function createLinks(happ: AgentApp, agentName: string, count: number, queue?: AsyncQueue) {
    if(!createdLinks.get(agentName)) createdLinks.set(agentName, [])
    for(let i=0; i < count; i++) {
        if (queue) {
            await queue.add(async () => {
                let { data } = await create_link_expression(happ.cells[0], agentName);
                createdLinks.get(agentName)!.push(data)
            }).catch((e) => {
                console.log("Error in create links queue", e);
            })
        } else {
            let { data } = await create_link_expression(happ.cells[0], agentName);
            createdLinks.get(agentName)!.push(data)
        }
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

async function waitDhtConsistency(hash: Buffer, conductor: Conductor) {
    while ((await conductor.appWs().networkInfo({dnas: [hash]}))[0].fetch_queue_info.op_bytes_to_fetch != 0) {
        console.log("waiting for consistency...");
        await sleep(1000);
    }
}

//@ts-ignore
export async function stressTest(t) {
    const aliceQueue = new AsyncQueue();
    const bobQueue = new AsyncQueue();

    const scenario = new Scenario();
    const aliceHapps = await scenario.addPlayerWithApp(
        {
            bundle: {
                manifest: {
                    manifest_version: "1",
                    name: "perspective-diff-sync",
                    roles: [{
                        name: "main",
                        dna: {
                            //@ts-ignore
                            path: resolve(dnas[0].source.path)
                        }
                    }]
                },
                resources: {}
            },
        }
    );
    aliceHapps.conductor.appWs().on("signal", async (signal) => {
        console.log("Alice Received Signal:",signal);
        if (signal.payload.diff) {
            if (signal.payload.diff && signal.payload.reference_hash && signal.payload.reference) {
                await aliceQueue.add(async () => {
                    await call(aliceHapps, "fast_forward_signal", signal.payload);
                }).catch((e) => {
                    console.log("Error in signal alice queue", e);
                });
            }
        }
    });
    const bobHapps = await scenario.addPlayerWithApp(
        {
            bundle: {
                manifest: {
                    manifest_version: "1",
                    name: "perspective-diff-sync",
                    roles: [{
                        name: "main",
                        dna: {
                            //@ts-ignore
                            path: resolve(dnas[0].source.path)
                        }
                    }]
                },
                resources: {}
            }
        }
    );
    bobHapps.conductor.appWs().on("signal", async (signal) => {
        console.log("Bob Received Signal:",signal)
        if (signal.payload.diff) {
            if (signal.payload.diff && signal.payload.reference_hash && signal.payload.reference) {
                await bobQueue.add(async () => {
                    await call(bobHapps, "fast_forward_signal", signal.payload);
                }).catch((e) => {
                    console.log("Error in signal bob queue", e);
                })
            }
        }
    })

    let aliceConductor = aliceHapps.conductor
    let bobConductor = bobHapps.conductor;
    let hash = Buffer.from((await aliceConductor.adminWs().listDnas())[0]);

    await addAllAgentsToAllConductors([aliceConductor, bobConductor]);

    console.log("==============================================")
    console.log("=================START========================")
    console.log("==============================================")
    for(let i=0; i < 10; i++) {
        console.log("-------------------------");
        console.log("Iteration: ", i)
        console.log("-------------------------");
        const start = hrtime.bigint();
        await Promise.all([
            createLinks(aliceHapps, "alice", 20, aliceQueue),
            createLinks(bobHapps, "bob", 20, bobQueue)
        ])
        const end = hrtime.bigint();
        console.log(`Creating links took ${divide(end - start, 1000000)} ms`);

        console.log("-------------------------");
        console.log("Created 20 links each (Alice and Bob)");
        console.log("waiting a second");
        console.log("-------------------------");

        await waitDhtConsistency(hash, aliceConductor);
        await waitDhtConsistency(hash, bobConductor);

        console.log("-------------------------");
        console.log("Now pulling on both agents...");
        console.log("-------------------------");

        let pullSuccessful = false
        while(!pullSuccessful) {
            try {
                const startA = hrtime.bigint();
                await aliceQueue.add(async () => {
                    await call(aliceHapps, "pull")
                });
                const endA = hrtime.bigint();
                console.log(`Alice pull took ${divide(endA - startA, 1000000)} ms`);

                const startB = hrtime.bigint();
                await bobQueue.add(async () => {
                    await call(bobHapps, "pull");
                });
                const endB = hrtime.bigint();
                console.log(`Bob pull took ${divide(endB - startB,1000000)} ms`);

                //await call(aliceHapps, "pull");
                //await call(bobHapps, "pull");
                pullSuccessful = true
            } catch(e) {
                console.error("Pulling failed with error:", e)
                await waitDhtConsistency(hash, aliceConductor);
                await waitDhtConsistency(hash, bobConductor);
            }
        }
        
        await waitDhtConsistency(hash, aliceConductor);
        await waitDhtConsistency(hash, bobConductor);
        

        //let alice_latest_revision = await call(aliceHapps, "latest_revision")
        //let bob_latest_revision = await call(bobHapps, "latest_revision")
        let alice_current_revision = await call(aliceHapps, "current_revision")
        let bob_current_revision = await call(bobHapps, "current_revision")

        //@ts-ignore
        //t.isEqual(alice_latest_revision.toString("base64"), bob_latest_revision.toString("base64"))
        //@ts-ignore
        //t.isEqual(alice_current_revision.toString("base64"), bob_current_revision.toString("base64"))
        //@ts-ignore
        //t.isEqual(alice_latest_revision.toString("base64"), alice_current_revision.toString("base64"))

        console.log("-------------------------");
        console.log("All good :)))))))))))))))");
        console.log("-------------------------");

    }

    // Wait for gossip of latest_revision, needed for render
    await waitDhtConsistency(hash, aliceConductor);
    await waitDhtConsistency(hash, bobConductor);

    const startRenderA = hrtime.bigint();
    await call(aliceHapps, "pull");
    let alice_rendered = await call(aliceHapps, "render") as Perspective
    const endRenderA = hrtime.bigint();
    console.log(`Alice pull + render took ${divide(endRenderA - startRenderA, 1000000)} ms`);

    // Wait for gossip of latest_revision, needed for render
    await waitDhtConsistency(hash, aliceConductor);
    await waitDhtConsistency(hash, bobConductor);

    const startRenderB = hrtime.bigint();
    await call(bobHapps, "pull");
    let bob_rendered = await call(bobHapps, "render") as Perspective
    const endRenderB = hrtime.bigint();
    console.log(`Bob pull + render took ${divide(endRenderB - startRenderB, 1000000)} ms`);

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
    t.end()
})
