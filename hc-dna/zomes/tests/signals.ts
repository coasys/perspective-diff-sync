import { Scenario } from "@holochain/tryorama";
import { sleep, generate_link_expression } from "./utils";
import { dnas } from "./common";
import test from "tape-promise/tape.js";

//@ts-ignore
export async function signals(t) {
    const scenario = new Scenario();
    let aliceSignalCount = 0;
    let bobSignalCount = 0;
    
    const aliceHapps = await scenario.addPlayerWithHapp({
        dnas: dnas, 
        signalHandler: (signal) => {
            console.log("Alice Received Signal:",signal)
            aliceSignalCount += 1;
        }
    });
    const bobHapps = await scenario.addPlayerWithHapp({
        dnas: dnas, 
        signalHandler: (signal) => {
            console.log("Bob Received Signal:",signal)
            bobSignalCount += 1;
        }
    });

    await scenario.shareAllAgents();
    //Register as active agent
    await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "add_active_agent_link"
    })
    
    //Register as active agent
    await bobHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "add_active_agent_link"
    })
    
    //Sleep to give time for bob active agent link to arrive at alice
    await sleep(500)
    
    //Test case where subject object and predicate are given
    let link_data = generate_link_expression("alice");
    await aliceHapps.cells[0].callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "commit", 
        payload: {additions: [link_data], removals: []}
    });
    //Sleep to give time for signals to arrive
    await sleep(1000)
    
    t.deepEqual(bobSignalCount, 1);

    await scenario.cleanUp();
}

test("signals", async (t) => {
    await signals(t)
    t.end()
})