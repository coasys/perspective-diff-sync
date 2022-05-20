use hdk::prelude::*;
use chrono::{Utc, DateTime, NaiveDateTime};

use crate::{
    errors::{SocialContextResult, SocialContextError}, PerspectiveDiffEntry
};
use crate::{
    Perspective, PerspectiveDiff, LocalHashReference, HashAnchor, HashReference
};

fn get_now() -> SocialContextResult<DateTime<Utc>> {
    let now = sys_time()?.as_seconds_and_nanos();
    Ok(DateTime::<Utc>::from_utc(
        NaiveDateTime::from_timestamp(now.0, now.1),
        Utc,
    ))
}

fn update_latest_revision(hash: HoloHash<holo_hash::hash_type::Header>, timestamp: DateTime<Utc>) -> SocialContextResult<()> {
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

fn update_current_revision(hash: HoloHash<holo_hash::hash_type::Header>, timestamp: DateTime<Utc>) -> SocialContextResult<()> {
    let hash_ref = HashReference {
        hash,
        timestamp
    };
    create_entry(hash_ref.clone())?;
    hc_time_index::index_entry(String::from("current_rev"), hash_ref, LinkTag::new(""))?;
    Ok(())
}

pub fn latest_revision() -> SocialContextResult<Option<HoloHash<holo_hash::hash_type::Header>>> {
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

pub fn current_revision() -> SocialContextResult<Option<HoloHash<holo_hash::hash_type::Header>>> {
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

pub fn pull() -> SocialContextResult<PerspectiveDiff> {
    let latest = latest_revision()?;
    let current = current_revision()?;

    if latest != current {
        Ok(PerspectiveDiff {
            removals: vec![],
            additions: vec![]
        })
    } else {
        Ok(PerspectiveDiff {
            removals: vec![],
            additions: vec![]
        })
    }
}

pub fn render() -> SocialContextResult<Perspective> {
    Ok(Perspective {
        links: vec![]
    })
}

pub fn commit(diff: PerspectiveDiff) -> SocialContextResult<HoloHash<holo_hash::hash_type::Header>> {
    //if(currentRevision != currentRevision) pull()

    let parent = current_revision()?;
    let diff_entry = PerspectiveDiffEntry {
        diff,
        parent
    };
    let diff_entry_create = create_entry(diff_entry)?;
    let now = get_now()?;

    update_latest_revision(diff_entry_create.clone(), now.clone())?;
    update_current_revision(diff_entry_create.clone(), now)?;

    //TODO: send signal to active agents

    Ok(diff_entry_create)
}

pub fn add_active_agent_link() -> SocialContextResult<()> {
    Ok(())
}