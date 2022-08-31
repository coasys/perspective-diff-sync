use perspective_diff_sync_integrity::{PerspectiveDiffEntryReference};
use std::collections::BTreeMap;
use hdk::prelude::*;
use std::sync::Mutex;

use crate::Hash;
use crate::errors::{SocialContextResult, SocialContextError};

pub trait PerspectiveDiffRetreiver {
    fn get(hash: Hash) -> SocialContextResult<PerspectiveDiffEntryReference>;
}

pub struct HolochainRetreiver;

impl PerspectiveDiffRetreiver for HolochainRetreiver {
    fn get(hash: Hash) -> SocialContextResult<PerspectiveDiffEntryReference> {
        get(hash, GetOptions::latest())?
            .ok_or(SocialContextError::InternalError(
                "Could not find entry while populating search",
            ))?
            .entry()
            .to_app_option::<PerspectiveDiffEntryReference>()?
            .ok_or(SocialContextError::InternalError(
                "Expected element to contain app entry data",
            ))
    }
}

pub struct MockPerspectiveGraph {
    pub graph: Vec<PerspectiveDiffEntryReference>,
    pub graph_map: BTreeMap<Hash, PerspectiveDiffEntryReference>,
}

impl PerspectiveDiffRetreiver for MockPerspectiveGraph {
    fn get(hash: Hash) -> SocialContextResult<PerspectiveDiffEntryReference> {
        Ok(GLOBAL_MOCKED_GRAPH.lock().expect("Could not get lock on graph map").graph_map.get(&hash).expect("Could not find entry in map").to_owned())
    }
}

pub struct GraphInput {
    nodes: u8,
    associations: Vec<Associations>
}

pub struct Associations {
    pub node_source: u8,
    pub node_targets: Vec<u8>,
}

//TODO; we need a more intuitive way to input graphs rather than this GraphInput struct
impl MockPerspectiveGraph {
    pub fn new(graph_input: GraphInput) -> MockPerspectiveGraph {
        let mut graph = MockPerspectiveGraph {
            graph: vec![],
            graph_map: BTreeMap::new()
        };

        for n in 0..graph_input.nodes {
            let mocked_hash = ActionHash::from_raw_36(vec![n; 36]);
            let associations: Vec<&Associations> = graph_input.associations.iter().filter(|association| association.node_source == n).collect();
            let parents = if associations.len() > 0 {
                let mut temp = vec![];
                for association in associations.clone() {
                    for targets in association.node_targets.clone() {
                        temp.push(ActionHash::from_raw_36(vec![targets; 36]))
                    };
                };
                Some(temp)
            } else {
                None
            };
            let mocked_diff = PerspectiveDiffEntryReference {
                diff: mocked_hash.clone(),
                parents: parents
            };
            graph.graph.push(mocked_diff.clone());
            graph.graph_map.insert(mocked_hash, mocked_diff);
        }
        graph
    }
}

lazy_static!{
    static ref GLOBAL_MOCKED_GRAPH: Mutex<MockPerspectiveGraph> = Mutex::new(MockPerspectiveGraph::new(GraphInput {
        nodes: 6,
        associations: vec![
            Associations {
                node_source: 1,
                node_targets: vec![0]
            },
            Associations {
                node_source: 2,
                node_targets: vec![0]
            },
            Associations {
                node_source: 3,
                node_targets: vec![1]
            },
            Associations {
                node_source: 4,
                node_targets: vec![2]
            },
            Associations {
                node_source: 5,
                node_targets: vec![3, 4]
            }
        ]
    }));
}

#[test]
fn can_create_graph() {
    let test = MockPerspectiveGraph::new(GraphInput {
        nodes: 6,
        associations: vec![
            Associations {
                node_source: 1,
                node_targets: vec![0]
            },
            Associations {
                node_source: 2,
                node_targets: vec![0]
            },
            Associations {
                node_source: 3,
                node_targets: vec![1]
            },
            Associations {
                node_source: 4,
                node_targets: vec![2]
            },
            Associations {
                node_source: 5,
                node_targets: vec![3, 4]
            }
        ]
    });
    assert_eq!(test.graph.len(), 6);
    println!("Got graph: {:#?}", test.graph);
}

#[test]
fn example_test() {
    use crate::workspace::Workspace;
    
    let mut workspace = Workspace::new();
    let res = workspace.collect_until_common_ancestor::<MockPerspectiveGraph>(ActionHash::from_raw_36(vec![5; 36]), ActionHash::from_raw_36(vec![4; 36]));
    println!("Got result: {:#?}", res);
}