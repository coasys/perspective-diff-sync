use crate::Hash;
use crate::errors::{SocialContextResult, SocialContextError};

mod holochain;
mod mock;

pub trait PerspectiveDiffRetreiver {
    fn get<T>(hash: Hash) -> SocialContextResult<T>;
    fn create_entry<T>(entry: T) -> SocialContextResult<Hash>;
    fn current_revision() -> SocialContextResult<Option<Hash>>;
    fn latest_revision() -> SocialContextResult<Option<Hash>>;
    fn update_current_revision(rev: Hash) -> SocialContextResult<()>;
    fn update_latest_revision(rev: Hash) -> SocialContextResult<()>;
}


