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
    let start = get_now()?.time();
    let seen_diffs = workspace.all_ancestors(&current.hash)?;
    let end = get_now()?.time();
    debug!("Took: {} to calculated all_ancestors for current", (end - start).num_milliseconds());
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
                56 [ label = "56" ]
                57 [ label = "57" ]
                58 [ label = "58" ]
                59 [ label = "59" ]
                60 [ label = "60" ]
                61 [ label = "61" ]
                62 [ label = "62" ]
                63 [ label = "63" ]
                64 [ label = "64" ]
                65 [ label = "65" ]
                66 [ label = "66" ]
                67 [ label = "67" ]
                68 [ label = "68" ]
                69 [ label = "69" ]
                70 [ label = "70" ]
                71 [ label = "71" ]
                72 [ label = "72" ]
                73 [ label = "73" ]
                74 [ label = "74" ]
                75 [ label = "75" ]
                76 [ label = "76" ]
                77 [ label = "77" ]
                78 [ label = "78" ]
                79 [ label = "79" ]
                80 [ label = "80" ]
                81 [ label = "81" ]
                82 [ label = "82" ]
                83 [ label = "83" ]
                84 [ label = "84" ]
                85 [ label = "85" ]
                86 [ label = "86" ]
                87 [ label = "87" ]
                88 [ label = "88" ]
                89 [ label = "89" ]
                90 [ label = "90" ]
                91 [ label = "91" ]
                92 [ label = "92" ]
                93 [ label = "93" ]
                94 [ label = "94" ]
                95 [ label = "95" ]
                96 [ label = "96" ]
                97 [ label = "97" ]
                98 [ label = "98" ]
                99 [ label = "99" ]
                100 [ label = "100" ]
                101 [ label = "101" ]
                102 [ label = "102" ]
                103 [ label = "103" ]
                104 [ label = "104" ]
                105 [ label = "105" ]
                106 [ label = "106" ]
                107 [ label = "107" ]
                108 [ label = "108" ]
                109 [ label = "109" ]
                110 [ label = "110" ]
                111 [ label = "111" ]
                112 [ label = "112" ]
                113 [ label = "113" ]
                114 [ label = "114" ]
                115 [ label = "115" ]
                116 [ label = "116" ]
                117 [ label = "117" ]
                118 [ label = "118" ]
                119 [ label = "119" ]
                120 [ label = "120" ]
                121 [ label = "121" ]
                122 [ label = "122" ]
                123 [ label = "123" ]
                124 [ label = "124" ]
                125 [ label = "125" ]
                126 [ label = "126" ]
                127 [ label = "127" ]
                128 [ label = "128" ]
                129 [ label = "129" ]
                130 [ label = "130" ]
                131 [ label = "131" ]
                132 [ label = "132" ]
                133 [ label = "133" ]
                134 [ label = "134" ]
                135 [ label = "135" ]
                136 [ label = "136" ]
                137 [ label = "137" ]
                138 [ label = "138" ]
                139 [ label = "139" ]
                140 [ label = "140" ]
                141 [ label = "141" ]
                142 [ label = "142" ]
                143 [ label = "143" ]
                144 [ label = "144" ]
                145 [ label = "145" ]
                146 [ label = "146" ]
                147 [ label = "147" ]
                148 [ label = "148" ]
                149 [ label = "149" ]
                150 [ label = "150" ]
                151 [ label = "151" ]
                152 [ label = "152" ]
                153 [ label = "153" ]
                154 [ label = "154" ]
                155 [ label = "155" ]
                156 [ label = "156" ]
                157 [ label = "157" ]
                158 [ label = "158" ]
                159 [ label = "159" ]
                160 [ label = "160" ]
                161 [ label = "161" ]
                162 [ label = "162" ]
                163 [ label = "163" ]
                164 [ label = "164" ]
                165 [ label = "165" ]
                166 [ label = "166" ]
                167 [ label = "167" ]
                168 [ label = "168" ]
                169 [ label = "169" ]
                170 [ label = "170" ]
                171 [ label = "171" ]
                172 [ label = "172" ]
                173 [ label = "173" ]
                174 [ label = "174" ]
                175 [ label = "175" ]
                176 [ label = "176" ]
                177 [ label = "177" ]
                178 [ label = "178" ]
                179 [ label = "179" ]
                180 [ label = "180" ]
                181 [ label = "181" ]
                182 [ label = "182" ]
                183 [ label = "183" ]
                184 [ label = "184" ]
                185 [ label = "185" ]
                186 [ label = "186" ]
                187 [ label = "187" ]
                188 [ label = "188" ]
                189 [ label = "189" ]
                190 [ label = "190" ]
                191 [ label = "191" ]
                192 [ label = "192" ]
                193 [ label = "193" ]
                194 [ label = "194" ]
                195 [ label = "195" ]
                196 [ label = "196" ]
                197 [ label = "197" ]
                198 [ label = "198" ]
                199 [ label = "199" ]
                200 [ label = "200" ]
                201 [ label = "201" ]
                202 [ label = "202" ]
                203 [ label = "203" ]
                204 [ label = "204" ]
                205 [ label = "205" ]
                206 [ label = "206" ]
                207 [ label = "207" ]
                208 [ label = "208" ]
                209 [ label = "209" ]
                210 [ label = "210" ]
                211 [ label = "211" ]
                212 [ label = "212" ]
                213 [ label = "213" ]
                214 [ label = "214" ]
                215 [ label = "215" ]
                216 [ label = "216" ]
                217 [ label = "217" ]
                218 [ label = "218" ]
                219 [ label = "219" ]
                220 [ label = "220" ]
                221 [ label = "221" ]
                222 [ label = "222" ]
                223 [ label = "223" ]
                224 [ label = "224" ]
                225 [ label = "225" ]
                226 [ label = "226" ]
                227 [ label = "227" ]
                228 [ label = "228" ]
                229 [ label = "229" ]
                230 [ label = "230" ]
                231 [ label = "231" ]
                232 [ label = "232" ]
                233 [ label = "233" ]
                234 [ label = "234" ]
                235 [ label = "235" ]
                236 [ label = "236" ]
                237 [ label = "237" ]
                238 [ label = "238" ]
                239 [ label = "239" ]
                240 [ label = "240" ]
                241 [ label = "241" ]
                242 [ label = "242" ]
                243 [ label = "243" ]
                244 [ label = "244" ]
                245 [ label = "245" ]
                246 [ label = "246" ]
                247 [ label = "247" ]
                248 [ label = "248" ]
                249 [ label = "249" ]
                250 [ label = "250" ]
                251 [ label = "251" ]
                252 [ label = "252" ]
                253 [ label = "253" ]
                254 [ label = "254" ]
                255 [ label = "255" ]
                256 [ label = "256" ]
                257 [ label = "257" ]
                258 [ label = "258" ]
                259 [ label = "259" ]
                260 [ label = "260" ]
                261 [ label = "261" ]
                262 [ label = "262" ]
                263 [ label = "263" ]
                264 [ label = "264" ]
                265 [ label = "265" ]
                266 [ label = "266" ]
                267 [ label = "267" ]
                268 [ label = "268" ]
                269 [ label = "269" ]
                270 [ label = "270" ]
                271 [ label = "271" ]
                272 [ label = "272" ]
                273 [ label = "273" ]
                274 [ label = "274" ]
                275 [ label = "275" ]
                276 [ label = "276" ]
                277 [ label = "277" ]
                278 [ label = "278" ]
                279 [ label = "279" ]
                280 [ label = "280" ]
                281 [ label = "281" ]
                282 [ label = "282" ]
                283 [ label = "283" ]
                284 [ label = "284" ]
                285 [ label = "285" ]
                286 [ label = "286" ]
                287 [ label = "287" ]
                288 [ label = "288" ]
                289 [ label = "289" ]
                290 [ label = "290" ]
                291 [ label = "291" ]
                292 [ label = "292" ]
                293 [ label = "293" ]
                294 [ label = "294" ]
                295 [ label = "295" ]
                296 [ label = "296" ]
                297 [ label = "297" ]
                298 [ label = "298" ]
                299 [ label = "299" ]
                300 [ label = "300" ]
                301 [ label = "301" ]
                302 [ label = "302" ]
                303 [ label = "303" ]
                304 [ label = "304" ]
                305 [ label = "305" ]
                306 [ label = "306" ]
                307 [ label = "307" ]
                308 [ label = "308" ]
                309 [ label = "309" ]
                310 [ label = "310" ]
                311 [ label = "311" ]
                312 [ label = "312" ]
                313 [ label = "313" ]
                314 [ label = "314" ]
                1 -> 0 [ label = "()" ]
                2 -> 0 [ label = "()" ]
                3 -> 2 [ label = "()" ]
                4 -> 3 [ label = "()" ]
                5 -> 4 [ label = "()" ]
                6 -> 5 [ label = "()" ]
                7 -> 6 [ label = "()" ]
                8 -> 7 [ label = "()" ]
                9 -> 8 [ label = "()" ]
                10 -> 0 [ label = "()" ]
                11 -> 10 [ label = "()" ]
                11 -> 9 [ label = "()" ]
                12 -> 10 [ label = "()" ]
                13 -> 12 [ label = "()" ]
                14 -> 13 [ label = "()" ]
                14 -> 11 [ label = "()" ]
                15 -> 14 [ label = "()" ]
                16 -> 15 [ label = "()" ]
                17 -> 16 [ label = "()" ]
                18 -> 17 [ label = "()" ]
                19 -> 18 [ label = "()" ]
                20 -> 19 [ label = "()" ]
                21 -> 16 [ label = "()" ]
                22 -> 21 [ label = "()" ]
                23 -> 20 [ label = "()" ]
                23 -> 22 [ label = "()" ]
                24 -> 23 [ label = "()" ]
                25 -> 24 [ label = "()" ]
                26 -> 25 [ label = "()" ]
                27 -> 26 [ label = "()" ]
                28 -> 25 [ label = "()" ]
                29 -> 28 [ label = "()" ]
                30 -> 29 [ label = "()" ]
                30 -> 27 [ label = "()" ]
                31 -> 0 [ label = "()" ]
                32 -> 31 [ label = "()" ]
                32 -> 30 [ label = "()" ]
                33 -> 32 [ label = "()" ]
                34 -> 32 [ label = "()" ]
                35 -> 34 [ label = "()" ]
                35 -> 33 [ label = "()" ]
                36 -> 35 [ label = "()" ]
                37 -> 36 [ label = "()" ]
                38 -> 37 [ label = "()" ]
                39 -> 38 [ label = "()" ]
                40 -> 35 [ label = "()" ]
                41 -> 40 [ label = "()" ]
                42 -> 39 [ label = "()" ]
                42 -> 41 [ label = "()" ]
                43 -> 42 [ label = "()" ]
                44 -> 43 [ label = "()" ]
                45 -> 44 [ label = "()" ]
                46 -> 45 [ label = "()" ]
                47 -> 46 [ label = "()" ]
                48 -> 47 [ label = "()" ]
                49 -> 48 [ label = "()" ]
                50 -> 49 [ label = "()" ]
                51 -> 50 [ label = "()" ]
                52 -> 51 [ label = "()" ]
                53 -> 52 [ label = "()" ]
                54 -> 53 [ label = "()" ]
                55 -> 54 [ label = "()" ]
                56 -> 55 [ label = "()" ]
                57 -> 56 [ label = "()" ]
                58 -> 57 [ label = "()" ]
                59 -> 58 [ label = "()" ]
                60 -> 59 [ label = "()" ]
                61 -> 60 [ label = "()" ]
                62 -> 61 [ label = "()" ]
                63 -> 62 [ label = "()" ]
                64 -> 63 [ label = "()" ]
                65 -> 64 [ label = "()" ]
                66 -> 65 [ label = "()" ]
                67 -> 66 [ label = "()" ]
                68 -> 67 [ label = "()" ]
                69 -> 68 [ label = "()" ]
                70 -> 69 [ label = "()" ]
                71 -> 70 [ label = "()" ]
                72 -> 71 [ label = "()" ]
                73 -> 72 [ label = "()" ]
                74 -> 73 [ label = "()" ]
                75 -> 74 [ label = "()" ]
                76 -> 75 [ label = "()" ]
                77 -> 76 [ label = "()" ]
                78 -> 77 [ label = "()" ]
                79 -> 78 [ label = "()" ]
                80 -> 79 [ label = "()" ]
                81 -> 80 [ label = "()" ]
                82 -> 81 [ label = "()" ]
                83 -> 82 [ label = "()" ]
                84 -> 83 [ label = "()" ]
                85 -> 84 [ label = "()" ]
                86 -> 85 [ label = "()" ]
                87 -> 86 [ label = "()" ]
                88 -> 87 [ label = "()" ]
                89 -> 88 [ label = "()" ]
                90 -> 89 [ label = "()" ]
                91 -> 90 [ label = "()" ]
                92 -> 91 [ label = "()" ]
                93 -> 92 [ label = "()" ]
                94 -> 93 [ label = "()" ]
                95 -> 94 [ label = "()" ]
                96 -> 95 [ label = "()" ]
                97 -> 96 [ label = "()" ]
                98 -> 97 [ label = "()" ]
                99 -> 98 [ label = "()" ]
                100 -> 99 [ label = "()" ]
                101 -> 100 [ label = "()" ]
                102 -> 101 [ label = "()" ]
                103 -> 102 [ label = "()" ]
                104 -> 103 [ label = "()" ]
                105 -> 104 [ label = "()" ]
                106 -> 105 [ label = "()" ]
                107 -> 106 [ label = "()" ]
                108 -> 107 [ label = "()" ]
                109 -> 108 [ label = "()" ]
                110 -> 81 [ label = "()" ]
                111 -> 110 [ label = "()" ]
                112 -> 111 [ label = "()" ]
                113 -> 100 [ label = "()" ]
                113 -> 112 [ label = "()" ]
                114 -> 109 [ label = "()" ]
                114 -> 113 [ label = "()" ]
                115 -> 1 [ label = "()" ]
                115 -> 114 [ label = "()" ]
                116 -> 1 [ label = "()" ]
                116 -> 114 [ label = "()" ]
                117 -> 115 [ label = "()" ]
                117 -> 116 [ label = "()" ]
                118 -> 117 [ label = "()" ]
                119 -> 118 [ label = "()" ]
                120 -> 119 [ label = "()" ]
                121 -> 120 [ label = "()" ]
                122 -> 121 [ label = "()" ]
                123 -> 122 [ label = "()" ]
                124 -> 123 [ label = "()" ]
                125 -> 124 [ label = "()" ]
                126 -> 125 [ label = "()" ]
                127 -> 126 [ label = "()" ]
                128 -> 127 [ label = "()" ]
                129 -> 128 [ label = "()" ]
                130 -> 129 [ label = "()" ]
                131 -> 130 [ label = "()" ]
                132 -> 131 [ label = "()" ]
                133 -> 132 [ label = "()" ]
                134 -> 133 [ label = "()" ]
                135 -> 134 [ label = "()" ]
                136 -> 135 [ label = "()" ]
                137 -> 136 [ label = "()" ]
                138 -> 115 [ label = "()" ]
                138 -> 116 [ label = "()" ]
                139 -> 138 [ label = "()" ]
                140 -> 127 [ label = "()" ]
                140 -> 139 [ label = "()" ]
                141 -> 139 [ label = "()" ]
                142 -> 137 [ label = "()" ]
                142 -> 141 [ label = "()" ]
                143 -> 142 [ label = "()" ]
                143 -> 140 [ label = "()" ]
                144 -> 143 [ label = "()" ]
                145 -> 144 [ label = "()" ]
                146 -> 145 [ label = "()" ]
                147 -> 146 [ label = "()" ]
                148 -> 147 [ label = "()" ]
                149 -> 148 [ label = "()" ]
                150 -> 149 [ label = "()" ]
                151 -> 150 [ label = "()" ]
                152 -> 151 [ label = "()" ]
                153 -> 152 [ label = "()" ]
                154 -> 153 [ label = "()" ]
                155 -> 154 [ label = "()" ]
                156 -> 155 [ label = "()" ]
                157 -> 156 [ label = "()" ]
                158 -> 157 [ label = "()" ]
                159 -> 158 [ label = "()" ]
                160 -> 159 [ label = "()" ]
                161 -> 160 [ label = "()" ]
                162 -> 161 [ label = "()" ]
                163 -> 162 [ label = "()" ]
                164 -> 163 [ label = "()" ]
                165 -> 164 [ label = "()" ]
                166 -> 165 [ label = "()" ]
                167 -> 166 [ label = "()" ]
                168 -> 167 [ label = "()" ]
                169 -> 168 [ label = "()" ]
                170 -> 169 [ label = "()" ]
                171 -> 170 [ label = "()" ]
                172 -> 171 [ label = "()" ]
                173 -> 172 [ label = "()" ]
                174 -> 173 [ label = "()" ]
                175 -> 174 [ label = "()" ]
                176 -> 175 [ label = "()" ]
                177 -> 176 [ label = "()" ]
                178 -> 177 [ label = "()" ]
                179 -> 178 [ label = "()" ]
                180 -> 179 [ label = "()" ]
                181 -> 180 [ label = "()" ]
                182 -> 181 [ label = "()" ]
                183 -> 182 [ label = "()" ]
                184 -> 183 [ label = "()" ]
                185 -> 184 [ label = "()" ]
                186 -> 185 [ label = "()" ]
                187 -> 186 [ label = "()" ]
                188 -> 187 [ label = "()" ]
                189 -> 188 [ label = "()" ]
                190 -> 189 [ label = "()" ]
                191 -> 190 [ label = "()" ]
                192 -> 191 [ label = "()" ]
                193 -> 192 [ label = "()" ]
                194 -> 193 [ label = "()" ]
                195 -> 192 [ label = "()" ]
                196 -> 195 [ label = "()" ]
                197 -> 192 [ label = "()" ]
                198 -> 197 [ label = "()" ]
                199 -> 198 [ label = "()" ]
                199 -> 196 [ label = "()" ]
                200 -> 199 [ label = "()" ]
                200 -> 194 [ label = "()" ]
                201 -> 200 [ label = "()" ]
                202 -> 201 [ label = "()" ]
                203 -> 202 [ label = "()" ]
                204 -> 203 [ label = "()" ]
                205 -> 204 [ label = "()" ]
                206 -> 205 [ label = "()" ]
                207 -> 206 [ label = "()" ]
                208 -> 207 [ label = "()" ]
                209 -> 208 [ label = "()" ]
                210 -> 209 [ label = "()" ]
                211 -> 210 [ label = "()" ]
                212 -> 211 [ label = "()" ]
                213 -> 212 [ label = "()" ]
                214 -> 213 [ label = "()" ]
                215 -> 214 [ label = "()" ]
                216 -> 215 [ label = "()" ]
                217 -> 216 [ label = "()" ]
                218 -> 217 [ label = "()" ]
                219 -> 218 [ label = "()" ]
                220 -> 219 [ label = "()" ]
                221 -> 220 [ label = "()" ]
                222 -> 221 [ label = "()" ]
                223 -> 222 [ label = "()" ]
                224 -> 223 [ label = "()" ]
                225 -> 224 [ label = "()" ]
                226 -> 225 [ label = "()" ]
                227 -> 226 [ label = "()" ]
                228 -> 227 [ label = "()" ]
                229 -> 228 [ label = "()" ]
                230 -> 229 [ label = "()" ]
                231 -> 230 [ label = "()" ]
                232 -> 231 [ label = "()" ]
                233 -> 232 [ label = "()" ]
                234 -> 233 [ label = "()" ]
                235 -> 231 [ label = "()" ]
                236 -> 235 [ label = "()" ]
                237 -> 236 [ label = "()" ]
                238 -> 237 [ label = "()" ]
                239 -> 236 [ label = "()" ]
                240 -> 239 [ label = "()" ]
                240 -> 238 [ label = "()" ]
                241 -> 236 [ label = "()" ]
                242 -> 238 [ label = "()" ]
                242 -> 241 [ label = "()" ]
                243 -> 239 [ label = "()" ]
                243 -> 242 [ label = "()" ]
                244 -> 243 [ label = "()" ]
                245 -> 234 [ label = "()" ]
                245 -> 244 [ label = "()" ]
                246 -> 245 [ label = "()" ]
                247 -> 245 [ label = "()" ]
                247 -> 240 [ label = "()" ]
                248 -> 246 [ label = "()" ]
                248 -> 247 [ label = "()" ]
                249 -> 248 [ label = "()" ]
                250 -> 249 [ label = "()" ]
                251 -> 248 [ label = "()" ]
                252 -> 250 [ label = "()" ]
                252 -> 251 [ label = "()" ]
                253 -> 250 [ label = "()" ]
                253 -> 251 [ label = "()" ]
                254 -> 253 [ label = "()" ]
                254 -> 252 [ label = "()" ]
                255 -> 254 [ label = "()" ]
                256 -> 255 [ label = "()" ]
                257 -> 256 [ label = "()" ]
                258 -> 255 [ label = "()" ]
                259 -> 258 [ label = "()" ]
                259 -> 256 [ label = "()" ]
                260 -> 258 [ label = "()" ]
                261 -> 260 [ label = "()" ]
                262 -> 260 [ label = "()" ]
                262 -> 259 [ label = "()" ]
                263 -> 261 [ label = "()" ]
                263 -> 262 [ label = "()" ]
                264 -> 263 [ label = "()" ]
                264 -> 257 [ label = "()" ]
                265 -> 264 [ label = "()" ]
                266 -> 265 [ label = "()" ]
                267 -> 266 [ label = "()" ]
                268 -> 265 [ label = "()" ]
                269 -> 268 [ label = "()" ]
                269 -> 267 [ label = "()" ]
                270 -> 269 [ label = "()" ]
                271 -> 270 [ label = "()" ]
                272 -> 271 [ label = "()" ]
                273 -> 272 [ label = "()" ]
                274 -> 273 [ label = "()" ]
                275 -> 274 [ label = "()" ]
                276 -> 275 [ label = "()" ]
                277 -> 276 [ label = "()" ]
                278 -> 277 [ label = "()" ]
                279 -> 278 [ label = "()" ]
                280 -> 279 [ label = "()" ]
                281 -> 280 [ label = "()" ]
                282 -> 281 [ label = "()" ]
                283 -> 282 [ label = "()" ]
                284 -> 283 [ label = "()" ]
                285 -> 284 [ label = "()" ]
                286 -> 285 [ label = "()" ]
                287 -> 285 [ label = "()" ]
                288 -> 286 [ label = "()" ]
                288 -> 287 [ label = "()" ]
                289 -> 288 [ label = "()" ]
                290 -> 289 [ label = "()" ]
                291 -> 290 [ label = "()" ]
                292 -> 291 [ label = "()" ]
                293 -> 292 [ label = "()" ]
                294 -> 293 [ label = "()" ]
                295 -> 294 [ label = "()" ]
                296 -> 295 [ label = "()" ]
                297 -> 296 [ label = "()" ]
                298 -> 297 [ label = "()" ]
                299 -> 298 [ label = "()" ]
                300 -> 299 [ label = "()" ]
                301 -> 300 [ label = "()" ]
                302 -> 301 [ label = "()" ]
                303 -> 300 [ label = "()" ]
                304 -> 301 [ label = "()" ]
                304 -> 303 [ label = "()" ]
                305 -> 304 [ label = "()" ]
                305 -> 302 [ label = "()" ]
                306 -> 305 [ label = "()" ]
                307 -> 306 [ label = "()" ]
                308 -> 307 [ label = "()" ]
                309 -> 308 [ label = "()" ]
                310 -> 309 [ label = "()" ]
                311 -> 310 [ label = "()" ]
                312 -> 311 [ label = "()" ]
                313 -> 312 [ label = "()" ]
                314 -> 0 [ label = "()" ]
                }"#).unwrap();
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

        let mut expected_additions = vec![create_node_id_link_expression(314)];

        for addition in expected_additions.clone() {
            assert!(pull_res.additions.contains(&addition));
        };
        assert!(pull_res.additions.iter().all(|item| expected_additions.contains(item)));

        //ensure that a merge was created
        let latest = MockPerspectiveGraph::latest_revision();
        assert!(latest.unwrap().unwrap().hash != latest_node_hash);
    }
}