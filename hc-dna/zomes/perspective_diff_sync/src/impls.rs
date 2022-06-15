use hc_time_index::IndexableEntry;
use hdk::prelude::*;

use crate::{AgentReference, HashReference, PerspectiveDiff};

impl IndexableEntry for HashReference {
    fn entry_time(&self) -> chrono::DateTime<chrono::Utc> {
        self.timestamp
    }

    fn hash(&self) -> hdk::map_extern::ExternResult<holo_hash::EntryHash> {
        hash_entry(self)
    }
}

impl IndexableEntry for AgentReference {
    fn entry_time(&self) -> chrono::DateTime<chrono::Utc> {
        self.timestamp
    }

    fn hash(&self) -> hdk::map_extern::ExternResult<holo_hash::EntryHash> {
        hash_entry(self)
    }
}

impl PerspectiveDiff {
    pub fn get_sb(self) -> ExternResult<SerializedBytes> {
        Ok(self.try_into()?)
    }
}
