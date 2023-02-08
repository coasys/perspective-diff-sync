use hdk::prelude::*;
use perspective_diff_sync_integrity::PerspectiveExpression;

use super::status::get_dids_agent_key;
use crate::retriever::holochain::get_active_agents;
use crate::{errors::SocialContextResult, inputs::SignalData};

pub fn send_signal(signal_data: SignalData) -> SocialContextResult<()> {
    let agent = get_dids_agent_key(signal_data.agent)?;
    match agent {
        Some(agent) => remote_signal(signal_data.perspective_expression.get_sb()?, vec![agent])?,
        None => {
            debug!("PerspectiveDiffSync.send_signal(): Could not send signal since we could not get the agents pub key from did");
        }
    }
    Ok(())
}

pub fn send_broadcast(data: PerspectiveExpression) -> SocialContextResult<()> {
    let active_agents = get_active_agents()?;

    remote_signal(data.get_sb()?, active_agents)?;

    Ok(())
}
