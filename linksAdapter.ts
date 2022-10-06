import type { LinkSyncAdapter, PerspectiveDiffObserver, HolochainLanguageDelegate, LanguageContext, PerspectiveDiff, LinkExpression } from "@perspect3vism/ad4m";
import type { DID } from "@perspect3vism/ad4m/lib/DID";
import { Perspective } from "@perspect3vism/ad4m";
import { DNA_NICK, ZOME_NAME } from "./dna";

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export class LinkAdapter implements LinkSyncAdapter {
  hcDna: HolochainLanguageDelegate;
  linkCallback?: PerspectiveDiffObserver

  constructor(context: LanguageContext) {
    //@ts-ignore
    this.hcDna = context.Holochain as HolochainLanguageDelegate;
  }

  writable(): boolean {
    return true;
  }

  public(): boolean {
    return false;
  }

  async others(): Promise<DID[]> {
    return []
  }

  async latestRevision(): Promise<string> {
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "latestRevision", null);
    return res as string;
  }

  async currentRevision(): Promise<string> {
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "currentRevision", null);
    return res as string;
  }

  async pull(): Promise<PerspectiveDiff> {
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "pull", null);
    return res as PerspectiveDiff;
  }

  async render(): Promise<Perspective> {
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "render", null);
    return new Perspective(res.links);
  }

  async commit(diff: PerspectiveDiff): Promise<string> {
    let prep_diff = {
      additions: diff.additions.map((diff) => prepareLinkExpression(diff)),
      removals: diff.removals.map((diff) => prepareLinkExpression(diff))
    }
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "commit", prep_diff);
    return res as string;
  }

  addCallback(callback: PerspectiveDiffObserver): number {
    this.linkCallback = callback;
    return 1;
  }

  async handleHolochainSignal(signal: any): Promise<void> {
    //Check if this signal came from another agent & contains a diff and reference_hash
    if (signal.data.payload.diff && signal.data.payload.reference_hash && signal.data.payload.reference) {
      console.log("PerspectiveDiffSync.handleHolochainSignal: received a signal from another agent, checking if we can fast forward to this signal");

      //First just emit the signal since we dont want to wait for the fast forward to finish
      if (this.linkCallback) {
        this.linkCallback(signal.data.payload.diff);
      }

      //wait 500ms to be sure we will find the diff in the agents data
      await sleep(500);
      //Note; when we have many signals coming in it could cause many fast forward to be build up in the dna request queue (since all DNA calls are sync) and thus block other calls from coming in
      await this.hcDna.call(DNA_NICK, ZOME_NAME, "fast_forward_signal", signal.data.payload);
    } else {
      console.log("PerspectiveDiffSync.handleHolochainSignal: received a signals from ourselves in fast_forward_signal");
      //This signal only contains link data and no reference, and therefore came from us in a pull in fast_forward_signal
      if (this.linkCallback) {
        this.linkCallback(signal.data.payload);
      }
    }
  }

  async addActiveAgentLink(hcDna: HolochainLanguageDelegate): Promise<any> {
    if (hcDna == undefined) {
      console.warn("===Perspective-diff-sync: Error tried to add an active agent link but received no hcDna to add the link onto");
    } else {
      return await hcDna.call(
        DNA_NICK,
        ZOME_NAME,
        "add_active_agent_link",
        null
      );
    }
  }
}

function prepareLinkExpression(link: LinkExpression): object {
  const data = Object.assign(link);
  if (data.data.source == "") {
    data.data.source = null;
  }
  if (data.data.target == "") {
    data.data.target = null;
  }
  if (data.data.predicate == "") {
    data.data.predicate = null;
  }
  if (data.data.source == undefined) {
    data.data.source = null;
  }
  if (data.data.target == undefined) {
    data.data.target = null;
  }
  if (data.data.predicate == undefined) {
    data.data.predicate = null;
  }
  return data;
}
