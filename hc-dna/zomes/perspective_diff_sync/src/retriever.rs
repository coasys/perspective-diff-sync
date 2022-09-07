use crate::Hash;
use crate::errors::{SocialContextResult, SocialContextError};
use hdk::prelude::*;

pub mod holochain;
pub mod mock;

pub use holochain::HolochainRetreiver;
pub use mock::*;

pub trait PerspectiveDiffRetreiver {
    fn get<T>(hash: Hash) -> SocialContextResult<T> 
        where
        T: TryFrom<SerializedBytes, Error = SerializedBytesError>;

    fn create_entry<I, E, E2>(entry: I) -> SocialContextResult<Hash>
        where
        ScopedEntryDefIndex: for<'a> TryFrom<&'a I, Error = E2>,
        EntryVisibility: for<'a> From<&'a I>,
        Entry: TryFrom<I, Error = E>,
        WasmError: From<E>,
        WasmError: From<E2>
    ;
    fn current_revision() -> SocialContextResult<Option<Hash>>;
    fn latest_revision() -> SocialContextResult<Option<Hash>>;
    fn update_current_revision(rev: Hash) -> SocialContextResult<()>;
    fn update_latest_revision(rev: Hash) -> SocialContextResult<()>;
}


