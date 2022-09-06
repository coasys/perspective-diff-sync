use hdk::prelude::*;
use perspective_diff_sync_integrity::{EntryTypes, PerspectiveDiff, PerspectiveDiffEntryReference};
use crate::errors::{SocialContextError, SocialContextResult};
use crate::revisions::{
    current_revision, latest_revision, update_current_revision, update_latest_revision,
};
use crate::utils::get_now;
use crate::workspace::{Workspace, NULL_NODE};
use crate::retriever::HolochainRetreiver;
use crate::Hash;

fn merge(latest: Hash, current: Hash) -> SocialContextResult<()> {
    //Create the merge entry
    let merge_entry = create_entry(EntryTypes::PerspectiveDiff(PerspectiveDiff {
        additions: vec![],
        removals: vec![]
    }))?;
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
    Ok(())
}

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
        workspace.collect_only_from_latest::<HolochainRetreiver>(latest.clone())?;
        let diff = workspace.squashed_diff()?;
        update_current_revision(latest, get_now()?)?;
        return Ok(diff);
    }

    let current = current.expect("current missing handled above");

    match workspace.build_diffs::<HolochainRetreiver>(latest.clone(), current.clone()) {
        Err(SocialContextError::NoCommonAncestorFound) => {
            workspace.collect_only_from_latest::<HolochainRetreiver>(latest.clone())?;
            let diff = workspace.squashed_diff()?;
            merge(latest, current)?;
            return Ok(diff)
        },

        _ => {
            // continue with the rest below...
        }
    }

    //See what fast forward paths exist between latest and current
    let fast_foward_paths = workspace.get_paths(&latest, &current);
    //Get all the diffs which exist between current and the last ancestor that we got
    let seen_diffs = workspace.get_paths(&current, workspace.common_ancestors.last().to_owned().expect("Should be atleast 1 common ancestor"));
    debug!("Got the seen diffs: {:#?}", seen_diffs);
    //Get all the diffs in the graph which we havent seen
    let unseen_diffs = if seen_diffs.len() > 0 {
        let diffs = workspace.sorted_diffs.clone().expect("should be unseen diffs after build_diffs() call").into_iter().filter(|val| {
            if val.0 == NULL_NODE() {
                return false;
            };
            let node_index = workspace.get_node_index(&val.0).expect("Should find the node index for a given diff ref");
            for seen_diff in &seen_diffs {
                if seen_diff.contains(node_index) {
                    return false;
                };
            };
            true
        }).collect::<Vec<(Hash, PerspectiveDiffEntryReference)>>();
        diffs
    } else {
        workspace.sorted_diffs.expect("should be unseen diffs after build_diffs() call").into_iter().filter(|val| {
            val.0 != NULL_NODE() && val.0 != current
        }).collect::<Vec<(Hash, PerspectiveDiffEntryReference)>>()
    };
    debug!("Got the unseen diffs: {:#?}", unseen_diffs);

    if fast_foward_paths.len() > 0 {
        debug!("There are paths between current and latest, lets fast forward the changes we have missed!");
        //Using now as the timestamp here may cause problems
        update_current_revision(latest, get_now()?)?;
        let mut out = PerspectiveDiff {
            additions: vec![],
            removals: vec![]
        };
        for diff in unseen_diffs {
            let diff_entry = get(diff.1.diff.clone(), GetOptions::latest())?
                .ok_or(SocialContextError::InternalError(
                    "pull / fast forward / for diff in unseen_diffs / Could not retrieve diff entry from HC",
                ))?
                .entry()
                .to_app_option::<PerspectiveDiff>()?
                .ok_or(SocialContextError::InternalError(
                    "Expected element to contain app entry data",
                ))?;
            out
                .additions
                .append(&mut diff_entry.additions.clone());
            out
                .removals
                .append(&mut diff_entry.removals.clone());
        }
        Ok(out)
    } else {
        debug!("There are no paths between current and latest, we must merge current and latest");
        //Get the entries we missed from unseen diff
        let mut out = PerspectiveDiff {
            additions: vec![],
            removals: vec![]
        };
        for diff in unseen_diffs {
            let diff_entry = get(diff.1.diff.clone(), GetOptions::latest())?
                .ok_or(SocialContextError::InternalError(
                    "pull / merge / for diff in unseen_diffs / Could not retrieve diff entry from HC",
                ))?
                .entry()
                .to_app_option::<PerspectiveDiff>()?
                .ok_or(SocialContextError::InternalError(
                    "Expected element to contain app entry data",
                ))?;
            out
                .additions
                .append(&mut diff_entry.additions.clone());
            out
                .removals
                .append(&mut diff_entry.removals.clone());
        }

        merge(latest, current)?;

        Ok(out)
    }
}
