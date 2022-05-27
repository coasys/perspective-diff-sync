use hdk::prelude::*;
use chrono::{Utc, DateTime, NaiveDateTime};

use crate::{
    LocalHashReference, HashAnchor, HashReference,
    errors::{SocialContextResult, SocialContextError}
};
use crate::utils::get_now;

pub fn update_latest_revision(hash: HoloHash<holo_hash::hash_type::Header>, timestamp: DateTime<Utc>) -> SocialContextResult<()> {
    let hash_ref = HashReference {
        hash,
        timestamp
    };
    create_entry(hash_ref.clone())?;
    hc_time_index::index_entry(String::from("current_rev"), hash_ref, LinkTag::new(""))?;
    Ok(())
}

pub fn update_current_revision(hash: HoloHash<holo_hash::hash_type::Header>, timestamp: DateTime<Utc>) -> SocialContextResult<()> {
    let hash_anchor = hash_entry(HashAnchor(String::from("current_hashes")))?;
    let hash_ref = LocalHashReference {
        hash,
        timestamp
    };
    create_entry(hash_ref.clone())?;
    create_link(
        hash_anchor, 
        hash_entry(hash_ref)?, 
        LinkTag::new(String::from(""))
    )?;
    Ok(())
}

//Latest revision as seen from the DHT
pub fn latest_revision() -> SocialContextResult<Option<HoloHash<holo_hash::hash_type::Header>>> {
    let mut latest = hc_time_index::get_links_and_load_for_time_span::<HashReference>(
        String::from("current_rev"), get_now()?, DateTime::<Utc>::from_utc(
            NaiveDateTime::from_timestamp(0, 0),
            Utc,
        ), 
        None, 
        hc_time_index::SearchStrategy::Dfs,
        Some(1)
    )?;
    Ok(latest.pop().map(|val| val.hash))
}

//Latest revision as seen from our local state
pub fn current_revision() -> SocialContextResult<Option<HoloHash<holo_hash::hash_type::Header>>> {
    let hash_anchor = hash_entry(HashAnchor(String::from("current_hashes")))?;
    let links = get_links(hash_anchor.clone(), None)?;

    let mut refs = links.into_iter()
        .map(|link| match get(link.target, GetOptions::latest())? {
            Some(chunk) => Ok(Some(chunk.entry().to_app_option::<LocalHashReference>()?.ok_or(
                SocialContextError::InternalError("Expected element to contain app entry data"),
            )?)),
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
