use hdk::prelude::*;
use holo_hash::error::HoloHashError;
use std::convert::Infallible;

#[derive(thiserror::Error, Debug)]
pub enum SocialContextError {
    #[error(transparent)]
    Serialization(#[from] SerializedBytesError),
    #[error(transparent)]
    Infallible(#[from] Infallible),
    #[error(transparent)]
    EntryError(#[from] EntryError),
    #[error(transparent)]
    Wasm(#[from] WasmError),
    #[error(transparent)]
    HoloHashError(#[from] HoloHashError),
    #[error("Internal Error. Error: {0}")]
    InternalError(&'static str),
    #[error("No common ancestor found")]
    NoCommonAncestorFound,
}

pub type SocialContextResult<T> = Result<T, SocialContextError>;
