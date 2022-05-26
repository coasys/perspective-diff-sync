use std::collections::HashMap;
use petgraph::{graph::{UnGraph, DiGraph, Graph, NodeIndex}, algo::{all_simple_paths, dominators::simple_fast}};
use petgraph::dot::{Dot, Config};
use hdk::prelude::*;
use std::ops::Index;

use crate::{errors::{SocialContextResult, SocialContextError}, PerspectiveDiffEntryReference};

pub struct Search {
    pub graph: DiGraph<HoloHash<holo_hash::hash_type::Header>, ()>,
    pub undirected_graph: UnGraph<HoloHash<holo_hash::hash_type::Header>, ()>,
    pub node_index_map: HashMap<HoloHash<holo_hash::hash_type::Header>, NodeIndex<u32>>,
    pub entry_map: HashMap<HoloHash<holo_hash::hash_type::Header>, PerspectiveDiffEntryReference>
}


impl Search {
    pub fn new() -> Search {
        Search {
            graph: Graph::new(),
            undirected_graph: Graph::new_undirected(),
            node_index_map: HashMap::new(),
            entry_map: HashMap::new()
        }
    }

    pub fn add_node(&mut self, parents: Option<Vec<NodeIndex<u32>>>, diff: HoloHash<holo_hash::hash_type::Header>) -> NodeIndex<u32> {
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

    pub fn add_entry(&mut self, hash: HoloHash<holo_hash::hash_type::Header>, diff: PerspectiveDiffEntryReference) {
        self.entry_map.insert(hash, diff);
    }

    pub fn get_entry(&mut self, hash: &HoloHash<holo_hash::hash_type::Header>) -> Option<PerspectiveDiffEntryReference> {
        self.entry_map.remove(hash)
    }

    pub fn get_node_index(&self, node: &HoloHash<holo_hash::hash_type::Header>) -> Option<&NodeIndex<u32>> {
        self.node_index_map.get(node)
    }

    pub fn index(&mut self, index: NodeIndex) -> HoloHash<holo_hash::hash_type::Header> {
        self.graph.index(index).clone()
    }

    pub fn print(&self) {
        debug!("Directed: {:?}\n", Dot::with_config(&self.graph, &[Config::NodeIndexLabel]));
        debug!("Undirected: {:?}\n", Dot::with_config(&self.undirected_graph, &[]));
    }

    pub fn get_paths(&self, child: NodeIndex<u32>, ancestor: NodeIndex<u32>) -> Vec<Vec<NodeIndex>> {
        let paths = all_simple_paths::<Vec<_>, _>(&self.graph, child, ancestor, 0, None)
            .collect::<Vec<_>>();
        debug!("Simple paths: {:#?}", paths);
        paths
    }

    pub fn find_common_ancestor(&self, root: NodeIndex<u32>, second: NodeIndex<u32>) -> Option<NodeIndex> {
        let common = simple_fast(&self.undirected_graph, root).immediate_dominator(second);
        debug!("Common: {:#?}", common);
        common
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
pub fn populate_search(search: Option<Search>, latest: HoloHash<holo_hash::hash_type::Header>) -> SocialContextResult<Search> {
    let mut search_position = (latest, 0);
    let mut diffs = vec![];
    let mut unseen_parents = vec![];
    let mut depth = 0;

    let mut search = if search.is_none() {
        Search::new()
    } else {
        search.unwrap()
    };

    loop {
        let diff = get(search_position.0.clone(), GetOptions::latest())?.ok_or(SocialContextError::InternalError("Could not find entry while populating search"))?
        .entry().to_app_option::<PerspectiveDiffEntryReference>()?.ok_or(
            SocialContextError::InternalError("Expected element to contain app entry data"),
        )?;
        if !search.entry_map.contains_key(&search_position.0) {
            diffs.push((search_position.0, diff.clone()));
            depth +=1;
            if search_position.1 != 0 {
                let len = diffs.len();
                move_me(&mut diffs, len-1, len-1-search_position.1);
            }
        };
        if diff.parents.is_none() {
            //No parents, we have reached the end of the chain
            //Now move onto traversing parents
            if unseen_parents.len() == 0 {
                debug!("No more unseen items");
                break
            } else {
                debug!("Moving onto unseen fork items");
                search_position = unseen_parents.remove(0);
            }
        } else {
            //Do the fork traversals
            let mut parents = diff.parents.unwrap();
            //Check if all parents have already been seen, if so then break or move onto next unseen parents
            if parents.iter().all(|val| diffs.clone().into_iter().map(|val| val.0).collect::<Vec<_>>().contains(val)) {
                if unseen_parents.len() == 0 {
                    break;
                } else {
                    search_position = unseen_parents.remove(0);
                }
            } else {
                search_position = (parents.remove(0), 0);
                unseen_parents.append(&mut parents.into_iter().enumerate().map(|val| (val.1, depth+val.0)).collect::<Vec<_>>());
            };
        }
    }

    diffs.reverse();
    //debug!("Got diff list: {:#?}", diffs);

    //Add root node
    if search.get_node_index(&HeaderHash::from_raw_36(vec![0xdb; 36])).is_none() {
        search.add_node(None, HeaderHash::from_raw_36(vec![0xdb; 36]));
    };
    for diff in diffs {
        search.add_entry(diff.0.clone(), diff.1.clone());
        if diff.1.parents.is_some() {
            let mut parents = vec![];
            for parent in diff.1.parents.unwrap() {
                let parent = search.get_node_index(&parent).ok_or(SocialContextError::InternalError("Did not find parent"))?;
                parents.push(parent.clone());
            }
            search.add_node(Some(parents), diff.0);
        } else {
            search.add_node(Some(vec![NodeIndex::from(0)]), diff.0);
        }
    }

    Ok(search)
}
