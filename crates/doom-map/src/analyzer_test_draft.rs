use std::collections::{HashMap, HashSet};

pub struct SectorGraph {
    pub adjacency_list: HashMap<usize, HashSet<usize>>,
}

pub struct MapAnalyzer<'a> {
    graph: &'a SectorGraph,
}

impl<'a> MapAnalyzer<'a> {
    pub fn new(graph: &'a SectorGraph) -> Self {
        Self { graph }
    }

    pub fn chokepoints(&self) -> Vec<usize> {
        let mut visited = HashSet::new();
        let mut discovery_time = HashMap::new();
        let mut low_time = HashMap::new();
        let mut parent = HashMap::new();
        let mut articulation_points = HashSet::new();
        let mut time = 0;

        for &node in self.graph.adjacency_list.keys() {
            if !visited.contains(&node) {
                // Iterative DFS to avoid stack overflow on deep graphs.
                let empty_set = HashSet::new();
                let neighbors = self.graph.adjacency_list.get(&node).unwrap_or(&empty_set);
                let mut stack = vec![(node, neighbors.iter())];

                visited.insert(node);
                time += 1;
                discovery_time.insert(node, time);
                low_time.insert(node, time);
                let mut children_map: HashMap<usize, usize> = HashMap::new();

                while let Some((u, mut neighbors_iter)) = stack.pop() {
                    let mut pushed_child = false;

                    while let Some(&v) = neighbors_iter.next() {
                        if !self.graph.adjacency_list.contains_key(&v) {
                            continue;
                        }
                        if !visited.contains(&v) {
                            *children_map.entry(u).or_default() += 1;
                            parent.insert(v, u);

                            visited.insert(v);
                            time += 1;
                            discovery_time.insert(v, time);
                            low_time.insert(v, time);

                            stack.push((u, neighbors_iter));
                            let v_neighbors = self.graph.adjacency_list.get(&v).unwrap_or(&empty_set);
                            stack.push((v, v_neighbors.iter()));
                            pushed_child = true;
                            break;
                        } else if parent.get(&u) != Some(&v) {
                            let (low_u, disc_v) =
                                (low_time.get(&u).copied(), discovery_time.get(&v).copied());
                            if let (Some(low_u), Some(disc_v)) = (low_u, disc_v) {
                                let new_low = low_u.min(disc_v);
                                low_time.insert(u, new_low);
                            }
                        }
                    }

                    if !pushed_child {
                        // After visiting all neighbors of u, if u is not root, update parent's low_time
                        if let Some(&p) = parent.get(&u) {
                            let (low_u, low_p, disc_p) = (
                                low_time.get(&u).copied(),
                                low_time.get(&p).copied(),
                                discovery_time.get(&p).copied(),
                            );
                            if let (Some(low_u), Some(low_p), Some(disc_p)) = (low_u, low_p, disc_p)
                            {
                                let new_low = low_p.min(low_u);
                                low_time.insert(p, new_low);

                                if low_u >= disc_p && parent.contains_key(&p) {
                                    articulation_points.insert(p);
                                }
                            }
                        } else if *children_map.get(&u).unwrap_or(&0) > 1 {
                            articulation_points.insert(u);
                        }
                    }
                }
            }
        }

        let mut ap_vec: Vec<usize> = articulation_points.into_iter().collect();
        ap_vec.sort_unstable();
        ap_vec
    }
}

fn main() {
    let mut adj = HashMap::new();
    adj.insert(0, HashSet::from([1, 2]));
    adj.insert(1, HashSet::from([0]));

    let graph = SectorGraph { adjacency_list: adj };
    let analyzer = MapAnalyzer::new(&graph);

    println!("{:?}", analyzer.chokepoints());
}
