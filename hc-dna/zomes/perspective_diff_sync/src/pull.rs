use hdk::prelude::*;
use perspective_diff_sync_integrity::{EntryTypes, PerspectiveDiff, PerspectiveDiffEntryReference};
use crate::errors::{SocialContextError, SocialContextResult};
use crate::revisions::{
    current_revision, latest_revision, update_current_revision, update_latest_revision,
};
use crate::utils::get_now;
use crate::workspace::{Workspace, NULL_NODE};
use crate::retriever::{PerspectiveDiffRetreiver};
use crate::Hash;

fn merge<Retriever: PerspectiveDiffRetreiver>(latest: Hash, current: Hash) -> SocialContextResult<()> {
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
    debug!("Commited merge entry: {:#?}", hash);
    let now = get_now()?;
    update_current_revision::<Retriever>(hash.clone(), now)?;
    update_latest_revision::<Retriever>(hash, now)?;
    Ok(())
}

pub fn pull<Retriever: PerspectiveDiffRetreiver>() -> SocialContextResult<PerspectiveDiff> {
    let latest = latest_revision::<Retriever>()?;
    let current = current_revision::<Retriever>()?;
    println!(
        "Pull made with latest: {:#?} and current: {:#?}",
        latest, current
    );

    if latest == current {
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
        workspace.collect_only_from_latest::<Retriever>(latest.clone())?;
        let diff = workspace.squashed_diff::<Retriever>()?;
        update_current_revision::<Retriever>(latest, get_now()?)?;
        return Ok(diff);
    }

    let current = current.expect("current missing handled above");

    match workspace.build_diffs::<Retriever>(latest.clone(), current.clone()) {
        Err(SocialContextError::NoCommonAncestorFound) => {
            println!("Did not find a common ancestor, workspace looks like: {:#?}", workspace.entry_map);
            workspace.collect_only_from_latest::<Retriever>(latest.clone())?;
            let diff = workspace.squashed_diff::<Retriever>()?;
            merge::<Retriever>(latest, current)?;
            return Ok(diff)
        },

        _ => {
            // continue with the rest below...
        }
    }

    //See what fast forward paths exist between latest and current
    let fast_foward_paths = workspace.get_paths(&latest, &current);
    //Get all the diffs which exist between current and the last ancestor that we got
    let seen_diffs = workspace.get_paths(&current, workspace.common_ancestors.last().to_owned().expect("Should be atleast 1 common ancestor"));
    println!("Got the seen diffs: {:#?}", seen_diffs);
    //Get all the diffs in the graph which we havent seen
    let unseen_diffs = if seen_diffs.len() > 0 {
        let diffs = workspace.sorted_diffs.clone().expect("should be unseen diffs after build_diffs() call").into_iter().filter(|val| {
            if val.0 == NULL_NODE() {
                return false;
            };
            if val.0 == current {
                return false;
            };
            let node_index = workspace.get_node_index(&val.0).expect("Should find the node index for a given diff ref");
            for seen_diff in &seen_diffs {
                if seen_diff.contains(node_index) {
                    return false;
                };
            };
            true
        }).collect::<Vec<(Hash, PerspectiveDiffEntryReference)>>();
        diffs
    } else {
        workspace.sorted_diffs.expect("should be unseen diffs after build_diffs() call").into_iter().filter(|val| {
            val.0 != NULL_NODE() && val.0 != current
        }).collect::<Vec<(Hash, PerspectiveDiffEntryReference)>>()
    };
    println!("Got the unseen diffs: {:#?}", unseen_diffs);

    if fast_foward_paths.len() > 0 {
        println!("There are paths between current and latest, lets fast forward the changes we have missed!");
        //Using now as the timestamp here may cause problems
        update_current_revision::<Retriever>(latest, get_now()?)?;
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
        Ok(out)
    } else {
        println!("There are no paths between current and latest, we must merge current and latest");
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

        merge::<Retriever>(latest, current)?;

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use dot_structures;

    use super::pull;
    use crate::retriever::{GLOBAL_MOCKED_GRAPH, MockPerspectiveGraph, node_id_hash, PerspectiveDiffRetreiver};
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

        assert!(new_latest.unwrap() != latest_node_hash);
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
}