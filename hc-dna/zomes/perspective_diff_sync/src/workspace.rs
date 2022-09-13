use hdk::prelude::*;
use petgraph::{
    algo::{all_simple_paths, dominators::simple_fast},
    dot::{Config, Dot},
    graph::{DiGraph, Graph, NodeIndex, UnGraph},
};
use std::collections::{BTreeMap, VecDeque};
use perspective_diff_sync_integrity::{PerspectiveDiff, PerspectiveDiffEntryReference, Snapshot, LinkTypes};
use std::cell::RefCell;
use itertools::Itertools;

use crate::Hash;
use crate::errors::{SocialContextError, SocialContextResult};
use crate::topo_sort::topo_sort_diff_references;
use crate::retriever::PerspectiveDiffRetreiver;
use crate::utils::get_now;

pub struct Workspace {
    pub graph: DiGraph<Hash, ()>,
    pub undirected_graph: UnGraph<Hash, ()>,
    pub node_index_map: BTreeMap<Hash, NodeIndex<u32>>,
    pub entry_map: BTreeMap<Hash, PerspectiveDiffEntryReference>,
    pub sorted_diffs: Option<Vec<(Hash, PerspectiveDiffEntryReference)>>,
    pub common_ancestors: Vec<Hash>,
    pub diffs: BTreeMap<Hash, PerspectiveDiffEntryReference>,
    pub back_links: BTreeMap::<Hash, Vec<Hash>>,
    unexplored_side_branches: Vec<Hash>
}

#[derive(Clone, Debug)]
struct BfsSearch {
    pub found_ancestors: RefCell<Vec<Hash>>,
    pub bfs_branches: RefCell<VecDeque<Hash>>,
    pub reached_end: bool
}

#[allow(non_snake_case)]
pub fn NULL_NODE() -> ActionHash { ActionHash::from_raw_36(vec![0xdb; 36]) }

impl BfsSearch {
    pub fn new(start: Hash) -> BfsSearch {
        let branches = RefCell::new(VecDeque::from([start]));
        BfsSearch {
            found_ancestors: RefCell::new(Vec::new()),
            bfs_branches: branches,
            reached_end: false
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
enum SearchSide {
    Theirs,
    Ours,
}

fn other_side(side: &SearchSide) -> SearchSide {
    match side {
        SearchSide::Theirs => SearchSide::Ours,
        SearchSide::Ours => SearchSide::Theirs,
    }
}

impl Workspace {
    pub fn new() -> Workspace {
        Workspace {
            graph: Graph::new(),
            undirected_graph: Graph::new_undirected(),
            node_index_map: BTreeMap::new(),
            entry_map: BTreeMap::new(),
            sorted_diffs: None,
            common_ancestors: vec![],
            diffs: BTreeMap::new(),
            back_links: BTreeMap::new(),
            unexplored_side_branches: Vec::new(),
        }
    }

    // This is the easy case when we only build from one hash.
    // (either latest or our current hash, like in render).
    // We don't have to check for forks, we just deep search from the given
    // diff and terminate at leafs and snapshots.
    // Since we don't have to detect and handle forks, we don't need
    // to unroll snapshots and just treat them as leafs.
    pub fn collect_only_from_latest<Retriever: PerspectiveDiffRetreiver>(&mut self, latest: Hash) 
        -> SocialContextResult<()> 
    {
        debug!("===Workspace.collect_only_from_latest(): Function start");
        let fn_start = get_now()?.time();

        // Initializing with only one branch starting from the given hash.
        let mut unprocessed_branches = VecDeque::new();
        unprocessed_branches.push_back(latest);

        while !unprocessed_branches.is_empty() {
            let current_hash = unprocessed_branches[0].clone();

            if self.entry_map.contains_key(&current_hash) {
                debug!("===Workspace.collect_only_from_latest(): CIRCLE DETECTED! Closing current branch...");
                unprocessed_branches.pop_front();
                continue;
            }

            let current_diff = Self::get_p_diff_reference::<Retriever>(current_hash.clone())?;
            
            if current_diff.diffs_since_snapshot == 0 {
                debug!("===Workspace.collect_only_from_latest(): Found a perspective diff reference containing a snapshot!");
                let snapshot = Self::get_snapshot(current_diff.clone())?;

                if snapshot.is_none() {
                    debug!("===Workspace.collect_only_from_latest(): ERROR: Expected to find snapshot link on current_diff where diffs_since_snapshot was 0");
                    self.handle_parents(current_diff, current_hash, &mut unprocessed_branches);
                } else {
                    let snapshot = snapshot.unwrap();

                    let mut last_diff = None;
                    for i in 0..snapshot.diff_chunks.len() {
                        let diff_chunk = &snapshot.diff_chunks[i];
                        self.entry_map.insert(diff_chunk.clone(), PerspectiveDiffEntryReference::new(diff_chunk.clone(), last_diff.clone()));
                        last_diff = Some(vec![diff_chunk.clone()]); 
                    }
    
                    self.entry_map.insert(current_hash.clone(), PerspectiveDiffEntryReference::new(current_diff.diff, last_diff.clone()));
                    // Snapshot terminates like an orphan.
                    // So we can close this branch and potentially continue
                    // with other unprocessed branches, if they exist.
                    unprocessed_branches.pop_front();
                };
            } else {
                self.handle_parents(current_diff, current_hash, &mut unprocessed_branches);
            }
        }

        let fn_end = get_now()?.time();
        debug!("===Workspace.collect_only_from_latest() - Profiling: Took: {} to complete collect_only_from_latest() function", (fn_end - fn_start).num_milliseconds()); 

        Ok(())
    }

    fn handle_parents(&mut self, current_diff: PerspectiveDiffEntryReference, current_hash: Hash, unprocessed_branches: &mut VecDeque<Hash>) {
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

    pub fn sort_graph(&mut self) -> SocialContextResult<()> {
        debug!("===Workspace.sort_graph(): Function start");
        let fn_start = get_now()?.time();

        let common_ancestor = self.common_ancestors.last().unwrap();

        let mut sorted: Vec<(Hash, PerspectiveDiffEntryReference)> = Vec::new();
        let mut next: VecDeque<Hash> = VecDeque::new();
        self.unexplored_side_branches = Vec::new();

        next.push_back(common_ancestor.clone());

        while !next.is_empty() {
            let current = next.pop_front().expect("must be Ok since next !is_empty()");
            debug!("current: {:?}", current);
            match self.back_links.get(&current) {
                Some(children) => {
                    debug!("--> has {} children, checking the children to see if there is a missing parent link", children.len());
                    for child in children.iter() {
                        let diff = self.diffs.get(&child).expect("Should child must exist");
                        if diff.parents.is_some() {
                            for parent in diff.parents.as_ref().unwrap() {
                                if parent != &current {
                                    self.unexplored_side_branches.push(parent.clone());
                                }
                            }
                        }
                    }
                    next.append(&mut children.iter().cloned().collect());
                },
                None => {}
            };
            let current_diff = self.diffs.get(&current).expect("diffs should be populated");
            sorted.push((current.clone(), current_diff.clone()));
            if self.entry_map.get(&current).is_none() {
                self.entry_map.insert(current, current_diff.clone());
            };
        }

        self.unexplored_side_branches = self.unexplored_side_branches
            .iter()
            .filter(|b| !sorted.iter().find(|s| s.0 == **b).is_some())
            .cloned()
            .collect();

        self.sorted_diffs = Some(sorted.into_iter().unique().collect());

        let fn_end = get_now()?.time();
        debug!("===Workspace.sort_graph() - Profiling: Took: {} to complete sort_graph() function", (fn_end - fn_start).num_milliseconds()); 

        Ok(())
    }

    pub fn build_diffs<Retriever: PerspectiveDiffRetreiver>(&mut self, theirs: Hash, ours: Hash) -> SocialContextResult<()> {
        debug!("===Workspace.build_diffs(): Function start");
        let fn_start = get_now()?.time();

        let common_ancestor = self.collect_until_common_ancestor::<Retriever>(theirs, ours)?;
        self.common_ancestors.push(common_ancestor);

        // debug!("===PerspectiveDiffSunc.build_diffs(): Got diffs: {:?}", self.diffs.iter().map(|x| x.0).collect::<Vec<_>>());
        // debug!("===PerspectiveDiffSunc.build_diffs(): Got back_links: {:?}", self.back_links);
        
        self.sort_graph()?;
        // debug!("===PerspectiveDiffSunc.build_diffs(): Got unexplored side branches parent: {:#?}", self.unexplored_side_branches);
        
        while self.unexplored_side_branches.len() > 0 {
            let unexplored_side_branch = self.unexplored_side_branches.pop().unwrap();
            let common_ancestor = self.collect_until_common_ancestor::<Retriever>(
                unexplored_side_branch,
                self.common_ancestors.last().expect("There should have been a common ancestor above").to_owned()
            )?;
            // debug!("===PerspectiveDiffSunc.build_diffs(): Got common ancestor: {:?}", common_ancestor);
            self.common_ancestors.push(common_ancestor);
            self.sort_graph()?;
        }

        let sorted_diffs = self.sorted_diffs.as_mut().unwrap();
        sorted_diffs.get_mut(0).unwrap().1.parents = None;
        self.sorted_diffs = Some(topo_sort_diff_references(sorted_diffs)?);
        // debug!("===PerspectiveDiffSunc.build_diffs(): Got sorted diffs: {:#?}", self.sorted_diffs);

        self.build_graph()?;
        self.print_graph_debug();
        
        let fn_end = get_now()?.time();
        debug!("===Workspace.build_diffs() - Profiling: Took: {} to complete build_diffs() function", (fn_end - fn_start).num_milliseconds()); 

        Ok(())
    }

    pub fn collect_until_common_ancestor<Retriever: PerspectiveDiffRetreiver>(&mut self, theirs: Hash, ours: Hash)
        -> SocialContextResult<Hash>
    {
        debug!("===Workspace.collect_until_common_ancestor(): Function start");
        let fn_start = get_now()?.time();

        let mut common_ancestor: Option<Hash> = None;

        let mut searches = btreemap! {
            SearchSide::Theirs => BfsSearch::new(theirs),
            SearchSide::Ours => BfsSearch::new(ours),
        };

        while common_ancestor.is_none() && (!searches.get(&SearchSide::Theirs).unwrap().bfs_branches.borrow().is_empty() | !searches.get(&SearchSide::Ours).unwrap().bfs_branches.borrow().is_empty()) {
            println!("===Workspace.collect_until_common_ancestor(): collect_until_common_ancestor 2: {:#?}", searches.get(&SearchSide::Theirs).unwrap().bfs_branches.borrow());
            println!("===Workspace.collect_until_common_ancestor(): collect_until_common_ancestor 2: {:#?}", searches.get(&SearchSide::Ours).unwrap().bfs_branches.borrow());
            // do the same BFS for theirs_branches and ours_branches..
            for side in vec![SearchSide::Theirs, SearchSide::Ours] {
                // println!("Checking side: {:#?}", side);
                let search_clone = searches.clone();
                let other = search_clone.get(&other_side(&side)).ok_or(SocialContextError::InternalError("other search side not found"))?;
                let search = searches.get_mut(&side).ok_or(SocialContextError::InternalError("search side not found"))?;
                let branches = search.bfs_branches.get_mut();

                for branch_index in 0..branches.len() {
                    println!("===Workspace.collect_until_common_ancestor(): collect_until_common_ancestor 2.1");
                    let current_hash = branches[branch_index].clone();
                    println!("Checking current hash: {:#?}", current_hash);

                    let already_visited = search.found_ancestors.borrow().contains(&current_hash);
                    let seen_on_other_side = other.found_ancestors.borrow().contains(&current_hash) || other.bfs_branches.borrow().contains(&current_hash);

                    if already_visited {
                        println!("===Workspace.collect_until_common_ancestor(): collect_until_common_ancestor 2.2 ALREADY VISITED");
                        // We've seen this diff on this side, so we are at the end of a branch.
                        // Just ignore this hash and close the branch.
                        branches.remove(branch_index);
                        break;
                    }
                    
                    if seen_on_other_side {
                        println!("===Workspace.collect_until_common_ancestor(): collect_until_common_ancestor 2.2 SEEN ON OTHER SIDE");

                        //Add the diff to both searches if it is not there 
                        if !search.found_ancestors.borrow().contains(&current_hash) {
                            search.found_ancestors.get_mut().push(current_hash.clone());
                        };
                        if !other.found_ancestors.borrow().contains(&current_hash) {
                            searches.get_mut(&other_side(&side)).ok_or(SocialContextError::InternalError("other search side not found"))?.found_ancestors.get_mut().push(current_hash.clone());
                        };
                        if self.diffs.get(&current_hash).is_none() && current_hash != NULL_NODE() {
                            let current_diff = Self::get_p_diff_reference::<Retriever>(current_hash.clone())?;
                            self.diffs.insert(current_hash.clone(), current_diff.clone());
                        };
                        // current hash is already in, so it must be our common ancestor!
                        common_ancestor = Some(current_hash);
                        break;
                    } 


                    println!("===Workspace.collect_until_common_ancestor(): collect_until_common_ancestor 2.3");
                    search.found_ancestors.get_mut().push(current_hash.clone());

                    if current_hash == NULL_NODE() {
                        branches.remove(branch_index);
                        break;
                    }

                    let current_diff = Self::get_p_diff_reference::<Retriever>(current_hash.clone())?;
                    self.diffs.insert(current_hash.clone(), current_diff.clone());
                    
                    match &current_diff.parents {
                        None => {
                            // We arrived at a leaf/orphan (no parents).
                            // So we can close this branch and potentially continue
                            // with other unprocessed branches, if they exist.
                            println!("===Workspace.collect_until_common_ancestor(): collect_until_common_ancestor 2.4, no more parents");
                            branches.remove(branch_index);
                            search.reached_end = true;
                            if common_ancestor.is_none() && other.reached_end == true {
                                common_ancestor = Some(NULL_NODE());

                                //Add the diff to both searches if it is not there 
                                if !search.found_ancestors.borrow().contains(&NULL_NODE()) {
                                    search.found_ancestors.get_mut().push(NULL_NODE());
                                };
                                if !other.found_ancestors.borrow().contains(&NULL_NODE()) {
                                    searches.get_mut(&other_side(&side)).ok_or(SocialContextError::InternalError("other search side not found"))?.found_ancestors.get_mut().push(NULL_NODE());
                                };
                                if self.diffs.get(&NULL_NODE()).is_none() {
                                    let current_diff = PerspectiveDiffEntryReference::new(NULL_NODE(), None);
                                    self.diffs.insert(NULL_NODE(), current_diff.clone());
                                };
                                let mut null_childs = vec![current_hash.clone()];
                                if let Some(other_last) = other.found_ancestors.borrow().last().clone() {
                                    null_childs.push(other_last.clone());
                                }
                                self.back_links.insert(NULL_NODE(), null_childs);
                            };
                            // We have to break out of loop to avoid having branch_index run out of bounds
                            break;
                        },
                        Some(parents) => {
                            println!("===Workspace.collect_until_common_ancestor(): collect_until_common_ancestor 2.4, more parents: {:#?}", parents);
                            for parent_index in 0..parents.len() {
                                println!("===Workspace.collect_until_common_ancestor(): collect_until_common_ancestor 2.5, more parents after filter");
                                let parent = parents[parent_index].clone();
                                if let Some(links) =  self.back_links.get_mut(&parent) {
                                    links.push(current_hash.clone());
                                } else {
                                    self.back_links.insert(parent.clone(), vec![current_hash.clone()]);
                                }
                                // The first parent is taken as the successor for the current branch.
                                // If there are multiple parents (i.e. merge commit), we create a new branch..
                                if parent_index == 0 {
                                    println!("Adding new parent to existing branch index");
                                    let _ = std::mem::replace(&mut branches[branch_index], parent.clone());
                                } else {
                                    let already_visited = search.found_ancestors.borrow().contains(&parent) || other.bfs_branches.borrow().contains(&parent);
                                    let seen_on_other_side = other.found_ancestors.borrow().contains(&parent) || other.bfs_branches.borrow().contains(&parent);
                                    if !already_visited && !seen_on_other_side {
                                        println!("===Workspace.collect_until_common_ancestor(): Adding a new branch");
                                        branches.push_back(parent.clone())
                                    }
                                }
                            }
                        }
                    };
        
                    println!("===Workspace.collect_until_common_ancestor(): collect_until_common_ancestor 2.7");
                }
            }
        }

        println!("===Workspace.collect_until_common_ancestor(): collect_until_common_ancestor 3: {:#?} and common ancestor is: {:#?}", searches, common_ancestor);
        
        let fn_end = get_now()?.time();
        debug!("===Workspace.collect_until_common_ancestor() - Profiling: Took: {} to complete collect_until_common_ancestor() function", (fn_end - fn_start).num_milliseconds()); 

        if common_ancestor.is_none() {
            return Err(SocialContextError::NoCommonAncestorFound);
        };

        Ok(common_ancestor.unwrap())
    }

    // pub fn topo_sort_graph(&mut self) -> SocialContextResult<()> {
    //     debug!("===Workspace.topo_sort_graph(): Function start");
    //     let fn_start = get_now()?.time();

    //     let entry_vec = self.entry_map
    //         .clone()
    //         .into_iter()
    //         .collect::<Vec<(Hash, PerspectiveDiffEntryReference)>>();

    //     let mut dot = Vec::<String>::new();

    //     dot.push("digraph {".to_string());
    //     for entry in entry_vec.iter() {
    //         dot.push(format!("{}", entry.0.clone()));
    //         if let Some(parents) = &entry.1.parents {
    //             for p in parents.iter() {
    //                 dot.push(format!("{} -> {}", entry.0, p));
    //             }
    //         } 
    //     }
    //     dot.push("}".to_string());

    //     println!("{}", dot.join("\n"));

    //     self.sorted_diffs = Some(topo_sort_diff_references(&entry_vec)?);

    //     let fn_end = get_now()?.time();
    //     debug!("===Workspace.topo_sort_graph() - Profiling: Took: {} to complete topo_sort_graph() function", (fn_end - fn_start).num_milliseconds()); 
    //     Ok(())
    // }

    pub fn build_graph(&mut self) -> SocialContextResult<()>  {
        debug!("===Workspace.build_graph(): Function start");
        let fn_start = get_now()?.time();

        match self.sorted_diffs.clone() {
            None => Err(SocialContextError::InternalError("Need to 1. collect diffs and then 2. sort them before building the graph")),
            Some(sorted_diffs) => {
                        //Add root node
                if self
                    .get_node_index(&NULL_NODE())
                    .is_none()
                {
                    self.add_node(None, NULL_NODE());
                };

                for diff in sorted_diffs {
                    if diff.0 != NULL_NODE() {
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
                }

                let fn_end = get_now()?.time();
                debug!("===Workspace.build_graph() - Profiling: Took: {} to complete build_graph() function", (fn_end - fn_start).num_milliseconds()); 

                Ok(())
            }
        }
    }

    fn get_p_diff_reference<Retriever: PerspectiveDiffRetreiver>(address: Hash) -> SocialContextResult<PerspectiveDiffEntryReference> {
        Retriever::get(address)
    }

    fn get_snapshot(address: PerspectiveDiffEntryReference) 
        -> SocialContextResult<Option<Snapshot>> 
    {
        debug!("===Workspace.get_snapshot(): Function start");
        let fn_start = get_now()?.time();

        let mut snapshot_links = get_links(
            hash_entry(address)?,
            LinkTypes::Snapshot,
            Some(LinkTag::new("snapshot")),
        )?;

        if snapshot_links.len() > 0 {
            let snapshot = get(snapshot_links.remove(0).target, GetOptions::latest())?
                .ok_or(SocialContextError::InternalError(
                    "Workspace::get_snapshot: Could not find entry while populating search",
                ))?
                .entry()
                .to_app_option::<Snapshot>()?
                .ok_or(SocialContextError::InternalError(
                    "Expected element to contain app entry data",
                ))?;

            let fn_end = get_now()?.time();
            debug!("===Workspace.get_snapshot() - Profiling: Took: {} to complete get_snapshot() function", (fn_end - fn_start).num_milliseconds()); 

            Ok(Some(snapshot))
        } else {
            let fn_end = get_now()?.time();
            debug!("===Workspace.get_snapshot() - Profiling: Took: {} to complete get_snapshot() function", (fn_end - fn_start).num_milliseconds()); 

            Ok(None)
        }
    }



    fn add_node(
        &mut self,
        parents: Option<Vec<NodeIndex<u32>>>,
        diff: Hash,
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
        node: &Hash,
    ) -> Option<&NodeIndex<u32>> {
        self.node_index_map.get(node)
    }

    pub fn get_paths(
        &self,
        child: &Hash,
        ancestor: &Hash,
    ) -> SocialContextResult<Vec<Vec<NodeIndex>>> {
        debug!("===Workspace.get_paths(): Function start");
        let fn_start = get_now()?.time();

        let child_node = self.get_node_index(child).expect("Could not get child node index");
        let ancestor_node = self.get_node_index(ancestor).expect("Could not get ancestor node index");
        let paths = all_simple_paths::<Vec<_>, _>(&self.graph, *child_node, *ancestor_node, 0, None)
            .collect::<Vec<_>>();

        let fn_end = get_now()?.time();
        debug!("===Workspace.get_paths() - Profiling: Took: {} to complete get_paths() function", (fn_end - fn_start).num_milliseconds()); 

        Ok(paths)
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

    pub fn squashed_diff<Retriever: PerspectiveDiffRetreiver>(&self) -> SocialContextResult<PerspectiveDiff> {
        debug!("===Workspace.squashed_diff(): Function start");
        let fn_start = get_now()?.time();

        let mut out = PerspectiveDiff {
            additions: vec![],
            removals: vec![],
        };
        for (_key, value) in self.entry_map.iter() {
            if value.diff == NULL_NODE() {
                continue;
            }
            let diff_entry = Retriever::get::<PerspectiveDiff>(value.diff.clone())?;
            out.additions.append(&mut diff_entry.additions.clone());
            out.removals.append(&mut diff_entry.removals.clone());
        }

        let fn_end = get_now()?.time();
        debug!("===Workspace.squashed_diff() - Profiling: Took: {} to complete squashed_diff() function", (fn_end - fn_start).num_milliseconds()); 

        Ok(out)
    }

    // pub fn squashed_fast_forward_from(&self, base: Hash) -> SocialContextResult<PerspectiveDiff> {
    //     match &self.sorted_diffs {
    //         None => Err(SocialContextError::InternalError("Need to sort first for this fast-forward optimzed squash")),
    //         Some(sorted_diffs) => {
    //             let mut base_found = false;
    //             let mut out = PerspectiveDiff {
    //                 additions: vec![],
    //                 removals: vec![],
    //             };
    //             for i in 0..sorted_diffs.len() {
    //                 let current = &sorted_diffs[i];
    //                 if !base_found {
    //                     if current.0 == base {
    //                         base_found = true;
    //                     }
    //                 } else {
    //                     let diff_entry = get(current.1.diff.clone(), GetOptions::latest())?
    //                         .ok_or(SocialContextError::InternalError(
    //                             "Could not find diff entry for given diff entry reference",
    //                         ))?
    //                         .entry()
    //                         .to_app_option::<PerspectiveDiff>()?
    //                         .ok_or(SocialContextError::InternalError(
    //                             "Expected element to contain app entry data",
    //                         ))?;
    //                     out.additions.append(&mut diff_entry.additions.clone());
    //                     out.removals.append(&mut diff_entry.removals.clone());
    //                 }
    //             }
    //             Ok(out)
    //         }
    //     }
    // }

    pub fn print_graph_debug(&self) {
        debug!(
            "Directed: {:?}\n",
            Dot::with_config(&self.graph, &[Config::NodeIndexLabel])
        );
        debug!(
           "Undirected: {:?}\n",
           Dot::with_config(&self.undirected_graph, &[])
        );
    }
}

#[cfg(test)]
mod tests {
    use dot_structures;
    use crate::retriever::{GLOBAL_MOCKED_GRAPH, MockPerspectiveGraph, node_id_hash};
    use crate::workspace::Workspace;
    use super::NULL_NODE;
    use crate::errors::SocialContextError;

    #[test]
    fn test_collect_until_common_ancestor_forked() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot("digraph {
                0 [ label = \"0\" ]
                1 [ label = \"1\" ]
                2 [ label = \"2\" ]
                3 [ label = \"3\" ]
                4 [ label = \"4\" ]
                5 [ label = \"5\" ]
                6 [ label = \"6\" ]
                7 [ label = \"7\" ]
                8 [ label = \"8\" ]
                9 [ label = \"9\" ]
                10 [ label = \"10\" ]
                11 [ label = \"11\" ]
                12 [ label = \"12\" ]
                1 -> 0 [ label = \"()\" ]
                2 -> 1 [ label = \"()\" ]
                3 -> 2 [ label = \"()\" ]
                4 -> 3 [ label = \"()\" ]
                5 -> 4 [ label = \"()\" ]
                6 -> 5 [ label = \"()\" ]
                7 -> 1 [ label = \"()\" ]
                8 -> 7 [ label = \"()\" ]
                9 -> 8 [ label = \"()\" ]
                10 -> 9 [ label = \"()\" ]
                11 -> 10 [ label = \"()\" ]
                12 -> 11 [ label = \"()\" ]
            }").unwrap();
        }
        update();
    
        let node_1 = node_id_hash(&dot_structures::Id::Plain(String::from("1")));
        let node_6 = node_id_hash(&dot_structures::Id::Plain(String::from("6")));
        let node_12 = node_id_hash(&dot_structures::Id::Plain(String::from("12")));
    
        let mut workspace = Workspace::new();
        let res = workspace.build_diffs::<MockPerspectiveGraph>(node_12.clone(), node_6.clone());
        assert!(res.is_ok());
        
        assert!(workspace.common_ancestors.len() == 1);
        assert_eq!(workspace.common_ancestors.first().unwrap(), &node_1);
    
        assert_eq!(workspace.entry_map.len(), 12);
    
        let node_2 = node_id_hash(&dot_structures::Id::Plain(String::from("2")));
        let node_3 = node_id_hash(&dot_structures::Id::Plain(String::from("3")));
        let node_4 = node_id_hash(&dot_structures::Id::Plain(String::from("4")));
        let node_5 = node_id_hash(&dot_structures::Id::Plain(String::from("5")));
        let node_7 = node_id_hash(&dot_structures::Id::Plain(String::from("7")));
        let node_8 = node_id_hash(&dot_structures::Id::Plain(String::from("8")));
        let node_9 = node_id_hash(&dot_structures::Id::Plain(String::from("9")));
        let node_10 = node_id_hash(&dot_structures::Id::Plain(String::from("10")));
        let node_11 = node_id_hash(&dot_structures::Id::Plain(String::from("11")));
    
        assert!(workspace.entry_map.get(&node_1).is_some());
        assert!(workspace.entry_map.get(&node_2).is_some());
        assert!(workspace.entry_map.get(&node_3).is_some());
        assert!(workspace.entry_map.get(&node_4).is_some());
        assert!(workspace.entry_map.get(&node_5).is_some());
        assert!(workspace.entry_map.get(&node_6).is_some());
        assert!(workspace.entry_map.get(&node_7).is_some());
        assert!(workspace.entry_map.get(&node_8).is_some());
        assert!(workspace.entry_map.get(&node_9).is_some());
        assert!(workspace.entry_map.get(&node_10).is_some());
        assert!(workspace.entry_map.get(&node_11).is_some());
        assert!(workspace.entry_map.get(&node_12).is_some());
    }

    #[test]
    fn test_collect_until_common_ancestor_forward_to_merge_commit() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot("digraph {
                0 [ label = \"0\" ]
                1 [ label = \"1\" ]
                2 [ label = \"2\" ]
                3 [ label = \"3\" ]
                4 [ label = \"4\" ]
                5 [ label = \"5\" ]
                6 [ label = \"6\" ]
                7 [ label = \"7\" ]
                8 [ label = \"8\" ]
                9 [ label = \"9\" ]
                10 [ label = \"10\" ]
                11 [ label = \"11\" ]
                12 [ label = \"12\" ]
                13 [ label = \"12\" ]

                1 -> 0 [ label = \"()\" ]
                2 -> 1 [ label = \"()\" ]
                3 -> 2 [ label = \"()\" ]
                4 -> 3 [ label = \"()\" ]
                5 -> 4 [ label = \"()\" ]
                6 -> 5 [ label = \"()\" ]

                7 -> 1 [ label = \"()\" ]
                8 -> 7 [ label = \"()\" ]
                9 -> 8 [ label = \"()\" ]
                10 -> 9 [ label = \"()\" ]
                11 -> 10 [ label = \"()\" ]

                12 -> 11 [ label = \"()\" ]
                12 -> 6  [ label = \"()\" ]

                13 -> 12 [ label = \"()\" ]
                
            }").unwrap();
        }
        update();
    
        let node_1 = node_id_hash(&dot_structures::Id::Plain(String::from("1")));
        let node_6 = node_id_hash(&dot_structures::Id::Plain(String::from("6")));
        let node_12 = node_id_hash(&dot_structures::Id::Plain(String::from("12")));
        let node_13 = node_id_hash(&dot_structures::Id::Plain(String::from("13")));

        let mut workspace = Workspace::new();
        let res = workspace.build_diffs::<MockPerspectiveGraph>(node_13.clone(), node_6.clone());
        assert!(res.is_ok());
        
        assert!(workspace.common_ancestors.len() == 1);
        assert_eq!(workspace.common_ancestors.first().unwrap(), &node_1);
        assert_eq!(workspace.entry_map.len(), 13);
    
        let node_2 = node_id_hash(&dot_structures::Id::Plain(String::from("2")));
        let node_3 = node_id_hash(&dot_structures::Id::Plain(String::from("3")));
        let node_4 = node_id_hash(&dot_structures::Id::Plain(String::from("4")));
        let node_5 = node_id_hash(&dot_structures::Id::Plain(String::from("5")));
        let node_7 = node_id_hash(&dot_structures::Id::Plain(String::from("7")));
        let node_8 = node_id_hash(&dot_structures::Id::Plain(String::from("8")));
        let node_9 = node_id_hash(&dot_structures::Id::Plain(String::from("9")));
        let node_10 = node_id_hash(&dot_structures::Id::Plain(String::from("10")));
        let node_11 = node_id_hash(&dot_structures::Id::Plain(String::from("11")));
    
        assert!(workspace.entry_map.get(&node_1).is_some());
        assert!(workspace.entry_map.get(&node_2).is_some());
        assert!(workspace.entry_map.get(&node_3).is_some());
        assert!(workspace.entry_map.get(&node_4).is_some());
        assert!(workspace.entry_map.get(&node_5).is_some());
        assert!(workspace.entry_map.get(&node_6).is_some());
        assert!(workspace.entry_map.get(&node_7).is_some());
        assert!(workspace.entry_map.get(&node_8).is_some());
        assert!(workspace.entry_map.get(&node_9).is_some());
        assert!(workspace.entry_map.get(&node_10).is_some());
        assert!(workspace.entry_map.get(&node_11).is_some());
        assert!(workspace.entry_map.get(&node_12).is_some());
        assert!(workspace.entry_map.get(&node_13).is_some());
    }

    #[test]
    fn test_collect_until_common_ancestor_multi_fork() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                0 [ label = "0" ]
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]
                4 [ label = "4" ]
                5 [ label = "5" ]

                1 -> 0 [ label = "()" ]
                2 -> 1 [ label = "()" ]

                3 -> 0 [ label = "()" ]

                4 -> 0 [ label = "()" ]
                5 -> 4 [ label = "()" ]
            }"#).unwrap();
        }
        update();
    
        let node_0 = node_id_hash(&dot_structures::Id::Plain(String::from("0")));
        let node_1 = node_id_hash(&dot_structures::Id::Plain(String::from("1")));
        let node_2 = node_id_hash(&dot_structures::Id::Plain(String::from("2")));
        let node_3 = node_id_hash(&dot_structures::Id::Plain(String::from("3")));
        //let node_4 = node_id_hash(&dot_structures::Id::Plain(String::from("4")));
        //let node_5 = node_id_hash(&dot_structures::Id::Plain(String::from("5")));

        let mut workspace = Workspace::new();
        let res = workspace.build_diffs::<MockPerspectiveGraph>(node_3.clone(), node_2.clone());
        assert!(res.is_ok());
        
        assert!(workspace.common_ancestors.len() == 1);
        assert_eq!(workspace.common_ancestors.first().unwrap(), &node_0);
        assert_eq!(workspace.entry_map.len(), 4);
    
        assert!(workspace.entry_map.get(&node_0).is_some());
        assert!(workspace.entry_map.get(&node_1).is_some());
        assert!(workspace.entry_map.get(&node_2).is_some());
        assert!(workspace.entry_map.get(&node_3).is_some()); 
    }


    #[test]
    fn test_collect_until_common_ancestor_fork_on_top_of_merge() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                0 [ label = "0" ]
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]
                4 [ label = "4" ]
                5 [ label = "5" ]

                1 -> 0 
                2 -> 1 
                3 -> 2 
                4 -> 3 
                5 -> 4
                
                6
                7
                8
                9
                10

                7 -> 6
                8 -> 7
                9 -> 8
                10 -> 9

                8 -> 0
                
            }"#).unwrap();
        }
        update();
    
        let node_0 = node_id_hash(&dot_structures::Id::Plain(String::from("0")));
        let node_1 = node_id_hash(&dot_structures::Id::Plain(String::from("1")));
        let node_2 = node_id_hash(&dot_structures::Id::Plain(String::from("2")));
        let node_3 = node_id_hash(&dot_structures::Id::Plain(String::from("3")));
        let node_4 = node_id_hash(&dot_structures::Id::Plain(String::from("4")));
        let node_5 = node_id_hash(&dot_structures::Id::Plain(String::from("5")));
        //let node_6 = node_id_hash(&dot_structures::Id::Plain(String::from("6")));
        //let node_7 = node_id_hash(&dot_structures::Id::Plain(String::from("7")));
        let node_8 = node_id_hash(&dot_structures::Id::Plain(String::from("8")));
        let node_9 = node_id_hash(&dot_structures::Id::Plain(String::from("9")));
        let node_10 = node_id_hash(&dot_structures::Id::Plain(String::from("10")));

        let mut workspace = Workspace::new();
        let res = workspace.build_diffs::<MockPerspectiveGraph>(node_5.clone(), node_10.clone());
        println!("Got result: {:#?}", res);
        assert!(res.is_ok());
        
        assert!(workspace.common_ancestors.len() == 2);
        assert_eq!(workspace.common_ancestors.first().unwrap(), &node_0);
        assert_eq!(workspace.common_ancestors.last().unwrap(), &NULL_NODE());
        assert_eq!(workspace.entry_map.len(), 12);
    
        assert!(workspace.entry_map.get(&node_0).is_some());
        assert!(workspace.entry_map.get(&node_1).is_some());
        assert!(workspace.entry_map.get(&node_2).is_some());
        assert!(workspace.entry_map.get(&node_3).is_some());
        assert!(workspace.entry_map.get(&node_4).is_some());
        assert!(workspace.entry_map.get(&node_5).is_some());
        //assert!(workspace.entry_map.get(&node_7).is_some());
        assert!(workspace.entry_map.get(&node_8).is_some());
        assert!(workspace.entry_map.get(&node_9).is_some());
        assert!(workspace.entry_map.get(&node_10).is_some()); 
    }


    #[test]
    fn test_collect_until_common_ancestor_unconnected_fork() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                0 [ label = "0" ]
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]
                4 [ label = "4" ]
                5 [ label = "5" ]

                1 -> 0 
                2 -> 1 
                3 -> 2 
                4 -> 3 
                5 -> 4
                
                6
                7
                8
                9
                10

                7 -> 6
                8 -> 7
                9 -> 8
                10 -> 9
            }"#).unwrap();
        }
        update();
    
        let node_0 = node_id_hash(&dot_structures::Id::Plain(String::from("0")));
        let node_1 = node_id_hash(&dot_structures::Id::Plain(String::from("1")));
        let node_2 = node_id_hash(&dot_structures::Id::Plain(String::from("2")));
        let node_3 = node_id_hash(&dot_structures::Id::Plain(String::from("3")));
        let node_4 = node_id_hash(&dot_structures::Id::Plain(String::from("4")));
        let node_5 = node_id_hash(&dot_structures::Id::Plain(String::from("5")));
        let node_6 = node_id_hash(&dot_structures::Id::Plain(String::from("6")));
        let node_7 = node_id_hash(&dot_structures::Id::Plain(String::from("7")));
        let node_8 = node_id_hash(&dot_structures::Id::Plain(String::from("8")));
        let node_9 = node_id_hash(&dot_structures::Id::Plain(String::from("9")));
        let node_10 = node_id_hash(&dot_structures::Id::Plain(String::from("10")));

        let mut workspace = Workspace::new();
        let res = workspace.build_diffs::<MockPerspectiveGraph>(node_5.clone(), node_10.clone());
        println!("Got result: {:#?}", res);
        assert!(res.is_ok());
        
        assert!(workspace.common_ancestors.len() == 1);
        assert_eq!(workspace.common_ancestors.first().unwrap(), &NULL_NODE());
        assert_eq!(workspace.entry_map.len(), 12);
    
        assert!(workspace.entry_map.get(&NULL_NODE()).is_some());
        assert!(workspace.entry_map.get(&node_0).is_some());
        assert!(workspace.entry_map.get(&node_1).is_some());
        assert!(workspace.entry_map.get(&node_2).is_some());
        assert!(workspace.entry_map.get(&node_3).is_some());
        assert!(workspace.entry_map.get(&node_4).is_some());
        assert!(workspace.entry_map.get(&node_5).is_some());
        assert!(workspace.entry_map.get(&node_6).is_some());
        assert!(workspace.entry_map.get(&node_7).is_some());
        assert!(workspace.entry_map.get(&node_8).is_some());
        assert!(workspace.entry_map.get(&node_9).is_some());
        assert!(workspace.entry_map.get(&node_10).is_some());
    }

    #[test]
    fn test_collect_until_common_ancestor_ff_to_merge() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                0 [ label = "0" ]
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]

                1 -> 0 
                2 -> 0 
                3 -> 1 
                3 -> 2
                
            }"#).unwrap();
        }
        update();
    
        let node_0 = node_id_hash(&dot_structures::Id::Plain(String::from("0")));
        let node_1 = node_id_hash(&dot_structures::Id::Plain(String::from("1")));
        let node_2 = node_id_hash(&dot_structures::Id::Plain(String::from("2")));
        let node_3 = node_id_hash(&dot_structures::Id::Plain(String::from("3")));

        let mut workspace = Workspace::new();
        let res = workspace.build_diffs::<MockPerspectiveGraph>(node_1.clone(), node_3.clone());
        println!("Got result: {:#?}", res);
        assert!(res.is_ok());
        
        println!("common ancestors: {:?}", workspace.common_ancestors);
        assert_eq!(workspace.common_ancestors.len(), 2);
        assert_eq!(workspace.common_ancestors.first().unwrap(), &node_1);
        assert_eq!(workspace.common_ancestors.last().unwrap(), &node_0);
        assert_eq!(workspace.entry_map.len(), 4);
    
        assert!(workspace.entry_map.get(&node_0).is_some());
        assert!(workspace.entry_map.get(&node_1).is_some());
        assert!(workspace.entry_map.get(&node_2).is_some());
        assert!(workspace.entry_map.get(&node_3).is_some());
    }

    #[test]
    fn test_collect_until_common_ancestor_complex_merge() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                0 [ label = "0" ]
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]
                4 [ label = "4" ]
                5 [ label = "5" ]
                6 [ label = "6" ]
                1 -> 0 [ label = "()" ]
                2 -> 0 [ label = "()" ]
                3 -> 0 [ label = "()" ]
                4 -> 2 [ label = "()" ]
                5 -> 4 [ label = "()" ]
                5 -> 3 [ label = "()" ]
                6 -> 5 [ label = "()" ]
            }"#).unwrap();
        }
        update();
    
        let node_0 = node_id_hash(&dot_structures::Id::Plain(String::from("0")));
        let node_1 = node_id_hash(&dot_structures::Id::Plain(String::from("1")));
        let node_2 = node_id_hash(&dot_structures::Id::Plain(String::from("2")));
        let node_3 = node_id_hash(&dot_structures::Id::Plain(String::from("3")));
        let node_4 = node_id_hash(&dot_structures::Id::Plain(String::from("4")));
        let node_5 = node_id_hash(&dot_structures::Id::Plain(String::from("5")));
        let node_6 = node_id_hash(&dot_structures::Id::Plain(String::from("6")));

        let mut workspace = Workspace::new();
        let res = workspace.build_diffs::<MockPerspectiveGraph>(node_1.clone(), node_6.clone());
        println!("Got result: {:#?}", res);
        assert!(res.is_ok());
        
        println!("common ancestors: {:?}", workspace.common_ancestors);
        assert_eq!(workspace.common_ancestors.len(), 1);
        assert_eq!(workspace.common_ancestors.last().unwrap(), &node_0);
        assert_eq!(workspace.entry_map.len(), 7);
    
        assert!(workspace.entry_map.get(&node_0).is_some());
        assert!(workspace.entry_map.get(&node_1).is_some());
        assert!(workspace.entry_map.get(&node_2).is_some());
        assert!(workspace.entry_map.get(&node_3).is_some());
        assert!(workspace.entry_map.get(&node_4).is_some());
        assert!(workspace.entry_map.get(&node_5).is_some());
        assert!(workspace.entry_map.get(&node_6).is_some());
    }

    #[test]
    fn test_collect_until_common_ancestor_complex_merge_implicit_zero() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]
                4 [ label = "4" ]
                5 [ label = "5" ]
                6 [ label = "6" ]
                4 -> 2 [ label = "()" ]
                5 -> 4 [ label = "()" ]
                5 -> 3 [ label = "()" ]
                6 -> 5 [ label = "()" ]
            }"#).unwrap();
        }
        update();
    
        let node_1 = node_id_hash(&dot_structures::Id::Plain(String::from("1")));
        let node_6 = node_id_hash(&dot_structures::Id::Plain(String::from("6")));

        let mut workspace = Workspace::new();
        let res = workspace.build_diffs::<MockPerspectiveGraph>(node_1.clone(), node_6.clone());
        println!("Got result: {:#?}", res);
        assert!(res.is_err());
        assert_eq!(format!("{:?}", res.err().unwrap()), format!("{:?}", SocialContextError::NoCommonAncestorFound));
    }

    #[test]
    fn real_world_graph() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                0 [ label = "0" ]
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]
                4 [ label = "4" ]
                5 [ label = "5" ]
                6 [ label = "6" ]
                7 [ label = "7" ]
                8 [ label = "8" ]
                9 [ label = "9" ]
                10 [ label = "10" ]
                11 [ label = "11" ]
                12 [ label = "12" ]
                13 [ label = "13" ]
                14 [ label = "14" ]
                15 [ label = "15" ]
                16 [ label = "16" ]
                1 -> 0 [ label = "()" ]
                2 -> 1 [ label = "()" ]
                3 -> 2 [ label = "()" ]
                4 -> 3 [ label = "()" ]
                5 -> 4 [ label = "()" ]
                6 -> 5 [ label = "()" ]
                7 -> 6 [ label = "()" ]
                8 -> 7 [ label = "()" ]
                9 -> 8 [ label = "()" ]
                10 -> 9 [ label = "()" ]
                11 -> 1 [ label = "()" ]
                12 -> 2 [ label = "()" ]
                12 -> 11 [ label = "()" ]
                13 -> 3 [ label = "()" ]
                13 -> 12 [ label = "()" ]
                14 -> 6 [ label = "()" ]
                14 -> 13 [ label = "()" ]
                15 -> 7 [ label = "()" ]
                15 -> 14 [ label = "()" ]
                16 -> 8 [ label = "()" ]
                16 -> 15 [ label = "()" ]
            }"#).unwrap();
        }
        update();
    
        let node_10 = node_id_hash(&dot_structures::Id::Plain(String::from("10")));
        let node_16 = node_id_hash(&dot_structures::Id::Plain(String::from("16")));
        let node_8 = node_id_hash(&dot_structures::Id::Plain(String::from("8")));

        let mut workspace = Workspace::new();
        let res = workspace.build_diffs::<MockPerspectiveGraph>(node_16.clone(), node_10.clone());
        assert!(res.is_ok());
        //assert_eq!(workspace.common_ancestors.len(), 1);
        assert_eq!(workspace.common_ancestors.last().unwrap(), &node_8);
        println!("Got result: {:#?}", res);
    }
}
