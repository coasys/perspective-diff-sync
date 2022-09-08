use chrono::{DateTime, NaiveDateTime, Utc};
use hc_time_index::SearchStrategy;
use hdk::prelude::*;
use perspective_diff_sync_integrity::{
    AgentReference, EntryTypes, LinkTypes, PerspectiveDiff, PerspectiveDiffEntryReference,
};

//use crate::errors::SocialContextError;
use crate::errors::SocialContextResult;
//use crate::pull::pull;
use crate::revisions::{current_revision, update_current_revision, update_latest_revision};
use crate::snapshots::generate_snapshot;
use crate::utils::{dedup, get_now};
use crate::{ACTIVE_AGENT_DURATION, ENABLE_SIGNALS, SNAPSHOT_INTERVAL};
use crate::retriever::PerspectiveDiffRetreiver;

pub fn commit<Retriever: PerspectiveDiffRetreiver>(
    diff: PerspectiveDiff,
) -> SocialContextResult<HoloHash<holo_hash::hash_type::Action>> {
    let now = get_now()?.time();
    let pre_current_revision = current_revision::<Retriever>()?;
    let after = get_now()?.time();
    debug!("Took {} to get current revision", (after - now).num_milliseconds());

    //if pre_current_revision != pre_latest_revision {
    //    let new_diffs = pull::<Retriever>()?;
    //    emit_signal(new_diffs)?;
    //    if pre_latest_revision.is_some() {
    //        entries_since_snapshot = get_entries_since_snapshot(latest_revision::<Retriever>()?.ok_or(
    //            SocialContextError::InternalError("Expected to have latest revision"),
    //        )?)?;
    //    };
    //} else {

    let mut entries_since_snapshot = 0;
    if pre_current_revision.is_some() {
        let current = Retriever::get::<PerspectiveDiffEntryReference>(pre_current_revision.clone().unwrap().hash)?;
        entries_since_snapshot = current.diffs_since_snapshot;
    };
    debug!("Entries since snapshot: {:#?}", entries_since_snapshot);
    //Add one since we are comitting an entry here
    entries_since_snapshot += 1;

    let create_snapshot_here = if entries_since_snapshot >= *SNAPSHOT_INTERVAL {
        entries_since_snapshot = 0;
        true
    } else {
        false
    };

    let parent = current_revision::<Retriever>()?;
    debug!("Parent entry is: {:#?}", parent);
    debug!("CREATE_ENTRY PerspectiveDiff");
    let diff_entry_create = Retriever::create_entry(EntryTypes::PerspectiveDiff(diff.clone()))?;
    //debug!("Created diff entry: {:#?}", diff_entry_create);
    let diff_entry_ref_entry = PerspectiveDiffEntryReference {
        diff: diff_entry_create.clone(),
        parents: parent.map(|val| vec![val.hash]),
        diffs_since_snapshot: entries_since_snapshot,
    };
    debug!("CREATE_ENTRY PerspectiveDiffEntryReference");
    let diff_entry_reference = Retriever::create_entry(EntryTypes::PerspectiveDiffEntryReference(
        diff_entry_ref_entry.clone(),
    ))?;
    debug!("Created diff entry ref: {:#?}", diff_entry_reference);

    if create_snapshot_here {
        //fetch all the diff's, we need a new function which will traverse graph and then return + diffs + next found snapshot
        //create new snapshot linked from above diff_entry_reference
        let now = get_now()?.time();
        let snapshot = generate_snapshot(diff_entry_reference.clone())?;
        let after = get_now()?.time();
        debug!("Took {} to generate the snapshot", (after - now).num_milliseconds());
        debug!("Creating snapshot");

        debug!("CREATE_ENTRY Snapshot");
        Retriever::create_entry(EntryTypes::Snapshot(snapshot.clone()))?;
        create_link(
            hash_entry(diff_entry_ref_entry)?,
            hash_entry(snapshot)?,
            LinkTypes::Snapshot,
            LinkTag::new("snapshot"),
        )?;
    };

    let now = get_now()?;
    update_latest_revision::<Retriever>(diff_entry_reference.clone(), now.clone())?;
    update_current_revision::<Retriever>(diff_entry_reference.clone(), now)?;

    if *ENABLE_SIGNALS {
        let now = sys_time()?.as_seconds_and_nanos();
        let now = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(now.0, now.1), Utc);
        //Get recent agents (agents which have marked themselves online in time period now -> ACTIVE_AGENT_DURATION as derived from DNA properties)
        let recent_agents = hc_time_index::get_links_and_load_for_time_span::<
            AgentReference,
            LinkTypes,
            LinkTypes,
        >(
            String::from("active_agent"),
            now - *ACTIVE_AGENT_DURATION,
            now,
            Some(LinkTag::new("")),
            SearchStrategy::Bfs,
            None,
            LinkTypes::Index,
            LinkTypes::TimePath,
        )?;
        let recent_agents = recent_agents
            .into_iter()
            .map(|val| val.agent)
            .collect::<Vec<AgentPubKey>>();
        let recent_agents = dedup(&recent_agents);
        debug!(
            "Social-Context.add_link: Sending signal to agents: {:#?}",
            recent_agents
        );
        remote_signal(diff.get_sb()?, recent_agents)?;
    };

    Ok(diff_entry_reference)
}

pub fn add_active_agent_link() -> SocialContextResult<Option<DateTime<Utc>>> {
    let now = get_now()?;
    //Get the recent agents so we can check that the current agent is not already
    let recent_agents =
        hc_time_index::get_links_and_load_for_time_span::<AgentReference, LinkTypes, LinkTypes>(
            String::from("active_agent"),
            now - *ACTIVE_AGENT_DURATION,
            now,
            Some(LinkTag::new("")),
            SearchStrategy::Bfs,
            None,
            LinkTypes::Index,
            LinkTypes::TimePath,
        )?;

    let current_agent_online = recent_agents.iter().find(|agent| {
        agent.agent
            == agent_info()
                .expect("Could not get agent info")
                .agent_latest_pubkey
    });
    match current_agent_online {
        Some(agent_ref) => {
            //If the agent is already marked online then return the timestamp of them being online so the zome caller can add another active_agent link at the correct time in the future
            //But for now this is TODO and we will just add an agent reference anyway
            let new_agent_ref = AgentReference {
                agent: agent_info()?.agent_initial_pubkey,
                timestamp: now,
            };
            debug!("CREATE_ENTRY AgentReference");
            create_entry(&EntryTypes::AgentReference(new_agent_ref.clone()))?;
            hc_time_index::index_entry(
                String::from("active_agent"),
                new_agent_ref,
                LinkTag::new(""),
                LinkTypes::Index,
                LinkTypes::TimePath,
            )?;
            Ok(Some(agent_ref.timestamp))
        }
        None => {
            //Agent is not marked online so lets add an online agent reference
            let agent_ref = AgentReference {
                agent: agent_info()?.agent_initial_pubkey,
                timestamp: now,
            };
            debug!("CREATE_ENTRY AgentReference");
            create_entry(&EntryTypes::AgentReference(agent_ref.clone()))?;
            hc_time_index::index_entry(
                String::from("active_agent"),
                agent_ref,
                LinkTag::new(""),
                LinkTypes::Index,
                LinkTypes::TimePath,
            )?;
            Ok(None)
        }
    }
}
