use chrono::{DateTime, Utc};
use hdk::prelude::*;
use lazy_static::lazy_static;

mod errors;
mod inputs;
mod methods;
mod impls;
mod search;

use inputs::*;

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct LinkExpression {
    pub author: String,
    pub data: Triple,
    pub timestamp: DateTime<Utc>,
    pub proof: ExpressionProof,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct PerspectiveDiff {
    pub additions: Vec<LinkExpression>,
    pub removals: Vec<LinkExpression>,
}

#[hdk_entry(id = "perspective_diff_entry", visibility = "public")]
#[serde(rename_all = "camelCase")]
#[derive(Clone)]
pub struct PerspectiveDiffEntry {
    pub diff: PerspectiveDiff,
    pub parents: Option<Vec<HoloHash<holo_hash::hash_type::Header>>>,
}

#[derive(Clone, Serialize, Debug)]
pub struct Perspective {
    pub links: Vec<LinkExpression>
}

//TODO: this can likely be removed and instead just reference the PerspectiveDiffEntry/MergeEntry directly?
#[hdk_entry(id = "hash_reference", visibility = "public")]
#[derive(Clone)]
pub struct HashReference {
    pub hash: HoloHash<holo_hash::hash_type::Header>,
    pub timestamp: DateTime<Utc>
}

#[hdk_entry(id = "local_hash_reference", visibility = "private")]
#[derive(Clone)]
pub struct LocalHashReference {
    pub hash: HoloHash<holo_hash::hash_type::Header>,
    pub timestamp: DateTime<Utc>
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

entry_defs![PerspectiveDiffEntry::entry_def(), HashReference::entry_def(), LocalHashReference::entry_def(), AgentReference::entry_def(), HashAnchor::entry_def(), PathEntry::entry_def()];

#[hdk_extern]
fn init(_: ()) -> ExternResult<InitCallbackResult> {
    let mut functions: GrantedFunctions = BTreeSet::new();
    functions.insert((ZomeName::from("social_context"), "recv_remote_signal".into()));

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
    let sig: PerspectiveDiffEntry = PerspectiveDiffEntry::try_from(signal.clone())?;
    Ok(emit_signal(&sig)?)
}

#[hdk_extern]
pub fn commit(diff: PerspectiveDiff) -> ExternResult<HoloHash<holo_hash::hash_type::Header>> {
    methods::commit(diff).map_err(|err| WasmError::Host(err.to_string()))
}

#[hdk_extern]
pub fn add_active_agent_link(_: ()) -> ExternResult<()> {
    methods::add_active_agent_link().map_err(|err| WasmError::Host(err.to_string()))
}

#[hdk_extern]
pub fn latest_revision(_: ()) -> ExternResult<Option<HoloHash<holo_hash::hash_type::Header>>> {
    methods::latest_revision().map_err(|err| WasmError::Host(err.to_string()))
}

#[hdk_extern]
pub fn current_revision(_: ()) -> ExternResult<Option<HoloHash<holo_hash::hash_type::Header>>> {
    methods::current_revision().map_err(|err| WasmError::Host(err.to_string()))
}

#[hdk_extern]
pub fn pull(_: ()) -> ExternResult<PerspectiveDiff> {
    methods::pull().map_err(|err| WasmError::Host(err.to_string()))
}

#[hdk_extern]
pub fn render(_: ()) -> ExternResult<Perspective> {
    methods::render().map_err(|err| WasmError::Host(err.to_string()))
}

#[hdk_extern]
pub fn update_current_revision(_hash: HoloHash<holo_hash::hash_type::Header>) -> ExternResult<()> {
    #[cfg(feature = "test")] {
        methods::update_current_revision(_hash, methods::get_now().unwrap()).map_err(|err| WasmError::Host(err.to_string()))?;
    }
    Ok(())
}

#[hdk_extern]
pub fn update_latest_revision(_hash: HoloHash<holo_hash::hash_type::Header>) -> ExternResult<()> {
    #[cfg(feature = "test")] {
        methods::update_latest_revision(_hash, methods::get_now().unwrap()).map_err(|err| WasmError::Host(err.to_string()))?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct SocialContextProperties {
    pub active_agent_duration_s: i64,
    pub enable_signals: bool,
}

lazy_static! {
    pub static ref ACTIVE_AGENT_DURATION: chrono::Duration = {
        let host_dna_config = zome_info()
            .expect("Could not get zome configuration")
            .properties;
        let properties = SocialContextProperties::try_from(host_dna_config)
            .expect("Could not convert zome dna properties to SocialContextProperties. Please ensure that your dna properties contains a SocialContextProperties field.");
        chrono::Duration::seconds(properties.active_agent_duration_s)
    };
    pub static ref ENABLE_SIGNALS: bool = {
        let host_dna_config = zome_info()
            .expect("Could not get zome configuration")
            .properties;
        let properties = SocialContextProperties::try_from(host_dna_config)
            .expect("Could not convert zome dna properties to SocialContextProperties. Please ensure that your dna properties contains a SocialContextProperties field.");
        properties.enable_signals
    };
}
