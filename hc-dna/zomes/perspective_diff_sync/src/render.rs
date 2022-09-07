use hdk::prelude::*;
use perspective_diff_sync_integrity::PerspectiveDiff;

use crate::errors::{SocialContextError, SocialContextResult};
use crate::revisions::current_revision;
use crate::Perspective;
use crate::workspace::Workspace;
use crate::retriever::PerspectiveDiffRetreiver;

pub fn render<Retriever: PerspectiveDiffRetreiver>() -> SocialContextResult<Perspective> {
    let current = current_revision::<Retriever>()?
        .ok_or(SocialContextError::InternalError("Can't render when we have no current revision"))?;
    
    debug!("render() current: {:?}", current);

    let mut workspace = Workspace::new();
    workspace.collect_only_from_latest::<Retriever>(current)?;
    workspace.topo_sort_graph()?;
    let sorted_diffs = &workspace.sorted_diffs.expect("must have sorted diffs after calling topo_sort_graph()");

    let mut perspective = Perspective { links: vec![] };
    for diff_node in sorted_diffs {
        debug!("render() adding diff_node: {:?}", diff_node);
        let diff_entry = Retriever::get::<PerspectiveDiff>(diff_node.1.diff.clone())?;

        for addition in diff_entry.additions {
            perspective.links.push(addition);
        }
        for removal in diff_entry.removals {
            perspective.links.retain(|l| l != &removal);
        }
    }
    
    Ok(perspective)
}
