use hdk::prelude::debug;
use chrono::{DateTime, Utc};
use perspective_diff_sync_integrity::{HashReference, LocalHashReference};

use crate::errors::SocialContextResult;
use crate::retriever::PerspectiveDiffRetreiver;
use crate::Hash;
use crate::utils::get_now;

pub fn update_latest_revision<Retriever: PerspectiveDiffRetreiver>(
    hash: Hash,
    timestamp: DateTime<Utc>
) -> SocialContextResult<()> {
    debug!("CREATE_ENTRY update_latest_revision HashReference");
    let now = get_now()?.time();
    let res = Retriever::update_latest_revision(hash, timestamp);
    let after = get_now()?.time();
    debug!("Took: {} to update latest revision", (after - now).num_milliseconds());
    res
}

pub fn update_current_revision<Retriever: PerspectiveDiffRetreiver>(
    hash: Hash,
    timestamp: DateTime<Utc>
) -> SocialContextResult<()> {
    debug!("CREATE_ENTRY update_current_revision LocalHashReference");
    let now = get_now()?.time();
    let res = Retriever::update_current_revision(hash, timestamp);
    let after = get_now()?.time();
    debug!("Took: {} to update current revision", (after - now).num_milliseconds());
    res
}

//Latest revision as seen from the DHT
pub fn latest_revision<Retriever: PerspectiveDiffRetreiver>() -> SocialContextResult<Option<HashReference>> {
    Retriever::latest_revision()
}

//Latest revision as seen from our local state
pub fn current_revision<Retriever: PerspectiveDiffRetreiver>() -> SocialContextResult<Option<LocalHashReference>> {
    Retriever::current_revision()
}
