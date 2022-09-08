use hdk::prelude::*;
use perspective_diff_sync_integrity::{EntryTypes, HashReference, LinkTypes, LocalHashReference, LocalTimestampReference};
use chrono::{DateTime, NaiveDateTime, Utc};

use crate::utils::get_now;
use crate::Hash;
use crate::errors::{SocialContextResult, SocialContextError};
use super::PerspectiveDiffRetreiver;


pub struct HolochainRetreiver;

impl PerspectiveDiffRetreiver for HolochainRetreiver {
    fn get<T>(hash: Hash) -> SocialContextResult<T> 
        where
        T: TryFrom<SerializedBytes, Error = SerializedBytesError>,
    {
        get(hash, GetOptions::latest())?
            .ok_or(SocialContextError::InternalError(
                "HolochainRetreiver: Could not find entry while populating search",
            ))?
            .entry()
            .to_app_option::<T>()?
            .ok_or(SocialContextError::InternalError(
                "Expected element to contain app entry data",
            ))
    }

    fn create_entry<I, E: std::fmt::Debug, E2>(entry: I) -> SocialContextResult<Hash>
        where
        ScopedEntryDefIndex: for<'a> TryFrom<&'a I, Error = E2>,
        EntryVisibility: for<'a> From<&'a I>,
        Entry: TryFrom<I, Error = E>,
        WasmError: From<E>,
        WasmError: From<E2>
    {
        create_entry::<I,E,E2>(entry).map_err(|e| SocialContextError::Wasm(e)) 
    }

    fn current_revision() -> SocialContextResult<Option<Hash>> {
        let revision = get_latest_local_entry::<LocalHashReference>()?;
        Ok(revision.map(|val| val.hash))
    }

    fn latest_revision() -> SocialContextResult<Option<Hash>> {
        //Get the last latest revision to help reduce index search space
        let last_latest = match get_latest_local_entry::<LocalTimestampReference>()? {
            Some(last_seen_latest) => last_seen_latest.timestamp,
            None => DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc)
        };

        //Get the latest hash reference
        let mut latest =
        hc_time_index::get_links_and_load_for_time_span::<HashReference, LinkTypes, LinkTypes>(
            String::from("current_rev"),
            get_now()?,
            last_latest,
            None,
            hc_time_index::SearchStrategy::Dfs,
            Some(1),
            LinkTypes::Index,
            LinkTypes::TimePath
        )?;
        let latest = latest.pop();

        if latest.is_some() {
            //Check if latest == last latest we saw, if not then save this latest for future reference
            if latest.clone().unwrap().timestamp != last_latest {
                //Save this latest entry so we can use it in future queries 
                let timestamp_ref = LocalTimestampReference {
                    timestamp: latest.clone().unwrap().timestamp
                };
                create_entry(EntryTypes::LocalTimestampReference(timestamp_ref))?;
            };
            Ok(latest.map(|val| val.hash))
        } else {
            debug!("Didnt find any DHT revision since the last time we received one... Just using our current revision...");
            //There was no latest found in the time bounds above, so we should return the last latest we received, which should be current
            HolochainRetreiver::current_revision()
        }
    }

    fn update_current_revision(hash: Hash, timestamp: DateTime<Utc>) -> SocialContextResult<()> {
        let hash_ref = LocalHashReference { hash, timestamp };
        create_entry(EntryTypes::LocalHashReference(hash_ref.clone()))?;
        Ok(())
    }

    fn update_latest_revision(hash: Hash, timestamp: DateTime<Utc>) -> SocialContextResult<()> {
        let hash_ref = HashReference { hash, timestamp };
        create_entry(EntryTypes::HashReference(hash_ref.clone()))?;
        hc_time_index::index_entry(
            String::from("current_rev"),
            hash_ref,
            LinkTag::new(""),
            LinkTypes::Index,
            LinkTypes::TimePath,
        )?;

        //Create local timestamp reference for the future
        let timestamp_ref = LocalTimestampReference {
            timestamp
        };
        create_entry(EntryTypes::LocalTimestampReference(timestamp_ref))?;

        Ok(())
    }
}

fn get_latest_local_entry<T>() -> SocialContextResult<Option<T>> where T: TryFrom<SerializedBytes, Error = SerializedBytesError> {
    let chain_head = agent_info()?.chain_head;
    let mut record = get_details(chain_head.0, GetOptions::latest())?.unwrap();
    let mut found_entry = None;

    while found_entry.is_none() {
        match record {
            Details::Record(record_details) => {
                let entry = record_details.record.entry.to_app_option::<T>();
                
                match entry {
                    Ok(deser_entry) => match deser_entry {
                        Some(deser_res) => found_entry = Some(deser_res),
                        None => {
                            debug!("Not T, moving on...")
                        }
                    },
                    Err(_err) => {
                        debug!("Not T, moving on...")
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

    Ok(found_entry)
}