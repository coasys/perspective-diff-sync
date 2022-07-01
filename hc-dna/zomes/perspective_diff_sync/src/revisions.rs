use chrono::{DateTime, NaiveDateTime, Utc};
use hdk::prelude::*;
use perspective_diff_sync_integrity::{EntryTypes, HashReference, LocalHashReference, LinkTypes};

use crate::utils::get_now;
use crate::{
    errors::{SocialContextError, SocialContextResult}
};

pub fn update_latest_revision(
    hash: HoloHash<holo_hash::hash_type::Action>,
    timestamp: DateTime<Utc>,
) -> SocialContextResult<()> {
    let hash_ref = HashReference { hash, timestamp };
    create_entry(EntryTypes::HashReference(hash_ref.clone()))?;
    hc_time_index::index_entry(String::from("current_rev"), hash_ref, LinkTag::new(""), LinkTypes::Index, LinkTypes::TimePath)?;
    Ok(())
}

pub fn update_current_revision(
    hash: HoloHash<holo_hash::hash_type::Action>,
    timestamp: DateTime<Utc>,
) -> SocialContextResult<()> {
    let hash_ref = LocalHashReference { hash, timestamp };
    create_entry(EntryTypes::LocalHashReference(hash_ref.clone()))?;
    Ok(())
}

//Latest revision as seen from the DHT
pub fn latest_revision() -> SocialContextResult<Option<HoloHash<holo_hash::hash_type::Action>>> {
    let mut latest = hc_time_index::get_links_and_load_for_time_span::<HashReference, LinkTypes, LinkTypes>(
        String::from("current_rev"),
        get_now()?,
        DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        None,
        hc_time_index::SearchStrategy::Dfs,
        Some(1),
        LinkTypes::Index, 
        LinkTypes::TimePath
    )?;
    Ok(latest.pop().map(|val| val.hash))
}

//Latest revision as seen from our local state
pub fn current_revision() -> SocialContextResult<Option<HoloHash<holo_hash::hash_type::Action>>> {
    let app_entry = AppEntryType::new(4.into(), 0.into(), EntryVisibility::Private);
    let filter = ChainQueryFilter::new().entry_type(EntryType::App(app_entry)).include_entries(true);
    let mut refs = query(filter)?
        .into_iter()
        .map(|val| {
            val.entry().to_app_option::<LocalHashReference>()?.ok_or(
                SocialContextError::InternalError("Expected element to contain app entry data"),
            )
        })
        .collect::<SocialContextResult<Vec<LocalHashReference>>>()?;
    refs.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());

    Ok(refs.pop().map(|val| val.hash))
}
