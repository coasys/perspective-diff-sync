import type { Expression, LinkSyncAdapter, NewDiffObserver, HolochainLanguageDelegate, LanguageContext, PerspectiveDiff } from "@perspect3vism/ad4m";
import type { DID } from "@perspect3vism/ad4m/lib/DID";
import { Perspective } from "@perspect3vism/ad4m";
import { DNA_NICK, ZOME_NAME } from "./dna";

export class LinkAdapter implements LinkSyncAdapter {
  hcDna: HolochainLanguageDelegate;
  linkCallback?: NewDiffObserver

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
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "latestRevision", {});
    return res as string;
  }

  async currentRevision(): Promise<string> {
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "currentRevision", {});
    return res as string;
  }

  async pull(): Promise<PerspectiveDiff> {
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "pull", {});
    return res as PerspectiveDiff;
  }

  async render(): Promise<Perspective> {
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "render", {});
    return new Perspective(res.links);
  }

  async commit(diff: PerspectiveDiff): Promise<string> {
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "commit", diff);
    return res as string;
  }

  addCallback(callback: NewDiffObserver): number {
    this.linkCallback = callback;
    return 1;
  }

  handleHolochainSignal(signal: any): void {
    if (this.linkCallback) {
      this.linkCallback(signal.data.payload.diff, signal.data.payload.revision);
    }
  }

  async addActiveAgentLink(hcDna: HolochainLanguageDelegate): Promise<any> {
    if (hcDna == undefined) {
      //@ts-ignore
      return await this.call(
        DNA_NICK,
        ZOME_NAME,
        "add_active_agent_link",
        null
      );
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

function prepareExpressionLink(link: Expression): object {
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
  return data;
}
