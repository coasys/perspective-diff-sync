use hc_time_index::IndexableEntry;
use hdk::prelude::*;

use crate::{AgentReference, HashReference, PerspectiveDiff};

impl IndexableEntry for HashReference {
    fn entry_time(&self) -> chrono::DateTime<chrono::Utc> {
        self.timestamp
    }

    fn hash(&self) -> ExternResult<hdk::prelude::HoloHash<holo_hash::hash_type::Entry>> {
        hash_entry(self)
    }
}

impl IndexableEntry for AgentReference {
    fn entry_time(&self) -> chrono::DateTime<chrono::Utc> {
        self.timestamp
    }

    fn hash(&self) -> ExternResult<hdk::prelude::HoloHash<holo_hash::hash_type::Entry>> {
        hash_entry(self)
    }
}

impl PerspectiveDiff {
    pub fn get_sb(self) -> ExternResult<SerializedBytes> {
        self.try_into().map_err(|error| wasm_error!(WasmErrorInner::Host(String::from(error))))
    }
}
