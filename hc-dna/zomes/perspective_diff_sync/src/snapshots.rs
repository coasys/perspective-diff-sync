use hdk::prelude::*;
use perspective_diff_sync_integrity::{
    LinkTypes, PerspectiveDiff, PerspectiveDiffEntryReference, Snapshot,
};

use crate::Hash;
use crate::errors::{SocialContextError, SocialContextResult};
use crate::chunked_diffs::ChunkedDiffs;
use crate::retriever::HolochainRetreiver;

struct SearchPosition {
    hash: Hash,
    is_unseen: bool
}

pub fn generate_snapshot(
    latest: HoloHash<holo_hash::hash_type::Action>,
) -> SocialContextResult<Snapshot> {
    let mut search_position = SearchPosition {
        hash: latest.clone(),
        is_unseen: false
    };
    let mut seen: HashSet<Hash> = HashSet::new();
    let mut unseen_parents = vec![];

    let mut all_additions = BTreeSet::new();
    let mut all_removals = BTreeSet::new();

    loop {
        let diff = get(search_position.hash.clone(), GetOptions::latest())?
            .ok_or(SocialContextError::InternalError(
                "generate_snapshot(): Could not find entry while populating search",
            ))?
            .entry()
            .to_app_option::<PerspectiveDiffEntryReference>()?
            .ok_or(SocialContextError::InternalError(
                "Expected element to contain app entry data",
            ))?;
        if diff.diffs_since_snapshot == 0 && search_position.hash != latest {
            let mut snapshot_links = get_links(
                hash_entry(&diff)?,
                LinkTypes::Snapshot,
                Some(LinkTag::new("snapshot")),
            )?;
            if snapshot_links.len() == 0 {
                return Err(SocialContextError::InternalError("Expected to find a snapshot where diff has diffs_since_snapshot"));
            };
            //get snapshot and add elements to out
            let snapshot = get(snapshot_links.remove(0).target, GetOptions::latest())?
                .ok_or(SocialContextError::InternalError(
                    "Could not find diff entry for given diff entry reference",
                ))?
                .entry()
                .to_app_option::<Snapshot>()?
                .ok_or(SocialContextError::InternalError(
                    "Expected element to contain app entry data",
                ))?;
            
            let diff = ChunkedDiffs::from_entries::<HolochainRetreiver>(snapshot.diff_chunks)?.into_aggregated_diff();
            for addition in diff.additions.iter() {
                all_additions.insert(addition.clone());
            }
            for removal in diff.removals.iter() {
                all_removals.insert(removal.clone());
            }
            for hash in snapshot.included_diffs.iter() {
                seen.insert(hash.clone());
            }
            
            //Be careful with break here where there are still unseen parents
            if unseen_parents.len() == 0 {
                debug!("No more unseen parents within snapshot block");
                break;
            } else {
                search_position = unseen_parents.remove(0);
            }
        } else {
            //Check if entry is already in graph
            if !seen.contains(&search_position.hash) {
                seen.insert(search_position.hash.clone());
                let diff_entry = get(diff.diff.clone(), GetOptions::latest())?
                    .ok_or(SocialContextError::InternalError(
                        "Could not find diff entry for given diff entry reference",
                    ))?
                    .entry()
                    .to_app_option::<PerspectiveDiff>()?
                    .ok_or(SocialContextError::InternalError(
                        "Expected element to contain app entry data",
                    ))?;

                for addition in diff_entry.additions.iter() {
                    all_additions.insert(addition.clone());
                }
                for removal in diff_entry.removals.iter() {
                    all_removals.insert(removal.clone());
                }

                if diff.parents.is_none() {
                    //No parents, we have reached the end of the chain
                    //Now move onto traversing unseen parents, or break if we dont have any other paths to search
                    if unseen_parents.len() == 0 {
                        debug!("No more unseen items within parent block");
                        break;
                    } else {
                        debug!("Moving onto unseen fork items within parent block");
                        search_position = unseen_parents.remove(0);
                    }
                } else {
                    //Do the fork traversals
                    let mut parents = diff.parents.unwrap();
                    //Check if all parents have already been seen, if so then break or move onto next unseen parents
                    //TODO; we should use a seen set here versus array iter
                    if parents.iter().all(|val| { seen.contains(val)}) {
                        if unseen_parents.len() == 0 {
                            debug!("Parents of item seen and unseen 0");
                            break;
                        } else {
                            debug!("last moving onto unseen");
                            search_position = unseen_parents.remove(0);
                        }
                    } else {
                        search_position = SearchPosition {
                            hash: parents.remove(0),
                            is_unseen: false
                        };
                        debug!("Appending parents to look up: {:?}", parents);
                        unseen_parents.append(
                            &mut parents.into_iter().map(|val| SearchPosition {
                                hash: val,
                                is_unseen: true
                            }).collect()
                        );
                    };
                }
            } else if search_position.is_unseen {
                //The parent for this branch is already seen so likely already explored and we are part of the main branch
                if unseen_parents.len() == 0 {
                    debug!("No more unseen items within parent block");
                    break;
                } else {
                    debug!("Moving onto unseen fork items within parent block");
                    search_position = unseen_parents.remove(0);
                }
            } else {
                if diff.parents.is_none() {
                    //No parents, we have reached the end of the chain
                    //Now move onto traversing unseen parents, or break if we dont have any other paths to search
                    if unseen_parents.len() == 0 {
                        debug!("No more unseen items within parent block");
                        break;
                    } else {
                        debug!("Moving onto unseen fork items within parent block");
                        search_position = unseen_parents.remove(0);
                    }
                } else {
                    //Do the fork traversals
                    let mut parents = diff.parents.unwrap();
                    //Check if all parents have already been seen, if so then break or move onto next unseen parents
                    //TODO; we should use a seen set here versus array iter
                    if parents.iter().all(|val| { seen.contains(val)}) {
                        if unseen_parents.len() == 0 {
                            debug!("Parents of item seen and unseen 0");
                            break;
                        } else {
                            debug!("last moving onto unseen");
                            search_position = unseen_parents.remove(0);
                        }
                    } else {
                        search_position = SearchPosition {
                            hash: parents.remove(0),
                            is_unseen: false
                        };
                        debug!("Appending parents to look up: {:?}", parents);
                        unseen_parents.append(
                            &mut parents.into_iter().map(|val| SearchPosition {
                                hash: val,
                                is_unseen: true
                            }).collect()
                        );
                    };
                }
            };
        }
    }


    let mut chunked_diffs = ChunkedDiffs::new(500);

    chunked_diffs.add_additions(all_additions.into_iter().collect());
    chunked_diffs.add_removals(all_removals.into_iter().collect());

    let snapshot = Snapshot {
        diff_chunks: chunked_diffs.into_entries::<HolochainRetreiver>()?,
        included_diffs: seen.into_iter().collect(),
    };

    Ok(snapshot)
}
