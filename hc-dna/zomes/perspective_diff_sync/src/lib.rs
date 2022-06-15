use chrono::{DateTime, Utc};
use hdk::prelude::*;
use lazy_static::lazy_static;

mod commit;
mod errors;
mod impls;
mod inputs;
mod pull;
mod render;
mod revisions;
mod search;
mod snapshots;
mod utils;

use inputs::*;

#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Eq, Hash)]
pub struct LinkExpression {
    pub author: String,
    pub data: Triple,
    pub timestamp: DateTime<Utc>,
    pub proof: ExpressionProof,
}

#[hdk_entry(id = "perspective_diff", visibility = "public")]
#[derive(Clone)]
pub struct PerspectiveDiff {
    pub additions: Vec<LinkExpression>,
    pub removals: Vec<LinkExpression>,
}

#[hdk_entry(id = "snapshot", visibility = "public")]
#[derive(Clone)]
pub struct Snapshot {
    pub diff: HoloHash<holo_hash::hash_type::Header>,
    pub diff_graph: Vec<(
        HoloHash<holo_hash::hash_type::Header>,
        PerspectiveDiffEntryReference,
    )>,
}

#[hdk_entry(id = "perspective_diff_entry_reference", visibility = "public")]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PerspectiveDiffEntryReference {
    pub diff: HoloHash<holo_hash::hash_type::Header>,
    pub parents: Option<Vec<HoloHash<holo_hash::hash_type::Header>>>,
}

#[derive(Clone, Serialize, Debug)]
pub struct Perspective {
    pub links: Vec<LinkExpression>,
}

//TODO: this can likely be removed and instead just reference the PerspectiveDiffEntry/MergeEntry directly?
#[hdk_entry(id = "hash_reference", visibility = "public")]
#[derive(Clone)]
pub struct HashReference {
    pub hash: HoloHash<holo_hash::hash_type::Header>,
    pub timestamp: DateTime<Utc>,
}

#[hdk_entry(id = "local_hash_reference", visibility = "private")]
#[derive(Clone)]
pub struct LocalHashReference {
    pub hash: HoloHash<holo_hash::hash_type::Header>,
    pub timestamp: DateTime<Utc>,
}

#[hdk_entry(id = "hash_anchor", visibility = "private")]
#[derive(Clone)]
pub struct HashAnchor(String);

#[hdk_entry(id = "agent_reference", visbility = "public")]
#[derive(Clone)]
pub struct AgentReference {
    pub agent: AgentPubKey,
    pub timestamp: DateTime<Utc>,
}

entry_defs![
    PerspectiveDiff::entry_def(),
    PerspectiveDiffEntryReference::entry_def(),
    HashReference::entry_def(),
    LocalHashReference::entry_def(),
    AgentReference::entry_def(),
    HashAnchor::entry_def(),
    PathEntry::entry_def(),
    Snapshot::entry_def()
];

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    let mut functions: GrantedFunctions = BTreeSet::new();
    functions.insert((
        ZomeName::from("perspective_diff_sync"),
        "recv_remote_signal".into(),
    ));

    create_cap_grant(CapGrantEntry {
        tag: "".into(),
        // empty access converts to unrestricted
        access: ().into(),
        functions,
    })?;
    //Create the initial entry which will be updated to keep the current_revision
    create_entry(HashAnchor(String::from("current_hashes")))?;
    Ok(InitCallbackResult::Pass)
}

#[hdk_extern]
fn recv_remote_signal(signal: SerializedBytes) -> ExternResult<()> {
    let sig: PerspectiveDiff = PerspectiveDiff::try_from(signal.clone())?;
    Ok(emit_signal(&sig)?)
}

#[hdk_extern]
pub fn commit(diff: PerspectiveDiff) -> ExternResult<HoloHash<holo_hash::hash_type::Header>> {
    commit::commit(diff).map_err(|err| WasmError::Host(err.to_string()))
}

#[hdk_extern]
pub fn add_active_agent_link(_: ()) -> ExternResult<Option<DateTime<Utc>>> {
    commit::add_active_agent_link().map_err(|err| WasmError::Host(err.to_string()))
}

#[hdk_extern]
pub fn latest_revision(_: ()) -> ExternResult<Option<HoloHash<holo_hash::hash_type::Header>>> {
    revisions::latest_revision().map_err(|err| WasmError::Host(err.to_string()))
}

#[hdk_extern]
pub fn current_revision(_: ()) -> ExternResult<Option<HoloHash<holo_hash::hash_type::Header>>> {
    revisions::current_revision().map_err(|err| WasmError::Host(err.to_string()))
}

#[hdk_extern]
pub fn pull(_: ()) -> ExternResult<PerspectiveDiff> {
    pull::pull()
        .map_err(|err| WasmError::Host(err.to_string()))
        .map(|res| res)
}

#[hdk_extern]
pub fn render(_: ()) -> ExternResult<Perspective> {
    render::render().map_err(|err| WasmError::Host(err.to_string()))
}

#[hdk_extern]
pub fn update_current_revision(_hash: HoloHash<holo_hash::hash_type::Header>) -> ExternResult<()> {
    #[cfg(feature = "test")]
    {
        revisions::update_current_revision(_hash, utils::get_now().unwrap())
            .map_err(|err| WasmError::Host(err.to_string()))?;
    }
    Ok(())
}

#[hdk_extern]
pub fn update_latest_revision(_hash: HoloHash<holo_hash::hash_type::Header>) -> ExternResult<()> {
    #[cfg(feature = "test")]
    {
        revisions::update_latest_revision(_hash, utils::get_now().unwrap())
            .map_err(|err| WasmError::Host(err.to_string()))?;
    }
    Ok(())
}

//not loading from DNA properies since dna zome properties is always null for some reason
lazy_static! {
    pub static ref ACTIVE_AGENT_DURATION: chrono::Duration = chrono::Duration::seconds(300);
    pub static ref ENABLE_SIGNALS: bool = true;
    //TODO: 1 is a test value; this should be updated to a higher value for production
    pub static ref SNAPSHOT_INTERVAL: usize = 2;
}
