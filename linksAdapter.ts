import { LinkSyncAdapter, PerspectiveDiffObserver, HolochainLanguageDelegate, LanguageContext, PerspectiveDiff, LinkExpression, DID } from "@perspect3vism/ad4m";
import { Perspective } from "@perspect3vism/ad4m";
import { DNA_NICK, ZOME_NAME } from "./dna";

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

class PeerInfo {
  currentRevision: string;
  lastSeen: Date;
};

export class LinkAdapter implements LinkSyncAdapter {
  hcDna: HolochainLanguageDelegate;
  linkCallback?: PerspectiveDiffObserver
  peers: Map<DID, PeerInfo> = new Map();
  me: DID

  constructor(context: LanguageContext) {
    //@ts-ignore
    this.hcDna = context.Holochain as HolochainLanguageDelegate;
    this.me = context.agent.did;
  }

  writable(): boolean {
    return true;
  }

  public(): boolean {
    return false;
  }

  async others(): Promise<DID[]> {
    return await this.hcDna.call(DNA_NICK, ZOME_NAME, "get_others", null);
  }

  async latestRevision(): Promise<string> {
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "current_revision", null);
    return res as string;
  }

  async currentRevision(): Promise<string> {
    let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "current_revision", null);
    return res as string;
  }

  async pull(): Promise<PerspectiveDiff> {
    await this.hcDna.call(DNA_NICK, ZOME_NAME, "sync", null);
    await this.gossip();
    return new PerspectiveDiff()
  }

  async gossip() {
    let lostPeers: DID[] = [];

    this.peers.forEach( (peerInfo, peer) => {
      if (peerInfo.lastSeen.getTime() + 10000 < new Date().getTime()) {
        lostPeers.push(peer);
      }
    });

    for (const peer of lostPeers) {
      this.peers.delete(peer);
    }

    // flatten the map into an array of peers
    let peers = Array.from(this.peers.keys());
    peers.push(this.me);
    
    // Lexically sort the peers
    peers.sort();

    // If we are the first peer, we are the scribe
    let is_scribe = peers[0] == this.me;
    
    // Get a deduped set of all peer's current revisions
    let revisions = new Set<string>();
    for (const peer of peers) {
      revisions.add(this.peers.get(peer)?.currentRevision);
    }

    revisions.forEach( async (hash) => {
      await this.hcDna.call(DNA_NICK, ZOME_NAME, "pull", { 
        hash,
        is_scribe 
      });
    })
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
    let p = signal.payload;
    //Check if this signal came from another agent & contains a diff and reference_hash
    if (p.diff && p.reference_hash && p.reference && p.broadcast_author) {
      this.peers.set(p.broadcast_author, { currentRevision: p.reference_hash, lastSeen: new Date() });
    } else {
      console.log("PerspectiveDiffSync.handleHolochainSignal: received a signals from ourselves in fast_forward_signal or in a pull");
      //This signal only contains link data and no reference, and therefore came from us in a pull in fast_forward_signal
      if (this.linkCallback) {
        this.linkCallback(signal.payload);
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
