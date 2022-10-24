import type { Address, Language, Interaction, HolochainLanguageDelegate, LanguageContext } from "@perspect3vism/ad4m";
import { LinkAdapter } from "./linksAdapter";
import { DNA, DNA_NICK } from "./dna";

function interactions(expression: Address): Interaction[] {
  return [];
}

const activeAgentDurationSecs = 3600;

//@ad4m-template-variable
const name = "perspective-diff-sync";

export default async function create(context: LanguageContext): Promise<Language> {
  const Holochain = context.Holochain as HolochainLanguageDelegate;

  const linksAdapter = new LinkAdapter(context);

  await Holochain.registerDNAs(
    [{ file: DNA, nick: DNA_NICK }],
    async (signal) => { await linksAdapter.handleHolochainSignal(signal) }
  );

  await linksAdapter.addActiveAgentLink(Holochain);
  const activeAgentInterval = setInterval(
    async () => {
      console.log("===Perspective-diff-sync: attempting to add a new active agent link");
      await linksAdapter.addActiveAgentLink(Holochain)
    },
    activeAgentDurationSecs * 1000
  );

  const teardown = function() {
    clearInterval(activeAgentInterval);
  }

  return {
    name,
    linksAdapter,
    interactions,
    teardown
  } as Language;
}
