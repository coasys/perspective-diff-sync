import type { Address, Language, Interaction, HolochainLanguageDelegate, LanguageContext } from "@perspect3vism/ad4m";
import { LinkAdapter } from "./linksAdapter";
import { DNA, DNA_NICK } from "./dna";

function interactions(expression: Address): Interaction[] {
  return [];
}

const activeAgentDurationSecs = 300;

//@ad4m-template-variable
const name = "perspective-diff-sync";

export default async function create(context: LanguageContext): Promise<Language> {
  const Holochain = context.Holochain as HolochainLanguageDelegate;

  const linksAdapter = new LinkAdapter(context);

  await Holochain.registerDNAs(
    [{ file: DNA, nick: DNA_NICK }],
    (signal) => { linksAdapter.handleHolochainSignal(signal) }
  );

  await linksAdapter.addActiveAgentLink(Holochain);
  setInterval(
    async () => await linksAdapter.addActiveAgentLink.bind(Holochain),
    activeAgentDurationSecs * 1000
  );

  return {
    name,
    linksAdapter,
    interactions,
  } as Language;
}
