import { Config, InstallAgentsHapps } from '@holochain/tryorama'
import path from "path";

export const conductorConfig = Config.gen();

export const installation: InstallAgentsHapps = [
  // agent 0
  [
    // happ 0
    [path.join("../../workdir/perspective-diff-sync.dna")]
  ]
]
