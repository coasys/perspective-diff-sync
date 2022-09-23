use hdk::prelude::*;
use perspective_diff_sync_integrity::{EntryTypes, PerspectiveDiff, PerspectiveDiffEntryReference};

use crate::errors::SocialContextResult;
use crate::revisions::{
    current_revision, latest_revision, update_current_revision, update_latest_revision,
};
use crate::utils::get_now;
use crate::workspace::{Workspace, NULL_NODE};
use crate::retriever::{PerspectiveDiffRetreiver};
use crate::Hash;

fn merge<Retriever: PerspectiveDiffRetreiver>(latest: Hash, current: Hash) -> SocialContextResult<()> {
    debug!("===PerspectiveDiffSync.merge(): Function start");
    let fn_start = get_now()?.time();

    let latest_diff = Retriever::get::<PerspectiveDiffEntryReference>(latest.clone())?;
    let current_diff = Retriever::get::<PerspectiveDiffEntryReference>(current.clone())?;
    //Create the merge entry
    let merge_entry = Retriever::create_entry(EntryTypes::PerspectiveDiff(PerspectiveDiff {
        additions: vec![],
        removals: vec![]
    }))?;
    //Create the merge entry
    let hash = Retriever::create_entry(EntryTypes::PerspectiveDiffEntryReference(
        PerspectiveDiffEntryReference {
            parents: Some(vec![latest, current]),
            diff: merge_entry.clone(),
            diffs_since_snapshot: latest_diff.diffs_since_snapshot + current_diff.diffs_since_snapshot + 1,
        },
    ))?;
    debug!("===PerspectiveDiffSync.merge(): Commited merge entry: {:#?}", hash);
    
    let now = get_now()?;
    update_current_revision::<Retriever>(hash.clone(), now)?;
    update_latest_revision::<Retriever>(hash, now)?;

    let fn_end = get_now()?.time();
    debug!("===PerspectiveDiffSync.merge() - Profiling: Took: {} to complete merge() function", (fn_end - fn_start).num_milliseconds()); 
    Ok(())
}

pub fn pull<Retriever: PerspectiveDiffRetreiver>() -> SocialContextResult<PerspectiveDiff> {
    debug!("===PerspectiveDiffSync.pull(): Function start");
    let fn_start = get_now()?.time();

    let latest = latest_revision::<Retriever>()?;
    let latest_hash = latest.clone().map(|val| val.hash);
    let current = current_revision::<Retriever>()?;
    let current_hash = current.clone().map(|val| val.hash);
    debug!(
        "===PerspectiveDiffSync.pull(): Pull made with latest: {:#?} and current: {:#?}",
        latest, current
    );

    if latest_hash == current_hash {
        return Ok(PerspectiveDiff {
            removals: vec![],
            additions: vec![],
        })
    }

    if latest.is_none() {
        return Ok(PerspectiveDiff {
            removals: vec![],
            additions: vec![],
        })
    }

    let latest = latest.expect("latest missing handled above");
    let mut workspace = Workspace::new();

    if current.is_none() {
        workspace.collect_only_from_latest::<Retriever>(latest.hash.clone())?;
        let diff = workspace.squashed_diff::<Retriever>()?;
        update_current_revision::<Retriever>(latest.hash, latest.timestamp)?;
        return Ok(diff);
    }

    let current = current.expect("current missing handled above");

    workspace.build_diffs::<Retriever>(latest.hash.clone(), current.hash.clone())?;

    let fast_forward_possible = workspace.common_ancestors.contains(&current.hash);
    // println!("fast_forward_possible: {}, {:#?}", fast_forward_possible, workspace.common_ancestors);
    
    //Get all the diffs which exist between current and the last ancestor that we got
    let seen_diffs = workspace.all_ancestors(&current.hash)?;
    // println!("SEEN DIFFS: {:#?}", seen_diffs);
    
    //Get all the diffs in the graph which we havent seen
    let unseen_diffs = if seen_diffs.len() > 0 {
        let diffs = workspace.sorted_diffs.clone().expect("should be unseen diffs after build_diffs() call").into_iter().filter(|val| {
            if val.0 == NULL_NODE() {
                return false;
            };
            if val.0 == current.hash {
                return false;
            };
            if seen_diffs.contains(&val.0) {
                return false;
            };
            true
        }).collect::<Vec<(Hash, PerspectiveDiffEntryReference)>>();
        diffs
    } else {
        workspace.sorted_diffs.expect("should be unseen diffs after build_diffs() call").into_iter().filter(|val| {
            val.0 != NULL_NODE() && val.0 != current.hash
        }).collect::<Vec<(Hash, PerspectiveDiffEntryReference)>>()
    };

    if fast_forward_possible {
        debug!("===PerspectiveDiffSync.pull(): There are paths between current and latest, lets fast forward the changes we have missed!");
        let mut out = PerspectiveDiff {
            additions: vec![],
            removals: vec![]
        };
        for diff in unseen_diffs {
            let diff_entry = Retriever::get::<PerspectiveDiff>(diff.1.diff.clone())?;
            out
                .additions
                .append(&mut diff_entry.additions.clone());
            out
                .removals
                .append(&mut diff_entry.removals.clone());
        }
        update_current_revision::<Retriever>(latest.hash, latest.timestamp)?;
        let fn_end = get_now()?.time();
        debug!("===PerspectiveDiffSync.pull() - Profiling: Took: {} to complete pull() function", (fn_end - fn_start).num_milliseconds()); 
        Ok(out)
    } else {
        debug!("===PerspectiveDiffSync.pull():There are no paths between current and latest, we must merge current and latest");
        //Get the entries we missed from unseen diff
        let mut out = PerspectiveDiff {
            additions: vec![],
            removals: vec![]
        };
        for diff in unseen_diffs {
            let diff_entry = Retriever::get::<PerspectiveDiff>(diff.1.diff.clone())?;
            out
                .additions
                .append(&mut diff_entry.additions.clone());
            out
                .removals
                .append(&mut diff_entry.removals.clone());
        }

        merge::<Retriever>(latest.hash, current.hash)?;
        let fn_end = get_now()?.time();
        debug!("===PerspectiveDiffSync.pull() - Profiling: Took: {} to complete pull() function", (fn_end - fn_start).num_milliseconds()); 
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use dot_structures;
    use super::pull;
    use crate::retriever::{GLOBAL_MOCKED_GRAPH, MockPerspectiveGraph, PerspectiveDiffRetreiver,
        node_id_hash, create_node_id_vec, create_node_id_link_expression
    };
    use crate::utils::create_link_expression;

    #[test]
    fn test_fast_forward_merge() {
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

        let latest_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("3")));
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash, chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("2")));
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let pull_res = pull::<MockPerspectiveGraph>();
        assert!(pull_res.is_ok());
        let pull_res = pull_res.unwrap();
        
        let node_1 = &node_id_hash(&dot_structures::Id::Plain(String::from("1"))).to_string();
        let node_3 = &node_id_hash(&dot_structures::Id::Plain(String::from("3"))).to_string();
        let expected_additions = vec![create_link_expression(node_1, node_1), create_link_expression(node_3, node_3)];

        assert!(pull_res.additions.iter().all(|item| expected_additions.contains(item)));
    }

    #[test]
    fn test_complex_merge() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]
                4 [ label = "4" ]
                5 [ label = "5" ]
                6 [ label = "6" ]
            
                3 -> 2
                4 -> 2
                5 -> 3
                5 -> 4
                6 -> 5
            }"#).unwrap();
        }
        update();

        let latest_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("6")));
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash.clone(), chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("1")));
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let pull_res = pull::<MockPerspectiveGraph>();
        assert!(pull_res.is_ok());
        let pull_res = pull_res.unwrap();
        
        let node_2 = &node_id_hash(&dot_structures::Id::Plain(String::from("2"))).to_string();
        let node_3 = &node_id_hash(&dot_structures::Id::Plain(String::from("3"))).to_string();
        let node_4 = &node_id_hash(&dot_structures::Id::Plain(String::from("4"))).to_string();
        let node_5 = &node_id_hash(&dot_structures::Id::Plain(String::from("5"))).to_string();
        let node_6 = &node_id_hash(&dot_structures::Id::Plain(String::from("6"))).to_string();
        let expected_additions = vec![
            create_link_expression(node_2, node_2), 
            create_link_expression(node_3, node_3),  
            create_link_expression(node_4, node_4),  
            create_link_expression(node_5, node_5),  
            create_link_expression(node_6, node_6)
        ];

        assert!(pull_res.additions.iter().all(|item| expected_additions.contains(item)));

        //Test that a merge actually happened and latest was updated
        let new_latest = MockPerspectiveGraph::latest_revision();
        assert!(new_latest.is_ok());
        let new_latest = new_latest.unwrap();

        assert!(new_latest.unwrap().hash != latest_node_hash);
    }

    #[test]
    fn test_complex_fast_forward() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]
                4 [ label = "4" ]
                5 [ label = "5" ]
                6 [ label = "6" ]
            
                3 -> 2
                4 -> 2
                5 -> 3
                5 -> 4
                6 -> 5
            }"#).unwrap();
        }
        update();

        let latest_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("6")));
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash.clone(), chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("4")));
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let pull_res = pull::<MockPerspectiveGraph>();
        assert!(pull_res.is_ok());
        let pull_res = pull_res.unwrap();
        
        let node_3 = &node_id_hash(&dot_structures::Id::Plain(String::from("3"))).to_string();
        let node_5 = &node_id_hash(&dot_structures::Id::Plain(String::from("5"))).to_string();
        let node_6 = &node_id_hash(&dot_structures::Id::Plain(String::from("6"))).to_string();
        let expected_additions = vec![ 
            create_link_expression(node_3, node_3),  
            create_link_expression(node_5, node_5),  
            create_link_expression(node_6, node_6)
        ];

        assert!(pull_res.additions.iter().all(|item| expected_additions.contains(item)));
    }

    #[test]
    fn test_fast_forward_after_merge() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]
                4 [ label = "4" ]
                5 [ label = "5" ]
                6 [ label = "6" ]
                7 [ label = "7" ]
            
                3 -> 2
                4 -> 2
                5 -> 3
                5 -> 4
                6 -> 5
                7 -> 1
                7 -> 6
            }"#).unwrap();
        }
        update();

        let latest_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("7")));
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash.clone(), chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("6")));
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let pull_res = pull::<MockPerspectiveGraph>();
        assert!(pull_res.is_ok());
        let pull_res = pull_res.unwrap();
        
        let node_1 = &node_id_hash(&dot_structures::Id::Plain(String::from("1"))).to_string();
        let node_7 = &node_id_hash(&dot_structures::Id::Plain(String::from("7"))).to_string();
        let expected_additions = vec![ 
            create_link_expression(node_1, node_1),
            create_link_expression(node_7, node_7)
        ];

        assert!(pull_res.additions.iter().all(|item| expected_additions.contains(item)));
    }

    #[test]
    fn test_pull_complex_merge_implicit_zero() {
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

        let latest_node_hash = node_1;
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash.clone(), chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_6;
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let node_1 = &node_id_hash(&dot_structures::Id::Plain(String::from("1"))).to_string();
        let expected_additions = vec![ 
            create_link_expression(node_1, node_1),
        ];

        let pull_res = pull::<MockPerspectiveGraph>();
        assert!(pull_res.is_ok());
        assert!(pull_res.unwrap().additions.iter().all(|item| expected_additions.contains(item)));

        //ensure that merge was created and thus latest revision updated
        let latest = MockPerspectiveGraph::latest_revision();
        assert!(latest.unwrap().unwrap().hash != latest_node_hash);
    }

    #[test]
    fn test_pull_complex_merge_implicit_zero_reversed() {
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

        let latest_node_hash = node_6;
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash.clone(), chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_1;
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let node_6 = &node_id_hash(&dot_structures::Id::Plain(String::from("6"))).to_string();
        let node_5 = &node_id_hash(&dot_structures::Id::Plain(String::from("5"))).to_string();
        let node_4 = &node_id_hash(&dot_structures::Id::Plain(String::from("4"))).to_string();
        let node_3 = &node_id_hash(&dot_structures::Id::Plain(String::from("3"))).to_string();
        let node_2 = &node_id_hash(&dot_structures::Id::Plain(String::from("2"))).to_string();
        let expected_additions = vec![ 
            create_link_expression(node_6, node_6),
            create_link_expression(node_5, node_5),
            create_link_expression(node_4, node_4),
            create_link_expression(node_3, node_3),
            create_link_expression(node_2, node_2),
        ];

        let pull_res = pull::<MockPerspectiveGraph>();
        assert!(pull_res.is_ok());
        assert!(pull_res.unwrap().additions.iter().all(|item| expected_additions.contains(item)));

        //ensure that merge was created and thus latest revision updated
        let latest = MockPerspectiveGraph::latest_revision();
        assert!(latest.unwrap().unwrap().hash != latest_node_hash);
    }

    #[test]
    fn test_three_null_parents() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]
                4 [ label = "4" ]
                5 [ label = "5" ]

                4 -> 2
                4 -> 3
                5 -> 4
                5 -> 1
            }"#).unwrap();
        }
        update();

        let latest_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("5")));
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash.clone(), chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("2")));
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let pull_res = pull::<MockPerspectiveGraph>();
        assert!(pull_res.is_ok());
        println!("{:#?}", pull_res);
        let pull_res = pull_res.unwrap();
        
        let node_5 = &node_id_hash(&dot_structures::Id::Plain(String::from("5"))).to_string();
        let node_4 = &node_id_hash(&dot_structures::Id::Plain(String::from("4"))).to_string();
        let node_3 = &node_id_hash(&dot_structures::Id::Plain(String::from("3"))).to_string();
        let node_1 = &node_id_hash(&dot_structures::Id::Plain(String::from("1"))).to_string();
        let expected_additions = vec![ 
            create_link_expression(node_5, node_5),
            create_link_expression(node_4, node_4),
            create_link_expression(node_3, node_3),
            create_link_expression(node_1, node_1),
        ];

        assert!(pull_res.additions.iter().all(|item| expected_additions.contains(item)));

        //ensure that no merge was created
        let latest = MockPerspectiveGraph::latest_revision();
        assert!(latest.unwrap().unwrap().hash == latest_node_hash);
    }


    #[test]
    fn test_four_null_parents() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
                1 [ label = "1" ]
                2 [ label = "2" ]
                3 [ label = "3" ]
                4 [ label = "4" ]
                5 [ label = "5" ]
                6 [ label = "6" ]

                4 -> 2
                4 -> 3
                5 -> 4
                5 -> 1
            }"#).unwrap();
        }
        update();

        let latest_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("5")));
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash.clone(), chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("6")));
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let pull_res = pull::<MockPerspectiveGraph>();
        assert!(pull_res.is_ok());
        println!("{:#?}", pull_res);
        let pull_res = pull_res.unwrap();
        
        let node_5 = &node_id_hash(&dot_structures::Id::Plain(String::from("5"))).to_string();
        let node_4 = &node_id_hash(&dot_structures::Id::Plain(String::from("4"))).to_string();
        let node_3 = &node_id_hash(&dot_structures::Id::Plain(String::from("3"))).to_string();
        let node_2 = &node_id_hash(&dot_structures::Id::Plain(String::from("2"))).to_string();
        let node_1 = &node_id_hash(&dot_structures::Id::Plain(String::from("1"))).to_string();
        let expected_additions = vec![ 
            create_link_expression(node_5, node_5),
            create_link_expression(node_4, node_4),
            create_link_expression(node_3, node_3),
            create_link_expression(node_2, node_2),
            create_link_expression(node_1, node_1),
        ];

        assert!(pull_res.additions.iter().all(|item| expected_additions.contains(item)));

        //ensure that a merge was created
        let latest = MockPerspectiveGraph::latest_revision();
        assert!(latest.unwrap().unwrap().hash != latest_node_hash);
    }

    #[test]
    fn test_high_complex_graph() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(r#"digraph {
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
                17 [ label = "17" ]
                18 [ label = "18" ]
                19 [ label = "19" ]
                20 [ label = "20" ]
                21 [ label = "21" ]
                22 [ label = "22" ]
                23 [ label = "23" ]
                24 [ label = "24" ]
                25 [ label = "25" ]
                26 [ label = "26" ]
                27 [ label = "27" ]
                28 [ label = "28" ]
                29 [ label = "29" ]
                30 [ label = "30" ]
                31 [ label = "31" ]
                32 [ label = "32" ]
                33 [ label = "33" ]
                34 [ label = "34" ]
                35 [ label = "35" ]
                36 [ label = "36" ]
                37 [ label = "37" ]
                38 [ label = "38" ]
                39 [ label = "39" ]
                40 [ label = "40" ]
                41 [ label = "41" ]
                42 [ label = "42" ]
                43 [ label = "43" ]
                44 [ label = "44" ]
                45 [ label = "45" ]
                46 [ label = "46" ]
                47 [ label = "47" ]
                48 [ label = "48" ]
                49 [ label = "49" ]
                50 [ label = "50" ]
                51 [ label = "51" ]
                52 [ label = "52" ]
                53 [ label = "53" ]
                54 [ label = "54" ]
                55 [ label = "55" ]
                2 -> 1 [ label = "()" ]
                5 -> 4 [ label = "()" ]
                6 -> 5 [ label = "()" ]
                7 -> 6 [ label = "()" ]
                8 -> 7 [ label = "()" ]
                9 -> 8 [ label = "()" ]
                10 -> 9 [ label = "()" ]
                11 -> 10 [ label = "()" ]
                12 -> 11 [ label = "()" ]
                13 -> 3 [ label = "()" ]
                13 -> 12 [ label = "()" ]
                14 -> 13 [ label = "()" ]
                15 -> 14 [ label = "()" ]
                16 -> 15 [ label = "()" ]
                18 -> 17 [ label = "()" ]
                18 -> 16 [ label = "()" ]
                19 -> 18 [ label = "()" ]
                20 -> 19 [ label = "()" ]
                21 -> 20 [ label = "()" ]
                22 -> 2 [ label = "()" ]
                22 -> 19 [ label = "()" ]
                23 -> 22 [ label = "()" ]
                23 -> 21 [ label = "()" ]
                24 -> 23 [ label = "()" ]
                25 -> 24 [ label = "()" ]
                26 -> 25 [ label = "()" ]
                27 -> 26 [ label = "()" ]
                28 -> 27 [ label = "()" ]
                29 -> 28 [ label = "()" ]
                30 -> 29 [ label = "()" ]
                31 -> 30 [ label = "()" ]
                32 -> 31 [ label = "()" ]
                33 -> 32 [ label = "()" ]
                34 -> 33 [ label = "()" ]
                35 -> 33 [ label = "()" ]
                36 -> 34 [ label = "()" ]
                36 -> 35 [ label = "()" ]
                37 -> 36 [ label = "()" ]
                38 -> 37 [ label = "()" ]
                39 -> 38 [ label = "()" ]
                40 -> 39 [ label = "()" ]
                42 -> 41 [ label = "()" ]
                42 -> 40 [ label = "()" ]
                43 -> 42 [ label = "()" ]
                44 -> 41 [ label = "()" ]
                44 -> 40 [ label = "()" ]
                45 -> 41 [ label = "()" ]
                45 -> 40 [ label = "()" ]
                46 -> 43 [ label = "()" ]
                46 -> 45 [ label = "()" ]
                47 -> 44 [ label = "()" ]
                47 -> 46 [ label = "()" ]
                48 -> 44 [ label = "()" ]
                48 -> 46 [ label = "()" ]
                49 -> 46 [ label = "()" ]
                50 -> 49 [ label = "()" ]
                50 -> 47 [ label = "()" ]
                51 -> 49 [ label = "()" ]
                51 -> 48 [ label = "()" ]
                52 -> 51 [ label = "()" ]
                52 -> 50 [ label = "()" ]
                54 -> 53 [ label = "()" ]
                55 -> 54 [ label = "()" ]
                55 -> 22 [ label = "()" ]
            }"#).unwrap();
        }
        update();

        let latest_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("52")));
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash.clone(), chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("55")));
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let pull_res = pull::<MockPerspectiveGraph>();
        assert!(pull_res.is_ok());
        //println!("{:#?}", pull_res);
        let pull_res = pull_res.unwrap();

        let mut expected_additions = create_node_id_vec(23, 52);
        expected_additions.push(create_node_id_link_expression(20));
        expected_additions.push(create_node_id_link_expression(21));

        for addition in expected_additions.clone() {
            assert!(pull_res.additions.contains(&addition));
        };
        assert!(pull_res.additions.iter().all(|item| expected_additions.contains(item)));

        //ensure that a merge was created
        let latest = MockPerspectiveGraph::latest_revision();
        assert!(latest.unwrap().unwrap().hash != latest_node_hash);
    }

    #[test]
    fn test_late_join() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(&crate::test_graphs::LATE_JOIN).unwrap();
        }
        update();

        let latest_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("314")));
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash.clone(), chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("313")));
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let pull_res = pull::<MockPerspectiveGraph>();
        assert!(pull_res.is_ok());
        //println!("{:#?}", pull_res);
        let pull_res = pull_res.unwrap();

        let expected_additions = vec![create_node_id_link_expression(314)];

        assert!(pull_res.additions.iter().all(|item| expected_additions.contains(item)));

        //ensure that a merge was created
        let latest = MockPerspectiveGraph::latest_revision();
        assert!(latest.unwrap().unwrap().hash != latest_node_hash);
    }

    #[test]
    fn test_late_join_from_syncd() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(&crate::test_graphs::LATE_JOIN2).unwrap();
        }
        update();

        let latest_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("304")));
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash.clone(), chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("301")));
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let pull_res = pull::<MockPerspectiveGraph>();
        println!("{:#?}", pull_res);
        assert!(pull_res.is_ok());
        let pull_res = pull_res.unwrap();

        // let expected_additions = vec![create_node_id_link_expression(314)];

        // assert!(pull_res.additions.iter().all(|item| expected_additions.contains(item)));

        // //ensure that a merge was created
        // let latest = MockPerspectiveGraph::latest_revision();
        // assert!(latest.unwrap().unwrap().hash != latest_node_hash);
    }

    #[test]
    fn test_late_join_from_unsyncd() {
        fn update() {
            let mut graph = GLOBAL_MOCKED_GRAPH.lock().unwrap();
            *graph = MockPerspectiveGraph::from_dot(&crate::test_graphs::LATE_JOIN2).unwrap();
        }
        update();

        let latest_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("301")));
        let update_latest = MockPerspectiveGraph::update_latest_revision(latest_node_hash.clone(), chrono::Utc::now());
        assert!(update_latest.is_ok());

        let current_node_hash = node_id_hash(&dot_structures::Id::Plain(String::from("304")));
        let update_current = MockPerspectiveGraph::update_current_revision(current_node_hash, chrono::Utc::now());
        assert!(update_current.is_ok());

        let pull_res = pull::<MockPerspectiveGraph>();
        println!("{:#?}", pull_res);
        assert!(pull_res.is_ok());
        let pull_res = pull_res.unwrap();

        // let expected_additions = vec![create_node_id_link_expression(314)];

        // assert!(pull_res.additions.iter().all(|item| expected_additions.contains(item)));

        // //ensure that a merge was created
        // let latest = MockPerspectiveGraph::latest_revision();
        // assert!(latest.unwrap().unwrap().hash != latest_node_hash);
    }
}