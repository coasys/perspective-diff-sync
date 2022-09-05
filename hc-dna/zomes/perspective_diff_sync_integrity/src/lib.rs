use chrono::{DateTime, Utc};
use core::cmp::Ordering;
use hdi::prelude::*;

pub mod impls;

#[derive(Serialize, Deserialize, Clone, SerializedBytes, Debug, PartialEq, Eq, Hash)]
pub struct ExpressionProof {
    pub signature: String,
    pub key: String,
}

#[derive(Serialize, Deserialize, Clone, SerializedBytes, Debug, PartialEq, Eq, Hash)]
pub struct Triple {
    pub source: Option<String>,
    pub target: Option<String>,
    pub predicate: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq, Hash)]
pub struct LinkExpression {
    pub author: String,
    pub data: Triple,
    pub timestamp: DateTime<Utc>,
    pub proof: ExpressionProof,
}

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct PerspectiveDiff {
    pub additions: Vec<LinkExpression>,
    pub removals: Vec<LinkExpression>,
}

impl PerspectiveDiff {
    pub fn new() -> Self {
        Self {
            additions: Vec::new(),
            removals: Vec::new(),
        }
    }
    pub fn total_diff_number(&self) -> usize {
        self.additions.len() + self.removals.len()
    }
}

app_entry!(PerspectiveDiff);

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct Snapshot {
    pub diff_chunks: Vec<HoloHash<holo_hash::hash_type::Action>>,
    pub included_diffs: Vec<HoloHash<holo_hash::hash_type::Action>>,
    //pub diff_graph: Vec<(
    //    HoloHash<holo_hash::hash_type::Action>,
    //    PerspectiveDiffEntryReference,
    //)>,
}

app_entry!(Snapshot);

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Eq, Hash)]
pub struct PerspectiveDiffEntryReference {
    pub diff: HoloHash<holo_hash::hash_type::Action>,
    pub parents: Option<Vec<HoloHash<holo_hash::hash_type::Action>>>,
}

impl PartialOrd for PerspectiveDiffEntryReference {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.diff.partial_cmp(&other.diff)
    }
}

impl Ord for PerspectiveDiffEntryReference {
    fn cmp(&self, other: &Self) -> Ordering {
        self.diff.cmp(&other.diff)
    }
}

app_entry!(PerspectiveDiffEntryReference);

#[derive(Clone, Serialize, Debug)]
pub struct Perspective {
    pub links: Vec<LinkExpression>,
}

//TODO: this can likely be removed and instead just reference the PerspectiveDiffEntry/MergeEntry directly?
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct HashReference {
    pub hash: HoloHash<holo_hash::hash_type::Action>,
    pub timestamp: DateTime<Utc>,
}

app_entry!(HashReference);

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct LocalHashReference {
    pub hash: HoloHash<holo_hash::hash_type::Action>,
    pub timestamp: DateTime<Utc>,
}

app_entry!(LocalHashReference);

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes)]
pub struct AgentReference {
    pub agent: AgentPubKey,
    pub timestamp: DateTime<Utc>,
}

app_entry!(AgentReference);

#[hdk_entry_defs]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    #[entry_def(visibility = "public")]
    PerspectiveDiff(PerspectiveDiff),
    #[entry_def(visibility = "public")]
    Snapshot(Snapshot),
    #[entry_def(visibility = "public")]
    HashReference(HashReference),
    #[entry_def(visibility = "public")]
    PerspectiveDiffEntryReference(PerspectiveDiffEntryReference),
    #[entry_def(visibility = "private")]
    LocalHashReference(LocalHashReference),
    #[entry_def(visibility = "public")]
    AgentReference(AgentReference),
}

#[hdk_link_types]
pub enum LinkTypes {
    Snapshot,
    ActiveAgent,
    HashRef,
    TimePath,
    Index,
}
