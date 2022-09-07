use hdk::prelude::*;
use crate::revisions::*;
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
        current_revision()
    }

    fn latest_revision() -> SocialContextResult<Option<Hash>> {
        latest_revision()
    }

    fn update_current_revision(rev: Hash) -> SocialContextResult<()> {
        let now = get_now()?;
        update_current_revision(rev, now.clone())
    }

    fn update_latest_revision(rev: Hash) -> SocialContextResult<()> {
        let now = get_now()?;
        update_latest_revision(rev, now.clone())
    }
}