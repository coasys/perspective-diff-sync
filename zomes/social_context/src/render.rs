use hdk::prelude::*;
use petgraph::graph::NodeIndex;

use crate::search;
use crate::{
    Perspective, PerspectiveDiff, PerspectiveDiffEntryReference,
    errors::{SocialContextResult, SocialContextError}
};
use crate::utils::get_now;
use crate::revisions::{current_revision, latest_revision, update_current_revision, update_latest_revision};

pub fn render() -> SocialContextResult<Perspective> {
    Ok(Perspective {
        links: vec![]
    })
}