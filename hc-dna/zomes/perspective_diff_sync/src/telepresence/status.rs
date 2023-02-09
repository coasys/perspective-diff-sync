use hdk::prelude::*;

use perspective_diff_sync_integrity::{
    Anchor, EntryTypes, LinkTypes, OnlineAgent, OnlineAgentAndAction, PerspectiveExpression,
};

use crate::errors::{SocialContextError, SocialContextResult};
use crate::retriever::holochain::get_active_agents;

pub fn set_online_status(status: PerspectiveExpression) -> SocialContextResult<()> {
    let existing_status = get_online_status()?;
    if existing_status.status_action.is_some() {
        delete_entry(existing_status.status_action.unwrap())?;
    };
    let entry = EntryTypes::PerspectiveExpression(status);
    create_entry(&entry)?;
    Ok(())
}

pub fn get_online_status() -> SocialContextResult<OnlineAgentAndAction> {
    //Try and get my did
    let my_did = get_my_did()?.ok_or(SocialContextError::NoDidFound)?;

    //Try and get my online status
    let query = query(
        QueryFilter::new()
            .entry_type(EntryType::App(AppEntryDef {
                entry_index: 7.into(),
                zome_index: 0.into(),
                visibility: EntryVisibility::Private,
            }))
            .include_entries(true)
            .descending(),
    );

    let status = match query {
        Ok(records) => {
            if records.len() == 0 {
                None
            } else {
                let record = records[0].clone();
                let entry = record
                    .entry
                    .to_app_option::<PerspectiveExpression>()
                    .unwrap()
                    .unwrap();
                Some((entry, record.action_address().to_owned()))
            }
        }
        Err(e) => {
            debug!(
                "PerspectiveDiffSync.current_revision(): Error when getting current revision: {:?}",
                e
            );
            None
        }
    };

    let online_status = OnlineAgentAndAction {
        did: my_did,
        status: status.clone().map(|status| status.0),
        status_action: status.map(|status| status.1),
    };
    Ok(online_status)
}

pub fn create_did_pub_key_link(did: String) -> SocialContextResult<()> {
    let agent_key = agent_info()?.agent_latest_pubkey;
    let did_links = get_links(agent_key.clone(), LinkTypes::DidLink, None)?;
    if did_links.len() == 0 {
        let entry = EntryTypes::Anchor(Anchor(did));
        let _did_entry = create_entry(&entry)?;
        let did_entry_hash = hash_entry(entry)?;
        create_link(
            agent_key.clone(),
            did_entry_hash.clone(),
            LinkTypes::DidLink,
            LinkTag::new("did_link"),
        )?;
        create_link(
            did_entry_hash,
            agent_key,
            LinkTypes::DidLink,
            LinkTag::new("did_link"),
        )?;
    }
    Ok(())
}

pub fn get_my_did() -> SocialContextResult<Option<String>> {
    let mut did_links = get_links(agent_info()?.agent_latest_pubkey, LinkTypes::DidLink, None)?;
    if did_links.len() > 0 {
        let did = get(
            did_links
                .remove(0)
                .target
                .into_entry_hash()
                .expect("Could not get entry_hash"),
            GetOptions::latest(),
        )?
        .ok_or(SocialContextError::InternalError(
            "Could not find did entry for given did entry reference",
        ))?
        .entry()
        .to_app_option::<Anchor>()?
        .ok_or(SocialContextError::InternalError(
            "Expected element to contain app entry data",
        ))?;
        Ok(Some(did.0))
    } else {
        Ok(None)
    }
}

pub fn get_dids_agent_key(did: String) -> SocialContextResult<Option<AgentPubKey>> {
    let did_entry = Anchor(did);
    let did_entry_hash = hash_entry(EntryTypes::Anchor(did_entry.clone()))?;
    let did_links = get_links(did_entry_hash, LinkTypes::DidLink, None)?;
    if did_links.len() > 0 {
        Ok(Some(AgentPubKey::from(EntryHash::from(
            did_links[0].target.clone(),
        ))))
    } else {
        Ok(None)
    }
}

pub fn get_online_agents() -> SocialContextResult<Vec<OnlineAgent>> {
    let active_agents = get_active_agents()?;
    let mut online_agents = Vec::new();
    for active_agent in active_agents {
        let online_agent_status = call_remote(
            active_agent.clone(),
            "perspective_diff_sync",
            "get_online_status".into(),
            None,
            {},
        );
        if online_agent_status.is_ok() {
            let online_agent_response = online_agent_status.unwrap();
            match online_agent_response {
                ZomeCallResponse::Ok(online_agent) => {
                    let online_agent = online_agent.decode::<OnlineAgentAndAction>()?;
                    online_agents.push(OnlineAgent {
                        did: online_agent.did,
                        status: online_agent.status,
                    });
                }
                ZomeCallResponse::Unauthorized(..) => {
                    debug!("Unauthorized to call agent {}", active_agent);
                }
                ZomeCallResponse::NetworkError(..) => {
                    debug!("Agent {} is offline", active_agent);
                }
                ZomeCallResponse::CountersigningSession(_) => {
                    debug!("Agent {} had countersigning session error", active_agent);
                }
            }
        }
    }
    Ok(online_agents)
}