use hdk::prelude::*;
use hc_time_index::IndexableEntry;

use crate::HashReference;

impl IndexableEntry for HashReference {
    fn entry_time(&self) -> chrono::DateTime<chrono::Utc> {
        self.timestamp
    }

    fn hash(&self) -> hdk::map_extern::ExternResult<holo_hash::EntryHash> {
        hash_entry(self)
    }
}