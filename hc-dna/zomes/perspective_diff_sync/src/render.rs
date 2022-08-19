use hdk::prelude::*;
use crate::errors::{SocialContextError, SocialContextResult};
use crate::revisions::current_revision;
use crate::search::populate_search;
use crate::Perspective;
use perspective_diff_sync_integrity::PerspectiveDiff;

pub fn render() -> SocialContextResult<Perspective> {
    let current = current_revision()?;
    debug!("render() current: {:?}", current);
    if current.is_none() {
        return Err(SocialContextError::InternalError("Can't render when we have no current revision"));
    }


    let search = populate_search(None, current.expect("must be some since we checked above"), None, true)?;
    let mut perspective = Perspective { links: vec![] };
    for diff_node in search.sorted_diffs {
        debug!("render() adding diff_node: {:?}", diff_node);
        let diff_entry_ref = diff_node.1;
        let diff_entry = get(diff_entry_ref.diff.clone(), GetOptions::latest())?
            .ok_or(SocialContextError::InternalError(
                "Could not find diff entry for given diff entry reference",
            ))?
            .entry()
            .to_app_option::<PerspectiveDiff>()?
            .ok_or(SocialContextError::InternalError(
                "Expected element to contain app entry data",
            ))?;

        for addition in diff_entry.additions {
            perspective.links.push(addition);
        }
        for removal in diff_entry.removals {
            perspective.links.retain(|l| l != &removal);
        }
    }
    
    Ok(perspective)
}
