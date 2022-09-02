use chrono::{DateTime, NaiveDateTime, Utc};
use hdk::prelude::*;
use perspective_diff_sync_integrity::{EntryTypes, HashReference, LinkTypes, LocalHashReference};

use crate::errors::SocialContextResult;
use crate::utils::get_now;
use crate::Hash;

pub fn update_latest_revision(
    hash: Hash,
    timestamp: DateTime<Utc>,
) -> SocialContextResult<()> {
    let hash_ref = HashReference { hash, timestamp };
    create_entry(EntryTypes::HashReference(hash_ref.clone()))?;
    hc_time_index::index_entry(
        String::from("current_rev"),
        hash_ref,
        LinkTag::new(""),
        LinkTypes::Index,
        LinkTypes::TimePath,
    )?;
    Ok(())
}

pub fn update_current_revision(
    hash: Hash,
    timestamp: DateTime<Utc>,
) -> SocialContextResult<()> {
    let hash_ref = LocalHashReference { hash, timestamp };
    create_entry(EntryTypes::LocalHashReference(hash_ref.clone()))?;
    Ok(())
}

//Latest revision as seen from the DHT
pub fn latest_revision() -> SocialContextResult<Option<Hash>> {
    let mut latest =
        hc_time_index::get_links_and_load_for_time_span::<HashReference, LinkTypes, LinkTypes>(
            String::from("current_rev"),
            get_now()?,
            DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            None,
            hc_time_index::SearchStrategy::Dfs,
            Some(1),
            LinkTypes::Index,
            LinkTypes::TimePath,
        )?;
    Ok(latest.pop().map(|val| val.hash))
}

//Latest revision as seen from our local state
pub fn current_revision() -> SocialContextResult<Option<Hash>> {
    let chain_head = agent_info()?.chain_head;
    let mut record = get_details(chain_head.0, GetOptions::latest())?.unwrap();
    let mut revision = None;

    while revision.is_none() {
        match record {
            Details::Record(record_details) => {
                let entry = record_details.record.entry.to_app_option::<LocalHashReference>();
                
                match entry {
                    Ok(deser_entry) => match deser_entry {
                        Some(local_hash_reference) => revision = Some(local_hash_reference),
                        None => {
                            debug!("Not a LocalHashReference, moving on...")
                        }
                    },
                    Err(_err) => {
                        debug!("Not a LocalHashReference, moving on...")
                    }
                }
                let prev_action = record_details.record.action().prev_action();
                match prev_action {
                    Some(prev_action) => {
                        record = get_details(prev_action.to_owned(), GetOptions::latest())?.unwrap();
                    },
                    None => break
                }
            },
            _ => unreachable!()
        }
    }

    Ok(revision.map(|val| val.hash))
}
