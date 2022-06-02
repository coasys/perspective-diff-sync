use hdk::prelude::*;

use crate::{PerspectiveDiffEntryReference, PerspectiveDiff};
use crate::errors::{SocialContextResult, SocialContextError};


pub fn get_entries_since_snapshot(latest: HoloHash<holo_hash::hash_type::Header>) -> SocialContextResult<usize> {
    let mut search_position = latest;
    let mut depth = 0;
    let mut seen = HashSet::new();
    let mut unseen_parents = vec![];

    loop {
        //Check if entry is already in graph
        if !seen.contains(&search_position) {
            seen.insert(search_position.clone());
            //TODO; Should we only increase depth if entry is not a merge entry?
            depth +=1;
        };
        debug!("Checking: {:#?}", search_position);
        let diff = get(search_position, GetOptions::latest())?.ok_or(SocialContextError::InternalError("Could not find entry while populating search"))?
            .entry().to_app_option::<PerspectiveDiffEntryReference>()?.ok_or(
                SocialContextError::InternalError("Expected element to contain app entry data"),
            )?;
        let diff_entry_hash = hash_entry(&diff)?;
        let check_snapshot = get_links(diff_entry_hash, Some(LinkTag::new("snapshot")))?;
        if check_snapshot.len() != 0 {
            break;
        }

        if diff.parents.is_none() {
            //No parents, we have reached the end of the chain
            //Now move onto traversing parents
            if unseen_parents.len() == 0 {
                debug!("No more unseen items");
                break
            } else {
                debug!("Moving onto unseen fork items");
                search_position = unseen_parents.remove(0);
            }
        } else {
            let mut parents = diff.parents.unwrap();
            //Check if all parents have already been seen, if so then break or move onto next unseen parents
            if parents.iter().all(|val| seen.contains(val)) {
                if unseen_parents.len() == 0 {
                    //TODO; consider what happens here where snapshot has not been found in block above
                    break;
                } else {
                    search_position = unseen_parents.remove(0);
                };
            } else {
                search_position = parents.remove(0);
                unseen_parents.append(&mut parents);
            };
        }
    };
    Ok(depth)
}

pub fn generate_snapshot(latest: HoloHash<holo_hash::hash_type::Header>) -> SocialContextResult<PerspectiveDiff> {
    let mut search_position = latest;
    let mut seen = HashSet::new();

    let mut out = PerspectiveDiff {
        additions: vec![],
        removals: vec![]
    };

    loop  {
        let diff = get(search_position.clone(), GetOptions::latest())?.ok_or(SocialContextError::InternalError("Could not find entry while populating search"))?
            .entry().to_app_option::<PerspectiveDiffEntryReference>()?.ok_or(
                SocialContextError::InternalError("Expected element to contain app entry data"),
            )?;
        debug!("Checking: {:#?}", diff);
        let diff_entry_hash = hash_entry(&diff)?;
        let mut snapshot_links = get_links(diff_entry_hash, Some(LinkTag::new("snapshot")))?;
        if snapshot_links.len() != 0 {
            //get snapshot and add elements to out
            let snapshot = get(snapshot_links.remove(0).target, GetOptions::latest())?.ok_or(SocialContextError::InternalError("Could not find diff entry for given diff entry reference"))?
                .entry().to_app_option::<PerspectiveDiff>()?.ok_or(
                    SocialContextError::InternalError("Expected element to contain app entry data"),
                )?;
            out.additions.append(&mut snapshot.additions.clone());
            out.removals.append(&mut snapshot.removals.clone());
            debug!("Breaking at snapshot");
            break;
        } else {
            //Check if entry is already in graph
            if !seen.contains(&search_position) {
                seen.insert(search_position.clone());
                let diff_entry = get(diff.diff.clone(), GetOptions::latest())?.ok_or(SocialContextError::InternalError("Could not find diff entry for given diff entry reference"))?
                    .entry().to_app_option::<PerspectiveDiff>()?.ok_or(
                        SocialContextError::InternalError("Expected element to contain app entry data"),
                    )?;
                out.additions.append(&mut diff_entry.additions.clone());
                out.removals.append(&mut diff_entry.removals.clone());
            };
        }

        if diff.parents.is_none() {
            break;
        } else {
            let mut parents = diff.parents.unwrap();
            //Check if all parents have already been seen, if so then break or move onto next unseen parents
            if parents.iter().all(|val| seen.contains(val)) {
                break;
            } else {
                search_position = parents.remove(0);
            };
        }
    }

    Ok(out)
}