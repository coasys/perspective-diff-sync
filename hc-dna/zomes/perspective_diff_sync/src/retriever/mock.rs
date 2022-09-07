use perspective_diff_sync_integrity::{PerspectiveDiffEntryReference};
use std::collections::BTreeMap;
use std::sync::Mutex;
use dot_structures;
use graphviz_rust;
use hdk::prelude::*;
use crate::Hash;
use crate::errors::{SocialContextResult, SocialContextError};
use super::PerspectiveDiffRetreiver;

#[derive(Debug)]
pub struct MockPerspectiveGraph {
    pub graph: Vec<PerspectiveDiffEntryReference>,
    pub graph_map: BTreeMap<Hash, PerspectiveDiffEntryReference>,
}

impl PerspectiveDiffRetreiver for MockPerspectiveGraph {
    //fn get(hash: Hash) -> SocialContextResult<PerspectiveDiffEntryReference> 
    //{
    //    Ok(GLOBAL_MOCKED_GRAPH.lock().expect("Could not get lock on graph map").graph_map.get(&hash).expect("Could not find entry in map").to_owned())
    //}

    fn get<T>(hash: Hash) -> SocialContextResult<T> 
        where
        T: TryFrom<SerializedBytes, Error = SerializedBytesError>,
    {
        Err(SocialContextError::InternalError("Not implemented"))
    }



    fn create_entry<I, E, E2>(entry: I) -> SocialContextResult<Hash>
        where
        ScopedEntryDefIndex: for<'a> TryFrom<&'a I, Error = E2>,
        EntryVisibility: for<'a> From<&'a I>,
        Entry: TryFrom<I, Error = E>,
        WasmError: From<E>,
        WasmError: From<E2>
    {
        Err(SocialContextError::InternalError("Not implemented"))
    }

    fn current_revision() -> SocialContextResult<Option<Hash>> {
        let revision = *CURRENT_REVISION.lock().expect("Could not get lock on CURRENT_REVISION");
        if revision == ActionHash::from_raw_36(vec![999; 36]) {
            Ok(None)
        } else {
            Ok(Some(revision))
        }
    }

    fn latest_revision() -> SocialContextResult<Option<Hash>> {
        let revision = *LATEST_REVISION.lock().expect("Could not get lock on LATEST_REVISION");
        if revision == ActionHash::from_raw_36(vec![999; 36]) {
            Ok(None)
        } else {
            Ok(Some(revision))
        }
    }

    fn update_current_revision(rev: Hash) -> SocialContextResult<()> {
        let revision = CURRENT_REVISION.lock().expect("Could not get lock on LATEST_REVISION");
        *revision = rev;
        Ok(())
    }

    fn update_latest_revision(rev: Hash) -> SocialContextResult<()> {
        let revision = LATEST_REVISION.lock().expect("Could not get lock on LATEST_REVISION");
        *revision = rev;
        Ok(())
    }

}

pub struct GraphInput {
    pub nodes: u8,
    pub associations: Vec<Associations>
}

pub struct Associations {
    pub node_source: u8,
    pub node_targets: Vec<u8>,
}

#[allow(dead_code)]
pub fn node_id_hash(id: &dot_structures::Id) -> Hash {
    let mut string = match id {
        dot_structures::Id::Html(s) => s,
        dot_structures::Id::Escaped(s) => s,
        dot_structures::Id::Plain(s) => s,
        dot_structures::Id::Anonymous(s) => s,
    }.clone();
    if string.len() > 36 {
        let _ = string.split_off(36);
    } else {
        while string.len() < 36 {
            string.push_str("x");
        }
    }
    ActionHash::from_raw_36(string.into_bytes())
}

#[allow(dead_code)]
fn unwrap_vertex(v: dot_structures::Vertex) -> Option<dot_structures::NodeId> {
    match v {
        dot_structures::Vertex::N(id) => Some(id),
        _ => None
    }
}

#[allow(dead_code)]
fn unwrap_edge(edge: dot_structures::Edge) -> Option<(dot_structures::NodeId, dot_structures::NodeId)> {
    match edge.ty {
        dot_structures::EdgeTy::Pair(a, b) => {
            let au = unwrap_vertex(a);
            let ab = unwrap_vertex(b);
            if au.is_some() && ab.is_some() {
                Some((au.unwrap(), ab.unwrap()))
            } else {
                None
            }
        }
        _ => None
    }
}

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

    #[allow(dead_code)]
    pub fn from_dot(source: &str) -> SocialContextResult<MockPerspectiveGraph> {
        match graphviz_rust::parse(source).map_err(|_| SocialContextError::InternalError("Can't parse as DOT string"))? {
            dot_structures::Graph::Graph{..} => Err(SocialContextError::InternalError("Can't work with undirected DOT graphs")),
            dot_structures::Graph::DiGraph{stmts, ..} => {
                let mut graph = MockPerspectiveGraph {
                    graph: vec![],
                    graph_map: BTreeMap::new()
                };

                let mut hashes = Vec::<Hash>::new();
                let mut parents: BTreeMap<Hash, Vec<Hash>> = BTreeMap::new();

                for s in stmts.iter() {
                    match s {
                        dot_structures::Stmt::Node(node) => hashes.push(node_id_hash(&node.id.0)),
                        dot_structures::Stmt::Edge(edge) => {
                            if let Some(e) = unwrap_edge(edge.clone()) {
                                let id_0 = e.0.0;
                                let id_1 = e.1.0;
                                let child = node_id_hash(&id_0);
                                let parent = node_id_hash(&id_1);
                                println!("Edge: {} -> {}", id_0, id_1);
                                println!("Edge: {} -> {}", child, parent);
                                match parents.remove(&child) {
                                    None => parents.insert(child, vec![parent]),
                                    Some(mut prev) => {
                                        prev.push(parent);
                                        parents.insert(child, prev)
                                    }
                                };
                            }
                        }
                        _ => {}
                    }
                };

                for hash in hashes.iter() {
                    let diff = PerspectiveDiffEntryReference {
                        diff: hash.clone(),
                        parents: parents.get(hash).as_ref().cloned().cloned(),
                    };
                    graph.graph.push(diff.clone());
                    graph.graph_map.insert(hash.clone(), diff);
                }

                Ok(graph)
            }
        }
    }
}

lazy_static!{
    pub static ref GLOBAL_MOCKED_GRAPH: Mutex<MockPerspectiveGraph> = Mutex::new(MockPerspectiveGraph::new(GraphInput {
        nodes: 1,
        associations: vec![]
    }));
    pub static ref CURRENT_REVISION: Mutex<Hash> = Mutex::new(ActionHash::from_raw_36(vec![999; 36]));
    pub static ref LATEST_REVISION: Mutex<Hash> = Mutex::new(ActionHash::from_raw_36(vec![999; 36]));
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
fn can_create_graph_from_dot() {
    let dot = "digraph {
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
        12 -> 10 [ label = \"()\" ]
    }";

    let graph = MockPerspectiveGraph::from_dot(dot).expect("from_dot not to return error");
    assert_eq!(graph.graph.len(), 13);

    let node_12 = node_id_hash(&dot_structures::Id::Plain(String::from("12")));
    let node_11 = node_id_hash(&dot_structures::Id::Plain(String::from("11")));
    let node_10 = node_id_hash(&dot_structures::Id::Plain(String::from("10")));

    let diff_12 = graph.graph_map.get(&node_12).unwrap();
    assert_eq!(diff_12.parents, Some(vec![node_11, node_10]));
}

#[test]
fn example_test() {
    use crate::workspace::Workspace;

    fn update() {
        let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
        *graph = MockPerspectiveGraph::new(GraphInput {
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
    }
    update();

    let mut workspace = Workspace::new();
    let res = workspace.collect_until_common_ancestor::<MockPerspectiveGraph>(ActionHash::from_raw_36(vec![5; 36]), ActionHash::from_raw_36(vec![4; 36]));
    println!("Got result: {:#?}", res);
}