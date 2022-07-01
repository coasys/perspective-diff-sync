// import { testRevisionUpdates } from "./revisions"
// import { unSyncFetch, mergeFetch, complexMerge } from "./pull";
// import { signals } from "./signals";
// import { render, renderMerges } from "./render";

import { unSyncFetch, mergeFetch } from "./pull";
import { testRevisionUpdates } from "./revisions";
import test from "tape-promise/tape.js";

// test("unsynced fetch", async (t) => {
//     await unSyncFetch(t);
// })

test("merge fetch", async (t) => {
    await mergeFetch(t);
})

// test("test revision updates", async (t) => {
//     await testRevisionUpdates(t);
// })


// let orchestrator = new Orchestrator()

// testRevisionUpdates(orchestrator)
// orchestrator.run()

// orchestrator = new Orchestrator()

// unSyncFetch(orchestrator)
// orchestrator.run()

// orchestrator = new Orchestrator()

// mergeFetch(orchestrator)
// orchestrator.run()

// orchestrator = new Orchestrator()

// complexMerge(orchestrator)
// orchestrator.run()

// orchestrator = new Orchestrator()

// signals(orchestrator)
// orchestrator.run()

// orchestrator = new Orchestrator();
// render(orchestrator)
// orchestrator.run()

// orchestrator = new Orchestrator();
// renderMerges(orchestrator)
// orchestrator.run()

// // Run all registered scenarios as a final step, and gather the report,
// // if you set up a reporter
// const report = orchestrator.run()

// // Note: by default, there will be no report
// console.log(report)