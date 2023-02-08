import type { Address, Language, Interaction, HolochainLanguageDelegate, LanguageContext } from "@perspect3vism/ad4m";
import { LinkAdapter } from "./linksAdapter";
import { TelepresenceAdapterImplementation } from "./telepresenceAdapter";
import { DNA, DNA_NICK, ZOME_NAME } from "./dna";

function interactions(expression: Address): Interaction[] {
  return [];
}

//@ad4m-template-variable
const name = "perspective-diff-sync";

export default async function create(context: LanguageContext): Promise<Language> {
  const Holochain = context.Holochain as HolochainLanguageDelegate;

  const linksAdapter = new LinkAdapter(context);
  const telepresenceAdapter = new TelepresenceAdapterImplementation(context);

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
    async (signal) => { 
      //@ts-ignore
      if (signal.payload.diff) {
        await linksAdapter.handleHolochainSignal(signal)
      } else {
        for (const callback of telepresenceAdapter.signalCallbacks) {
          callback(signal.payload);
        }
      }
    }
  );

  return {
    name,
    linksAdapter,
    interactions,
    telepresenceAdapter
  } as Language;
}
