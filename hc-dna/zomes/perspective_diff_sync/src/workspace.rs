use hdk::prelude::*;
use petgraph::{
    algo::{all_simple_paths, dominators::simple_fast},
    graph::{DiGraph, Graph, NodeIndex, UnGraph},
};
use std::collections::{BTreeMap, VecDeque};
use std::ops::Index;
use perspective_diff_sync_integrity::{PerspectiveDiff, PerspectiveDiffEntryReference, Snapshot, LinkTypes};
use crate::errors::{SocialContextError, SocialContextResult};
//use crate::snapshots::get_snapshot;
use crate::topo_sort::topo_sort_diff_references;

type Hash = HoloHash<holo_hash::hash_type::Action>;


pub struct Workspace {
    pub graph: DiGraph<HoloHash<holo_hash::hash_type::Action>, ()>,
    pub undirected_graph: UnGraph<HoloHash<holo_hash::hash_type::Action>, ()>,
    pub node_index_map: BTreeMap<HoloHash<holo_hash::hash_type::Action>, NodeIndex<u32>>,
    pub entry_map: BTreeMap<HoloHash<holo_hash::hash_type::Action>, PerspectiveDiffEntryReference>,
    pub sorted_diffs: Option<Vec<(HoloHash<holo_hash::hash_type::Action>, PerspectiveDiffEntryReference)>>,
    pub common_ancestor: Option<Hash>,
}

impl Workspace {
    pub fn new() -> Workspace {
        Workspace {
            graph: Graph::new(),
            undirected_graph: Graph::new_undirected(),
            node_index_map: BTreeMap::new(),
            entry_map: BTreeMap::new(),
            sorted_diffs: None,
            common_ancestor: None,
        }
    }

    // This is the easy case when we only build from one hash.
    // (either latest or our current hash, like in render).
    // We don't have to check for forks, we just deep search from the given
    // diff and terminate at leafs and snapshots.
    // Since we don't have to detect and handle forks, we don't need
    // to unroll snapshots and just treat them as leafs.
    pub fn collect_only_from_latest(&mut self, latest: Hash) 
        -> SocialContextResult<()> 
    {
        debug!("WORKSPACE collect_only_from_latest 1");
        // Initializing with only one branch starting from the given hash.
        let mut unprocessed_branches = VecDeque::new();
        unprocessed_branches.push_back(latest);

        while !unprocessed_branches.is_empty() {
            let current_hash = unprocessed_branches[0].clone();
            
            if let Some(snapshot) = Self::get_snapshot(current_hash.clone())? {
                self.entry_map.insert(current_hash.clone(), PerspectiveDiffEntryReference {
                    diff: snapshot.diff,
                    parents: None,
                });
                // Snapshot terminates like an orphan.
                // So we can close this branch and potentially continue
                // with other unprocessed branches, if they exist.
                unprocessed_branches.pop_front();
            } else {
                let current_diff = Self::get_p_diff_reference(current_hash.clone())?;
                if let Some(parents) = &current_diff.parents {
                    for i in 0..parents.len() {
                        // Depth-first search:
                        // We are replacing our search position (==current_hash==unprocessed_branches[0])
                        // with the first parent.
                        // Other parents are pushed on the vec as new branches to search later..
                        if i==0 {
                            unprocessed_branches[0] = parents[i].clone();
                        } else {
                            unprocessed_branches.push_back(parents[i].clone())
                        }
                    }
                } else {
                    // We arrived at a leaf/orphan (no parents).
                    // So we can close this branch and potentially continue
                    // with other unprocessed branches, if they exist.
                    unprocessed_branches.pop_front();
                }

                self.entry_map.insert(current_hash, current_diff);
            }
        }

        debug!("WORKSPACE collect_only_from_latest 2");

        Ok(())
    }

    pub fn collect_until_common_ancestor(&mut self, theirs: Hash, ours: Hash)
        -> SocialContextResult<()>
    {
        debug!("WORKSPACE collect_until_common_ancestor 1");
        // Initializing with only one branch starting from the given hash.
        let mut breadth_first_branches = VecDeque::new();
        breadth_first_branches.push_back(theirs);
        breadth_first_branches.push_back(ours);

        let mut common_ancestor: Option<Hash> = None;

        while common_ancestor.is_none() && !breadth_first_branches.is_empty() {
            debug!("WORKSPACE collect_until_common_ancestor 2");
            for branch_index in 0..breadth_first_branches.len() {
                let current_hash = breadth_first_branches[branch_index].clone();
                let already_seen = self.entry_map.contains_key(&current_hash);
                if already_seen {
                    // current hash is already in, so it must be our common ancestor!
                    common_ancestor = Some(current_hash);
                    break;
                } else {
                    let current_diff = Self::get_p_diff_reference(current_hash.clone())?;
                    
                    match &current_diff.parents {
                        None => {
                            // We arrived at a leaf/orphan (no parents).
                            // So we can close this branch and potentially continue
                            // with other unprocessed branches, if they exist.
                            breadth_first_branches.remove(branch_index);
                            // We have to break out of loop to avoid having branch_index run out of bounds
                            break;
                        },
                        Some(parents) => {
                            let filtered_parents = parents
                                .iter()
                                .filter(|p| !self.entry_map.contains_key(p))
                                .cloned()
                                .collect::<Vec<Hash>>();

                            for parent_index in 0..filtered_parents.len() {
                                // The first parent is taken as the successor for the current branch.
                                // If there are multiple parents (i.e. merge commit), we create a new branch..
                                if parent_index == 0 {
                                    breadth_first_branches[branch_index] = parents[parent_index].clone();
                                } else {
                                    breadth_first_branches.push_back(parents[parent_index].clone())
                                }
                            }
                        }
                    };
        
                    self.entry_map.insert(current_hash, current_diff);
                }
            }
        }

        debug!("WORKSPACE collect_until_common_ancestor 3");

        if common_ancestor.is_none() {
            return Err(SocialContextError::NoCommonAncestorFound);
        }

        
        let mut common_ancestor_diff = self.entry_map
            .get(common_ancestor.as_ref().ok_or(SocialContextError::InternalError("None handled above"))?)
            .ok_or(SocialContextError::InternalError("Common ancestor must have been added to map above"))?
            .clone();
        

        // For the topological sorting, we need to have the graph be complete, without cycles,
        // and without dangling parent links.
        // Since we stopped the (parallel) breadth first search once we found the common ancestor,
        // we will likely have branches point to diffs that have parents, which are not yet processed.
        
        // So first we prune anything before (in time) the common ancestor since we don't need that...

        if let Some(parents) = common_ancestor_diff.parents {
            let mut ancestor_parents: VecDeque<Hash> = parents.clone().into_iter().collect();
            while ancestor_parents.len() > 0 {
                let hash = ancestor_parents[0].clone();
                if let Some(current_diff) = self.entry_map.get(&hash) {
                    match &current_diff.parents {
                        None => {
                            // We arrived at a leaf/orphan (no parents).
                            // So we can close this branch and potentially continue
                            // with other unprocessed branches, if they exist.
                            ancestor_parents.pop_front();
                        },
                        Some(parents) => {
                            for parent_index in 0..parents.len() {
                                // The first parent is taken as the successor for the current branch.
                                // If there are multiple parents (i.e. merge commit), we create a new branch..
                                if parent_index == 0 {
                                    ancestor_parents[0] = parents[parent_index].clone();
                                } else {
                                    ancestor_parents.push_back(parents[parent_index].clone())
                                }
                            }
                        }
                    }
                    self.entry_map.remove(&hash);
                } else {
                    // Our current top hash (which we got in as parent)
                    // is not in our map, so we reached the end of the crawled history.
                    ancestor_parents.pop_front();
                }
            }
        }
        

        common_ancestor_diff.parents = None;
        // Let's store the common ancestor in self for later use.    
        self.common_ancestor = common_ancestor.clone();
        self.entry_map.insert(
            common_ancestor.ok_or(SocialContextError::InternalError("None case handled above"))?, 
            common_ancestor_diff
        );

        // And then we terminate any remaining open branches by just removing the parents.
        for branch_index in 0..breadth_first_branches.len() {
            let hash = breadth_first_branches[branch_index].clone();
            if let Some(diff_ref) = self.entry_map.get(&hash) {
                let mut diff = diff_ref.clone();
                diff.parents = None;
                self.entry_map.insert(hash, diff);
            }
        }

        Ok(())
    }

    pub fn topo_sort_graph(&mut self) -> SocialContextResult<()> {
        let entry_vec = self.entry_map
            .clone()
            .into_iter()
            .collect::<Vec<(HoloHash<holo_hash::hash_type::Action>, PerspectiveDiffEntryReference)>>();
        
        self.sorted_diffs = Some(topo_sort_diff_references(&entry_vec)?);
        Ok(())
    }

    pub fn build_graph(&mut self) -> SocialContextResult<()>  {
        match self.sorted_diffs.clone() {
            None => Err(SocialContextError::InternalError("Need to 1. collect diffs and then 2. sort them before building the graph")),
            Some(sorted_diffs) => {
                        //Add root node
                if self
                    .get_node_index(&ActionHash::from_raw_36(vec![0xdb; 36]))
                    .is_none()
                {
                    self.add_node(None, ActionHash::from_raw_36(vec![0xdb; 36]));
                };

                for diff in sorted_diffs {
                    if diff.1.parents.is_some() {
                        let mut parents = vec![];
                        for parent in diff.1.parents.as_ref().unwrap() {
                            let parent = self
                                .get_node_index(&parent)
                                .ok_or(SocialContextError::InternalError("Did not find parent"))?;
                            parents.push(parent.clone());
                        }
                        self.add_node(Some(parents), diff.0.clone());
                    } else {
                        self.add_node(Some(vec![NodeIndex::from(0)]), diff.0.clone());
                    }
                }
                Ok(())
            }
        }
    }

    fn get_p_diff_reference(address: Hash) -> SocialContextResult<PerspectiveDiffEntryReference> {
        get(address, GetOptions::latest())?
            .ok_or(SocialContextError::InternalError(
                "Could not find entry while populating search",
            ))?
            .entry()
            .to_app_option::<PerspectiveDiffEntryReference>()?
            .ok_or(SocialContextError::InternalError(
                "Expected element to contain app entry data",
            ))
    }

    fn get_snapshot(address: Hash) 
        -> SocialContextResult<Option<Snapshot>> 
    {
        let mut snapshot_links = get_links(
            address,
            LinkTypes::Snapshot,
            Some(LinkTag::new("snapshot")),
        )?;

        if snapshot_links.len() > 0 {
            let snapshot = get(snapshot_links.remove(0).target, GetOptions::latest())?
                .ok_or(SocialContextError::InternalError(
                    "Could not find entry while populating search",
                ))?
                .entry()
                .to_app_option::<Snapshot>()?
                .ok_or(SocialContextError::InternalError(
                    "Expected element to contain app entry data",
                ))?;

            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }



    fn add_node(
        &mut self,
        parents: Option<Vec<NodeIndex<u32>>>,
        diff: HoloHash<holo_hash::hash_type::Action>,
    ) -> NodeIndex<u32> {
        let index = self.graph.add_node(diff.clone());
        self.undirected_graph.add_node(diff.clone());
        self.node_index_map.insert(diff, index);
        if parents.is_some() {
            for parent in parents.unwrap() {
                self.graph.add_edge(index, parent, ());
                self.undirected_graph.add_edge(index, parent, ());
            }
        }
        index
    }

    pub fn get_node_index(
        &self,
        node: &HoloHash<holo_hash::hash_type::Action>,
    ) -> Option<&NodeIndex<u32>> {
        self.node_index_map.get(node)
    }

    pub fn index(&self, index: NodeIndex) -> HoloHash<holo_hash::hash_type::Action> {
        self.graph.index(index).clone()
    }

    pub fn get_paths(
        &self,
        child: &Hash,
        ancestor: &Hash,
    ) -> Vec<Vec<NodeIndex>> {
        let child_node = self.get_node_index(child).unwrap();
        let ancestor_node = self.get_node_index(ancestor).unwrap();
        let paths = all_simple_paths::<Vec<_>, _>(&self.graph, *child_node, *ancestor_node, 0, None)
            .collect::<Vec<_>>();
        paths
    }

    pub fn _find_common_ancestor(
        &self,
        root: NodeIndex<u32>,
        second: NodeIndex<u32>,
    ) -> Option<NodeIndex> {
        let imm = simple_fast(&self.undirected_graph, root);
        let imm = imm.dominators(second);
        let mut index: Option<NodeIndex> = None;
        match imm {
            Some(imm_iter) => {
                for dom in imm_iter {
                    match index {
                        Some(current_index) => {
                            if current_index.index() > dom.index() {
                                index = Some(dom)
                            }
                        }
                        None => index = Some(dom),
                    };
                }
            }
            None => {}
        };
        index
    }

    pub fn squashed_diff(&self) -> SocialContextResult<PerspectiveDiff> {
        let mut out = PerspectiveDiff {
            additions: vec![],
            removals: vec![],
        };
        for (_key, value) in self.entry_map.iter() {
            let diff_entry = get(value.diff.clone(), GetOptions::latest())?
                .ok_or(SocialContextError::InternalError(
                    "Could not find diff entry for given diff entry reference",
                ))?
                .entry()
                .to_app_option::<PerspectiveDiff>()?
                .ok_or(SocialContextError::InternalError(
                    "Expected element to contain app entry data",
                ))?;
            out.additions.append(&mut diff_entry.additions.clone());
            out.removals.append(&mut diff_entry.removals.clone());
        }

        Ok(out)
    }

    pub fn squashed_fast_forward_from(&self, base: Hash) -> SocialContextResult<PerspectiveDiff> {
        match &self.sorted_diffs {
            None => Err(SocialContextError::InternalError("Need to sort first for this fast-forward optimzed squash")),
            Some(sorted_diffs) => {
                let mut base_found = false;
                let mut out = PerspectiveDiff {
                    additions: vec![],
                    removals: vec![],
                };
                for i in 0..sorted_diffs.len() {
                    let current = &sorted_diffs[i];
                    if !base_found {
                        if current.0 == base {
                            base_found = true;
                        }
                    } else {
                        let diff_entry = get(current.1.diff.clone(), GetOptions::latest())?
                            .ok_or(SocialContextError::InternalError(
                                "Could not find diff entry for given diff entry reference",
                            ))?
                            .entry()
                            .to_app_option::<PerspectiveDiff>()?
                            .ok_or(SocialContextError::InternalError(
                                "Expected element to contain app entry data",
                            ))?;
                        out.additions.append(&mut diff_entry.additions.clone());
                        out.removals.append(&mut diff_entry.removals.clone());
                    }
                }
                Ok(out)
            }
        }
    }
}