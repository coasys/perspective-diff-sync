import type { Address, Language, Interaction, HolochainLanguageDelegate, LanguageContext } from "@perspect3vism/ad4m";
import { LinkAdapter } from "./linksAdapter";
import { DNA, DNA_NICK, ZOME_NAME } from "./dna";

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
    //@ts-ignore
    [{ file: DNA, nick: DNA_NICK, zomeCalls: 
      [
        [ZOME_NAME, "latest_revision"],
        [ZOME_NAME, "current_revision"],
        [ZOME_NAME, "pull"],
        [ZOME_NAME, "render"],
        [ZOME_NAME, "commit"],
        [ZOME_NAME, "fast_forward_signal"]
      ]
    }],
    async (signal) => { await linksAdapter.handleHolochainSignal(signal) }
  );

  return {
    name,
    linksAdapter,
    interactions
  } as Language;
}
