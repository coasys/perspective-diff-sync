import { Orchestrator } from '@holochain/tryorama'
import { testRevisionUpdates } from "./revisions"
import { unSyncFetch, mergeFetch, complexMerge } from "./pull";
import { signals } from "./signals";

let orchestrator = new Orchestrator()

testRevisionUpdates(orchestrator)
orchestrator.run()

orchestrator = new Orchestrator()

unSyncFetch(orchestrator)
orchestrator.run()

orchestrator = new Orchestrator()

mergeFetch(orchestrator)
orchestrator.run()

orchestrator = new Orchestrator()

complexMerge(orchestrator)
orchestrator.run()

orchestrator = new Orchestrator()

signals(orchestrator)
orchestrator.run()

// // Run all registered scenarios as a final step, and gather the report,
// // if you set up a reporter
// const report = orchestrator.run()

// // Note: by default, there will be no report
// console.log(report)