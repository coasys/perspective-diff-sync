import type { TelepresenceAdapter, OnlineAgent, PerspectiveExpression, TelepresenceSignalCallback, HolochainLanguageDelegate, LanguageContext } from "@perspect3vism/ad4m";

export class TelepresenceAdapterImplementation implements TelepresenceAdapter {
    hcDna: HolochainLanguageDelegate;
    signalCallbacks: TelepresenceSignalCallback[] = [];

    constructor(context: LanguageContext) {
        this.hcDna = context.Holochain as HolochainLanguageDelegate;
    }

    async setOnlineStatus(status: PerspectiveExpression): Promise<void> {
        return Promise.resolve();
    }

    async getOnlineAgents(): Promise<OnlineAgent[]> {
        return Promise.resolve([]);
    }

    async sendSignal(remoteAgentDid: string, payload: PerspectiveExpression): Promise<object> {
        return Promise.resolve({});
    }

    async sendBroadcast(payload: PerspectiveExpression): Promise<object> {
        return Promise.resolve({});
    }

    async registerSignalCallback(callback: TelepresenceSignalCallback): Promise<void> {
        this.signalCallbacks.push(callback);
    }
}