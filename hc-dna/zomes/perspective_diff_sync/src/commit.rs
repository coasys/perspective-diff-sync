use chrono::{DateTime, NaiveDateTime, Utc};
use hc_time_index::SearchStrategy;
use hdk::prelude::*;

use crate::errors::{SocialContextError, SocialContextResult};
use crate::pull::pull;
use crate::revisions::{current_revision, latest_revision};
use crate::snapshots::{generate_snapshot, get_entries_since_snapshot};
use crate::utils::{dedup, get_now};
use crate::{
    AgentReference, PerspectiveDiff, PerspectiveDiffEntryReference, ACTIVE_AGENT_DURATION,
    ENABLE_SIGNALS, SNAPSHOT_INTERVAL,
};

#[cfg(feature = "prod")]
use crate::revisions::update_current_revision;
#[cfg(feature = "prod")]
use crate::revisions::update_latest_revision;


pub fn commit(
    diff: PerspectiveDiff,
) -> SocialContextResult<HoloHash<holo_hash::hash_type::Header>> {
    let pre_current_revision = current_revision()?;
    let pre_latest_revision = latest_revision()?;
    let mut entries_since_snapshot = 0;

    if pre_current_revision != pre_latest_revision {
        //TODO; should any data that gets pulled here be returned by the commit function?
        //Otherwise how will the UI ever get it if we have already pulled and updated
        //maybe ommit as a signal?
        let new_diffs = pull()?;
        emit_signal(new_diffs)?;
        if pre_latest_revision.is_some() {
            entries_since_snapshot = get_entries_since_snapshot(latest_revision()?.ok_or(
                SocialContextError::InternalError("Expected to have latest revision"),
            )?)?;
        };
    } else {
        if pre_latest_revision.is_some() {
            entries_since_snapshot =
                get_entries_since_snapshot(pre_latest_revision.clone().unwrap())?;
        };
    }
    debug!("Entries since snapshot: {:#?}", entries_since_snapshot);
    //Add one since we are comitting an entry here
    entries_since_snapshot += 1;

    let parent = current_revision()?;
    debug!("Parent entry is: {:#?}", parent);
    let diff_entry_create = create_entry(diff.clone())?;
    debug!("Created diff entry: {:#?}", diff_entry_create);
    let diff_entry_ref_entry = PerspectiveDiffEntryReference {
        diff: diff_entry_create.clone(),
        parents: parent.map(|val| vec![val]),
    };
    let diff_entry_reference = create_entry(diff_entry_ref_entry.clone())?;
    debug!("Created diff entry ref: {:#?}", diff_entry_reference);

    if pre_latest_revision.is_some() && entries_since_snapshot >= *SNAPSHOT_INTERVAL {
        //fetch all the diff's, we need a new function which will traverse graph and then return + diffs + next found snapshot
        //create new snapshot linked from above diff_entry_reference
        let snapshot = generate_snapshot(diff_entry_reference.clone())?;
        debug!("Creating snapshot");

        create_entry(snapshot.clone())?;
        create_link(
            hash_entry(diff_entry_ref_entry)?,
            hash_entry(snapshot)?,
            LinkTag::new("snapshot"),
        )?;
    };

    //This allows us to turn of revision updates when testing so we can artifically test pulling with varying agent states
    #[cfg(feature = "prod")]
    {
        let now = get_now()?;
        update_latest_revision(diff_entry_create.clone(), now.clone())?;
        update_current_revision(diff_entry_create, now)?;
    }

    if *ENABLE_SIGNALS {
        let now = sys_time()?.as_seconds_and_nanos();
        let now = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(now.0, now.1), Utc);
        //Get recent agents (agents which have marked themselves online in time period now -> ACTIVE_AGENT_DURATION as derived from DNA properties)
        let recent_agents = hc_time_index::get_links_and_load_for_time_span::<AgentReference>(
            String::from("active_agent"),
            now - *ACTIVE_AGENT_DURATION,
            now,
            Some(LinkTag::new("")),
            SearchStrategy::Bfs,
            None,
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
    let recent_agents = hc_time_index::get_links_and_load_for_time_span::<AgentReference>(
        String::from("active_agent"),
        now - *ACTIVE_AGENT_DURATION,
        now,
        Some(LinkTag::new("")),
        SearchStrategy::Bfs,
        None,
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
            create_entry(&new_agent_ref)?;
            hc_time_index::index_entry(
                String::from("active_agent"),
                new_agent_ref,
                LinkTag::new(""),
            )?;
            Ok(Some(agent_ref.timestamp))
        }
        None => {
            //Agent is not marked online so lets add an online agent reference
            let agent_ref = AgentReference {
                agent: agent_info()?.agent_initial_pubkey,
                timestamp: now,
            };
            create_entry(&agent_ref)?;
            hc_time_index::index_entry(String::from("active_agent"), agent_ref, LinkTag::new(""))?;
            Ok(None)
        }
    }
}
