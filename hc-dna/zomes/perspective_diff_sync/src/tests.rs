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
    println!("Got result: {:#?}", res);
    assert!(res.is_ok());
    
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
    println!("Got result: {:#?}", res);
    assert!(res.is_ok());
}