use chrono::{DateTime, NaiveDateTime, Utc};
use hdk::prelude::*;
use std::hash::Hash;

use crate::errors::SocialContextResult;
use perspective_diff_sync_integrity::{LinkExpression, Triple, ExpressionProof};

pub fn get_now() -> SocialContextResult<DateTime<Utc>> {
    match sys_time() {
        Ok(time) => {
            let now = time.as_seconds_and_nanos();
            Ok(DateTime::<Utc>::from_utc(
                NaiveDateTime::from_timestamp(now.0, now.1),
                Utc,
            ))
        },
        Err(_err) => {
            Ok(Utc::now())
        }
    }
}

pub fn dedup<T: Eq + Hash + Clone>(vs: &Vec<T>) -> Vec<T> {
    let hs = vs.iter().cloned().collect::<HashSet<T>>();

    hs.into_iter().collect()
}

pub(crate) fn err(reason: &str) -> WasmError {
    wasm_error!(WasmErrorInner::Host(String::from(reason)))
}

pub fn remove_from_vec<T: PartialEq>(vec: &mut Vec<T>, values: &Vec<T>) {
    let mut i = 0;
    while i < vec.len() {
        if values.contains(&vec[i]) {
            vec.remove(i);
        } else {
            i += 1;
        }
    }
}

#[allow(dead_code)]
pub fn create_link_expression(source: &str, target: &str) -> LinkExpression {
    LinkExpression {
        author: String::from("Test author"),
        data: Triple {
            source: Some(String::from(source)),
            predicate: None,
            target: Some(String::from(target))
        },
        timestamp: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        proof: ExpressionProof {
            signature: String::from("sig"),
            key: String::from("key"),
        },
    }
}