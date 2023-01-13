#[macro_use]
extern crate lazy_static;

use hdk::prelude::*;
use lazy_static::lazy_static;

use perspective_diff_sync_integrity::{Perspective, PerspectiveDiff, PerspectiveDiffReference};

mod chunked_diffs;
mod commit;
mod errors;
mod inputs;
mod pull;
mod render;
mod retriever;
mod revisions;
mod snapshots;
mod test_graphs;
mod tests;
mod topo_sort;
mod utils;
mod workspace;

#[macro_use]
extern crate maplit;

pub type Hash = HoloHash<holo_hash::hash_type::Action>;

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
    commit::add_active_agent_link::<retriever::HolochainRetreiver>()
        .map_err(|error| utils::err(&format!("{}", error)))?;
    Ok(InitCallbackResult::Pass)
}

#[hdk_extern]
fn recv_remote_signal(signal: SerializedBytes) -> ExternResult<()> {
    let sig: PerspectiveDiffReference = PerspectiveDiffReference::try_from(signal.clone())
        .map_err(|error| utils::err(&format!("{}", error)))?;
    emit_signal(sig)?;
    Ok(())
}

#[hdk_extern]
pub fn commit(diff: PerspectiveDiff) -> ExternResult<Hash> {
    commit::commit::<retriever::HolochainRetreiver>(diff)
        .map_err(|error| utils::err(&format!("{}", error)))
}

#[hdk_extern]
pub fn latest_revision(_: ()) -> ExternResult<Option<Hash>> {
    revisions::latest_revision::<retriever::HolochainRetreiver>()
        .map_err(|error| utils::err(&format!("{}", error)))
        .map(|val| val.map(|val| val.hash))
}

#[hdk_extern]
pub fn current_revision(_: ()) -> ExternResult<Option<Hash>> {
    revisions::current_revision::<retriever::HolochainRetreiver>()
        .map_err(|error| utils::err(&format!("{}", error)))
        .map(|val| val.map(|val| val.hash))
}

#[hdk_extern]
pub fn pull(_: ()) -> ExternResult<PerspectiveDiff> {
    pull::pull::<retriever::HolochainRetreiver>()
        .map_err(|error| utils::err(&format!("{}", error)))
        .map(|res| res)
}

#[hdk_extern]
pub fn render(_: ()) -> ExternResult<Perspective> {
    render::render::<retriever::HolochainRetreiver>()
        .map_err(|error| utils::err(&format!("{}", error)))
}

#[hdk_extern]
pub fn update_current_revision(_hash: Hash) -> ExternResult<()> {
    #[cfg(feature = "test")]
    {
        revisions::update_current_revision::<retriever::HolochainRetreiver>(
            _hash,
            utils::get_now().unwrap(),
        )
        .map_err(|err| utils::err(&format!("{}", err)))?;
    }
    Ok(())
}

#[hdk_extern]
pub fn update_latest_revision(_hash: Hash) -> ExternResult<()> {
    #[cfg(feature = "test")]
    {
        revisions::update_latest_revision::<retriever::HolochainRetreiver>(
            _hash,
            utils::get_now().unwrap(),
        )
        .map_err(|err| utils::err(&format!("{}", err)))?;
    }
    Ok(())
}

#[hdk_extern]
pub fn fast_forward_signal(perspective_diff_ref: PerspectiveDiffReference) -> ExternResult<()> {
    pull::fast_forward_signal::<retriever::HolochainRetreiver>(perspective_diff_ref)
        .map_err(|error| utils::err(&format!("{}", error)))
}

//not loading from DNA properies since dna zome properties is always null for some reason
lazy_static! {
    pub static ref ACTIVE_AGENT_DURATION: chrono::Duration = chrono::Duration::seconds(3600);
    pub static ref ENABLE_SIGNALS: bool = true;
    pub static ref SNAPSHOT_INTERVAL: usize = 100;
    pub static ref CHUNK_SIZE: u16 = 10000;
}
