use chrono::{DateTime, NaiveDateTime, Utc};
use hdk::prelude::*;
use std::hash::Hash;

use crate::errors::SocialContextResult;

pub fn get_now() -> SocialContextResult<DateTime<Utc>> {
    let now = sys_time()?.as_seconds_and_nanos();
    Ok(DateTime::<Utc>::from_utc(
        NaiveDateTime::from_timestamp(now.0, now.1),
        Utc,
    ))
}

pub fn dedup<T: Eq + Hash + Clone>(vs: &Vec<T>) -> Vec<T> {
    let hs = vs.iter().cloned().collect::<HashSet<T>>();

    hs.into_iter().collect()
}

pub(crate) fn err(reason: &str) -> WasmError {
    wasm_error!(WasmErrorInner::Host(String::from(reason)))
}
