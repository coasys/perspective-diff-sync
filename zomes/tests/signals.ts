import { conductorConfig, installation } from './common';
import {sleep, generate_link_expression} from "./utils";


export function signals(orchestrator) {
    orchestrator.registerScenario("test signals", async (s, t) => {
        const [alice, bob] = await s.players([conductorConfig, conductorConfig])
        const [[alice_happ]] = await alice.installAgentsHapps(installation)
        const [[bob_happ]] = await bob.installAgentsHapps(installation)
        await s.shareAllNodes([alice, bob])
      
        let aliceSignalCount = 0;
        let bobSignalCount = 0;
        alice.setSignalHandler((signal) => {
            console.log("Alice Received Signal:",signal)
            aliceSignalCount += 1;
        });
        bob.setSignalHandler((signal) => {
            console.log("Bob Received Signal:",signal)
            bobSignalCount += 1;
        });
        //Create another index for one day ago
        var dateOffset = (24*60*60*1000) / 2; //12 hr ago
        var date = new Date();
        date.setTime(date.getTime() - dateOffset);
      
        //Register as active agent
        await alice_happ.cells[0].call("social_context", "add_active_agent_link")
      
        //Register as active agent
        await bob_happ.cells[0].call("social_context", "add_active_agent_link")
      
        //Sleep to give time for bob active agent link to arrive at alice
        await sleep(200)
      
        //Test case where subject object and predicate are given
        let link_data = generate_link_expression("alice");
        await alice_happ.cells[0].call("social_context", "commit", {additions: [link_data], removals: []});
        //Sleep to give time for signals to arrive
        await sleep(2000)
      
        t.deepEqual(aliceSignalCount, 1);
        t.deepEqual(bobSignalCount, 1);
      })
      
}