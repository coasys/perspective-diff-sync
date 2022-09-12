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
    debug!("===PerspectiveDiffSunc.update_latest_revision(): Function start");
    let now = get_now()?.time();
    let res = Retriever::update_latest_revision(hash, timestamp);
    let after = get_now()?.time();
    debug!("===PerspectiveDiffSunc.update_latest_revision() - Profiling: Took: {} to update latest_revision", (after - now).num_milliseconds());
    res
}

pub fn update_current_revision<Retriever: PerspectiveDiffRetreiver>(
    hash: Hash,
    timestamp: DateTime<Utc>
) -> SocialContextResult<()> {
    debug!("===PerspectiveDiffSunc.update_current_revision(): Function start");
    let now = get_now()?.time();
    let res = Retriever::update_current_revision(hash, timestamp);
    let after = get_now()?.time();
    debug!("===PerspectiveDiffSunc.update_current_revision() - Profiling: Took: {} to update current_revision", (after - now).num_milliseconds());
    res
}

//Latest revision as seen from the DHT
pub fn latest_revision<Retriever: PerspectiveDiffRetreiver>() -> SocialContextResult<Option<HashReference>> {
    debug!("===PerspectiveDiffSunc.latest_revision(): Function start");
    let now = get_now()?.time();
    let rev = Retriever::latest_revision()?;
    let after = get_now()?.time();
    debug!("===PerspectiveDiffSunc.latest_revision() - Profiling: Took: {} to get the latest_revision", (after - now).num_milliseconds());
    Ok(rev)
}

//Latest revision as seen from our local state
pub fn current_revision<Retriever: PerspectiveDiffRetreiver>() -> SocialContextResult<Option<LocalHashReference>> {
    debug!("===PerspectiveDiffSunc.current_revision(): Function start");
    let now = get_now()?.time();
    let rev = Retriever::current_revision()?;
    let after = get_now()?.time();
    debug!("===PerspectiveDiffSunc.current_revision() - Profiling: Took: {} to get the current_revision", (after - now).num_milliseconds());
    Ok(rev)
}
