use hdk::prelude::*;
use perspective_diff_sync_integrity::{EntryTypes, PerspectiveDiff, PerspectiveDiffEntryReference};
use petgraph::graph::NodeIndex;

use crate::errors::{SocialContextError, SocialContextResult};
use crate::revisions::{
    current_revision, latest_revision, update_current_revision, update_latest_revision,
};
use crate::utils::{dedup, get_now};
use crate::workspace::Workspace;

pub fn pull() -> SocialContextResult<PerspectiveDiff> {
    let latest = latest_revision()?;
    let current = current_revision()?;
    debug!(
        "Pull made with latest: {:#?} and current: {:#?}",
        latest, current
    );

    if latest == current {
        return Ok(PerspectiveDiff {
            removals: vec![],
            additions: vec![],
        })
    }

    if latest.is_none() {
        return Ok(PerspectiveDiff {
            removals: vec![],
            additions: vec![],
        })
    }

    let latest = latest.expect("latest missing handled above");
    let mut workspace = Workspace::new();

    if current.is_none() {
        workspace.collect_only_from_latest(latest.clone())?;
        let diff = workspace.squashed_diff()?;
        update_current_revision(latest, get_now()?)?;
        return Ok(diff);
    }

    let current = current.expect("current missing handled above");

    match workspace.collect_until_common_ancestor(latest.clone(), current.clone()) {
        Err(SocialContextError::NoCommonAncestorFound) => {
            // Just merge
            let diff_entry = create_entry(
                EntryTypes::PerspectiveDiff(
                    PerspectiveDiff {
                        additions: vec![],
                        removals: vec![],
                    }
                )
            )?;
            //Create the merge entry
            let hash = create_entry(
                EntryTypes::PerspectiveDiffEntryReference(
                    PerspectiveDiffEntryReference {
                        parents: Some(vec![latest.clone(), current]),
                        diff: diff_entry.clone(),
                    },
                )
            )?;
            debug!("Commited merge entry: {:#?}", hash);
            let now = get_now()?;
            update_current_revision(hash.clone(), now)?;
            update_latest_revision(hash, now)?;

            // Return all of the foreign branch as changes
            let mut ws2 = Workspace::new();
            ws2.collect_only_from_latest(latest)?;
            Ok(ws2.squashed_diff()?)
        },
        // pass through other errors
        Err(error) => return Err(error),
        // 
        Ok(()) => {
            workspace.topo_sort_graph()?;
            workspace.build_graph()?;
            debug!("completed current search population");
            workspace.print_graph_debug();
        
        
            //Check if latest diff is a child of current diff
            let ancestor_status = workspace.get_paths(&latest, &current);
            //debug!("Ancestor status: {:#?}", ancestor_status);
        
            if ancestor_status.len() > 0 {
                //Latest diff contains in its chain our current diff, fast forward and get all changes between now and then
                let fast_forward_squash = workspace.squashed_fast_forward_from(current)?;
                println!("Setting current to: {:#?}", latest);
                //Using now as the timestamp here may cause problems
                update_current_revision(latest, get_now()?)?;
                Ok(fast_forward_squash)
            } else {
                debug!("Fork detected, attempting merge...");
                //There is a fork, find all the diffs from a fork and apply in merge with latest and current revisions as parents
                //Since we used workspace.collect_until_common_ancestor(..) above and sorted afterwards,
                //the first entry in sorted_diffs must be our common ancestor.
                //Common ancestor is then used as the starting point of gathering diffs on a fork
                let common_ancestor_hash = &workspace.sorted_diffs.as_ref().expect("sorted before")[0].0;
                //search
                //    .find_common_ancestor(latest_index, current_index)
                //    .expect("Could not find common ancestor");
                let fork_paths = workspace.get_paths(&current, common_ancestor_hash);
                let latest_paths = workspace.get_paths(&latest, common_ancestor_hash);
                let mut fork_direction: Option<Vec<NodeIndex>> = None;
        
                let current_index = workspace.get_node_index(&current).expect("to get index after build_graph()");
                let common_ancestor = workspace.get_node_index(common_ancestor_hash).expect("to get index after build_graph()");
        
                //debug!("Paths of fork: {:#?}", fork_paths);
                //debug!("Paths of latest: {:#?}", latest_paths);
                //debug!("Common ancestor: {:#?}", common_ancestor);
        
                //Use items in path to recurse from common_ancestor going in direction of fork
                for path in fork_paths.clone() {
                    if path.contains(&current_index) {
                        fork_direction = Some(path);
                        break;
                    };
                }
                let mut latest_paths = latest_paths.into_iter().flatten().collect::<Vec<_>>();
                latest_paths = dedup(&latest_paths);
                latest_paths.retain(|val| val != common_ancestor);
        
                //Create the merge entry
                let mut merge_entry = PerspectiveDiff {
                    additions: vec![],
                    removals: vec![],
                };
                if let Some(mut diffs) = fork_direction {
                    diffs.reverse();
                    diffs.retain(|val| val != common_ancestor);
                    for diff in diffs {
                        let hash = workspace.index(diff);
                        let current_diff = workspace.entry_map.get(&hash).expect("got hash through index above");
                        let diff_entry = get(current_diff.diff.clone(), GetOptions::latest())?
                            .ok_or(SocialContextError::InternalError(
                                "Could not find diff entry for given diff entry reference",
                            ))?
                            .entry()
                            .to_app_option::<PerspectiveDiff>()?
                            .ok_or(SocialContextError::InternalError(
                                "Expected element to contain app entry data",
                            ))?;
                        merge_entry
                            .additions
                            .append(&mut diff_entry.additions.clone());
                        merge_entry
                            .removals
                            .append(&mut diff_entry.removals.clone());
                    }
                }
        
                //debug!(
                //    "Will merge entries: {:#?} and {:#?}. With diff data: {:#?}",
                //    latest, current, merge_entry
                //);
                let merge_entry = create_entry(EntryTypes::PerspectiveDiff(merge_entry))?;
                //Create the merge entry
                let hash = create_entry(EntryTypes::PerspectiveDiffEntryReference(
                    PerspectiveDiffEntryReference {
                        parents: Some(vec![latest, current]),
                        diff: merge_entry.clone(),
                    },
                ))?;
                debug!("Commited merge entry: {:#?}", hash);
                let now = get_now()?;
                update_current_revision(hash.clone(), now)?;
                update_latest_revision(hash, now)?;
        
                //Return the diffs unseen by the user
                let mut unseen_entry = PerspectiveDiff {
                    additions: vec![],
                    removals: vec![],
                };
        
                for diff in latest_paths {
                    let hash = workspace.index(diff);
                    let current_diff = workspace.entry_map.get(&hash).expect("got hash through index above");
                    
                    if current_diff.parents.is_some() {
                        //Filter out the merge entries to avoid duplicate results
                        if current_diff.parents.as_ref().unwrap().len() == 1 {
                            let diff_entry = get(current_diff.diff.clone(), GetOptions::latest())?
                                .ok_or(SocialContextError::InternalError(
                                    "Could not find diff entry for given diff entry reference",
                                ))?
                                .entry()
                                .to_app_option::<PerspectiveDiff>()?
                                .ok_or(SocialContextError::InternalError(
                                    "Expected element to contain app entry data",
                                ))?;
                            unseen_entry
                                .additions
                                .append(&mut diff_entry.additions.clone());
                            unseen_entry
                                .removals
                                .append(&mut diff_entry.removals.clone());
                        }
                    } else {
                        let diff_entry = get(current_diff.diff.clone(), GetOptions::latest())?
                            .ok_or(SocialContextError::InternalError(
                                "Could not find diff entry for given diff entry reference",
                            ))?
                            .entry()
                            .to_app_option::<PerspectiveDiff>()?
                            .ok_or(SocialContextError::InternalError(
                                "Expected element to contain app entry data",
                            ))?;
                        unseen_entry
                            .additions
                            .append(&mut diff_entry.additions.clone());
                        unseen_entry
                            .removals
                            .append(&mut diff_entry.removals.clone());
                    }
                }
                Ok(unseen_entry)
            }
        }
    }
    

}
