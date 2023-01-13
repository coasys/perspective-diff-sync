use hdk::prelude::*;
use perspective_diff_sync_integrity::{
    Anchor, EntryTypes, LinkTypes, PerspectiveDiff, PerspectiveDiffEntryReference,
    PerspectiveDiffReference,
};

//use crate::errors::SocialContextError;
use crate::errors::SocialContextResult;
//use crate::pull::pull;
use crate::retriever::PerspectiveDiffRetreiver;
use crate::revisions::{current_revision, update_current_revision, update_latest_revision};
use crate::snapshots::generate_snapshot;
use crate::utils::{dedup, get_now};
use crate::{ENABLE_SIGNALS, SNAPSHOT_INTERVAL};

pub fn commit<Retriever: PerspectiveDiffRetreiver>(
    diff: PerspectiveDiff,
) -> SocialContextResult<HoloHash<holo_hash::hash_type::Action>> {
    debug!("===PerspectiveDiffSync.commit(): Function start");
    let now_fn_start = get_now()?.time();
    let current_revision = current_revision::<Retriever>()?;

    let mut entries_since_snapshot = 0;
    if current_revision.is_some() {
        let current = Retriever::get::<PerspectiveDiffEntryReference>(
            current_revision.clone().unwrap().hash,
        )?;
        entries_since_snapshot = current.diffs_since_snapshot;
    };
    debug!(
        "===PerspectiveDiffSync.commit(): Entries since snapshot: {:#?}",
        entries_since_snapshot
    );
    //Add one since we are comitting an entry here
    entries_since_snapshot += 1;

    let create_snapshot_here = if entries_since_snapshot >= *SNAPSHOT_INTERVAL {
        entries_since_snapshot = 0;
        true
    } else {
        false
    };

    let now = get_now()?.time();
    let diff_entry_create = Retriever::create_entry(EntryTypes::PerspectiveDiff(diff.clone()))?;
    let diff_entry_ref_entry = PerspectiveDiffEntryReference {
        diff: diff_entry_create.clone(),
        parents: current_revision.map(|val| vec![val.hash]),
        diffs_since_snapshot: entries_since_snapshot,
    };
    let diff_entry_reference = Retriever::create_entry(EntryTypes::PerspectiveDiffEntryReference(
        diff_entry_ref_entry.clone(),
    ))?;
    let after = get_now()?.time();
    debug!(
        "===PerspectiveDiffSync.commit(): Created diff entry ref: {:#?}",
        diff_entry_reference
    );
    debug!(
        "===PerspectiveDiffSync.commit() - Profiling: Took {} to create a PerspectiveDiff",
        (after - now).num_milliseconds()
    );

    if create_snapshot_here {
        //fetch all the diff's, we need a new function which will traverse graph and then return + diffs + next found snapshot
        //create new snapshot linked from above diff_entry_reference
        let snapshot = generate_snapshot(diff_entry_reference.clone())?;

        let now = get_now()?.time();
        Retriever::create_entry(EntryTypes::Snapshot(snapshot.clone()))?;
        create_link(
            hash_entry(diff_entry_ref_entry.clone())?,
            hash_entry(snapshot)?,
            LinkTypes::Snapshot,
            LinkTag::new("snapshot"),
        )?;
        let after = get_now()?.time();
        debug!("===PerspectiveDiffSync.commit() - Profiling: Took {} to create snapshot entry and link", (after - now).num_milliseconds());
    };

    let now = get_now()?;
    let now_profile = get_now()?.time();
    update_latest_revision::<Retriever>(diff_entry_reference.clone(), now.clone())?;
    let after = get_now()?.time();
    debug!(
        "===PerspectiveDiffSync.commit() - Profiling: Took {} to update the latest revision",
        (after - now_profile).num_milliseconds()
    );
    update_current_revision::<Retriever>(diff_entry_reference.clone(), now)?;

    if *ENABLE_SIGNALS {
        let now_profile = get_now()?.time();
        let recent_agents = get_links(
            hash_entry(get_active_agent_anchor())?,
            LinkTypes::Index,
            Some(LinkTag::new("active_agent")),
        )?;

        let after = get_now()?.time();
        debug!(
            "===PerspectiveDiffSync.commit() - Profiling: Took {} to get the active agents",
            (after - now_profile).num_milliseconds()
        );
        let recent_agents = recent_agents
            .into_iter()
            .map(|val| AgentPubKey::from(EntryHash::from(val.target)))
            .collect::<Vec<AgentPubKey>>();

        //Dedup the agents
        let mut recent_agents = dedup(&recent_agents);
        //Remove ourself from the agents
        let me = agent_info()?.agent_latest_pubkey;
        let index = recent_agents.iter().position(|x| *x == me);
        if let Some(index) = index {
            recent_agents.remove(index);
        }

        debug!(
            "Social-Context.add_link: Sending signal to agents: {:#?}",
            recent_agents
        );

        let now = get_now()?.time();
        let signal_data = PerspectiveDiffReference {
            diff,
            reference: diff_entry_ref_entry,
            reference_hash: diff_entry_reference.clone(),
        };
        remote_signal(signal_data.get_sb()?, recent_agents)?;
        let after = get_now()?.time();
        debug!(
            "===PerspectiveDiffSync.commit() - Profiling: Took {} to send signal to active agents",
            (after - now).num_milliseconds()
        );
    };

    let after_fn_end = get_now()?.time();
    debug!(
        "===PerspectiveDiffSync.commit() - Profiling: Took {} to complete whole commit function",
        (after_fn_end - now_fn_start).num_milliseconds()
    );
    Ok(diff_entry_reference)
}

pub fn add_active_agent_link<Retriever: PerspectiveDiffRetreiver>() -> SocialContextResult<()> {
    debug!("===PerspectiveDiffSync.add_active_agent_link(): Function start");
    let now_fn_start = get_now()?.time();
    let agent_root_entry = get_active_agent_anchor();
    let _agent_root_entry_action =
        Retriever::create_entry(EntryTypes::Anchor(agent_root_entry.clone()))?;

    let agent = agent_info()?.agent_initial_pubkey;
    create_link(
        hash_entry(agent_root_entry)?,
        agent,
        LinkTypes::Index,
        LinkTag::new("active_agent"),
    )?;
    let after_fn_end = get_now()?.time();
    debug!("===PerspectiveDiffSync.add_active_agent_link() - Profiling: Took {} to complete whole add_active_agent_link()", (after_fn_end - now_fn_start).num_milliseconds());
    Ok(())
}

fn get_active_agent_anchor() -> Anchor {
    Anchor("active_agent".to_string())
}
