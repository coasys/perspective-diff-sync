use chrono::{DateTime, NaiveDateTime, Utc};
use hdk::prelude::*;
use perspective_diff_sync_integrity::{EntryTypes, HashReference, LinkTypes, LocalHashReference};

use super::PerspectiveDiffRetreiver;
use crate::errors::{SocialContextError, SocialContextResult};
use crate::utils::get_now;
use crate::Hash;

pub struct HolochainRetreiver;

impl PerspectiveDiffRetreiver for HolochainRetreiver {
    fn get<T>(hash: Hash) -> SocialContextResult<T>
    where
        T: TryFrom<SerializedBytes, Error = SerializedBytesError>,
    {
        get(hash, GetOptions::latest())?
            .ok_or(SocialContextError::InternalError(
                "HolochainRetreiver: Could not find entry",
            ))?
            .entry()
            .to_app_option::<T>()?
            .ok_or(SocialContextError::InternalError(
                "Expected element to contain app entry data",
            ))
    }

    fn get_with_timestamp<T>(hash: Hash) -> SocialContextResult<(T, DateTime<Utc>)>
    where
        T: TryFrom<SerializedBytes, Error = SerializedBytesError>,
    {
        let element = get(hash, GetOptions::latest())?;
        let element = element.ok_or(SocialContextError::InternalError(
            "HolochainRetreiver: Could not find entry",
        ))?;
        let entry = element.entry();
        let timestamp = element.action().timestamp().0 as u64;
        let duration = std::time::Duration::from_micros(timestamp);
        let timestamp = DateTime::<Utc>::from_utc(
            NaiveDateTime::from_timestamp(duration.as_secs() as i64, duration.subsec_nanos()),
            Utc,
        );
        let entry = entry
            .to_app_option::<T>()?
            .ok_or(SocialContextError::InternalError(
                "Expected element to contain app entry data",
            ))?;
        Ok((entry, timestamp))
    }

    fn create_entry<I, E: std::fmt::Debug, E2>(entry: I) -> SocialContextResult<Hash>
    where
        ScopedEntryDefIndex: for<'a> TryFrom<&'a I, Error = E2>,
        EntryVisibility: for<'a> From<&'a I>,
        Entry: TryFrom<I, Error = E>,
        WasmError: From<E>,
        WasmError: From<E2>,
    {
        create_entry::<I, E, E2>(entry).map_err(|e| SocialContextError::Wasm(e))
    }

    fn current_revision() -> SocialContextResult<Option<LocalHashReference>> {
        let query = query(
            QueryFilter::new()
                .entry_type(EntryType::App(AppEntryDef {
                    entry_index: 4.into(),
                    zome_index: 0.into(),
                    visibility: EntryVisibility::Private,
                }))
                .include_entries(true)
                .descending(),
        );

        let revision = match query {
            Ok(records) => {
                if records.len() == 0 {
                    None
                } else {
                    let record = records[0].clone();
                    let entry = record
                        .entry
                        .to_app_option::<LocalHashReference>()
                        .unwrap()
                        .unwrap();
                    Some(entry)
                }
            }
            Err(e) => {
                debug!("PerspectiveDiffSync.current_revision(): Error when getting current revision: {:?}", e);
                None
            }
        };
        Ok(revision)
    }

    fn latest_revision() -> SocialContextResult<Option<HashReference>> {
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
        Ok(latest.pop())
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
        Ok(())
    }
}
