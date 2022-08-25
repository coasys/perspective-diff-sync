use hdk::prelude::*;
use perspective_diff_sync_integrity::{EntryTypes, PerspectiveDiff, PerspectiveDiffEntryReference};
use petgraph::graph::NodeIndex;

use crate::errors::{SocialContextError, SocialContextResult};
use crate::revisions::{
    current_revision, latest_revision, update_current_revision, update_latest_revision,
};
use crate::search;
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

    let latest = latest.expect("No latest handled above");
    let mut workspace = Workspace::new();

    if current.is_none() {
        workspace.collect_only_from_latest(latest)?;
        let diff = workspace.squashed_diff()?;
        update_current_revision(latest, get_now()?)?;
        return Ok(diff);
    }

    let current = current.exepct("No current handled above");

    workspace.collect_until_common_ancestor(latest, current)?;
    debug!("completed current search population");


    //Check if latest diff is a child of current diff
    let ancestor_status = workspace.get_paths(&latest, &current);
    //debug!("Ancestor status: {:#?}", ancestor_status);

    if ancestor_status.len() > 0 {
        //Latest diff contains in its chain our current diff, fast forward and get all changes between now and then

        //Get all diffs between is_ancestor latest and current_revision
        //ancestor status contains all paths between latest and current revision, this can be used to get all the diffs when all paths are dedup'd together
        //Then update current revision to latest revision
        let mut diffs: Vec<NodeIndex> = ancestor_status.into_iter().flatten().collect();
        diffs = dedup(&diffs);
        diffs.reverse();
        diffs.retain(|val| val != &current_index);
        let mut out = PerspectiveDiff {
            additions: vec![],
            removals: vec![],
        };

        for diff in diffs {
            let hash = search.index(diff);
            let current_diff = search.get_entry(&hash);
            if let Some(val) = current_diff {
                let diff_entry = get(val.diff, GetOptions::latest())?
                    .ok_or(SocialContextError::InternalError(
                        "Could not find diff entry for given diff entry reference",
                    ))?
                    .entry()
                    .to_app_option::<PerspectiveDiff>()?
                    .ok_or(SocialContextError::InternalError(
                        "Expected element to contain app entry data",
                    ))?;
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
        let common_ancestor = search
            .find_common_ancestor(latest_index, current_index)
            .expect("Could not find common ancestor");
        let fork_paths = search.get_paths(current_index.clone(), common_ancestor.clone());
        let latest_paths = search.get_paths(latest_index.clone(), common_ancestor.clone());
        let mut fork_direction: Option<Vec<NodeIndex>> = None;

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
        latest_paths.retain(|val| val != &common_ancestor);

        //Create the merge entry
        let mut merge_entry = PerspectiveDiff {
            additions: vec![],
            removals: vec![],
        };
        if let Some(mut diffs) = fork_direction {
            diffs.reverse();
            diffs.retain(|val| val != &common_ancestor);
            for diff in diffs {
                let hash = search.index(diff);
                let current_diff = search.get_entry(&hash);
                if let Some(val) = current_diff {
                    let diff_entry = get(val.diff, GetOptions::latest())?
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
            let hash = search.index(diff);
            let current_diff = search.get_entry(&hash);
            if let Some(val) = current_diff {
                if val.parents.is_some() {
                    //Filter out the merge entries to avoid duplicate results
                    if val.parents.unwrap().len() == 1 {
                        let diff_entry = get(val.diff, GetOptions::latest())?
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
                    let diff_entry = get(val.diff, GetOptions::latest())?
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
        }

        Ok(unseen_entry)
    }

}
