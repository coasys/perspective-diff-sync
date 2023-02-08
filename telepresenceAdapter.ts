import type { TelepresenceAdapter, OnlineAgent, PerspectiveExpression, TelepresenceSignalCallback, HolochainLanguageDelegate, LanguageContext } from "@perspect3vism/ad4m";
import { DNA_NICK, ZOME_NAME } from "./dna";

export class TelepresenceAdapterImplementation implements TelepresenceAdapter {
    hcDna: HolochainLanguageDelegate;
    signalCallbacks: TelepresenceSignalCallback[] = [];

    constructor(context: LanguageContext) {
        this.hcDna = context.Holochain as HolochainLanguageDelegate;
    }

    async setOnlineStatus(status: PerspectiveExpression): Promise<void> {
        await this.hcDna.call(DNA_NICK, ZOME_NAME, "set_online_status", status);
    }

    async getOnlineAgents(): Promise<OnlineAgent[]> {
        let onlineAgents = await this.hcDna.call(DNA_NICK, ZOME_NAME, "get_online_agents", null);
        return onlineAgents as OnlineAgent[];
    }

    async sendSignal(remoteAgentDid: string, payload: PerspectiveExpression): Promise<object> {
        let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "send_signal", {remote_agent_did: remoteAgentDid, payload});
        return res;
    }

    async sendBroadcast(payload: PerspectiveExpression): Promise<object> {
        let res = await this.hcDna.call(DNA_NICK, ZOME_NAME, "send_broadcast", payload);
        return res;
    }

    async registerSignalCallback(callback: TelepresenceSignalCallback): Promise<void> {
        this.signalCallbacks.push(callback);
    }
}