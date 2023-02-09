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
        const onlineAgents = [];
        const getActiveAgents = await this.hcDna.call(DNA_NICK, ZOME_NAME, "get_active_agents", null);
        for (const activeAgent of getActiveAgents) {
            await new Promise((resolve) => {
                setTimeout(async () => {
                    const getOnlineStatus = await this.hcDna.call(DNA_NICK, ZOME_NAME, "get_online_status", activeAgent.agent_did);
                    if (getOnlineStatus) {
                        onlineAgents.push(getOnlineStatus);
                    };
                    resolve("")
                }, 1000)
            })
        }
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