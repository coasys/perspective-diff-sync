use std::collections::HashSet;

use crate::Perspective;
use crate::errors::SocialContextResult;
use crate::snapshots::get_latest_snapshot;
use crate::revisions::latest_revision;

pub fn render() -> SocialContextResult<Perspective> {
    let latest = latest_revision()?;
    if latest.is_some() {
        let mut perspective =  Perspective {
            links: vec![]
        };
        let mut link_set = HashSet::new();
        let snapshot = get_latest_snapshot(latest.unwrap())?;
        for link in snapshot.additions {
            link_set.insert(link);
        };
        for link in snapshot.removals {
            link_set.remove(&link);
        };
        for link in link_set.into_iter() {
            perspective.links.push(link);
        };
        //TODO: update current revision?
        Ok(perspective)
    } else {
        Ok(Perspective {
            links: vec![]
        })
    }
}