use hdk::prelude::debug;
use chrono::{DateTime, Utc};

use crate::errors::SocialContextResult;
use crate::retriever::PerspectiveDiffRetreiver;
use crate::Hash;

pub fn update_latest_revision<Retriever: PerspectiveDiffRetreiver>(
    hash: Hash,
    timestamp: DateTime<Utc>
) -> SocialContextResult<()> {
    debug!("CREATE_ENTRY update_latest_revision HashReference");
    Retriever::update_latest_revision(hash, timestamp)
}

pub fn update_current_revision<Retriever: PerspectiveDiffRetreiver>(
    hash: Hash,
    timestamp: DateTime<Utc>
) -> SocialContextResult<()> {
    debug!("CREATE_ENTRY update_current_revision LocalHashReference");
    Retriever::update_current_revision(hash, timestamp)
}

//Latest revision as seen from the DHT
pub fn latest_revision<Retriever: PerspectiveDiffRetreiver>() -> SocialContextResult<Option<Hash>> {
    Retriever::latest_revision()
}

//Latest revision as seen from our local state
pub fn current_revision<Retriever: PerspectiveDiffRetreiver>() -> SocialContextResult<Option<Hash>> {
    Retriever::current_revision()
}
