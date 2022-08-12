import { AgentHapp, CallableCell, Conductor } from "@holochain/tryorama";
import faker from "faker";
import { dnas } from './common';
import { createConductor } from "@holochain/tryorama";

export function generate_link_expression(agent: string) {
    return {
      data: {source: faker.name.findName(), target: faker.name.findName(), predicate: faker.name.findName()},
      author: agent, 
      timestamp: new Date().toISOString(), 
      proof: {signature: "sig", key: "key"},
   }
}

export async function create_link_expression(cell: CallableCell, agent: string, update_local_revision: boolean, update_dht_revision: boolean): Promise<{commit: string, data: any}> {
    let link_data = generate_link_expression(agent);
    let commit = await cell.callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "commit", 
        payload: {additions: [link_data], removals: []}
    });
    //@ts-ignore
    console.warn("\nCreated link expression with commit hash: ", commit.toString("base64"));
    await cell.callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_latest_revision", 
        payload: commit
    });
    await cell.callZome({
        zome_name: "perspective_diff_sync", 
        fn_name: "update_current_revision", 
        payload: commit
    });
    //@ts-ignore
    return {commit: commit.toString("base64"), data: link_data}
}

export function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

export async function createConductors(num: number): Promise<{agent_happ: AgentHapp, conductor: Conductor}[]> {
    let out = [] as {agent_happ: AgentHapp, conductor: Conductor}[];
    for (let n of Array(num).keys()) {
        let conductor = await createConductor();
        let [happ] = await conductor.installAgentsHapps({
            agentsDnas: [{dnas}],
        });
        out.push({
            agent_happ: happ,
            conductor
        })
    }
    return out
}