use chrono::{DateTime, NaiveDateTime, Utc};
use hdk::prelude::*;
use perspective_diff_sync_integrity::{EntryTypes, HashReference, LinkTypes, LocalHashReference};

use crate::errors::{SocialContextError, SocialContextResult};
use crate::utils::get_now;
use crate::Hash;

pub fn update_latest_revision(
    hash: Hash,
    timestamp: DateTime<Utc>,
) -> SocialContextResult<()> {
    let hash_ref = HashReference { hash, timestamp };
    create_entry(EntryTypes::HashReference(hash_ref.clone()))?;
    hc_time_index::index_entry(
        String::from("current_rev"),
        hash_ref,
        LinkTag::new(""),
        LinkTypes::Index,
        LinkTypes::TimePath,
    )?;
    Ok(())
}

pub fn update_current_revision(
    hash: Hash,
    timestamp: DateTime<Utc>,
) -> SocialContextResult<()> {
    let hash_ref = LocalHashReference { hash, timestamp };
    create_entry(EntryTypes::LocalHashReference(hash_ref.clone()))?;
    Ok(())
}

//Latest revision as seen from the DHT
pub fn latest_revision() -> SocialContextResult<Option<Hash>> {
    let mut latest =
        hc_time_index::get_links_and_load_for_time_span::<HashReference, LinkTypes, LinkTypes>(
            String::from("current_rev"),
            get_now()?,
            DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            None,
            hc_time_index::SearchStrategy::Dfs,
            Some(1),
            LinkTypes::Index,
            LinkTypes::TimePath,
        )?;
    Ok(latest.pop().map(|val| val.hash))
}

//Latest revision as seen from our local state
pub fn current_revision() -> SocialContextResult<Option<Hash>> {
    let mut end_index = 10;
    let mut revisions = vec![];
    loop {
        let start_index = if end_index == 10 { 1 } else { end_index - 9 };
        let filter = ChainQueryFilter::new().sequence_range(ChainQueryFilterRange::ActionSeqRange(
            start_index,
            end_index,
        ));
        let refs = query(filter)?;
        end_index += 10;

        for entry in refs.clone() {
            match entry.signed_action.hashed.content {
                Action::Create(create_data) => match create_data.entry_type {
                    EntryType::App(app_entry) => {
                        if app_entry.visibility == EntryVisibility::Private {
                            revisions.push(create_data.entry_hash);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
            //debug!("{:#?}", entry.signed_action.hashed.content);
        }

        if refs.len() != 10 {
            break;
        }
    }

    if revisions.len() > 0 {
        debug!("Got some revisions");
        Ok(Some(
            get(revisions.pop().unwrap(), GetOptions::latest())?
                .ok_or(SocialContextError::InternalError(
                    "Could not find local revision reference entry",
                ))?
                .entry()
                .to_app_option::<LocalHashReference>()?
                .ok_or(SocialContextError::InternalError(
                    "Expected element to contain app entry data",
                ))?
                .hash,
        ))
    } else {
        Ok(None)
    }
}
