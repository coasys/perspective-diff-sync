#[macro_use]
extern crate lazy_static;

use chrono::{DateTime, Utc};
use hdk::prelude::*;
use lazy_static::lazy_static;
use perspective_diff_sync_integrity::{Perspective, PerspectiveDiff};

mod commit;
mod errors;
mod inputs;
mod pull;
mod render;
mod revisions;
//mod search;
mod snapshots;
mod topo_sort;
mod utils;
mod workspace;
mod retriever;
mod tests;

#[macro_use] extern crate maplit;

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
    Ok(InitCallbackResult::Pass)
}

#[hdk_extern]
fn recv_remote_signal(signal: SerializedBytes) -> ExternResult<()> {
    let sig: PerspectiveDiff = PerspectiveDiff::try_from(signal.clone())
        .map_err(|error| utils::err(&format!("{}", error)))?;
    Ok(emit_signal(&sig)?)
}

#[hdk_extern]
pub fn commit(diff: PerspectiveDiff) -> ExternResult<HoloHash<holo_hash::hash_type::Action>> {
    commit::commit(diff).map_err(|error| utils::err(&format!("{}", error)))
}

#[hdk_extern]
pub fn add_active_agent_link(_: ()) -> ExternResult<Option<DateTime<Utc>>> {
    commit::add_active_agent_link().map_err(|error| utils::err(&format!("{}", error)))
}

#[hdk_extern]
pub fn latest_revision(_: ()) -> ExternResult<Option<HoloHash<holo_hash::hash_type::Action>>> {
    revisions::latest_revision().map_err(|error| utils::err(&format!("{}", error)))
}

#[hdk_extern]
pub fn current_revision(_: ()) -> ExternResult<Option<HoloHash<holo_hash::hash_type::Action>>> {
    revisions::current_revision().map_err(|error| utils::err(&format!("{}", error)))
}

#[hdk_extern]
pub fn pull(_: ()) -> ExternResult<PerspectiveDiff> {
    pull::pull()
        .map_err(|error| utils::err(&format!("{}", error)))
        .map(|res| res)
}

#[hdk_extern]
pub fn render(_: ()) -> ExternResult<Perspective> {
    render::render().map_err(|error| utils::err(&format!("{}", error)))
}

#[hdk_extern]
pub fn update_current_revision(_hash: HoloHash<holo_hash::hash_type::Action>) -> ExternResult<()> {
    #[cfg(feature = "test")]
    {
        revisions::update_current_revision(_hash, utils::get_now().unwrap())
            .map_err(|err| utils::err(&format!("{}", err)))?;
    }
    Ok(())
}

#[hdk_extern]
pub fn update_latest_revision(_hash: HoloHash<holo_hash::hash_type::Action>) -> ExternResult<()> {
    #[cfg(feature = "test")]
    {
        revisions::update_latest_revision(_hash, utils::get_now().unwrap())
            .map_err(|err| utils::err(&format!("{}", err)))?;
    }
    Ok(())
}

//not loading from DNA properies since dna zome properties is always null for some reason
lazy_static! {
    pub static ref ACTIVE_AGENT_DURATION: chrono::Duration = chrono::Duration::seconds(300);
    pub static ref ENABLE_SIGNALS: bool = true;
    //TODO: 1 is a test value; this should be updated to a higher value for production
    pub static ref SNAPSHOT_INTERVAL: usize = 100;
}
