use hdk::prelude::*;
use chrono::{Utc, DateTime, NaiveDateTime};
use petgraph::graph::NodeIndex;
use hc_time_index::SearchStrategy;
use std::hash::Hash;

use crate::search;
use crate::{
    Perspective, PerspectiveDiff, LocalHashReference, HashAnchor, HashReference, PerspectiveDiffEntryReference, AgentReference, ACTIVE_AGENT_DURATION, ENABLE_SIGNALS,
    errors::{SocialContextResult, SocialContextError}
};

pub fn get_now() -> SocialContextResult<DateTime<Utc>> {
    let now = sys_time()?.as_seconds_and_nanos();
    Ok(DateTime::<Utc>::from_utc(
        NaiveDateTime::from_timestamp(now.0, now.1),
        Utc,
    ))
}

pub fn update_latest_revision(hash: HoloHash<holo_hash::hash_type::Header>, timestamp: DateTime<Utc>) -> SocialContextResult<()> {
    let hash_ref = HashReference {
        hash,
        timestamp
    };
    create_entry(hash_ref.clone())?;
    hc_time_index::index_entry(String::from("current_rev"), hash_ref, LinkTag::new(""))?;
    Ok(())
}

pub fn update_current_revision(hash: HoloHash<holo_hash::hash_type::Header>, timestamp: DateTime<Utc>) -> SocialContextResult<()> {
    let hash_anchor = hash_entry(HashAnchor(String::from("current_hashes")))?;
    let hash_ref = LocalHashReference {
        hash,
        timestamp
    };
    create_entry(hash_ref.clone())?;
    create_link(
        hash_anchor, 
        hash_entry(hash_ref)?, 
        LinkTag::new(String::from(""))
    )?;
    Ok(())
}

//Latest revision as seen from the DHT
pub fn latest_revision() -> SocialContextResult<Option<HoloHash<holo_hash::hash_type::Header>>> {
    let mut latest = hc_time_index::get_links_and_load_for_time_span::<HashReference>(
        String::from("current_rev"), get_now()?, DateTime::<Utc>::from_utc(
            NaiveDateTime::from_timestamp(0, 0),
            Utc,
        ), 
        None, 
        hc_time_index::SearchStrategy::Dfs,
        Some(1)
    )?;
    Ok(latest.pop().map(|val| val.hash))
}

//Latest revision as seen from our local state
pub fn current_revision() -> SocialContextResult<Option<HoloHash<holo_hash::hash_type::Header>>> {
    let hash_anchor = hash_entry(HashAnchor(String::from("current_hashes")))?;
    let links = get_links(hash_anchor.clone(), None)?;

    let mut refs = links.into_iter()
        .map(|link| match get(link.target, GetOptions::latest())? {
            Some(chunk) => Ok(Some(chunk.entry().to_app_option::<LocalHashReference>()?.ok_or(
                SocialContextError::InternalError("Expected element to contain app entry data"),
            )?)),
            None => Ok(None),
        })
        .filter_map(|val| {
            if val.is_ok() {
                let val = val.unwrap();
                if val.is_some() {
                    Some(Ok(val.unwrap()))
                } else {
                    None
                }
            } else {
                Some(Err(val.err().unwrap()))
            }
        })
        .collect::<SocialContextResult<Vec<LocalHashReference>>>()?;
    refs.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());

    Ok(refs.pop().map(|val| val.hash))
}

pub fn pull() -> SocialContextResult<PerspectiveDiff> {
    let latest = latest_revision()?;
    let current = current_revision()?;
    debug!("Pull made with latest: {:#?} and current: {:#?}", latest, current);

    if latest != current {
        if !latest.is_none() {
            let latest = latest.unwrap();

            //Populate the search algorithm
            let mut search = search::populate_search(None, latest.clone())?;

            if current.is_none() {
                let mut out = PerspectiveDiff {
                    additions: vec![],
                    removals: vec![]
                };
                for (_key, value) in search.entry_map.iter() {
                    let diff_entry = get(value.diff.clone(), GetOptions::latest())?.ok_or(SocialContextError::InternalError("Could not find diff entry for given diff entry reference"))?
                        .entry().to_app_option::<PerspectiveDiff>()?.ok_or(
                            SocialContextError::InternalError("Expected element to contain app entry data"),
                        )?;
                    out.additions.append(&mut diff_entry.additions.clone());
                    out.removals.append(&mut diff_entry.removals.clone());
                } 
                return Ok(out)
            }
            let current = current.unwrap();
            //also populate the search from the current_latest
            search = search::populate_search(Some(search), current.clone())?;
            search.print();

            //Get index for current and latest indexes
            let current_index = search.get_node_index(&current).expect("Could not find value in map").clone();
            let latest_index = search.get_node_index(&latest).expect("Could not find value in map").clone();

            //Check if latest diff is a child of current diff
            let ancestor_status = search.get_paths(latest_index.clone(), current_index.clone());
            debug!("Ancestor status: {:#?}", ancestor_status);
            
            if ancestor_status.len() > 0 {
                //Latest diff contains in its chain our current diff, fast forward and get all changes between now and then
                
                //Get all diffs between is_ancestor latest and current_revision
                //ancestor status contains all paths between latest and current revision, this can be used to get all the diffs when all paths are dedup'd together
                //Then update current revision to latest revision
                let mut diffs: Vec<NodeIndex> = ancestor_status.into_iter().flatten().collect();
                diffs.dedup();
                diffs.reverse();
                diffs.retain(|val| val != &current_index);
                let mut out = PerspectiveDiff {
                    additions: vec![],
                    removals: vec![]
                };
    
                for diff in diffs {
                    let hash = search.index(diff);
                    let current_diff = search.get_entry(&hash);
                    if let Some(val) = current_diff {
                        let diff_entry = get(val.diff, GetOptions::latest())?.ok_or(SocialContextError::InternalError("Could not find diff entry for given diff entry reference"))?
                            .entry().to_app_option::<PerspectiveDiff>()?.ok_or(
                                SocialContextError::InternalError("Expected element to contain app entry data"),
                            )?;
                        out.additions.append(&mut diff_entry.additions.clone());
                        out.removals.append(&mut diff_entry.removals.clone());
                    }
                }
                println!("Setting current to: {:#?}", latest);
                //Using now as the timestamp here may cause problems
                update_current_revision(latest, get_now()?)?;
                Ok(out)
            } else {
                debug!("Fork detected, attempting merge...");
                //There is a fork, find all the diffs from a fork and apply in merge with latest and current revisions as parents
                //Calculate the place where a common ancestor is shared between current and latest revisions
                //Common ancestor is then used as the starting point of gathering diffs on a fork
                let common_ancestor = search.find_common_ancestor(latest_index, current_index).expect("Could not find common ancestor");
                let fork_paths = search.get_paths(current_index.clone(), common_ancestor.clone());
                let latest_paths = search.get_paths(latest_index.clone(), common_ancestor.clone());
                let mut fork_direction: Option<Vec<NodeIndex>> = None;

                debug!("Paths of fork: {:#?}", fork_paths);
                debug!("Paths of latst: {:#?}", latest_paths);
                debug!("Common ancestor: {:#?}", common_ancestor);

                //Use items in path to recurse from common_ancestor going in direction of fork
                for path in fork_paths.clone() {
                    if path.contains(&current_index) {
                        fork_direction = Some(path);
                        break
                    };
                }
                let mut latest_paths = latest_paths.into_iter().flatten().collect::<Vec<_>>();
                latest_paths.dedup();
                latest_paths.retain(|val| val != &common_ancestor);

                //Create the merge entry
                let mut merge_entry = PerspectiveDiff {
                    additions: vec![],
                    removals: vec![]
                };
                if let Some(mut diffs) = fork_direction {    
                    diffs.reverse();
                    diffs.retain(|val| val != &common_ancestor);
                    for diff in diffs {
                        let hash = search.index(diff);
                        let current_diff = search.get_entry(
                            &hash
                        );
                        if let Some(val) = current_diff {
                            let diff_entry = get(val.diff, GetOptions::latest())?.ok_or(SocialContextError::InternalError("Could not find diff entry for given diff entry reference"))?
                                .entry().to_app_option::<PerspectiveDiff>()?.ok_or(
                                    SocialContextError::InternalError("Expected element to contain app entry data"),
                                )?;
                            merge_entry.additions.append(&mut diff_entry.additions.clone());
                            merge_entry.removals.append(&mut diff_entry.removals.clone());
                        }
                    }
                }
                
                debug!("Will merge entries: {:#?} and {:#?}. With diff data: {:#?}", latest, current, merge_entry);
                let merge_entry = create_entry(merge_entry)?;
                //Create the merge entry
                let hash = create_entry(PerspectiveDiffEntryReference {
                    parents: Some(vec![latest, current]),
                    diff: merge_entry.clone()
                })?;
                debug!("Commited merge entry: {:#?}", hash);
                let now = get_now()?;
                update_current_revision(hash.clone(), now)?;
                update_latest_revision(hash, now)?;

                //Return the diffs unseen by the user
                let mut unseen_entry = PerspectiveDiff {
                    additions: vec![],
                    removals: vec![]
                };

                for diff in latest_paths {
                    let hash = search.index(diff);
                    let current_diff = search.get_entry(
                        &hash
                    );
                    if let Some(val) = current_diff {
                        if val.parents.is_some() {
                            //Filter out the merge entries to avoid duplicate results
                            if val.parents.unwrap().len() == 1 {
                                let diff_entry = get(val.diff, GetOptions::latest())?.ok_or(SocialContextError::InternalError("Could not find diff entry for given diff entry reference"))?
                                .entry().to_app_option::<PerspectiveDiff>()?.ok_or(
                                    SocialContextError::InternalError("Expected element to contain app entry data"),
                                )?;
                                unseen_entry.additions.append(&mut diff_entry.additions.clone());
                                unseen_entry.removals.append(&mut diff_entry.removals.clone());
                            }
                        } else {
                            let diff_entry = get(val.diff, GetOptions::latest())?.ok_or(SocialContextError::InternalError("Could not find diff entry for given diff entry reference"))?
                            .entry().to_app_option::<PerspectiveDiff>()?.ok_or(
                                SocialContextError::InternalError("Expected element to contain app entry data"),
                            )?;
                            unseen_entry.additions.append(&mut diff_entry.additions.clone());
                            unseen_entry.removals.append(&mut diff_entry.removals.clone());
                        }
                    }
                }

                Ok(unseen_entry)
            }
        } else {
            Ok(PerspectiveDiff {
                removals: vec![],
                additions: vec![]
            })
        }
    } else {
        Ok(PerspectiveDiff {
            removals: vec![],
            additions: vec![]
        })
    }
}

pub fn render() -> SocialContextResult<Perspective> {
    Ok(Perspective {
        links: vec![]
    })
}

pub fn commit(diff: PerspectiveDiff) -> SocialContextResult<HoloHash<holo_hash::hash_type::Header>> {
    let pre_current_revision = current_revision()?;
    let pre_latest_revision = latest_revision()?;
    
    if pre_current_revision != pre_latest_revision {
        pull()?;
    };

    let parent = current_revision()?;
    debug!("Parent entry is: {:#?}", parent);
    let diff_entry_create = create_entry(diff.clone())?;
    debug!("Created diff entry: {:#?}", diff_entry_create);
    let diff_entry_reference = create_entry(PerspectiveDiffEntryReference {
        diff: diff_entry_create,
        parents: parent.map(|val| vec![val])
    })?;

    
    //This allows us to turn of revision updates when testing so we can artifically test pulling with varying agent states
    #[cfg(feature = "prod")] {
        let now = get_now()?;
        update_latest_revision(diff_entry_create.clone(), now.clone())?;
        update_current_revision(diff_entry_create.clone(), now)?;
    }

    if *ENABLE_SIGNALS {
        let now = sys_time()?.as_seconds_and_nanos();
        let now = DateTime::<Utc>::from_utc(
            NaiveDateTime::from_timestamp(now.0, now.1),
            Utc,
        );
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
        let recent_agents =  dedup(&recent_agents);
        debug!("Social-Context.add_link: Sending signal to agents: {:#?}", recent_agents);
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

    let current_agent_online = recent_agents
        .iter()
        .find(|agent| {
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
        },
        None => {
            //Agent is not marked online so lets add an online agent reference
            let agent_ref = AgentReference {
                agent: agent_info()?.agent_initial_pubkey,
                timestamp: now,
            };
            create_entry(&agent_ref)?;
            hc_time_index::index_entry(
                String::from("active_agent"),
                agent_ref,
                LinkTag::new(""),
            )?;
            Ok(None)
        }
    }
}

fn dedup<T: Eq + Hash + Clone>(vs: &Vec<T>) -> Vec<T> {
    let hs = vs.iter().cloned().collect::<HashSet<T>>();

    hs.into_iter().collect()
}