use chrono::{DateTime, NaiveDateTime, Utc};
use hdk::prelude::*;
use perspective_diff_sync_integrity::{EntryTypes, HashAnchor, HashReference, LocalHashReference, LinkTypes};

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
    hc_time_index::index_entry(String::from("current_rev"), hash_ref, LinkTag::new(""))?;
    Ok(())
}

pub fn update_current_revision(
    hash: HoloHash<holo_hash::hash_type::Action>,
    timestamp: DateTime<Utc>,
) -> SocialContextResult<()> {
    let hash_anchor = hash_entry(HashAnchor(String::from("current_hashes")))?;
    let hash_ref = LocalHashReference { hash, timestamp };
    create_entry(EntryTypes::LocalHashReference(hash_ref.clone()))?;
    create_link(
        hash_anchor,
        hash_entry(hash_ref)?,
        LinkTypes::HashRef,
        LinkTag::new(String::from("")),
    )?;
    Ok(())
}

//Latest revision as seen from the DHT
pub fn latest_revision() -> SocialContextResult<Option<HoloHash<holo_hash::hash_type::Action>>> {
    let mut latest = hc_time_index::get_links_and_load_for_time_span::<HashReference>(
        String::from("current_rev"),
        get_now()?,
        DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        None,
        hc_time_index::SearchStrategy::Dfs,
        Some(1),
    )?;
    Ok(latest.pop().map(|val| val.hash))
}

//Latest revision as seen from our local state
pub fn current_revision() -> SocialContextResult<Option<HoloHash<holo_hash::hash_type::Action>>> {
    let hash_anchor = hash_entry(HashAnchor(String::from("current_hashes")))?;
    let links = get_links(hash_anchor.clone(), LinkTypes::HashRef, None)?;

    let mut refs = links
        .into_iter()
        .map(|link| match get(link.target, GetOptions::latest())? {
            Some(chunk) => Ok(Some(
                chunk.entry().to_app_option::<LocalHashReference>()?.ok_or(
                    SocialContextError::InternalError("Expected element to contain app entry data"),
                )?,
            )),
            None => Ok(None),
        })
        .filter_map(|val| {
            if val.is_ok() {
                let val = val.unwrap();
                if val.is_some() {
                    Some(Ok(val.unwrap()))
                } else {
                    None
                }
            } else {
                Some(Err(val.err().unwrap()))
            }
        })
        .collect::<SocialContextResult<Vec<LocalHashReference>>>()?;
    refs.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());

    Ok(refs.pop().map(|val| val.hash))
}
