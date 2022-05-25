use hdk::prelude::*;
use chrono::{Utc, DateTime, NaiveDateTime};
use petgraph::graph::NodeIndex;

use crate::search;
use crate::{
    errors::{SocialContextResult, SocialContextError}, PerspectiveDiffEntry
};
use crate::{
    Perspective, PerspectiveDiff, LocalHashReference, HashAnchor, HashReference
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
                    out.additions.append(&mut value.diff.additions.clone());
                    out.removals.append(&mut value.diff.removals.clone());
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
                        out.additions.append(&mut val.diff.additions.clone());
                        out.removals.append(&mut val.diff.removals.clone());
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
                            merge_entry.additions.append(&mut val.diff.additions.clone());
                            merge_entry.removals.append(&mut val.diff.removals.clone());
                        }
                    }
                }
                
                debug!("Will merge entries: {:#?} and {:#?}. With diff data: {:#?}", latest, current, merge_entry);
                //Create the merge entry
                let hash = create_entry(PerspectiveDiffEntry {
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
                                unseen_entry.additions.append(&mut val.diff.additions.clone());
                                unseen_entry.removals.append(&mut val.diff.removals.clone());
                            }
                        } else {
                            unseen_entry.additions.append(&mut val.diff.additions.clone());
                            unseen_entry.removals.append(&mut val.diff.removals.clone());
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
    let diff_entry = PerspectiveDiffEntry {
        diff,
        parents: parent.map(|val| vec![val])
    };
    let diff_entry_create = create_entry(diff_entry)?;
    debug!("Created diff entry: {:#?}", diff_entry_create);
    
    //This allows us to turn of revision updates when testing so we can artifically test pulling with varying agent states
    #[cfg(feature = "prod")] {
        let now = get_now()?;
        update_latest_revision(diff_entry_create.clone(), now.clone())?;
        update_current_revision(diff_entry_create.clone(), now)?;
    }

    //TODO: send signal to active agents

    Ok(diff_entry_create)
}

pub fn add_active_agent_link() -> SocialContextResult<()> {
    Ok(())
}