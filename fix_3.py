import sys

def replace(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    target = """    fn process_neighbors<'b>(
        &self,
        u: usize,
        neighbors_iter: &mut std::collections::hash_set::Iter<'b, usize>,
        visited: &mut HashSet<usize>,
        time: &mut usize,
        discovery_time: &mut HashMap<usize, usize>,
        low_time: &mut HashMap<usize, usize>,
        parent: &mut HashMap<usize, usize>,
        children_map: &mut HashMap<usize, usize>,
    ) -> Option<usize> {"""

    replacement = """    fn process_neighbors<'b>(
        &self,
        u: usize,
        neighbors_iter: &mut std::collections::hash_set::Iter<'b, usize>,
        ctx: &mut AnalyzerContext,
    ) -> Option<usize> {"""

    target_body = """            if !visited.contains(&v) {
                *children_map.entry(u).or_default() += 1;
                parent.insert(v, u);

                visited.insert(v);
                *time += 1;
                discovery_time.insert(v, *time);
                low_time.insert(v, *time);

                return Some(v);
            } else if parent.get(&u) != Some(&v) {
                let (low_u, disc_v) = (low_time.get(&u).copied(), discovery_time.get(&v).copied());
                if let (Some(low_u), Some(disc_v)) = (low_u, disc_v) {
                    let new_low = low_u.min(disc_v);
                    low_time.insert(u, new_low);
                }
            }"""

    replacement_body = """            if !ctx.visited.contains(&v) {
                *ctx.children_map.entry(u).or_default() += 1;
                ctx.parent.insert(v, u);

                ctx.visited.insert(v);
                ctx.time += 1;
                ctx.discovery_time.insert(v, ctx.time);
                ctx.low_time.insert(v, ctx.time);

                return Some(v);
            } else if ctx.parent.get(&u) != Some(&v) {
                let (low_u, disc_v) = (ctx.low_time.get(&u).copied(), ctx.discovery_time.get(&v).copied());
                if let (Some(low_u), Some(disc_v)) = (low_u, disc_v) {
                    let new_low = low_u.min(disc_v);
                    ctx.low_time.insert(u, new_low);
                }
            }"""

    target_update = """    fn update_parent_low_time(
        &self,
        u: usize,
        parent: &HashMap<usize, usize>,
        low_time: &mut HashMap<usize, usize>,
        discovery_time: &HashMap<usize, usize>,
        articulation_points: &mut HashSet<usize>,
        children_map: &HashMap<usize, usize>,
    ) {"""

    replacement_update = """    fn update_parent_low_time(
        &self,
        u: usize,
        ctx: &mut AnalyzerContext,
    ) {"""

    target_update_body = """        if let Some(&p) = parent.get(&u) {
            let (low_u, low_p, disc_p) = (
                low_time.get(&u).copied(),
                low_time.get(&p).copied(),
                discovery_time.get(&p).copied(),
            );
            if let (Some(low_u), Some(low_p), Some(disc_p)) = (low_u, low_p, disc_p) {
                let new_low = low_p.min(low_u);
                low_time.insert(p, new_low);

                if low_u >= disc_p && parent.contains_key(&p) {
                    articulation_points.insert(p);
                }
            }
        } else if *children_map.get(&u).unwrap_or(&0) > 1 {
            articulation_points.insert(u);
        }"""

    replacement_update_body = """        if let Some(&p) = ctx.parent.get(&u) {
            let (low_u, low_p, disc_p) = (
                ctx.low_time.get(&u).copied(),
                ctx.low_time.get(&p).copied(),
                ctx.discovery_time.get(&p).copied(),
            );
            if let (Some(low_u), Some(low_p), Some(disc_p)) = (low_u, low_p, disc_p) {
                let new_low = low_p.min(low_u);
                ctx.low_time.insert(p, new_low);

                if low_u >= disc_p && ctx.parent.contains_key(&p) {
                    ctx.articulation_points.insert(p);
                }
            }
        } else if *ctx.children_map.get(&u).unwrap_or(&0) > 1 {
            ctx.articulation_points.insert(u);
        }"""

    target_struct = """impl<'a> MapAnalyzer<'a> {"""

    replacement_struct = """struct AnalyzerContext {
    visited: HashSet<usize>,
    discovery_time: HashMap<usize, usize>,
    low_time: HashMap<usize, usize>,
    parent: HashMap<usize, usize>,
    articulation_points: HashSet<usize>,
    children_map: HashMap<usize, usize>,
    time: usize,
}

impl Default for AnalyzerContext {
    fn default() -> Self {
        Self {
            visited: HashSet::new(),
            discovery_time: HashMap::new(),
            low_time: HashMap::new(),
            parent: HashMap::new(),
            articulation_points: HashSet::new(),
            children_map: HashMap::new(),
            time: 0,
        }
    }
}

impl<'a> MapAnalyzer<'a> {"""

    target_chokepoints = """        let mut visited = HashSet::new();
        let mut discovery_time = HashMap::new();
        let mut low_time = HashMap::new();
        let mut parent = HashMap::new();
        let mut articulation_points = HashSet::new();
        let mut time = 0;

        for &node in self.graph.adjacency_list.keys() {
            if !visited.contains(&node) {
                // Iterative DFS to avoid stack overflow on deep graphs.
                let mut stack = vec![(node, self.graph.adjacency_list.get(&node).unwrap().iter())];

                visited.insert(node);
                time += 1;
                discovery_time.insert(node, time);
                low_time.insert(node, time);
                let mut children_map: HashMap<usize, usize> = HashMap::new();"""

    replacement_chokepoints = """        let mut ctx = AnalyzerContext::default();

        for &node in self.graph.adjacency_list.keys() {
            if !ctx.visited.contains(&node) {
                // Iterative DFS to avoid stack overflow on deep graphs.
                let mut stack = vec![(node, self.graph.adjacency_list.get(&node).unwrap().iter())];

                ctx.visited.insert(node);
                ctx.time += 1;
                ctx.discovery_time.insert(node, ctx.time);
                ctx.low_time.insert(node, ctx.time);"""

    target_call1 = """                    if let Some(v) = self.process_neighbors(
                        u,
                        &mut neighbors_iter,
                        &mut visited,
                        &mut time,
                        &mut discovery_time,
                        &mut low_time,
                        &mut parent,
                        &mut children_map,
                    ) {"""

    replacement_call1 = """                    if let Some(v) = self.process_neighbors(
                        u,
                        &mut neighbors_iter,
                        &mut ctx,
                    ) {"""

    target_call2 = """                        self.update_parent_low_time(
                            u,
                            &parent,
                            &mut low_time,
                            &discovery_time,
                            &mut articulation_points,
                            &children_map,
                        );"""

    replacement_call2 = """                        self.update_parent_low_time(
                            u,
                            &mut ctx,
                        );"""

    target_end = """        let mut ap_vec: Vec<usize> = articulation_points.into_iter().collect();"""
    replacement_end = """        let mut ap_vec: Vec<usize> = ctx.articulation_points.into_iter().collect();"""

    new_content = content.replace(target, replacement)
    new_content = new_content.replace(target_body, replacement_body)
    new_content = new_content.replace(target_update, replacement_update)
    new_content = new_content.replace(target_update_body, replacement_update_body)
    new_content = new_content.replace(target_struct, replacement_struct)
    new_content = new_content.replace(target_chokepoints, replacement_chokepoints)
    new_content = new_content.replace(target_call1, replacement_call1)
    new_content = new_content.replace(target_call2, replacement_call2)
    new_content = new_content.replace(target_end, replacement_end)

    with open(filepath, 'w') as f:
        f.write(new_content)

    print("Changes applied.")

replace('crates/doom-map/src/analyzer.rs')
