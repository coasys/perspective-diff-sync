use hdk::prelude::*;
use petgraph::dot::{Config, Dot};
use petgraph::{
    algo::{all_simple_paths, dominators::simple_fast},
    graph::{DiGraph, Graph, NodeIndex, UnGraph},
};
use std::collections::HashMap;
use std::ops::Index;
use perspective_diff_sync_integrity::{PerspectiveDiffEntryReference, Snapshot, LinkTypes};

use crate::errors::{SocialContextError, SocialContextResult};

pub fn bubble_sort_diff_references(mut arr: &mut Vec<(HoloHash<holo_hash::hash_type::Action>, PerspectiveDiffEntryReference)>) {
    for i in 0..arr.len() {
        let mut j = i;
        while j < arr.len() {
            if i!=j { 
                let current_parents = &arr[i].1.parents;
                let next_hash = &arr[j].0;
    
                if let Some(parents) = current_parents{
                    if parents.contains(next_hash) {
                        move_me(&mut arr, i, j);
                        j=i;
                    }
                }
            };

            j = j+1;
        }
    }
}

pub struct Search {
    pub graph: DiGraph<HoloHash<holo_hash::hash_type::Action>, ()>,
    pub undirected_graph: UnGraph<HoloHash<holo_hash::hash_type::Action>, ()>,
    pub node_index_map: HashMap<HoloHash<holo_hash::hash_type::Action>, NodeIndex<u32>>,
    pub entry_map: HashMap<HoloHash<holo_hash::hash_type::Action>, PerspectiveDiffEntryReference>,
}

impl Search {
    pub fn new() -> Search {
        Search {
            graph: Graph::new(),
            undirected_graph: Graph::new_undirected(),
            node_index_map: HashMap::new(),
            entry_map: HashMap::new(),
        }
    }

    pub fn add_node(
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

    pub fn add_entry(
        &mut self,
        hash: HoloHash<holo_hash::hash_type::Action>,
        diff: PerspectiveDiffEntryReference,
    ) {
        self.entry_map.insert(hash, diff);
    }

    pub fn get_entry(
        &mut self,
        hash: &HoloHash<holo_hash::hash_type::Action>,
    ) -> Option<PerspectiveDiffEntryReference> {
        self.entry_map.remove(hash)
    }

    pub fn get_node_index(
        &self,
        node: &HoloHash<holo_hash::hash_type::Action>,
    ) -> Option<&NodeIndex<u32>> {
        self.node_index_map.get(node)
    }

    pub fn index(&mut self, index: NodeIndex) -> HoloHash<holo_hash::hash_type::Action> {
        self.graph.index(index).clone()
    }

    pub fn print(&self) {
        debug!(
            "Directed: {:?}\n",
            Dot::with_config(&self.graph, &[Config::NodeIndexLabel])
        );
        debug!(
            "Undirected: {:?}\n",
            Dot::with_config(&self.undirected_graph, &[])
        );
    }

    pub fn get_paths(
        &self,
        child: NodeIndex<u32>,
        ancestor: NodeIndex<u32>,
    ) -> Vec<Vec<NodeIndex>> {
        let paths = all_simple_paths::<Vec<_>, _>(&self.graph, child, ancestor, 0, None)
            .collect::<Vec<_>>();
        paths
    }

    pub fn find_common_ancestor(
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
}

fn move_me<T>(arr: &mut Vec<T>, old_index: usize, new_index: usize) {
    if old_index < new_index {
        arr[old_index..=new_index].rotate_left(1);
    } else {
        arr[new_index..=old_index].rotate_right(1);
    }
}

//TODO; add ability to determine depth of recursion
pub fn populate_search(
    search: Option<Search>,
    latest: HoloHash<holo_hash::hash_type::Action>,
    break_on: Option<HoloHash<holo_hash::hash_type::Action>>,
) -> SocialContextResult<Search> {
    let mut search_position = (latest, -1 as i64);
    let mut diffs = vec![];
    let mut unseen_parents = vec![];
    let mut depth = 0 as i64;

    let mut search = if search.is_none() {
        Search::new()
    } else {
        search.unwrap()
    };

    //Search up the chain starting from the latest known hash
    loop {
        let diff = get(search_position.0.clone(), GetOptions::latest())?
            .ok_or(SocialContextError::InternalError(
                "Could not find entry while populating search",
            ))?
            .entry()
            .to_app_option::<PerspectiveDiffEntryReference>()?
            .ok_or(SocialContextError::InternalError(
                "Expected element to contain app entry data",
            ))?;

        //Check if entry is already in graph
        if !search.entry_map.contains_key(&search_position.0) {
            diffs.push((search_position.0.clone(), diff.clone()));
            depth += 1;
            //Check if this diff was found in a fork traversal (which happen after the main route traversal)
            //search position = -1 means that diff was found in main traversal
            //any other value denotes where in the array it should be moved to as to keep consistent order of diffs
            //it is important to keep the correct order so when we add to the graph there is always a parent entry for the node
            //to link from
            if search_position.1 != -1 {
                let len = diffs.len() - 1;
                let move_index = if search_position.1 == 0 {
                    if len == 0 {
                        len
                    } else {
                        len - 1
                    }
                } else {
                    (len as i64 - search_position.1) as usize
                };
                move_me(&mut diffs, len, move_index);
            }
        };
        if let Some(ref break_on_hash) = break_on {
            if &search_position.0 == break_on_hash && unseen_parents.len() == 0 {
                debug!("Breaking on supplied hash");
                break;
            }
        };
        //check if diff has a snapshot entry
        let mut snapshot_links = get_links(hash_entry(&diff)?, LinkTypes::Snapshot, Some(LinkTag::new("snapshot")))?;
        if snapshot_links.len() > 0 {
            debug!("Found snapshot");
            let mut snapshot = get(snapshot_links.remove(0).target, GetOptions::latest())?
                .ok_or(SocialContextError::InternalError(
                    "Could not find entry while populating search",
                ))?
                .entry()
                .to_app_option::<Snapshot>()?
                .ok_or(SocialContextError::InternalError(
                    "Expected element to contain app entry data",
                ))?;
            diffs.append(&mut snapshot.diff_graph);
            //Be careful with break here where there are still unseen parents
            if unseen_parents.len() == 0 {
                debug!("No more unseen parents within snapshot block");
                break;
            } else {
                search_position = unseen_parents.remove(0);
            }
        } else {
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
                if parents.iter().all(|val| {
                    diffs
                        .clone()
                        .into_iter()
                        .map(|val| val.0)
                        .collect::<Vec<_>>()
                        .contains(val)
                }) {
                    if unseen_parents.len() == 0 {
                        debug!("Parents of item seen and unseen 0");
                        break;
                    } else {
                        debug!("last moving onto unseen");
                        search_position = unseen_parents.remove(0);
                    }
                } else {
                    search_position = (parents.remove(0), -1);
                    unseen_parents.append(
                        &mut parents
                            .into_iter()
                            .map(|val| (val, depth - 1))
                            .collect::<Vec<_>>(),
                    );
                };
            };
        };
    }

    debug!("diff list BEFORE sort: {:#?}", diffs);
    bubble_sort_diff_references(&mut diffs);
    debug!("diff list AFTER sort: {:#?}", diffs);

    //Add root node
    if search
        .get_node_index(&ActionHash::from_raw_36(vec![0xdb; 36]))
        .is_none()
    {
        search.add_node(None, ActionHash::from_raw_36(vec![0xdb; 36]));
    };
    for diff in diffs {
        if !search.entry_map.contains_key(&diff.0) {
            search.add_entry(diff.0.clone(), diff.1.clone());
            if diff.1.parents.is_some() {
                let mut parents = vec![];
                for parent in diff.1.parents.unwrap() {
                    let parent = search
                        .get_node_index(&parent)
                        .ok_or(SocialContextError::InternalError("Did not find parent"))?;
                    parents.push(parent.clone());
                }
                search.add_node(Some(parents), diff.0);
            } else {
                search.add_node(Some(vec![NodeIndex::from(0)]), diff.0);
            }
        }
    }

    Ok(search)
}
