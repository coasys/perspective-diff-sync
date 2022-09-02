#[test]
pub fn test_merge_fast_forward() {
    use hdk::prelude::*;

    use crate::retriever::{GLOBAL_MOCKED_GRAPH, MockPerspectiveGraph, GraphInput, Associations};
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
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), ActionHash::from_raw_36(vec![0; 36]));
}

#[test]
pub fn test_fork_with_none_source() {
    use hdk::prelude::*;

    use crate::retriever::{GLOBAL_MOCKED_GRAPH, MockPerspectiveGraph, GraphInput};
    use crate::workspace::Workspace;


    fn update() {
        let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
        *graph = MockPerspectiveGraph::new(GraphInput {
            nodes: 2,
            associations: vec![]
        });
    }
    update();

    let mut workspace = Workspace::new();
    let res = workspace.collect_until_common_ancestor::<MockPerspectiveGraph>(ActionHash::from_raw_36(vec![0; 36]), ActionHash::from_raw_36(vec![1; 36]));
    assert!(res.is_ok());
    //TODO; this is a problem since our pull code is not expecting to find a common ancestor, since both tips are forks
    //but in the case below where we have a merge entry we need to register the None node as a common ancestor so we can traverse the "their" branch back until the root
    //and not break the traversal with common ancestor as the "ours" node as was happening before
    //
    //So what do we actually need to return here?
    assert_eq!(res.unwrap(), ActionHash::from_raw_36(vec![0xdb; 36]));
}

#[test]
pub fn test_merge_fast_forward_none_source() {
    use hdk::prelude::*;

    use crate::retriever::{GLOBAL_MOCKED_GRAPH, MockPerspectiveGraph, GraphInput, Associations};
    use crate::workspace::Workspace;


    fn update() {
        let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
        *graph = MockPerspectiveGraph::new(GraphInput {
            nodes: 3,
            associations: vec![
                Associations {
                    node_source: 2,
                    node_targets: vec![0, 1]
                }
            ]
        });
    }
    update();

    let mut workspace = Workspace::new();
    let res = workspace.collect_until_common_ancestor::<MockPerspectiveGraph>(ActionHash::from_raw_36(vec![2; 36]), ActionHash::from_raw_36(vec![1; 36]));
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), ActionHash::from_raw_36(vec![0xdb; 36]));
}

#[test]
pub fn test_fork() {
    use dot_structures;
    use crate::retriever::{GLOBAL_MOCKED_GRAPH, MockPerspectiveGraph, node_id_hash};
    use crate::workspace::Workspace;

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
    let res = workspace.collect_until_common_ancestor::<MockPerspectiveGraph>(node_12, node_6);
    assert!(res.is_ok());
    
    assert_eq!(res.unwrap(), node_1);
}