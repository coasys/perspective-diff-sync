use hdk::prelude::*;
use perspective_diff_sync_integrity::{
    EntryTypes, LinkExpression, PerspectiveDiff,
};

use crate::Hash;
use crate::errors::{SocialContextResult};
use crate::retriever::{PerspectiveDiffRetreiver};

pub struct ChunkedDiffs {
    max_changes_per_chunk: u16,
    pub chunks: Vec<PerspectiveDiff>
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

    pub fn into_entries<Retreiver: PerspectiveDiffRetreiver>(self) -> SocialContextResult<Vec<Hash>> {
        debug!("ChunkedDiffs.into_entries()");
        self.chunks
            .into_iter()
            .map(|chunk_diff| {
                debug!("ChunkedDiffs writing chunk of size: {}", chunk_diff.total_diff_number());
                Retreiver::create_entry(EntryTypes::PerspectiveDiff(chunk_diff))
            })
            .collect() 
    }

    pub fn from_entries<Retreiver: PerspectiveDiffRetreiver>(hashes: Vec<Hash>) -> SocialContextResult<Self> {
        let mut diffs = Vec::new();
        for hash in hashes.into_iter() {
            diffs.push(Retreiver::get::<PerspectiveDiff>(hash)?);
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


#[cfg(test)]
mod tests {
    use super::ChunkedDiffs;
    use crate::utils::create_link_expression;

    #[test]
    fn can_chunk() {
        let mut chunks = ChunkedDiffs::new(5);

        chunks.add_additions(vec![
            create_link_expression("a", "1"),
            create_link_expression("a", "2"),
            create_link_expression("a", "3"),
        ]);

        assert_eq!(chunks.chunks.len(), 1);

        chunks.add_additions(vec![
            create_link_expression("a", "4"),
            create_link_expression("a", "5"),
            create_link_expression("a", "6"),
        ]);

        assert_eq!(chunks.chunks.len(), 2);

        chunks.add_removals(vec![
            create_link_expression("a", "1"),
            create_link_expression("a", "2"),
            create_link_expression("a", "3"),
            create_link_expression("a", "4"),
            create_link_expression("a", "5"),
            create_link_expression("a", "6"),
        ]);

        assert_eq!(chunks.chunks.len(), 3);
    }

    #[test]
    fn can_aggregate() {
        let mut chunks = ChunkedDiffs::new(5);

        let _a1 = create_link_expression("a", "1");
        let _a2 = create_link_expression("a", "2");
        let _r1 = create_link_expression("r", "1");
        let _r2 = create_link_expression("r", "2");
        let _r3 = create_link_expression("r", "3");
        let _r4 = create_link_expression("r", "4");


        chunks.add_additions(vec![_a1.clone()]);
        chunks.add_additions(vec![_a2.clone()]);
        chunks.add_removals(vec![_r1.clone(),_r2.clone(),_r3.clone(),_r4.clone()]);

        assert_eq!(chunks.chunks.len(), 2);

        let diff = chunks.into_aggregated_diff();

        assert_eq!(diff.additions, vec![_a1,_a2]);
        assert_eq!(diff.removals, vec![_r1,_r2,_r3,_r4]);
    }
}
