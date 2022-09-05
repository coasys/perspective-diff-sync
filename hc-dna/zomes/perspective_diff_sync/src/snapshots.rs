use hdk::prelude::*;
use perspective_diff_sync_integrity::{
    EntryTypes, LinkExpression, LinkTypes, PerspectiveDiff, PerspectiveDiffEntryReference, Snapshot,
};

use crate::Hash;
use crate::errors::{SocialContextError, SocialContextResult};

pub fn get_entries_since_snapshot(
    latest: HoloHash<holo_hash::hash_type::Action>,
) -> SocialContextResult<usize> {
    let mut search_position = latest;
    let mut depth = 0;
    let mut seen = HashSet::new();
    let mut unseen_parents = vec![];

    loop {
        let diff = get(search_position.clone(), GetOptions::latest())?
            .ok_or(SocialContextError::InternalError(
                "Could not find entry while populating search",
            ))?
            .entry()
            .to_app_option::<PerspectiveDiffEntryReference>()?
            .ok_or(SocialContextError::InternalError(
                "Expected element to contain app entry data",
            ))?;
        //Check if entry is already in graph
        if !seen.contains(&search_position) {
            seen.insert(search_position.clone());
            //Only increase depth if entry is not a merge entry?
            if diff.parents.is_some() {
                if diff.parents.clone().unwrap().len() < 2 {
                    depth += 1;
                }
            } else {
                depth += 1;
            }
        };
        let diff_entry_hash = hash_entry(&diff)?;
        let check_snapshot = get_links(
            diff_entry_hash,
            LinkTypes::Snapshot,
            Some(LinkTag::new("snapshot")),
        )?;
        if check_snapshot.len() != 0 {
            depth -= 1;
            break;
        }

        if diff.parents.is_none() {
            //No parents, we have reached the end of the chain
            //Now move onto traversing parents
            if unseen_parents.len() == 0 {
                debug!("No more unseen items");
                break;
            } else {
                debug!("Moving onto unseen fork items");
                search_position = unseen_parents.remove(0);
            }
        } else {
            let mut parents = diff.parents.unwrap();
            //Check if all parents have already been seen, if so then break or move onto next unseen parents
            if parents.iter().all(|val| seen.contains(val)) {
                if unseen_parents.len() == 0 {
                    debug!("Reached end of graph");
                    break;
                } else {
                    search_position = unseen_parents.remove(0);
                };
            } else {
                search_position = parents.remove(0);
                unseen_parents.append(&mut parents);
            };
        }
    }
    Ok(depth)
}

pub struct ChunkedDiffs {
    max_changes_per_chunk: u16,
    chunks: Vec<PerspectiveDiff>
}

impl ChunkedDiffs {
    pub fn new(max: u16) -> Self {
        Self {
            max_changes_per_chunk: max,
            chunks: vec![PerspectiveDiff::new()],
        }
    }

    pub fn add_additions(&mut self, mut links: Vec<LinkExpression>) {
        let len = self.chunks.len();
        let current_chunk = self.chunks.get_mut(len-1).expect("must have at least one");

        if current_chunk.total_diff_number() + links.len() > self.max_changes_per_chunk.into() {
            self.chunks.push(PerspectiveDiff{
                additions: links,
                removals: Vec::new(),
            })
        } else {
            current_chunk.additions.append(&mut links)
        }
    }

    pub fn add_removals(&mut self, mut links: Vec<LinkExpression>) {
        let len = self.chunks.len();
        let current_chunk = self.chunks.get_mut(len-1).expect("must have at least one");

        if current_chunk.total_diff_number() + links.len() > self.max_changes_per_chunk.into() {
            self.chunks.push(PerspectiveDiff{
                additions: Vec::new(),
                removals: links,
            })
        } else {
            current_chunk.removals.append(&mut links)
        }
    }

    pub fn into_entries(self) -> SocialContextResult<Vec<Hash>> {
        self.chunks
            .into_iter()
            .map(|chunk_diff| {
                create_entry(EntryTypes::PerspectiveDiff(chunk_diff))
                    .map_err(|e| SocialContextError::Wasm(e)) 
            })
            .collect() 
    }

    pub fn from_entries(hashes: Vec<Hash>) -> SocialContextResult<Self> {
        let mut diffs = Vec::new();
        for hash in hashes.into_iter() {
            diffs.push(get(hash, GetOptions::latest())?
                .ok_or(SocialContextError::InternalError(
                    "Could not find diff entry for given diff entry reference",
                ))?
                .entry()
                .to_app_option::<PerspectiveDiff>()?
                .ok_or(SocialContextError::InternalError(
                    "Expected element to contain app entry data",
                ))?
            );
        }

        Ok(ChunkedDiffs {
            max_changes_per_chunk: 1000,
            chunks: diffs,
        })
    }

    pub fn into_aggregated_diff(self) -> PerspectiveDiff {
        self.chunks.into_iter().reduce(|accum, item| {
            let mut temp = accum.clone();
            temp.additions.append(&mut item.additions.clone());
            temp.removals.append(&mut item.removals.clone());
            temp
        })
        .unwrap_or(PerspectiveDiff::new())
    }
}

pub fn generate_snapshot(
    latest: HoloHash<holo_hash::hash_type::Action>,
) -> SocialContextResult<Snapshot> {
    let mut search_position = latest;
    let mut seen: HashSet<Hash> = HashSet::new();
    let diffs: Vec<Hash> = vec![];
    let mut unseen_parents = vec![];

    let mut chunked_diffs = ChunkedDiffs::new(1000);

    loop {
        let diff = get(search_position.clone(), GetOptions::latest())?
            .ok_or(SocialContextError::InternalError(
                "Could not find entry while populating search",
            ))?
            .entry()
            .to_app_option::<PerspectiveDiffEntryReference>()?
            .ok_or(SocialContextError::InternalError(
                "Expected element to contain app entry data",
            ))?;
        let mut snapshot_links = get_links(
            hash_entry(&diff)?,
            LinkTypes::Snapshot,
            Some(LinkTag::new("snapshot")),
        )?;
        if snapshot_links.len() > 0 {
            //get snapshot and add elements to out
            let snapshot = get(snapshot_links.remove(0).target, GetOptions::latest())?
                .ok_or(SocialContextError::InternalError(
                    "Could not find diff entry for given diff entry reference",
                ))?
                .entry()
                .to_app_option::<Snapshot>()?
                .ok_or(SocialContextError::InternalError(
                    "Expected element to contain app entry data",
                ))?;
            
            let diff = ChunkedDiffs::from_entries(snapshot.diff_chunks)?.into_aggregated_diff();
            chunked_diffs.add_additions(diff.additions.clone());
            chunked_diffs.add_removals(diff.removals.clone());
            for hash in snapshot.included_diffs.iter() {
                seen.insert(hash.clone());
            }
            
            //Be careful with break here where there are still unseen parents
            if unseen_parents.len() == 0 {
                debug!("No more unseen parents within snapshot block");
                break;
            } else {
                search_position = unseen_parents.remove(0);
            }
        } else {
            //Check if entry is already in graph
            if !seen.contains(&search_position) {
                seen.insert(search_position.clone());
                let diff_entry = get(diff.diff.clone(), GetOptions::latest())?
                    .ok_or(SocialContextError::InternalError(
                        "Could not find diff entry for given diff entry reference",
                    ))?
                    .entry()
                    .to_app_option::<PerspectiveDiff>()?
                    .ok_or(SocialContextError::InternalError(
                        "Expected element to contain app entry data",
                    ))?;

                chunked_diffs.add_additions(diff_entry.additions.clone());
                chunked_diffs.add_removals(diff_entry.removals.clone());
            };
            if diff.parents.is_none() {
                //No parents, we have reached the end of the chain
                //Now move onto traversing unseen parents, or break if we dont have any other paths to search
                if unseen_parents.len() == 0 {
                    debug!("No more unseen items within parent block");
                    break;
                } else {
                    debug!("Moving onto unseen fork items within parent block");
                    search_position = unseen_parents.remove(0);
                }
            } else {
                //Do the fork traversals
                let mut parents = diff.parents.unwrap();
                //Check if all parents have already been seen, if so then break or move onto next unseen parents
                //TODO; we should use a seen set here versus array iter
                if parents.iter().all(|val| { seen.contains(val)}) {
                    if unseen_parents.len() == 0 {
                        debug!("Parents of item seen and unseen 0");
                        break;
                    } else {
                        debug!("last moving onto unseen");
                        search_position = unseen_parents.remove(0);
                    }
                } else {
                    search_position = parents.remove(0);
                    debug!("Appending parents to look up: {:?}", parents);
                    unseen_parents.append(
                        &mut parents
                    );
                };
            }
        }
    }

    let snapshot = Snapshot {
        diff_chunks: chunked_diffs.into_entries()?,
        included_diffs: seen.into_iter().collect(),
    };

    Ok(snapshot)
}
