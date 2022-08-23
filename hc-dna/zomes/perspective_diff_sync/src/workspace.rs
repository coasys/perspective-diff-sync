use crate::topo_sort::topo_sort_diff_references;

type Hash = HoloHash<holo_hash::hash_type::Action>;


pub struct Workspace {
    pub graph: DiGraph<HoloHash<holo_hash::hash_type::Action>, ()>,
    pub undirected_graph: UnGraph<HoloHash<holo_hash::hash_type::Action>, ()>,
    pub node_index_map: HashMap<HoloHash<holo_hash::hash_type::Action>, NodeIndex<u32>>,
    pub entry_map: BTreeMap<HoloHash<holo_hash::hash_type::Action>, PerspectiveDiffEntryReference>,
    pub sorted_diffs: Option<Vec<(HoloHash<holo_hash::hash_type::Action>, PerspectiveDiffEntryReference)>>,
}

impl Workspace {
    pub fn new() -> Search {
        Search {
            graph: Graph::new(),
            undirected_graph: Graph::new_undirected(),
            node_index_map: HashMap::new(),
            entry_map: HashMap::new(),
            sorted_diffs: None,
        }
    }

    // This is the easy case when we only build from one hash.
    // (either latest or our current hash, like in render).
    // We don't have to check for forks, we just deep search from the given
    // diff and terminate at leafs and snapshots.
    // Since we don't have to detect and handle forks, we don't need
    // to unroll snapshots and just treat them as leafs.
    pub fn collect_only_from_latest(&mut self, latest: Hash) 
        -> SocialContextResult<()> 
    {
        // Initializing with only one branch starting from the given hash.
        let mut unprocessed_branches: VecDeque<Hash> = vec![latest];

        while !unprocessed_branches.is_empty() {
            let current_hash = unprocessed_branches[0];
            
            if let Some(snapshot) = get_snapshot(current_hash)? {
                self.entry_map.insert(current_hash, PerspectiveDiffEntryReference {
                    diff: snapshot.diff,
                    parents: None,
                });
            } else {
                let current_diff = get_p_diff_reference(current_hash)?;
                self.entry_map.insert(current_hash, current_diff);

                if let Some(parents) = current_diff.parents {
                    for i in 0..parents.len() {
                        // Depth-first search:
                        // We are replacing our search position (==current_hash==unprocessed_branches[0])
                        // with the first parent.
                        // Other parents are pushed on the vec as new branches to search later..
                        if i==0 {
                            unprocessed_branches[0] = parents[i];
                        } else {
                            unprocessed_branches.push(parents[i])
                        }
                    }
                } else {
                    // We arrived at a leaf/orphan (no parents).
                    // So we can close this branch and potentially continue
                    // with other unprocessed branches, if they exist.
                    unprocessed_branches.pop_front();
                }
            }
        }
    }

    pub fn topo_sort_graph(&mut self) {
        let entry_vec = self.entry_map
            .into_iter()
            .collect::<Vec<(HoloHash<holo_hash::hash_type::Action>, PerspectiveDiffEntryReference)>>();
        
        self.sorted_diffs = Some(topo_sort_diff_references(&entry_vec));
    }

    pub fn build_graph(&mut self) {

    }

    pub fn build(
        &mut self, 
        latest: Hash,
        current: Option<Hash>
    ) -> SocialContextResult<()> {

        let mut search_position = latest;

        let mut common_ancestor_found = search_position == current;
        while !common_ancestor_found {
            let diff = get_p_diff_reference(search_position)?;

        }
    }

    fn get_p_diff_reference(address: Hash) -> SocialContextResult<PerspectiveDiffEntryReference> {
        get(search_position.0.clone(), GetOptions::latest())?
            .ok_or(SocialContextError::InternalError(
                "Could not find entry while populating search",
            ))?
            .entry()
            .to_app_option::<PerspectiveDiffEntryReference>()?
            .ok_or(SocialContextError::InternalError(
                "Expected element to contain app entry data",
            ))?;
    }

    fn get_snapshot(address: Hash) 
        -> SocialContextError<Option<Snapshot>> 
    {
        let mut snapshot_links = get_links(
            hash_entry(&diff)?,
            LinkTypes::Snapshot,
            Some(LinkTag::new("snapshot")),
        )?;

        if snapshot_links.len() > 0 {
            let snapshot = get(snapshot_links.remove(0).target, GetOptions::latest())?
                .ok_or(SocialContextError::InternalError(
                    "Could not find entry while populating search",
                ))?
                .entry()
                .to_app_option::<Snapshot>()?
                .ok_or(SocialContextError::InternalError(
                    "Expected element to contain app entry data",
                ))?;

            Ok(Some(snapshot))
        } else {
            Ok(None)
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

    //pub fn print(&self) {
    //    debug!(
    //        "Directed: {:?}\n",
    //        Dot::with_config(&self.graph, &[Config::NodeIndexLabel])
    //    );
    //    debug!(
    //        "Undirected: {:?}\n",
    //        Dot::with_config(&self.undirected_graph, &[])
    //    );
    //}

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